mod mixer;
mod palette;
mod theme;
mod worker;

use eframe::egui;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, Sender},
};
use swatch_board_color::Format;
use swatch_board_config_core::Config;
use swatch_board_storage::Capture;
use wayland_client::{
    Connection, Proxy,
    backend::{Backend, ObjectId},
    protocol::wl_surface::WlSurface,
};
use worker::{Command, Event};

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
enum Tab {
    #[default]
    Mixer,
    Palette,
    Theme,
}

impl Tab {
    const ALL: [(Self, &'static str); 3] = [
        (Self::Mixer, "Color Mixer"),
        (Self::Palette, "Palette Editor"),
        (Self::Theme, "Theme Generator"),
    ];
}

struct Board {
    tab: Tab,
    config: Config,
    history: Vec<Capture>,
    mixer: mixer::Mixer,
    palette: palette::PaletteEditor,
    theme: theme::ThemeEditor,
    history_open: bool,
    connected: bool,
    loaded: bool,
    picking: bool,
    close_action: CloseAction,
    unmap_error: Option<String>,
    message: String,
    error: Option<String>,
    tx: Sender<Command>,
    rx: Receiver<Event>,
}

#[derive(Clone, Copy, PartialEq)]
enum CloseAction {
    Exit,
    Pick,
    Activate,
}

struct BoardApp {
    board: Arc<Mutex<Board>>,
    surface: Result<Option<(usize, usize)>, String>,
}

// Called by eframe while the winit window and display still exist, before the
// renderer and native window are destroyed. A NULL buffer unmaps a Wayland
// surface; the roundtrip makes that unmap visible before starting the picker.
fn unmap_wayland(surface: (usize, usize)) -> Result<(), String> {
    let (display, surface) = surface;
    // SAFETY: eframe invokes on_exit before dropping the window. Both borrowed
    // raw handles remain valid for this call, and the guest backend never owns
    // or closes the display. No proxy escapes this function.
    let backend = unsafe { Backend::from_foreign_display(display as *mut _) };
    let connection = Connection::from_backend(backend);
    let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface as *mut _) }
        .map_err(|error| format!("Cannot access the Board Wayland surface: {error}"))?;
    let surface = WlSurface::from_id(&connection, id)
        .map_err(|error| format!("Cannot access the Board Wayland surface: {error}"))?;
    surface.attach(None, 0, 0);
    surface.commit();
    connection
        .roundtrip()
        .map_err(|error| format!("Cannot unmap the Board window: {error}"))?;
    Ok(())
}

fn window_surface(cc: &eframe::CreationContext<'_>) -> Result<Option<(usize, usize)>, String> {
    let display = cc
        .display_handle()
        .map_err(|error| format!("Cannot access window display: {error}"))?
        .as_raw();
    let window = cc
        .window_handle()
        .map_err(|error| format!("Cannot access Board window: {error}"))?
        .as_raw();
    match (display, window) {
        (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(window)) => Ok(Some((
            display.display.as_ptr() as usize,
            window.surface.as_ptr() as usize,
        ))),
        (
            RawDisplayHandle::Xlib(_) | RawDisplayHandle::Xcb(_),
            RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_),
        ) => Ok(None),
        _ => Err("Unsupported window system for hiding the Board".into()),
    }
}

impl eframe::App for BoardApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        eframe::App::update(&mut *self.board.lock().unwrap(), ctx, frame);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::App::save(&mut *self.board.lock().unwrap(), storage);
    }

    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        let mut board = self.board.lock().unwrap();
        if board.close_action != CloseAction::Exit {
            board.unmap_error = match &self.surface {
                Ok(Some(surface)) => unmap_wayland(*surface).err(),
                Ok(None) => None,
                Err(error) => Some(error.clone()),
            };
        }
    }
}

impl Board {
    fn new(tx: Sender<Command>, rx: Receiver<Event>) -> Self {
        Self {
            tab: Tab::default(),
            config: Config::default(),
            history: Vec::new(),
            mixer: mixer::Mixer::default(),
            palette: palette::PaletteEditor::new(),
            theme: theme::ThemeEditor::new(),
            history_open: true,
            connected: false,
            loaded: false,
            picking: false,
            close_action: CloseAction::Exit,
            unmap_error: None,
            message: "Loading saved history and settings…".into(),
            error: None,
            tx,
            rx,
        }
    }

    fn restore(&mut self) {
        self.picking = false;
        self.message = "Ready. History is saved automatically.".into();
    }

    fn pick(&mut self, ctx: &egui::Context) {
        self.picking = true;
        self.error = None;
        self.message = "Picking… Left release selects; right release or Escape cancels.".into();
        if self.config.hide_board_while_picking {
            self.close_action = CloseAction::Pick;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.tx.send(Command::Pick).is_err() {
            self.restore();
            self.error = Some("Board background service stopped; close and reopen Board.".into());
        }
    }

    fn history_strip(&mut self, ui: &mut egui::Ui) {
        ui.strong("Color history");
        if self.history.is_empty() {
            ui.label(if self.loaded {
                "No captures yet. Pick a color from the tray to begin."
            } else {
                "Loading history…"
            });
            return;
        }
        let format = self
            .config
            .clipboard_format
            .parse::<Format>()
            .unwrap_or(Format::HexRgb);
        egui::ScrollArea::vertical()
            .id_salt("shared_history")
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    for capture in &self.history {
                        ui.vertical(|ui| {
                            let rgba = capture.color;
                            let color =
                                swatch_board_color::Color::new(rgba.r, rgba.g, rgba.b, rgba.a);
                            let response = mixer::swatch(
                                ui,
                                color,
                                egui::vec2(ui.available_width().min(180.0), 44.0),
                            );
                            let response = ui.interact(
                                response.rect,
                                ui.make_persistent_id(capture.id),
                                egui::Sense::click_and_drag(),
                            );
                            response.dnd_set_drag_payload(color);
                            if response.clicked() {
                                if self.tab == Tab::Palette {
                                    self.palette.add_history(color, &mut self.mixer);
                                } else {
                                    self.mixer.select(color);
                                    self.tab = Tab::Mixer;
                                }
                            }
                            response.on_hover_text(format!(
                                "{}\n{}\n{:?}\nClick to select or add; drag to mixer or palette",
                                color.format(Format::HexRgba),
                                capture.captured_at,
                                capture.source
                            ));
                            ui.label(color.format(format));
                        });
                    }
                });
            });
    }
}

impl eframe::App for Board {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.rx.try_recv() {
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
                    self.restore();
                    match result {
                        Ok(()) => self.message = "Pick finished. History reflects saved captures; cancellation adds nothing.".into(),
                        Err(error) => self.error = Some(error),
                    }
                }
                Event::Error(error) => self.error = Some(error),
                Event::Activate if !self.picking => {
                    self.close_action = CloseAction::Activate;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Event::Activate => {}
            }
        }
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Swatch Board");
                ui.separator();
                if ui
                    .add_enabled(
                        self.loaded && self.connected && !self.picking,
                        egui::Button::new("Pick Color"),
                    )
                    .clicked()
                {
                    self.pick(ctx);
                }
                if ui.button("Settings").clicked() && self.tx.send(Command::Configure).is_err() {
                    self.error =
                        Some("Could not open settings: background service stopped.".into());
                }
                if ui
                    .selectable_label(self.history_open, "Color history")
                    .clicked()
                {
                    self.history_open = !self.history_open;
                }
                ui.label(if self.connected {
                    "Tray connected"
                } else {
                    "Standalone · start tray to pick"
                });
            });
            ui.horizontal(|ui| {
                for (tab, label) in Tab::ALL {
                    ui.selectable_value(&mut self.tab, tab, label);
                }
            });
        });
        self.palette.sync_from_mixer(&self.mixer);
        egui::TopBottomPanel::bottom("active_palette")
            .resizable(true)
            .default_height(145.0)
            .min_height(130.0)
            .max_height(220.0)
            .show(ctx, |ui| {
                self.mixer.palette_ui(ui);
                self.palette.sync_from_mixer(&self.mixer);
                self.palette.pinned_ui(ui, &mut self.mixer);
            });
        if self.history_open {
            egui::SidePanel::left("history_sidebar")
                .resizable(true)
                .default_width(200.0)
                .width_range(140.0..=320.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("History");
                        if ui.small_button("Collapse").clicked() {
                            self.history_open = false;
                        }
                    });
                    self.history_strip(ui);
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.button("Dismiss error").clicked() {
                    self.error = None;
                }
                ui.separator();
            }
            ui.label(&self.message);
            ui.add_space(8.0);
            match self.tab {
                Tab::Mixer => {
                    let format = self
                        .config
                        .clipboard_format
                        .parse::<Format>()
                        .unwrap_or(Format::HexRgb);
                    self.mixer.ui(ui, format);
                }
                Tab::Palette => {
                    self.palette.ui(ui, &mut self.mixer);
                }
                Tab::Theme => {
                    self.theme.ui(
                        ui,
                        &self.palette,
                        self.config
                            .clipboard_format
                            .parse::<Format>()
                            .unwrap_or(Format::HexRgb),
                    );
                }
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "tab", &self.tab);
    }
}

fn main() -> eframe::Result {
    let (tx, rx) = match worker::start(egui::Context::default()) {
        Ok(channels) => channels,
        Err(error) if error == "Board is already open" => return Ok(()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let board = Arc::new(Mutex::new(Board::new(tx, rx)));
    let mut first_window = true;
    loop {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Swatch Board")
                .with_app_id("org.apartment38.SwatchBoard.Board")
                .with_inner_size([960.0, 640.0])
                .with_min_inner_size([540.0, 360.0]),
            ..Default::default()
        };
        let shared = board.clone();
        eframe::run_native(
            "Swatch Board",
            options,
            Box::new(move |cc| {
                {
                    let mut board = shared.lock().unwrap();
                    if first_window {
                        board.tab = cc
                            .storage
                            .and_then(|storage| eframe::get_value(storage, "tab"))
                            .unwrap_or_default();
                    }
                    if board
                        .tx
                        .send(Command::RepaintContext(cc.egui_ctx.clone()))
                        .is_err()
                    {
                        board.error = Some(
                            "Board background service stopped; close and reopen Board.".into(),
                        );
                    }
                }
                Ok(Box::new(BoardApp {
                    board: shared,
                    surface: window_surface(cc),
                }))
            }),
        )?;
        first_window = false;
        let mut board = board.lock().unwrap();
        let action = std::mem::replace(&mut board.close_action, CloseAction::Exit);
        if let Some(error) = board.unmap_error.take() {
            board.restore();
            board.error = Some(error);
            continue;
        }
        match action {
            CloseAction::Exit => return Ok(()),
            CloseAction::Activate => continue,
            CloseAction::Pick => {
                if board.tx.send(Command::Pick).is_err() {
                    board.restore();
                    board.error =
                        Some("Board background service stopped; close and reopen Board.".into());
                    continue;
                }
                loop {
                    match board.rx.recv() {
                        Ok(Event::Snapshot {
                            config,
                            history,
                            connected,
                        }) => {
                            board.config = config;
                            board.history = history;
                            board.connected = connected;
                            board.loaded = true;
                        }
                        Ok(Event::PickFinished(result)) => {
                            board.restore();
                            match result {
                                Ok(()) => board.message = "Pick finished. History reflects saved captures; cancellation adds nothing.".into(),
                                Err(error) => board.error = Some(error),
                            }
                            break;
                        }
                        Ok(Event::Error(error)) => board.error = Some(error),
                        Ok(Event::PickStarted | Event::Activate) => {}
                        Err(_) => {
                            board.restore();
                            board.error =
                                Some("Board background service stopped while picking.".into());
                            break;
                        }
                    }
                }
            }
        }
    }
}
