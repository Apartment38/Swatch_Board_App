use ashpd::desktop::global_shortcuts::Shortcut;
use tokio::time::timeout;

use super::{HISTORY, PICK, REQUEST_TIMEOUT};

const APP: &str = "org.apartment38.SwatchBoard.Tray";
type Keys = Vec<(Vec<i32>,)>;

#[zbus::proxy(
    interface = "org.kde.KGlobalAccel",
    default_service = "org.kde.kglobalaccel",
    default_path = "/kglobalaccel"
)]
trait Accel {
    #[zbus(name = "allActionsForComponent")]
    fn actions(&self, component: &[&str]) -> zbus::Result<Vec<Vec<String>>>;
    #[zbus(name = "shortcutKeys")]
    fn keys(&self, action: &[String]) -> zbus::Result<Keys>;
    #[zbus(name = "globalShortcutAvailable")]
    fn available(&self, key: &(Vec<i32>,), component: &str) -> zbus::Result<bool>;
    #[zbus(name = "setForeignShortcutKeys")]
    fn set_keys(&self, action: &[String], keys: &Keys) -> zbus::Result<()>;
}

async fn request<T>(
    operation: &str,
    future: impl Future<Output = zbus::Result<T>>,
) -> Result<T, String> {
    timeout(REQUEST_TIMEOUT, future)
        .await
        .map_err(|_| format!("KDE {operation} timed out"))?
        .map_err(|error| format!("KDE {operation} failed: {error}"))
}

// Input is the validated XDG trigger produced by portal_trigger. Qt combines
// uppercase key codes with modifier bits, rather than using XKB keysyms.
fn qt_key(trigger: &str) -> Result<i32, String> {
    let mut key = 0;
    for part in trigger.split('+') {
        key |= match part {
            "SHIFT" => 0x0200_0000,
            "CTRL" => 0x0400_0000,
            "ALT" => 0x0800_0000,
            "LOGO" => 0x1000_0000,
            value if value.len() == 1 && value.as_bytes()[0].is_ascii_alphanumeric() => {
                i32::from(value.as_bytes()[0].to_ascii_uppercase())
            }
            value => {
                let number = value
                    .strip_prefix('F')
                    .and_then(|value| value.parse::<i32>().ok())
                    .filter(|number| (1..=24).contains(number))
                    .ok_or_else(|| format!("unsupported KDE shortcut: {trigger}"))?;
                0x0100_0030 + number - 1
            }
        };
    }
    Ok(key)
}

fn assigned(keys: &Keys) -> bool {
    keys.iter()
        .any(|(sequence,)| sequence.iter().any(|key| *key != 0))
}

/// Configure only existing, assigned portal actions on explicit Save/reassign.
/// KGlobalAccel is not a permission API or a transaction: never register actions,
/// never fill empty bindings, and report partial results instead of claiming an
/// atomic change. Concurrent desktop configuration can still race these checks.
pub(super) async fn apply(
    triggers: &(String, String),
    approved: &[Shortcut],
) -> Result<(), String> {
    for id in [PICK, HISTORY] {
        if !approved.iter().any(|shortcut| {
            shortcut.id() == id && !shortcut.trigger_description().trim().is_empty()
        }) {
            return Err("both shortcuts must first be assigned through the desktop's permission/configuration dialog; no bindings changed".into());
        }
    }
    let connection = request("connection", zbus::Connection::session()).await?;
    let bus = request("bus proxy", zbus::fdo::DBusProxy::new(&connection)).await?;
    // Pin this operation to one daemon; a restart must not redirect remaining
    // writes to a replacement daemon with different state.
    let owner = bus.get_name_owner(
        "org.kde.kglobalaccel"
            .try_into()
            .map_err(|e| format!("{e}"))?,
    );
    let owner = timeout(REQUEST_TIMEOUT, owner)
        .await
        .map_err(|_| "KDE service lookup timed out".to_owned())?
        .map_err(|e| format!("KDE shortcut service unavailable: {e}"))?;
    let proxy = request(
        "shortcut proxy",
        AccelProxy::builder(&connection)
            .destination(owner)
            .map_err(|e| e.to_string())?
            .build(),
    )
    .await?;
    let actions = request("action lookup", proxy.actions(&[APP])).await?;
    let mut selected = Vec::with_capacity(2);
    for (id, trigger) in [(PICK, &triggers.0), (HISTORY, &triggers.1)] {
        let matches: Vec<_> = actions
            .iter()
            .filter(|action| action.len() == 4 && action[0] == APP && action[1] == id)
            .collect();
        let [action] = matches.as_slice() else {
            return Err(format!(
                "KDE action {id} is missing or ambiguous; no bindings changed"
            ));
        };
        let current = request("binding lookup", proxy.keys(action)).await?;
        if !assigned(&current) {
            return Err(format!(
                "KDE action {id} is unassigned; approve it in desktop settings first; no bindings changed"
            ));
        }
        let desired = vec![(vec![qt_key(trigger)?, 0, 0, 0],)];
        if current != desired
            && !request("conflict check", proxy.available(&desired[0], "")).await?
        {
            return Err(format!(
                "{trigger} is already assigned (including another Swatch Board action); no bindings changed. Use a free intermediate binding when swapping shortcuts"
            ));
        }
        selected.push((*action, current, desired));
    }
    for (action, original, desired) in &selected {
        // Do not overwrite revocation or a manual edit made since preflight.
        let current = request("binding recheck", proxy.keys(action)).await?;
        if &current != original {
            return Err("desktop bindings changed during Save; some changes may have applied. Inspect actual bindings before retrying".into());
        }
        if original != desired {
            if !request("conflict recheck", proxy.available(&desired[0], "")).await? {
                return Err("a shortcut became occupied during Save; some changes may have applied. Inspect actual bindings before retrying".into());
            }
            request(
                "assignment (possibly partial)",
                proxy.set_keys(action, desired),
            )
            .await?;
        }
    }
    for (action, _, desired) in &selected {
        if request("assignment readback (possibly partial)", proxy.keys(action)).await? != *desired
        {
            return Err("KDE did not retain both requested bindings; some changes may have applied. Inspect actual bindings before retrying".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_portal_triggers_to_qt_key_sequences() {
        assert_eq!(qt_key("LOGO+SHIFT+c").unwrap(), 0x1200_0043);
        assert_eq!(qt_key("CTRL+ALT+F1").unwrap(), 0x0d00_0030);
        assert_eq!(qt_key("SHIFT+F24").unwrap(), 0x0300_0047);
        assert!(qt_key("LOGO+F25").is_err());
    }
}
