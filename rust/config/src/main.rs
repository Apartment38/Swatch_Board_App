use serde_json::{Value, json};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};
use swatch_board_config_core::Config;
use swatch_board_ipc::SwatchBoardProxyBlocking;

enum Reply {
    Status(Result<String, String>),
    Applied(Result<String, String>),
    Rebound(Result<String, String>),
}

struct Settings {
    config: Config,
    load_error: Option<String>,
    message: String,
    message_detail: String,
    shortcut_status: String,
    shortcut_status_detail: String,
    validation_field: Option<String>,
    busy: bool,
    status_in_flight: bool,
    next_status: Instant,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
}

fn call(
    method: impl FnOnce(&SwatchBoardProxyBlocking<'_>) -> zbus::Result<String>,
) -> Result<String, String> {
    let bus = zbus::blocking::Connection::session().map_err(|e| e.to_string())?;
    let proxy = SwatchBoardProxyBlocking::new(&bus).map_err(|e| e.to_string())?;
    method(&proxy).map_err(|e| e.to_string())
}

impl Settings {
    fn new() -> Self {
        let (config, load_error) = match Config::load() {
            Ok(config) => (config, None),
            Err(error) => (Config::default(), Some(error.to_string())),
        };
        let (tx, rx) = mpsc::channel();
        Self {
            config,
            load_error,
            message: String::new(),
            message_detail: String::new(),
            shortcut_status: "Checking shortcut status…".into(),
            shortcut_status_detail: String::new(),
            validation_field: None,
            busy: false,
            status_in_flight: false,
            next_status: Instant::now(),
            tx,
            rx,
        }
    }

    fn snapshot(&self) -> Value {
        let mut config = serde_json::to_value(&self.config).expect("Config is serializable");
        config["history_limit"] = json!(self.config.history_limit.to_string());
        json!({
            "config": config, "load_error": self.load_error,
            "message": self.message, "message_detail": self.message_detail, "shortcut_status": self.shortcut_status,
            "shortcut_status_detail": self.shortcut_status_detail,
            "validation_field": self.validation_field,
            "busy": self.busy,
        })
    }

    fn request(&mut self, request: Value) -> Value {
        let mut changed = false;
        while let Ok(reply) = self.rx.try_recv() {
            changed = true;
            match reply {
                Reply::Status(result) => {
                    self.status_in_flight = false;
                    match result {
                        Ok(status) => {
                            self.shortcut_status = status;
                            self.shortcut_status_detail.clear();
                        }
                        Err(error) => {
                            self.shortcut_status = "Shortcut status unavailable.".into();
                            self.shortcut_status_detail = error;
                        }
                    }
                }
                Reply::Applied(result) => {
                    self.busy = false;
                    self.next_status = Instant::now();
                    self.message_detail.clear();
                    self.message = match result {
                        Ok(status) => format!("Settings saved and tray reload requested: {status}"),
                        Err(error) => {
                            self.message_detail = error;
                            "Settings saved, but the tray could not apply them; retry Save.".into()
                        }
                    };
                }
                Reply::Rebound(result) => {
                    self.message_detail.clear();
                    self.busy = false;
                    self.next_status = Instant::now();
                    self.message = result
                        .map(|s| format!("Shortcut reassignment requested: {s}"))
                        .unwrap_or_else(|e| format!("Shortcut reassignment failed: {e}"));
                }
            }
        }
        if !self.status_in_flight && Instant::now() >= self.next_status {
            self.status_in_flight = true;
            self.next_status = Instant::now() + Duration::from_secs(3);
            let sender = self.tx.clone();
            std::thread::spawn(move || {
                let _ = sender.send(Reply::Status(call(|proxy| proxy.shortcut_status())));
            });
        }
        match request["op"].as_str().unwrap_or("") {
            "poll" if !changed => return Value::Null,
            "poll" | "init" => {}
            "save" if self.load_error.is_none() && !self.busy => {
                self.validation_field = None;
                self.message_detail.clear();
                let mut value = request["config"].clone();
                if let Some(text) = value["history_limit"].as_str() {
                    match text.parse::<usize>() {
                        Ok(limit) if limit > 0 => value["history_limit"] = json!(limit),
                        Ok(_) => {
                            self.validation_field = Some("history_limit".into());
                            self.message = "Stored history limit must be greater than zero.".into();
                            return self.snapshot();
                        }
                        Err(_) => {
                            self.validation_field = Some("history_limit".into());
                            self.message =
                                "Enter a valid whole number for the stored history limit.".into();
                            return self.snapshot();
                        }
                    }
                }
                match serde_json::from_value::<Config>(value) {
                    Ok(config) => {
                        if let Err(error) = config.save() {
                            self.message = format!(
                                "Could not complete save: {error}. Settings have not been reloaded; check the file and autostart entry before retrying."
                            );
                        } else {
                            self.config = config;
                            self.busy = true;
                            self.message = "Settings saved; requesting tray reload…".into();
                            let sender = self.tx.clone();
                            std::thread::spawn(move || {
                                let _ = sender.send(Reply::Applied(call(|p| p.reload_config())));
                            });
                        }
                    }
                    Err(error) => self.message = format!("Invalid settings: {error}"),
                }
            }
            "rebind" if !self.busy => {
                self.validation_field = None;
                self.message_detail.clear();
                self.busy = true;
                self.message = "Requesting shortcut reassignment…".into();
                let sender = self.tx.clone();
                std::thread::spawn(move || {
                    let _ = sender.send(Reply::Rebound(call(|p| p.rebind_shortcuts())));
                });
            }
            "save" | "rebind" => {
                self.message_detail.clear();
                self.message =
                    "Settings are busy or could not be loaded; saving is disabled.".into();
            }
            _ => {
                self.message_detail.clear();
                self.message = "Unknown settings operation.".into();
            }
        };
        self.snapshot()
    }
}

fn main() {
    let mut settings = Settings::new();
    std::process::exit(swatch_board_qt::run("config", move |request| {
        settings.request(request)
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{process::Command, thread};
    use swatch_board_ipc::{PATH, SERVICE};

    struct TrayFixture;

    #[zbus::interface(name = "org.apartment38.SwatchBoard1")]
    impl TrayFixture {
        fn shortcut_status(&self) -> &str {
            "Global shortcuts ready"
        }
    }

    fn await_status(settings: &mut Settings, expected: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            settings.request(json!({"op": "poll"}));
            if settings.shortcut_status.contains(expected) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("expected {expected:?}, got {:?}", settings.shortcut_status);
    }

    #[test]
    fn status_recovers_when_tray_appears() {
        // The child owns a private bus and XDG root; this avoids altering the
        // user's settings or depending on an already running desktop session.
        if std::env::var_os("SWATCH_CONFIG_TEST_CHILD").is_none() {
            let root =
                std::env::temp_dir().join(format!("swatch-config-test-{}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            let output = Command::new("dbus-run-session")
                .arg("--")
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::status_recovers_when_tray_appears",
                    "--nocapture",
                ])
                .env("SWATCH_CONFIG_TEST_CHILD", "1")
                .env("XDG_CONFIG_HOME", root.join("config"))
                .env("XDG_DATA_HOME", root.join("data"))
                .env("XDG_STATE_HOME", root.join("state"))
                .output()
                .expect("dbus-run-session is required for the Settings integration test");
            let _ = std::fs::remove_dir_all(root);
            assert!(
                output.status.success(),
                "child failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let mut settings = Settings::new();
        settings.request(json!({"op": "init"}));
        let mut draft = settings.snapshot()["config"].clone();
        draft["history_limit"] = json!("0");
        let invalid = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(invalid["validation_field"], "history_limit");
        assert_eq!(
            invalid["message"],
            "Stored history limit must be greater than zero."
        );
        assert_eq!(settings.config.history_limit, 100);
        draft["history_limit"] = json!("99999999999999999999");
        let overflow = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(overflow["validation_field"], "history_limit");
        assert_eq!(
            overflow["message"],
            "Enter a valid whole number for the stored history limit."
        );
        draft["history_limit"] = json!("101");
        let valid = settings.request(json!({"op": "save", "config": draft}));
        assert!(valid["validation_field"].is_null());
        assert_eq!(Config::load().unwrap().history_limit, 101);
        await_status(&mut settings, "Shortcut status unavailable");
        let deadline = Instant::now() + Duration::from_secs(5);
        while settings.busy && Instant::now() < deadline {
            settings.request(json!({"op": "poll"}));
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!settings.message_detail.is_empty());
        settings.request(json!({"op": "rebind"}));
        assert!(settings.message_detail.is_empty());
        assert!(settings.validation_field.is_none());
        let _tray = zbus::blocking::connection::Builder::session()
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .serve_at(PATH, TrayFixture)
            .unwrap()
            .build()
            .unwrap();
        await_status(&mut settings, "Global shortcuts ready");
        assert_eq!(settings.shortcut_status, "Global shortcuts ready");
    }
}
