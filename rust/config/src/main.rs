use serde_json::{Value, json};
use std::{
    num::IntErrorKind,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};
use swatch_board_config_core::{Config, MAX_HISTORY_LIMIT};
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
    message_is_error: bool,
    shortcut_status: String,
    shortcut_status_detail: String,
    validation_field: Option<String>,
    busy: bool,
    status_in_flight: bool,
    next_status: Instant,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
}

fn parse_history_limit(value: &Value) -> Result<usize, String> {
    let range_error = || format!("Stored history limit must be between 1 and {MAX_HISTORY_LIMIT}.");
    let whole_number_error =
        || format!("Enter a valid whole number between 1 and {MAX_HISTORY_LIMIT}.");
    if let Some(text) = value.as_str() {
        match text.parse::<usize>() {
            Ok(limit) if (1..=MAX_HISTORY_LIMIT).contains(&limit) => Ok(limit),
            Ok(_) => Err(range_error()),
            Err(error) if *error.kind() == IntErrorKind::PosOverflow => Err(range_error()),
            Err(_) => Err(whole_number_error()),
        }
    } else if let Some(limit) = value.as_u64() {
        usize::try_from(limit)
            .ok()
            .filter(|limit| (1..=MAX_HISTORY_LIMIT).contains(limit))
            .ok_or_else(range_error)
    } else if value.as_i64().is_some() {
        Err(range_error())
    } else {
        Err(whole_number_error())
    }
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
            message_is_error: false,
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
            "message": self.message, "message_detail": self.message_detail,
            "message_is_error": self.message_is_error,
            "shortcut_status": self.shortcut_status,
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
                        Ok(status) => {
                            self.message_is_error = false;
                            format!("Settings saved and tray reload requested: {status}")
                        }
                        Err(error) => {
                            self.message_is_error = true;
                            self.message_detail = error;
                            "Settings saved, but the tray could not apply them; retry Save.".into()
                        }
                    };
                }
                Reply::Rebound(result) => {
                    self.message_detail.clear();
                    self.busy = false;
                    self.next_status = Instant::now();
                    self.message = match result {
                        Ok(status) => {
                            self.message_is_error = false;
                            format!("Shortcut reassignment requested: {status}")
                        }
                        Err(error) => {
                            self.message_is_error = true;
                            format!("Shortcut reassignment failed: {error}")
                        }
                    };
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
                if let Some(raw_limit) = value.get("history_limit") {
                    match parse_history_limit(raw_limit) {
                        Ok(limit) => value["history_limit"] = json!(limit),
                        Err(message) => {
                            self.validation_field = Some("history_limit".into());
                            self.message = message;
                            self.message_is_error = true;
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
                            self.message_is_error = true;
                        } else {
                            self.config = config;
                            self.busy = true;
                            self.message = "Settings saved; requesting tray reload…".into();
                            self.message_is_error = false;
                            let sender = self.tx.clone();
                            std::thread::spawn(move || {
                                let _ = sender.send(Reply::Applied(call(|p| p.reload_config())));
                            });
                        }
                    }
                    Err(error) => {
                        self.message = format!("Invalid settings: {error}");
                        self.message_is_error = true;
                    }
                }
            }
            "rebind" if !self.busy => {
                self.validation_field = None;
                self.message_detail.clear();
                self.busy = true;
                self.message = "Requesting shortcut reassignment…".into();
                self.message_is_error = false;
                let sender = self.tx.clone();
                std::thread::spawn(move || {
                    let _ = sender.send(Reply::Rebound(call(|p| p.rebind_shortcuts())));
                });
            }
            "save" | "rebind" => {
                self.message_detail.clear();
                self.message =
                    "Settings are busy or could not be loaded; saving is disabled.".into();
                self.message_is_error = true;
            }
            _ => {
                self.message_detail.clear();
                self.message = "Unknown settings operation.".into();
                self.message_is_error = true;
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

        fn reload_config(&self) -> &str {
            "Reloaded"
        }

        fn rebind_shortcuts(&self) -> &str {
            "Rebound"
        }
    }

    fn await_not_busy(settings: &mut Settings) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while settings.busy && Instant::now() < deadline {
            settings.request(json!({"op": "poll"}));
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!settings.busy, "timed out waiting for settings not busy");
    }

    fn test_settings() -> Settings {
        let (tx, rx) = mpsc::channel();
        Settings {
            config: Config::default(),
            load_error: None,
            message: String::new(),
            message_detail: String::new(),
            message_is_error: false,
            shortcut_status: "Checking shortcut status…".into(),
            shortcut_status_detail: String::new(),
            validation_field: None,
            busy: false,
            status_in_flight: false,
            next_status: Instant::now() + Duration::from_secs(3600),
            tx,
            rx,
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
        let initial = settings.request(json!({"op": "init"}));
        assert_eq!(initial["message_is_error"], false);
        let range_message =
            format!("Stored history limit must be between 1 and {MAX_HISTORY_LIMIT}.");
        let whole_number_message =
            format!("Enter a valid whole number between 1 and {MAX_HISTORY_LIMIT}.");
        let mut draft = settings.snapshot()["config"].clone();
        draft["history_limit"] = json!("0");
        let invalid = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(invalid["validation_field"], "history_limit");
        assert_eq!(invalid["message"], range_message);
        assert_eq!(invalid["message_is_error"], true);
        assert_eq!(settings.config.history_limit, 100);
        draft["history_limit"] = json!("abc");
        let non_numeric = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(non_numeric["validation_field"], "history_limit");
        assert_eq!(non_numeric["message"], whole_number_message);
        assert_eq!(non_numeric["message_is_error"], true);
        draft["history_limit"] = json!((MAX_HISTORY_LIMIT as u128 + 1).to_string());
        let above_max = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(above_max["validation_field"], "history_limit");
        assert_eq!(above_max["message"], range_message);
        assert_eq!(above_max["message_is_error"], true);
        assert_eq!(Config::load().unwrap().history_limit, 100);
        draft["history_limit"] = json!(u64::MAX.to_string());
        let max_u64 = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(max_u64["validation_field"], "history_limit");
        assert_eq!(max_u64["message"], range_message);
        assert_eq!(max_u64["message_is_error"], true);
        draft["history_limit"] = json!("99999999999999999999");
        let overflow = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(overflow["validation_field"], "history_limit");
        assert_eq!(overflow["message"], range_message);
        assert_eq!(overflow["message_is_error"], true);
        draft["history_limit"] = json!(MAX_HISTORY_LIMIT.to_string());
        let at_max = settings.request(json!({"op": "save", "config": draft}));
        assert!(at_max["validation_field"].is_null());
        assert_eq!(at_max["message_is_error"], false);
        assert_eq!(Config::load().unwrap().history_limit, MAX_HISTORY_LIMIT);
        await_not_busy(&mut settings);
        assert_eq!(settings.snapshot()["message_is_error"], true);
        draft["history_limit"] = json!("101");
        let valid = settings.request(json!({"op": "save", "config": draft}));
        assert!(valid["validation_field"].is_null());
        assert_eq!(valid["message_is_error"], false);
        assert_eq!(Config::load().unwrap().history_limit, 101);
        await_status(&mut settings, "Shortcut status unavailable");
        await_not_busy(&mut settings);
        assert!(!settings.message_detail.is_empty());
        assert_eq!(settings.snapshot()["message_is_error"], true);
        let rebind_started = settings.request(json!({"op": "rebind"}));
        assert_eq!(rebind_started["message_is_error"], false);
        assert!(settings.message_detail.is_empty());
        assert!(settings.validation_field.is_none());
        await_not_busy(&mut settings);
        assert_eq!(settings.snapshot()["message_is_error"], true);
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
        let saved_with_tray = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(saved_with_tray["message_is_error"], false);
        await_not_busy(&mut settings);
        assert_eq!(settings.snapshot()["message_is_error"], false);
        let rebind_with_tray = settings.request(json!({"op": "rebind"}));
        assert_eq!(rebind_with_tray["message_is_error"], false);
        await_not_busy(&mut settings);
        assert_eq!(settings.snapshot()["message_is_error"], false);
    }

    #[test]
    fn message_is_error_tracks_validation_save_reload_and_rebind_outcomes() {
        let mut settings = test_settings();
        assert_eq!(settings.snapshot()["message_is_error"], false);

        // Invalid history limit sets error severity.
        let mut draft = settings.snapshot()["config"].clone();
        draft["history_limit"] = json!("0");
        let invalid_limit = settings.request(json!({"op": "save", "config": draft}));
        assert_eq!(invalid_limit["message_is_error"], true);

        // Duplicate shortcuts fail in Config::save() and set error severity.
        let mut duplicate_shortcuts = settings.snapshot()["config"].clone();
        duplicate_shortcuts["history_shortcut"] = duplicate_shortcuts["pick_shortcut"].clone();
        let dup = settings.request(json!({"op": "save", "config": duplicate_shortcuts}));
        assert_eq!(dup["message_is_error"], true);
        assert!(
            dup["message"]
                .as_str()
                .unwrap()
                .contains("Could not complete save")
        );

        // Invalid JSON schema sets error severity.
        let bad_schema = settings.request(json!({"op": "save", "config": {"unknown_field": 1}}));
        assert_eq!(bad_schema["message_is_error"], true);

        // Unknown op sets error severity.
        let unknown = settings.request(json!({"op": "bogus"}));
        assert_eq!(unknown["message_is_error"], true);

        // Busy / load_error guard sets error severity.
        settings.load_error = Some("broken config".into());
        let blocked =
            settings.request(json!({"op": "save", "config": settings.snapshot()["config"]}));
        assert_eq!(blocked["message_is_error"], true);
        settings.load_error = None;

        // Reload failure sets error severity; reload success clears it.
        settings.busy = true;
        settings
            .tx
            .send(Reply::Applied(Err("tray offline".into())))
            .unwrap();
        let reload_failed = settings.request(json!({"op": "poll"}));
        assert_eq!(reload_failed["message_is_error"], true);

        settings.busy = true;
        settings
            .tx
            .send(Reply::Applied(Ok("Reloaded".into())))
            .unwrap();
        let reload_ok = settings.request(json!({"op": "poll"}));
        assert_eq!(reload_ok["message_is_error"], false);

        // Rebind failure sets error severity; rebind success clears it.
        settings.busy = true;
        settings
            .tx
            .send(Reply::Rebound(Err("portal rejected".into())))
            .unwrap();
        let rebind_failed = settings.request(json!({"op": "poll"}));
        assert_eq!(rebind_failed["message_is_error"], true);

        settings.busy = true;
        settings
            .tx
            .send(Reply::Rebound(Ok("Rebound".into())))
            .unwrap();
        let rebind_ok = settings.request(json!({"op": "poll"}));
        assert_eq!(rebind_ok["message_is_error"], false);
    }

    #[test]
    fn validates_history_limit_values() {
        assert_eq!(parse_history_limit(&json!("1")), Ok(1));
        assert_eq!(
            parse_history_limit(&json!(MAX_HISTORY_LIMIT.to_string())),
            Ok(MAX_HISTORY_LIMIT)
        );
        assert_eq!(parse_history_limit(&json!(42)), Ok(42));
        assert!(parse_history_limit(&json!("0")).is_err());
        assert!(parse_history_limit(&json!(0)).is_err());
        assert!(parse_history_limit(&json!(-5)).is_err());
        assert!(parse_history_limit(&json!((MAX_HISTORY_LIMIT as u128 + 1).to_string())).is_err());
        assert!(parse_history_limit(&json!(u64::MAX)).is_err());
        assert!(parse_history_limit(&json!(u64::MAX.to_string())).is_err());
        assert!(parse_history_limit(&json!("99999999999999999999")).is_err());
        assert!(parse_history_limit(&json!("not-a-number")).is_err());
    }
}
