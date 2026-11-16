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
    shortcut_status: String,
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
            shortcut_status: "Checking shortcut status…".into(),
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
            "message": self.message, "shortcut_status": self.shortcut_status,
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
                    self.shortcut_status =
                        result.unwrap_or_else(|e| format!("Shortcut status unavailable: {e}"));
                }
                Reply::Applied(result) => {
                    self.busy = false;
                    self.next_status = Instant::now();
                    self.message = match result {
                        Ok(status) => format!("Settings saved and tray reload requested: {status}"),
                        Err(error) => format!(
                            "Settings were saved, but applying them failed: {error}. The tray may still use previous settings; retry Save."
                        ),
                    };
                }
                Reply::Rebound(result) => {
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
                let mut value = request["config"].clone();
                if let Some(text) = value["history_limit"].as_str() {
                    match text.parse::<usize>() {
                        Ok(limit) => value["history_limit"] = json!(limit),
                        Err(_) => {
                            self.message = "Invalid retained history limit.".into();
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
                self.busy = true;
                self.message = "Requesting shortcut reassignment…".into();
                let sender = self.tx.clone();
                std::thread::spawn(move || {
                    let _ = sender.send(Reply::Rebound(call(|p| p.rebind_shortcuts())));
                });
            }
            "save" | "rebind" => {
                self.message =
                    "Settings are busy or could not be loaded; saving is disabled.".into();
            }
            _ => self.message = "Unknown settings operation.".into(),
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
