use eframe::egui;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};
use swatch_board_config_core::Config;
use swatch_board_ipc::SwatchBoardProxyBlocking;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Swatch Board Settings")
            .with_inner_size([560.0, 700.0])
            .with_min_inner_size([400.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Swatch Board Settings",
        options,
        Box::new(|cc| Ok(Box::new(Settings::new(cc)))),
    )
}

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
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
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

    fn save(&mut self, ctx: &egui::Context) {
        if let Err(error) = self.config.save() {
            self.message = format!(
                "Could not complete save: {error}. Settings have not been reloaded; check the file and autostart entry before retrying."
            );
            return;
        }
        self.message = "Settings saved; requesting tray reload…".into();
        self.busy = true;
        let sender = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = call(|proxy| proxy.reload_config());
            let _ = sender.send(Reply::Applied(result));
            ctx.request_repaint();
        });
    }

    fn rebind(&mut self, ctx: &egui::Context) {
        self.busy = true;
        self.message = "Requesting shortcut reassignment; portal approval may be required…".into();
        let sender = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = call(|proxy| proxy.rebind_shortcuts());
            let _ = sender.send(Reply::Rebound(result));
            ctx.request_repaint();
        });
    }
}

impl eframe::App for Settings {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(reply) = self.rx.try_recv() {
            match reply {
                Reply::Status(result) => {
                    self.status_in_flight = false;
                    self.shortcut_status =
                        result.unwrap_or_else(|e| format!("Shortcut status unavailable: {e}"));
                }
                Reply::Applied(result) => {
                    self.busy = false;
                    self.next_status = Instant::now();
                    match result {
                        Ok(status) => {
                            self.message =
                                format!("Settings saved and tray reload requested: {status}")
                        }
                        Err(error) => {
                            self.message = format!(
                                "Settings were saved, but applying them failed: {error}. The tray may still use previous settings; retry Save."
                            )
                        }
                    }
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
            let repaint = ctx.clone();
            std::thread::spawn(move || {
                let _ = sender.send(Reply::Status(call(|proxy| proxy.shortcut_status())));
                repaint.request_repaint();
            });
        }
        ctx.request_repaint_after(Duration::from_secs(3));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Swatch Board Settings");
            if let Some(error) = &self.load_error {
                ui.colored_label(egui::Color32::RED, format!("Cannot load settings: {error}. Fix the file and reopen settings; saving is disabled to protect it."));
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_enabled_ui(self.load_error.is_none() && !self.busy, |ui| {
                    ui.heading("History");
                    number(ui, "Retained colors", &mut self.config.history_limit, 1..=usize::MAX);
                    number(ui, "Displayed colors", &mut self.config.history_display_limit, 1..=10_000);
                    number(ui, "Columns", &mut self.config.history_columns, 1..=12);
                    number(ui, "Rows", &mut self.config.history_rows, 1..=12);
                    ui.separator();
                    ui.heading("Clipboard");
                    ui.checkbox(&mut self.config.auto_copy, "Automatically copy picked colors");
                    egui::ComboBox::from_label("Format").selected_text(&self.config.clipboard_format).show_ui(ui, |ui| {
                        for format in ["hex", "hex-rgba", "rgb", "rgba", "hsl", "hsv", "cmyk"] {
                            ui.selectable_value(&mut self.config.clipboard_format, format.to_owned(), format);
                        }
                    });
                    ui.separator();
                    ui.heading("Magnifier");
                    number(ui, "Radius (pixels)", &mut self.config.magnifier_radius, 0..=31);
                    number(ui, "Zoom", &mut self.config.magnifier_zoom, 4..=32);
                    ui.separator();
                    ui.heading("Shortcuts");
                    ui.horizontal(|ui| { ui.label("Pick color"); ui.text_edit_singleline(&mut self.config.pick_shortcut); });
                    ui.horizontal(|ui| { ui.label("Show history"); ui.text_edit_singleline(&mut self.config.history_shortcut); });
                    ui.label("Use modifiers and a letter, digit, or F1–F24, e.g. Meta+Shift+C.");
                    ui.label("On KDE, Save requests assignment of both shortcuts after desktop approval. Conflicts or unassigned actions require changes in KDE Shortcuts; check the status below.");
                    ui.separator();
                    ui.checkbox(&mut self.config.hide_board_while_picking, "Hide Board while picking");
                    ui.checkbox(&mut self.config.autostart, "Start tray automatically when signing in");
                });
                ui.separator();
                ui.label(format!("Shortcut status: {}", self.shortcut_status));
                if ui.add_enabled(!self.busy, egui::Button::new("Retry shortcut assignment")).clicked() { self.rebind(ctx); }
                if !self.message.is_empty() { ui.label(&self.message); }
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.load_error.is_none() && !self.busy, egui::Button::new("Save")).clicked() { self.save(ctx); }
                    if ui.button("Cancel / Close").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                });
            });
        });
    }
}

fn number<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(range));
    });
}
