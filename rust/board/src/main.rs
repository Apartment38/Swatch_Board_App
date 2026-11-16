mod mixer;
mod palette;
mod theme;
mod worker;

use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, Sender};
use swatch_board_color::{Color, Format};
use swatch_board_config_core::Config;
use swatch_board_storage::Capture;
use worker::{Command, Event};

struct Board {
    config: Config,
    history: Vec<Capture>,
    mixer: mixer::Mixer,
    palette: palette::PaletteEditor,
    theme: theme::ThemeEditor,
    connected: bool,
    loaded: bool,
    picking: bool,
    initial_show_palette: bool,
    message: String,
    error: Option<String>,
    tx: Sender<Command>,
    rx: Receiver<Event>,
}

impl Board {
    fn new(tx: Sender<Command>, rx: Receiver<Event>, show_palette: bool) -> Self {
        Self {
            config: Config::default(),
            history: Vec::new(),
            mixer: mixer::Mixer::default(),
            palette: palette::PaletteEditor::new(),
            theme: theme::ThemeEditor::new(),
            connected: false,
            loaded: false,
            picking: false,
            initial_show_palette: show_palette,
            message: "Loading saved history and settings…".into(),
            error: None,
            tx,
            rx,
        }
    }

    fn format(&self) -> Format {
        self.config
            .clipboard_format
            .parse()
            .unwrap_or(Format::HexRgb)
    }

    fn snapshot(&self, activate: bool, show_palette: bool, clipboard: Option<String>) -> Value {
        let format = self.format();
        json!({
            "mixer": self.mixer.snapshot(format),
            "palette": self.palette.snapshot(),
            "theme": self.theme.snapshot(&self.palette, format),
            "history": self.history.iter().map(|capture| {
                let rgba = capture.color;
                let color = Color::new(rgba.r, rgba.g, rgba.b, rgba.a);
                json!({
                    "color": color.format(Format::HexRgba),
                    "text": color.format(format),
                    "detail": format!("{}\n{:?}", capture.captured_at, capture.source),
                })
            }).collect::<Vec<_>>(),
            "connected": self.connected,
            "loaded": self.loaded,
            "picking": self.picking,
            "hide_board_while_picking": self.config.hide_board_while_picking,
            "message": self.message,
            "error": self.error,
            "activate": activate,
            "show_palette": show_palette,
            "clipboard": clipboard,
        })
    }

    fn request(&mut self, request: Value) -> Value {
        if request["op"].as_str() == Some("palette_preview_generation") {
            return match self.palette.preview_generation(&request) {
                Ok(colors) => json!({"colors": colors}),
                Err(error) => json!({"error": error}),
            };
        }
        let mut changed = false;
        let mut activate = false;
        let mut show_palette = std::mem::take(&mut self.initial_show_palette);
        while let Ok(event) = self.rx.try_recv() {
            changed = true;
            match event {
                Event::Snapshot {
                    config,
                    history,
                    connected,
                } => {
                    self.config = config;
                    self.history = history;
                    self.connected = connected;
                    self.loaded = true;
                    if !self.picking {
                        self.message = "History is saved automatically.".into();
                    }
                }
                Event::PickStarted => {}
                Event::PickFinished(result) => {
                    self.picking = false;
                    self.message = "Pick finished. Cancellation adds nothing to history.".into();
                    if let Err(error) = result {
                        self.error = Some(error);
                    }
                }
                Event::Error(error) => self.error = Some(error),
                Event::Activate => activate = !self.picking,
                Event::ActivatePalette => {
                    activate = !self.picking;
                    show_palette = true;
                }
            }
        }
        let op = request["op"].as_str().unwrap_or("");
        if op == "poll" {
            return if changed {
                self.snapshot(activate, show_palette, None)
            } else {
                Value::Null
            };
        }
        let mut clipboard = None;
        if op != "init" {
            self.error = None;
        }
        let result = match op {
            "init" => Ok(()),
            "dismiss_error" => {
                self.error = None;
                Ok(())
            }
            "pick" => {
                if !self.loaded || !self.connected || self.picking {
                    Err("The tray must be connected and idle before picking.".into())
                } else {
                    match self.tx.send(Command::Pick) {
                        Ok(()) => {
                            self.picking = true;
                            self.error = None;
                            self.message = "Picking… Escape or right release cancels.".into();
                            Ok(())
                        }
                        Err(_) => Err("Board background service stopped; reopen Board.".into()),
                    }
                }
            }
            "configure" => self
                .tx
                .send(Command::Configure)
                .map_err(|_| "Could not open settings: background service stopped.".into()),
            "history_select" => request["index"]
                .as_u64()
                .and_then(|i| self.history.get(i as usize))
                .ok_or_else(|| "History swatch no longer exists.".to_string())
                .map(|capture| {
                    let c = capture.color;
                    let color = Color::new(c.r, c.g, c.b, c.a);
                    if request["tab"].as_u64() == Some(1) {
                        self.palette.add_history(color, &mut self.mixer);
                    } else {
                        self.mixer.select(color);
                    }
                }),
            "mixer_copy" => {
                clipboard = Some(self.mixer.current().format(self.format()));
                Ok(())
            }
            _ if op.starts_with("mixer_") => {
                let result = self.mixer.command(&request);
                self.palette.sync_from_mixer(&self.mixer);
                result
            }
            _ if op.starts_with("palette_") => self.palette.command(&request, &mut self.mixer),
            _ if op.starts_with("theme_") => self.theme.command(&request, &self.palette),
            _ => Err(format!("Unknown Board operation: {op}")),
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
        self.snapshot(activate, show_palette, clipboard)
    }
}

fn main() {
    let show_palette = std::env::args().skip(1).any(|arg| arg == "--palette");
    let (tx, rx) = match worker::start(show_palette) {
        Ok(channels) => channels,
        Err(error) if error == "Board is already open" => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let mut board = Board::new(tx, rx, show_palette);
    std::process::exit(swatch_board_qt::run("board", move |request| {
        board.request(request)
    }));
}
