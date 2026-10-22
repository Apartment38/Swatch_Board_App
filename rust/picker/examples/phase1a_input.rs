//! Test-only KWin input driver. Never use this for permission-dialog automation.
//! Protocol vendored unmodified from KDE plasma-wayland-protocols/src/protocols/fake-input.xml.
use std::{error::Error, io, time::Duration};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, globals::registry_queue_init};

mod protocol {
    // Scanner-generated child modules use `super::wayland_client` explicitly.
    #[allow(clippy::single_component_path_imports)]
    use wayland_client;
    pub mod __interfaces {
        wayland_scanner::generate_interfaces!("tests/protocols/fake-input.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!("tests/protocols/fake-input.xml");
}
use protocol::org_kde_kwin_fake_input::{self, OrgKdeKwinFakeInput};

struct State;
// GlobalList tracks globals itself; this single-command client needs no hotplug handling.
impl
    Dispatch<
        wayland_client::protocol::wl_registry::WlRegistry,
        wayland_client::globals::GlobalListContents,
    > for State
{
    fn event(
        _: &mut Self,
        _: &wayland_client::protocol::wl_registry::WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<OrgKdeKwinFakeInput, ()> for State {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinFakeInput,
        event: org_kde_kwin_fake_input::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {}
    }
}

#[derive(Debug)]
enum Action {
    Move(f64, f64),
    Button(u32),
    Key(u32),
    Hold(u64),
}
fn parse(args: &[String]) -> Result<Action, Box<dyn Error>> {
    let usage = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: phase1a_input move X Y | left | right | escape | return | hold SECONDS (1..3600; test device only)",
        )
    };
    match args {
        [command, seconds] if command == "hold" => {
            let seconds: u64 = seconds.parse()?;
            if !(1..=3600).contains(&seconds) {
                return Err(usage().into());
            }
            Ok(Action::Hold(seconds))
        }
        [command, x, y] if command == "move" => {
            let (x, y): (f64, f64) = (x.parse()?, y.parse()?);
            if !x.is_finite() || !y.is_finite() || x.abs() > 8_000_000.0 || y.abs() > 8_000_000.0 {
                return Err(usage().into());
            }
            Ok(Action::Move(x, y))
        }
        [command] => match command.as_str() {
            "left" => Ok(Action::Button(0x110)),
            "right" => Ok(Action::Button(0x111)),
            // Linux evdev key codes, not XKB codes (do not add eight).
            "escape" => Ok(Action::Key(1)),
            "return" => Ok(Action::Key(28)),
            _ => Err(usage().into()),
        },
        _ => Err(usage().into()),
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    // Validate before connecting: malformed invocations never inject input.
    let action = parse(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
    // v4 supplies both absolute pointer motion (v3) and evdev keyboard keys (v4).
    let input: OrgKdeKwinFakeInput = globals
        .bind(&queue.handle(), 4..=6, ())
        .map_err(|error| io::Error::other(format!("KWin fake-input v4+ unavailable: {error}")))?;
    let mut state = State;
    input.authenticate("Swatch Board Phase 1A acceptance tests".into(),
        "Explicit test-only pointer and Escape/right-click injection to verify native picker selection, cancellation, focus and no click-through; never permission-dialog automation".into());
    queue.roundtrip(&mut state)?;
    eprintln!(
        "fake-input version={} action={action:?}; requests are not proof of compositor authorization or delivery",
        input.version()
    );
    match action {
        Action::Hold(seconds) => {
            eprintln!("persistent_test_device_ready seconds={seconds}; no input injected");
            std::thread::sleep(Duration::from_secs(seconds));
        }
        Action::Move(x, y) => input.pointer_motion_absolute(x, y),
        Action::Button(button) => {
            input.button(button, 1);
            queue.roundtrip(&mut state)?;
            std::thread::sleep(Duration::from_millis(60));
            input.button(button, 0);
        }
        Action::Key(key) => {
            input.keyboard_key(key, 1);
            queue.roundtrip(&mut state)?;
            std::thread::sleep(Duration::from_millis(60));
            input.keyboard_key(key, 0);
        }
    }
    queue.roundtrip(&mut state)?;
    if input.version() >= 5 {
        input.destroy();
        queue.roundtrip(&mut state)?;
    }
    eprintln!(
        "requests flushed and compositor roundtrip completed; verify fixture/picker logs for effect"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistent_test_device_duration_is_bounded() {
        assert!(parse(&["hold".into(), "60".into()]).is_ok());
        for seconds in ["0", "3601", "-1", "NaN"] {
            assert!(parse(&["hold".into(), seconds.into()]).is_err());
        }
    }
}
