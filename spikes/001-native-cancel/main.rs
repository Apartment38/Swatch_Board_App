//! Standalone, opt-in portal probe. No overlay, focus request, or key injection.
use std::{error::Error, time::Duration};

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;

type ProbeResult<T> = Result<T, Box<dyn Error>>;
const SHORTCUT_ID: &str = "cancel-native-pick";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("probe_error={error}");
        std::process::exit(1);
    }
}

async fn run() -> ProbeResult<()> {
    let mut authorize = false;
    let mut duration = 30_u64;
    let mut mode_seen = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--inspect" | "--authorize" if !mode_seen => {
                authorize = arg == "--authorize";
                mode_seen = true;
            }
            "--duration" => {
                duration = args
                    .next()
                    .ok_or("--duration requires seconds (1..=30)")?
                    .parse()?;
                if !(1..=30).contains(&duration) {
                    return Err("--duration must be 1..=30 seconds".into());
                }
            }
            "--help" => {
                println!("phase1a_cancel_probe [--inspect | --authorize] [--duration 1..=30]");
                println!(
                    "Default: inspect version only; --authorize can show a permission/configuration dialog."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown or repeated mode argument: {arg}").into()),
        }
    }

    if authorize {
        // Host registration must precede even portal property/version access.
        // This is our own probe identity, never another application's ID.
        ashpd::register_host_app("org.apartment38.SwatchBoard.NativeProbe".parse()?).await?;
        println!("host_app_registered=org.apartment38.SwatchBoard.NativeProbe");
    }
    let portal = GlobalShortcuts::new().await?;
    println!("global_shortcuts_version={}", portal.version());
    if !authorize {
        println!("mode=inspect; no session created; no authorization requested");
        return Ok(());
    }

    println!("mode=authorize; requesting preferred_trigger=Escape (no fallback)");
    let session = portal.create_session(Default::default()).await?;
    // ashpd's Session Debug implementation prints only its object path, not tokens/secrets.
    println!("session_handle={session:?}");
    // Every returned path after session creation goes through explicit close below.
    // Keep fallible setup inside this block, including subscribing before binding.
    let outcome: ProbeResult<()> = async {
        // Session::path is private in ashpd; serialize its public D-Bus object-path representation.
        let encoded = zbus::zvariant::to_bytes(
            zbus::zvariant::serialized::Context::new_dbus(zbus::zvariant::LE, 0),
            &session,
        )?;
        let (session_path, _) = encoded.deserialize::<zbus::zvariant::ObjectPath<'_>>()?;
        let session_path = session_path.to_string();
        let activated = portal.receive_activated().await?;
        futures_util::pin_mut!(activated);
        let shortcut = NewShortcut::new(SHORTCUT_ID, "Cancel native color selection (feasibility probe)")
            .preferred_trigger(Some("Escape"));
        // None is an empty parent window identifier; this CLI has no GUI window.
        // The timeout bounds activation waiting AFTER the user's bind-dialog response.
        let response = portal.bind_shortcuts(&session, &[shortcut], None, Default::default()).await?.response()?;
        for shortcut in response.shortcuts() {
            println!("bound_shortcut_id={}; actual_trigger_description={:?}", shortcut.id(), shortcut.trigger_description());
        }
        let bound = response.shortcuts().iter().find(|s| s.id() == SHORTCUT_ID)
            .ok_or("portal did not return the requested shortcut; bare Escape is unverified")?;
        if bound.trigger_description().trim().is_empty() {
            return Err("portal returned an unassigned trigger; bare Escape is NOT validated".into());
        }
        // XDG returns localized display text, not a machine-readable effective trigger.
        // Fail closed for a changed/ambiguous assignment rather than validating another shortcut.
        if !matches!(bound.trigger_description().trim(), "Esc" | "Escape") {
            return Err(format!(
                "effective trigger is not recognizable as bare Escape: {:?}; no alternate shortcut accepted (localized text may need manual investigation)",
                bound.trigger_description()
            ).into());
        }
        println!("waiting_for_activation_seconds={duration}; manually press Escape only if authorized");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(duration);
        loop {
            match tokio::time::timeout_at(deadline, activated.next()).await {
                Ok(Some(event)) => {
                    if event.session_handle().as_str() != session_path || event.shortcut_id() != SHORTCUT_ID {
                        continue;
                    }
                    println!("observed_shortcut_id={}; observed_session_handle={}; bound_trigger_description={:?}",
                        event.shortcut_id(), event.session_handle(), bound.trigger_description());
                    println!("activation_observed=true; confirm actual trigger and unchanged overlay focus separately");
                    return Ok(());
                }
                Ok(None) => return Err("Activated stream ended before observation".into()),
                Err(_) => return Err("activation timeout; Escape cancellation remains unverified".into()),
            }
        }
    }.await;

    let close_result = session.close().await;
    match &close_result {
        Ok(()) => println!("session_close=ok"),
        Err(error) => eprintln!("session_close=failed: {error}"),
    }
    // Report both operation and cleanup failures rather than masking one.
    match (outcome, close_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
        (Err(error), Err(close_error)) => {
            Err(format!("{error}; session cleanup also failed: {close_error}").into())
        }
    }
}
