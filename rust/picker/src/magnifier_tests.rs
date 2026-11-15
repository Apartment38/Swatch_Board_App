//! Private peer-bus tests: never change the user's KWin or desktop settings.
use super::*;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
struct State {
    zoom_loaded: bool,
    magnifier_loaded: bool,
    zoom: f64,
    magnifier: f64,
    supported: bool,
    fail_load: bool,
    fail_toggle: bool,
    defer_reset: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            zoom_loaded: true,
            magnifier_loaded: false,
            zoom: 1.0,
            magnifier: 1.0,
            supported: true,
            fail_load: false,
            fail_toggle: false,
            defer_reset: false,
        }
    }
}

struct Effects(Arc<Mutex<State>>);

#[zbus::interface(name = "org.kde.kwin.Effects")]
impl Effects {
    #[zbus(name = "isEffectLoaded")]
    fn loaded(&self, name: &str) -> bool {
        let state = self.0.lock().unwrap();
        match name {
            "zoom" => state.zoom_loaded,
            "magnifier" => state.magnifier_loaded,
            _ => panic!("unexpected effect"),
        }
    }

    #[zbus(name = "isEffectSupported")]
    fn supported(&self, _name: &str) -> bool {
        self.0.lock().unwrap().supported
    }

    #[zbus(name = "supportInformation")]
    fn info(&self, name: &str) -> String {
        let state = self.0.lock().unwrap();
        let target = match name {
            "zoom" if state.zoom_loaded => state.zoom,
            "magnifier" if state.magnifier_loaded => state.magnifier,
            _ => return String::new(),
        };
        format!("{name}:\ntargetZoom: {target}\n")
    }

    #[zbus(name = "loadEffect")]
    fn load(&self, name: &str) -> bool {
        let mut state = self.0.lock().unwrap();
        match name {
            "magnifier" => {
                if state.fail_load || state.zoom_loaded {
                    return false;
                }
                state.magnifier_loaded = true;
            }
            "zoom" => {
                if state.magnifier_loaded {
                    return false;
                }
                state.zoom_loaded = true;
                // KWin's two effects persist InitialZoom in the same group.
                state.zoom = state.magnifier;
            }
            _ => panic!("unexpected effect"),
        }
        true
    }

    #[zbus(name = "unloadEffect")]
    fn unload(&self, name: &str) {
        let mut state = self.0.lock().unwrap();
        match name {
            "zoom" => state.zoom_loaded = false,
            "magnifier" => state.magnifier_loaded = false,
            _ => panic!("unexpected effect"),
        }
    }
}

struct Shortcuts(Arc<Mutex<State>>);

#[zbus::interface(name = "org.kde.kglobalaccel.Component")]
impl Shortcuts {
    #[zbus(name = "invokeShortcut")]
    fn invoke(&self, name: &str) -> zbus::fdo::Result<()> {
        assert_eq!(name, "view_actual_size");
        let mut state = self.0.lock().unwrap();
        assert!(state.magnifier_loaded && !state.zoom_loaded);
        if state.fail_toggle {
            return Err(zbus::fdo::Error::Failed("shortcut unavailable".into()));
        }
        if state.magnifier <= 1.0 {
            state.magnifier = 2.0;
        } else if state.defer_reset {
            // The toggle cannot reset until KWin has rendered its first frame.
            state.defer_reset = false;
        } else {
            state.magnifier = 1.0;
        }
        Ok(())
    }
}

async fn peer(state: State) -> (Connection, Connection, Arc<Mutex<State>>) {
    let state = Arc::new(Mutex::new(state));
    let (server, client) = tokio::net::UnixStream::pair().unwrap();
    let (server, client) = tokio::join!(
        zbus::connection::Builder::unix_stream(server)
            .server(zbus::Guid::generate())
            .unwrap()
            .p2p()
            .serve_at("/Effects", Effects(state.clone()))
            .unwrap()
            .serve_at("/component/kwin", Shortcuts(state.clone()))
            .unwrap()
            .build(),
        zbus::connection::Builder::unix_stream(client).p2p().build(),
    );
    (server.unwrap(), client.unwrap(), state)
}

#[tokio::test]
async fn cursor_loupe_restores_zoom_even_when_pick_finishes_before_first_frame() {
    let (_server, client, state) = peer(State {
        defer_reset: true,
        ..State::default()
    })
    .await;
    let mut magnifier = Magnifier::start(&client).await.unwrap();
    {
        let state = state.lock().unwrap();
        assert!(state.magnifier_loaded && !state.zoom_loaded);
        assert_eq!(state.magnifier, 2.0);
    }
    magnifier.restore().await.unwrap();
    assert_eq!(*state.lock().unwrap(), State::default());
}

#[tokio::test]
async fn already_active_accessibility_magnification_is_preserved() {
    for original in [
        State {
            zoom: 3.0,
            ..State::default()
        },
        State {
            zoom_loaded: false,
            magnifier_loaded: true,
            magnifier: 4.0,
            ..State::default()
        },
    ] {
        let (_server, client, state) = peer(original.clone()).await;
        let mut magnifier = Magnifier::start(&client).await.unwrap();
        magnifier.restore().await.unwrap();
        assert_eq!(*state.lock().unwrap(), original);
    }
}

#[tokio::test]
async fn preloaded_idle_magnifier_is_not_unloaded_on_close() {
    let original = State {
        zoom_loaded: false,
        magnifier_loaded: true,
        ..State::default()
    };
    let (_server, client, state) = peer(original.clone()).await;
    let mut magnifier = Magnifier::start(&client).await.unwrap();
    assert_eq!(state.lock().unwrap().magnifier, 2.0);
    magnifier.restore().await.unwrap();
    assert_eq!(*state.lock().unwrap(), original);
}

#[tokio::test]
async fn partial_setup_failures_restore_original_effect_before_returning_error() {
    for original in [
        State {
            supported: false,
            ..State::default()
        },
        State {
            fail_load: true,
            ..State::default()
        },
        State {
            fail_toggle: true,
            ..State::default()
        },
    ] {
        let (_server, client, state) = peer(original.clone()).await;
        assert!(Magnifier::start(&client).await.is_err());
        assert_eq!(*state.lock().unwrap(), original);
    }
}

#[test]
fn unreadable_desktop_zoom_is_not_assumed_to_be_unmagnified() {
    for info in [
        "",
        "targetZoom: NaN",
        "targetZoom: inf",
        "targetZoom: 0",
        "other: 1",
    ] {
        assert!(target_zoom(info).is_err(), "{info}");
    }
    assert_eq!(target_zoom("zoom:\ntargetZoom: 2.5\n").unwrap(), 2.5);
}
