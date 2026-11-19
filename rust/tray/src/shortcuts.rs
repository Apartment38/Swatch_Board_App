mod kde;

use std::time::Duration;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
use futures_util::StreamExt;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::timeout,
};

const PICK: &str = "pick-color";
const HISTORY: &str = "show-history";
const TRAY_APP_ID: &str = "org.apartment38.SwatchBoard.Tray";
const RETRY: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const FALLBACK: &str = "KDE fallback: in System Settings > Shortcuts, add two custom command shortcuts: qdbus6 org.apartment38.SwatchBoard1 /org/apartment38/SwatchBoard1 org.apartment38.SwatchBoard1.PickColor and qdbus6 org.apartment38.SwatchBoard1 /org/apartment38/SwatchBoard1 org.apartment38.SwatchBoard1.ShowHistory. Assign the desired keys there; remove them if portal shortcuts are later enabled to avoid conflicts.";

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Pick,
    History,
}

pub struct Shortcuts {
    desired: watch::Sender<Option<(String, String, bool)>>,
    status: watch::Receiver<String>,
    worker: JoinHandle<()>,
}

impl Shortcuts {
    pub fn start(pick: String, history: String, events: mpsc::UnboundedSender<Action>) -> Self {
        let (desired, requests) = watch::channel(Some((pick, history, false)));
        let (updates, status) =
            watch::channel("Global shortcuts starting; awaiting desktop portal".to_owned());
        let worker = tokio::spawn(run(requests, updates, events));
        Self {
            desired,
            status,
            worker,
        }
    }

    /// Queue a new portal session without waiting for an interactive permission dialog.
    pub async fn rebind(&mut self, pick: String, history: String) -> Result<(), String> {
        let a = portal_trigger(&pick)?;
        let b = portal_trigger(&history)?;
        if a.eq_ignore_ascii_case(&b) {
            return Err("Pick and History shortcuts must differ".into());
        }
        self.desired
            .send(Some((pick, history, true)))
            .map_err(|_| "global shortcut worker stopped".to_owned())
    }

    pub fn status(&self) -> String {
        self.status.borrow().clone()
    }

    pub async fn shutdown(self) {
        let _ = self.desired.send(None);
        let _ = self.worker.await;
    }
}

// XDG shortcut syntax uses uppercase modifier names and XKB keysym names.
// The portal may ignore these preferences; its reported trigger is authoritative.
fn portal_trigger(value: &str) -> Result<String, String> {
    let mut mods = [false; 4];
    let mut key = None;
    for part in value.split('+').map(str::trim) {
        let index = match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(0),
            "alt" => Some(1),
            "shift" => Some(2),
            "meta" | "super" => Some(3),
            _ => None,
        };
        if let Some(i) = index {
            if mods[i] {
                return Err(format!("duplicate shortcut modifier: {part}"));
            }
            mods[i] = true;
        } else {
            let upper = part.to_ascii_uppercase();
            let valid = (upper.len() == 1 && upper.as_bytes()[0].is_ascii_alphanumeric())
                || upper
                    .strip_prefix('F')
                    .and_then(|n| n.parse::<u8>().ok())
                    .is_some_and(|n| (1..=24).contains(&n));
            if !valid || key.is_some() {
                return Err(format!("invalid shortcut: {value}"));
            }
            key = Some(upper);
        }
    }
    let key = key.ok_or_else(|| format!("shortcut has no key: {value}"))?;
    let mut result = String::new();
    for (enabled, name) in mods.into_iter().zip(["CTRL+", "ALT+", "SHIFT+", "LOGO+"]) {
        if enabled {
            result.push_str(name);
        }
    }
    if key.len() == 1 {
        result.push_str(&key.to_ascii_lowercase());
    } else {
        result.push_str(&key);
    }
    Ok(result)
}
fn describe(shortcuts: &[Shortcut]) -> String {
    let binding = |id| {
        shortcuts
            .iter()
            .find(|s| s.id() == id)
            .map(|s| s.trigger_description())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("unassigned")
    };
    let pick = binding(PICK);
    let history = binding(HISTORY);
    let result = format!("GlobalShortcuts desktop bindings: Pick: {pick}; History: {history}");
    if pick == "unassigned" || history == "unassigned" {
        format!("{result}. {FALLBACK}")
    } else {
        result
    }
}

fn report(status: &watch::Sender<String>, message: impl AsRef<str>) {
    status.send_replace(format!("{} {FALLBACK}", message.as_ref()));
}

struct Failure {
    message: String,
    retry: bool,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self {
            message,
            retry: true,
        }
    }
}
impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Self::from(message.to_owned())
    }
}

fn portal_failure(context: &str, error: ashpd::Error) -> Failure {
    let retry = !matches!(
        &error,
        ashpd::Error::Response(_)
            | ashpd::Error::Portal(ashpd::PortalError::NotAllowed(_))
            | ashpd::Error::Portal(ashpd::PortalError::Cancelled(_))
    ) && !matches!(
        &error,
        ashpd::Error::Zbus(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied"
                || name.as_str() == "org.freedesktop.portal.Error.NotAllowed"
                || name.as_str() == "org.freedesktop.portal.Error.Cancelled"
    );
    Failure {
        message: format!("{context}: {error}"),
        retry,
    }
}

fn registration_failure(error: ashpd::Error) -> Failure {
    let mut failure = portal_failure("host app registration failed", error);
    failure.message.push_str(&format!(
        ". If the portal cannot find the app, install {TRAY_APP_ID}.desktop in an XDG applications directory visible to the desktop session, then restart the tray. See Swatch Board troubleshooting"
    ));
    failure
}

async fn run(
    mut requests: watch::Receiver<Option<(String, String, bool)>>,
    status: watch::Sender<String>,
    events: mpsc::UnboundedSender<Action>,
) {
    let mut configure_once = true;
    loop {
        let Some((pick, history, configure)) = requests.borrow_and_update().clone() else {
            break;
        };
        let triggers = match (portal_trigger(&pick), portal_trigger(&history)) {
            (Ok(a), Ok(b)) if !a.eq_ignore_ascii_case(&b) => (a, b),
            _ => {
                report(
                    &status,
                    "Global shortcuts not registered: invalid or conflicting shortcut preferences.",
                );
                if requests.changed().await.is_err() {
                    break;
                }
                configure_once = true;
                continue;
            }
        };
        status.send_replace(
            "Global shortcuts connecting; portal authorization may be required".into(),
        );
        let result = session(
            &triggers,
            configure && configure_once,
            &status,
            &events,
            &mut requests,
        )
        .await;
        configure_once = false;
        if let Some(error) = result {
            if error.retry {
                report(
                    &status,
                    format!(
                        "Global shortcuts unavailable: {}. Retrying in {} seconds.",
                        error.message,
                        RETRY.as_secs()
                    ),
                );
                tokio::select! {
                    _ = tokio::time::sleep(RETRY) => {},
                    changed = requests.changed() => {
                        if changed.is_err() { break; }
                        configure_once = true;
                    },
                }
            } else {
                report(
                    &status,
                    format!(
                        "Global shortcuts denied or cancelled: {}. Retry explicitly after changing permission.",
                        error.message
                    ),
                );
                if requests.changed().await.is_err() {
                    break;
                }
                configure_once = true;
            }
        } else {
            configure_once = true;
        }
    }
    status.send_replace("Global shortcuts stopped".into());
}

async fn session(
    triggers: &(String, String),
    configure: bool,
    status: &watch::Sender<String>,
    events: &mpsc::UnboundedSender<Action>,
    requests: &mut watch::Receiver<Option<(String, String, bool)>>,
) -> Option<Failure> {
    let setup = async {
        let connection = timeout(REQUEST_TIMEOUT, zbus::Connection::session())
            .await
            .map_err(|_| Failure::from("portal bus connection timed out"))?
            .map_err(|e| Failure::from(format!("portal bus connection failed: {e}")))?;
        timeout(
            REQUEST_TIMEOUT,
            ashpd::register_host_app_with_connection(
                connection.clone(),
                TRAY_APP_ID
                    .parse()
                    .map_err(|e| Failure::from(format!("invalid app identity: {e}")))?,
            ),
        )
        .await
        .map_err(|_| Failure::from("host app registration timed out"))?
        .map_err(registration_failure)?;
        let portal = timeout(
            REQUEST_TIMEOUT,
            GlobalShortcuts::with_connection(connection),
        )
        .await
        .map_err(|_| Failure::from("portal connection timed out".to_owned()))?
        .map_err(|e| portal_failure("GlobalShortcuts portal not supported", e))?;
        let session = timeout(REQUEST_TIMEOUT, portal.create_session(Default::default()))
            .await
            .map_err(|_| Failure::from("portal session creation timed out".to_owned()))?
            .map_err(|e| portal_failure("portal session creation failed", e))?;
        Ok::<_, Failure>((portal, session))
    };
    let (portal, session) = tokio::select! {
        result = setup => match result { Ok(value) => value, Err(error) => return Some(error) },
        _ = requests.changed() => return None,
    };
    let result = active_session(
        &portal, &session, triggers, configure, status, events, requests,
    )
    .await
    .err();
    // Await Close even when the bind future was cancelled by rebind or shutdown.
    let _ = timeout(REQUEST_TIMEOUT, session.close()).await;
    result
}

async fn active_session(
    portal: &GlobalShortcuts,
    session: &ashpd::desktop::Session<GlobalShortcuts>,
    triggers: &(String, String),
    configure: bool,
    status: &watch::Sender<String>,
    events: &mpsc::UnboundedSender<Action>,
    requests: &mut watch::Receiver<Option<(String, String, bool)>>,
) -> Result<(), Failure> {
    let encoded = zbus::zvariant::to_bytes(
        zbus::zvariant::serialized::Context::new_dbus(zbus::zvariant::LE, 0),
        session,
    )
    .map_err(|e| e.to_string())?;
    let (path, _) = encoded
        .deserialize::<zbus::zvariant::ObjectPath<'_>>()
        .map_err(|e| e.to_string())?;
    let path = path.to_string();
    let activated = portal
        .receive_activated()
        .await
        .map_err(|e| format!("activation subscription failed: {e}"))?;
    let changed = portal
        .receive_shortcuts_changed()
        .await
        .map_err(|e| format!("change subscription failed: {e}"))?;
    let closed = session
        .receive_closed()
        .await
        .map_err(|e| format!("session close subscription failed: {e}"))?;
    futures_util::pin_mut!(activated, changed, closed);
    let shortcuts = [
        NewShortcut::new(PICK, "Pick a color with Swatch Board")
            .preferred_trigger(Some(triggers.0.as_str())),
        NewShortcut::new(HISTORY, "Show Swatch Board color history")
            .preferred_trigger(Some(triggers.1.as_str())),
    ];
    let bind = portal.bind_shortcuts(session, &shortcuts, None, Default::default());
    let response = tokio::select! {
        result = bind => result.map_err(|e| portal_failure("bind request failed", e))?.response().map_err(|e| portal_failure("shortcut authorization failed", e))?,
        _ = requests.changed() => return Ok(()),
        _ = closed.next() => return Err("portal session closed during authorization".into()),
    };
    let assignment_error = if configure {
        kde::apply(triggers, response.shortcuts()).await.err()
    } else {
        None
    };
    let binding_status = |shortcuts: &[Shortcut]| {
        let description = describe(shortcuts);
        match &assignment_error {
            Some(error) => format!(
                "{description}. KDE shortcut reassignment failed: {error}. If an assignment was attempted, changes may be partial; inspect both bindings before retrying"
            ),
            None => description,
        }
    };
    status.send_replace(binding_status(response.shortcuts()));
    let mut check = tokio::time::interval(Duration::from_secs(30));
    check.tick().await;
    loop {
        tokio::select! {
            _ = requests.changed() => return Ok(()),
            event = activated.next() => match event {
                Some(event) if event.session_handle().as_str() == path => {
                    let action = match event.shortcut_id() { PICK => Some(Action::Pick), HISTORY => Some(Action::History), _ => None };
                    if let Some(action) = action { if events.send(action).is_err() { return Ok(()); } }
                },
                Some(_) => {},
                None => return Err("portal activation stream ended".into()),
            },
            event = changed.next() => match event {
                Some(event) if event.session_handle().as_str() == path => { status.send_replace(binding_status(event.shortcuts())); },
                Some(_) => {},
                None => return Err("portal shortcut change stream ended".into()),
            },
            _ = closed.next() => return Err("portal session closed or revoked".into()),
            _ = check.tick() => {
                // Polling catches a restarted portal even when its D-Bus signal
                // streams remain open and no Closed signal was sent.
                let response = timeout(REQUEST_TIMEOUT, portal.list_shortcuts(session, Default::default())).await
                    .map_err(|_| "portal health check timed out".to_owned())?
                    .map_err(|e| format!("portal health check failed: {e}"))?
                    .response().map_err(|e| format!("portal health check failed: {e}"))?;
                status.send_replace(binding_status(response.shortcuts()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_error_keeps_portal_detail_and_offers_desktop_entry_help() {
        let error = ashpd::Error::Portal(ashpd::PortalError::NotFound("app not found".into()));
        let failure = registration_failure(error);
        assert!(failure.message.contains("app not found"));
        assert!(
            failure
                .message
                .contains("org.apartment38.SwatchBoard.Tray.desktop")
        );
        assert!(failure.retry);
    }

    #[test]
    fn registration_denial_is_still_not_retried_automatically() {
        let error = ashpd::Error::Portal(ashpd::PortalError::NotAllowed("denied".into()));
        let failure = registration_failure(error);
        assert!(failure.message.contains("denied"));
        assert!(!failure.retry);
    }
}
