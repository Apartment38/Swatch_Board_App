use eframe::egui;
use swatch_board_color::{
    Color, Format,
    generation::{GenerationMode, GenerationOptions, generate},
};
use swatch_board_storage::{GenerationRecord, PaletteRepository, Rgba8, SavedPalette};

use crate::mixer::{self, Mixer};

#[derive(Clone, PartialEq)]
struct Draft {
    name: String,
    colors: Vec<Color>,
    locked: Vec<bool>,
    names: Vec<String>,
    id: Option<String>,
}

pub struct PaletteEditor {
    draft: Draft,
    undo: Vec<Draft>,
    redo: Vec<Draft>,
    repository: Option<PaletteRepository>,
    saved: Vec<SavedPalette>,
    generations: Vec<GenerationRecord>,
    options: GenerationOptions,
    use_base: bool,
    base: String,
    use_contrast: bool,
    contrast: f64,
    search: String,
    name_edits: Vec<String>,
    edit: Vec<String>,
    error: Option<String>,
    lock_history: bool,
    /// Visible feedback about the last refused or completed swatch action.
    status: Option<String>,
    /// Swatch whose HEX field should receive keyboard focus on the next frame.
    pending_hex_focus: Option<usize>,
    /// Swatch the persistent action bar applies to.
    selected: Option<usize>,
}

fn to_rgba(c: Color) -> Rgba8 {
    Rgba8 {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}
fn from_rgba(c: Rgba8) -> Color {
    Color::new(c.r, c.g, c.b, c.a)
}

impl PaletteEditor {
    pub fn new() -> Self {
        let (repository, error) = match PaletteRepository::open_default() {
            Ok(repository) => (Some(repository), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let mut this = Self {
            draft: Draft {
                name: "Untitled".into(),
                colors: Vec::new(),
                names: Vec::new(),
                locked: Vec::new(),
                id: None,
            },
            undo: Vec::new(),
            redo: Vec::new(),
            repository,
            saved: Vec::new(),
            generations: Vec::new(),
            options: GenerationOptions::default(),
            use_base: false,
            base: "#247AC3".into(),
            use_contrast: false,
            contrast: 4.5,
            name_edits: Vec::new(),
            search: String::new(),
            edit: Vec::new(),
            error,
            lock_history: false,
            status: None,
            pending_hex_focus: None,
            selected: None,
        };
        this.refresh();
        this
    }

    fn refresh(&mut self) {
        if let Some(repo) = &self.repository {
            match (repo.list(), repo.generations()) {
                (Ok(saved), Ok(generations)) => {
                    self.saved = saved;
                    self.generations = generations;
                }
                (Err(e), _) | (_, Err(e)) => self.error = Some(e.to_string()),
            }
        }
    }

    fn fresh_name(names: &[String]) -> String {
        (1..)
            .map(|n| format!("Swatch {n}"))
            .find(|name| !names.contains(name))
            .unwrap()
    }

    pub fn theme_palette(&self) -> (Option<&str>, &str, Vec<(String, Color)>) {
        (
            self.draft.id.as_deref(),
            &self.draft.name,
            self.draft
                .names
                .iter()
                .cloned()
                .zip(self.draft.colors.iter().copied())
                .collect(),
        )
    }

    fn sync_fields(&mut self) {
        self.edit = self
            .draft
            .colors
            .iter()
            .map(|c| c.format(Format::HexRgba))
            .collect();
        self.name_edits = self.draft.names.clone();
    }

    fn commit(&mut self, previous: Draft, mixer: &mut Mixer) {
        if self.draft != previous {
            if self.undo.len() == 100 {
                self.undo.remove(0);
            }
            self.undo.push(previous);
            self.redo.clear();
            mixer.replace_palette(self.draft.colors.clone());
            self.sync_fields();
        }
    }

    pub fn sync_from_mixer(&mut self, mixer: &Mixer) {
        if self.draft.colors != mixer.colors() {
            let previous = self.draft.clone();
            self.draft.colors = mixer.colors().to_vec();
            self.draft.locked.resize(self.draft.colors.len(), false);
            self.draft.names.truncate(self.draft.colors.len());
            while self.draft.names.len() < self.draft.colors.len() {
                self.draft.names.push(Self::fresh_name(&self.draft.names));
            }
            if self.undo.len() == 100 {
                self.undo.remove(0);
            }
            self.undo.push(previous);
            self.redo.clear();
            self.sync_fields();
        }
    }

    fn restore(&mut self, draft: Draft, mixer: &mut Mixer) {
        self.draft = draft;
        mixer.replace_palette(self.draft.colors.clone());
        self.sync_fields();
    }

    pub fn add_history(&mut self, color: Color, mixer: &mut Mixer) {
        if self.draft.colors.len() >= 64 {
            self.error = Some("A palette can contain at most 64 colors.".into());
            return;
        }
        let previous = self.draft.clone();
        self.draft.colors.push(color);
        self.draft.names.push(Self::fresh_name(&self.draft.names));
        self.draft.locked.push(self.lock_history);
        self.commit(previous, mixer);
    }

    fn open_palette(&mut self, palette: &SavedPalette, mixer: &mut Mixer) {
        let previous = self.draft.clone();
        self.draft = Draft {
            name: palette.name.clone(),
            colors: palette.colors.iter().copied().map(from_rgba).collect(),
            locked: palette.locked.clone(),
            names: palette.swatch_names.clone(),
            id: Some(palette.id.clone()),
        };
        self.commit(previous, mixer);
    }

    fn generate(&mut self, mixer: &mut Mixer) {
        let mut options = self.options.clone();
        options.base = if self.use_base {
            match self.base.parse::<Color>() {
                Ok(c) => Some(c),
                Err(_) => {
                    self.error = Some("Invalid base color; use #RRGGBB or #RRGGBBAA.".into());
                    return;
                }
            }
        } else {
            None
        };
        options.minimum_contrast = self.use_contrast.then_some(self.contrast);
        let generated = match generate(&options) {
            Ok(colors) => colors,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let previous = self.draft.clone();
        // A requested size smaller than the locked population would discard locks.
        if self.draft.locked.iter().filter(|&&x| x).count() > options.count {
            self.error = Some("Palette size is smaller than the number of locked swatches.".into());
            return;
        }
        let locked: Vec<(Color, String)> = self
            .draft
            .colors
            .iter()
            .copied()
            .zip(self.draft.locked.iter().copied())
            .zip(self.draft.names.iter().cloned())
            .enumerate()
            .filter_map(|(i, ((color, lock), name))| {
                (lock && i >= options.count).then_some((color, name))
            })
            .collect();
        let mut colors = generated;
        let mut locks = vec![false; options.count];
        let mut names: Vec<String> = self
            .draft
            .names
            .iter()
            .take(options.count)
            .cloned()
            .collect();
        while names.len() < options.count {
            names.push(Self::fresh_name(&names));
        }
        for (i, (&color, &lock)) in self
            .draft
            .colors
            .iter()
            .zip(&self.draft.locked)
            .enumerate()
            .take(options.count)
        {
            if lock {
                colors[i] = color;
                locks[i] = true;
            }
        }
        for (color, name) in locked {
            if let Some(i) = locks.iter().position(|locked| !locked) {
                colors[i] = color;
                locks[i] = true;
                names[i] = name;
            }
        }
        if let Some(repo) = &mut self.repository {
            let values: Vec<_> = colors.iter().copied().map(to_rgba).collect();
            if let Err(e) = repo.record_generation(
                &format!("{:?} · seed {}", options.mode, options.seed),
                &values,
                &locks,
            ) {
                self.error = Some(e.to_string());
                return;
            }
        } else {
            self.error = Some("Storage unavailable; generation was not recorded.".into());
            return;
        }
        self.draft.colors = colors;
        self.draft.names = names;
        self.draft.locked = locks;
        self.commit(previous, mixer);
        self.error = None;
        self.refresh();
    }

    fn save(&mut self) {
        let Some(repo) = &mut self.repository else {
            self.error = Some("Palette storage unavailable.".into());
            return;
        };
        let colors: Vec<_> = self.draft.colors.iter().copied().map(to_rgba).collect();
        match repo.save_named(
            self.draft.id.as_deref(),
            &self.draft.name,
            &colors,
            &self.draft.locked,
            &self.draft.names,
        ) {
            Ok(palette) => {
                self.draft.id = Some(palette.id);
                self.draft.name = palette.name;
                self.error = None;
                self.refresh();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Draw the pinned working palette.
    ///
    /// The safety-relevant actions (lock/unlock, edit, delete, reorder) are
    /// always painted in a labelled action bar, never hidden behind hover, so
    /// they are discoverable with a pointer and reachable with the keyboard.
    /// The action bar names the swatch it applies to and states the lock state
    /// in words, never by colour alone.
    pub fn pinned_ui(&mut self, ui: &mut egui::Ui, mixer: &mut Mixer) {
        let mut action = None;
        let count = self.draft.colors.len();
        if count == 0 {
            self.selected = None;
        } else if self.selected.is_none_or(|i| i >= count) {
            self.selected = Some(0);
        }

        ui.horizontal_wrapped(|ui| {
            match self.selected {
                Some(i) => {
                    let locked = self.draft.locked[i];
                    let name = self
                        .draft
                        .names
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| format!("Swatch {}", i + 1));
                    ui.label(format!(
                        "{}/{} {name} ({})",
                        i + 1,
                        count,
                        if locked { "Locked" } else { "Unlocked" }
                    ));
                    let lock = ui
                        .small_button(if locked { "Unlock" } else { "Lock" })
                        .on_hover_text(if locked {
                            "Unlock this swatch so editing, deleting and replacing it are allowed again"
                        } else {
                            "Lock this swatch so editing, deleting and replacing it are refused"
                        });
                    if lock.clicked() {
                        action = Some(Action::Lock(i, !locked));
                    }
                    Self::paint_focus_ring(ui, &lock);
                    let edit = ui
                        .small_button("Edit")
                        .on_hover_text("Edit this swatch's HEX color");
                    if edit.clicked() {
                        action = Some(Action::Edit(i));
                    }
                    Self::paint_focus_ring(ui, &edit);
                    let delete = ui
                        .small_button("Delete")
                        .on_hover_text("Delete this swatch");
                    if delete.clicked() {
                        action = Some(Action::Delete(i));
                    }
                    Self::paint_focus_ring(ui, &delete);
                    let earlier = ui
                        .add_enabled(i > 0, egui::Button::new("Earlier").small())
                        .on_hover_text("Move this swatch earlier in the palette");
                    if earlier.clicked() {
                        action = Some(Action::MoveEarlier(i));
                    }
                    Self::paint_focus_ring(ui, &earlier);
                    let later = ui
                        .add_enabled(i + 1 < count, egui::Button::new("Later").small())
                        .on_hover_text("Move this swatch later in the palette");
                    if later.clicked() {
                        action = Some(Action::MoveLater(i));
                    }
                    Self::paint_focus_ring(ui, &later);
                }
                None => {
                    ui.label("The pinned palette has no swatches yet.");
                }
            }
            if let Some(status) = &self.status {
                ui.label(status.clone());
            }
        });

        // Make the existing horizontal navigation discoverable without hover.
        egui::ScrollArea::horizontal()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .id_salt("active_palette_swatches")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for i in 0..self.draft.colors.len() {
                        ui.push_id(i, |ui| {
                            ui.vertical(|ui| {
                                let painted = mixer::swatch(
                                    ui,
                                    self.draft.colors[i],
                                    egui::vec2(104.0, 48.0),
                                );
                                let rect = painted.rect;
                                let response = ui.interact(
                                    rect,
                                    ui.id().with("swatch_drag"),
                                    egui::Sense::click_and_drag(),
                                );
                                response.dnd_set_drag_payload(PaletteDrag {
                                    index: i,
                                    color: self.draft.colors[i],
                                });
                                if response.dnd_hover_payload::<PaletteDrag>().is_some() {
                                    if let Some(source) =
                                        response.dnd_release_payload::<PaletteDrag>()
                                    {
                                        if source.index != i {
                                            action = Some(Action::Move(source.index, i));
                                        }
                                    }
                                } else if response.dnd_hover_payload::<Color>().is_some() {
                                    if let Some(color) = response.dnd_release_payload::<Color>() {
                                        if self.draft.locked[i] {
                                            self.status = Some(format!(
                                                "Swatch {} is locked. Unlock it before replacing its color.",
                                                i + 1
                                            ));
                                        } else {
                                            action = Some(Action::Set(i, *color));
                                        }
                                    }
                                }
                                if response.clicked() {
                                    self.selected = Some(i);
                                }
                                if self.draft.locked[i] {
                                    Self::paint_locked_badge(ui, rect);
                                }
                                if self.selected == Some(i) {
                                    ui.painter().rect_stroke(
                                        rect.expand(1.0),
                                        4.0,
                                        egui::Stroke::new(2.0_f32, ui.visuals().selection.stroke.color),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                if response.has_focus() {
                                    ui.painter().rect_stroke(
                                        rect.expand(2.5),
                                        2.0,
                                        egui::Stroke::new(2.0_f32, ui.visuals().text_color()),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                                let hex = ui.add_enabled(
                                    !self.draft.locked[i],
                                    egui::TextEdit::singleline(&mut self.edit[i])
                                        .desired_width(104.0),
                                );
                                if self.pending_hex_focus == Some(i) {
                                    self.pending_hex_focus = None;
                                    hex.request_focus();
                                }
                                if hex.lost_focus() && action.is_none() && !self.draft.locked[i] {
                                    action = Some(Action::Apply(i));
                                }
                                let name_edit = ui.add(
                                    egui::TextEdit::singleline(&mut self.name_edits[i])
                                        .desired_width(104.0)
                                        .hint_text("Swatch name"),
                                );
                                if name_edit.lost_focus()
                                    && action.is_none()
                                    && self.name_edits[i] != self.draft.names[i]
                                {
                                    let name = &self.name_edits[i];
                                    if name.trim().is_empty()
                                        || self
                                            .draft
                                            .names
                                            .iter()
                                            .enumerate()
                                            .any(|(j, existing)| j != i && existing == name)
                                    {
                                        self.error = Some(
                                            "Swatch names must be nonempty and unique.".into(),
                                        );
                                        self.name_edits[i] = self.draft.names[i].clone();
                                    } else {
                                        action = Some(Action::Rename(i, name.clone()));
                                    }
                                }
                            });
                        });
                    }
                });
            });
        if let Some(action) = action {
            let previous = self.draft.clone();
            match action {
                Action::Set(i, c) => {
                    self.draft.colors[i] = c;
                    self.status = Some(format!("Swatch {} color replaced.", i + 1));
                }
                Action::Rename(i, name) => self.draft.names[i] = name,
                Action::Lock(i, value) => {
                    self.draft.locked[i] = value;
                    self.status = Some(if value {
                        format!("Swatch {} locked.", i + 1)
                    } else {
                        format!("Swatch {} unlocked.", i + 1)
                    });
                }
                Action::Edit(i) => {
                    if self.draft.locked[i] {
                        self.status = Some(format!(
                            "Swatch {} is locked. Unlock it before editing.",
                            i + 1
                        ));
                    } else {
                        self.pending_hex_focus = Some(i);
                        self.status = Some(format!(
                            "Editing swatch {}: type a HEX value and press Enter.",
                            i + 1
                        ));
                    }
                }
                Action::Delete(i) => {
                    if self.draft.locked[i] {
                        self.status = Some(format!(
                            "Swatch {} is locked. Unlock it before deleting.",
                            i + 1
                        ));
                    } else {
                        let name = self
                            .draft
                            .names
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("Swatch {}", i + 1));
                        self.draft.colors.remove(i);
                        self.draft.locked.remove(i);
                        self.draft.names.remove(i);
                        self.selected = None;
                        self.status = Some(format!("{name} deleted."));
                    }
                }
                Action::Move(from, to) => self.move_swatch(from, to),
                Action::MoveEarlier(i) => self.move_swatch(i, i - 1),
                Action::MoveLater(i) => self.move_swatch(i, i + 1),
                Action::Apply(i) => match self.edit[i].parse::<Color>() {
                    Ok(c) => {
                        self.draft.colors[i] = c;
                        self.error = None;
                        self.status = Some(format!("Swatch {} color updated.", i + 1));
                    }
                    Err(_) => self.error = Some("Invalid color; swatch unchanged.".into()),
                },
            }
            self.commit(previous, mixer);
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error.clone());
        }
    }

    /// Paint a visible focus ring around a keyboard-focused control.
    fn paint_focus_ring(ui: &egui::Ui, response: &egui::Response) {
        if response.has_focus() {
            ui.painter().rect_stroke(
                response.rect.expand(2.0),
                4.0,
                egui::Stroke::new(2.0_f32, ui.visuals().text_color()),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Move a swatch, keeping its color, name and lock state together.
    fn move_swatch(&mut self, from: usize, to: usize) {
        if from >= self.draft.colors.len() || to >= self.draft.colors.len() || from == to {
            return;
        }
        let color = self.draft.colors.remove(from);
        let name = self.draft.names.remove(from);
        let locked = self.draft.locked.remove(from);
        self.draft.colors.insert(to, color);
        self.draft.names.insert(to, name);
        self.draft.locked.insert(to, locked);
        self.selected = Some(to);
        self.status = Some(format!("Swatch moved to position {}.", to + 1));
    }

    /// Paint a readable, non-color-only "Locked" badge on a swatch.
    fn paint_locked_badge(ui: &egui::Ui, rect: egui::Rect) {
        let galley = ui.painter().layout_no_wrap(
            "Locked".to_owned(),
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
        let pos = rect.left_top() + egui::vec2(4.0, 3.0);
        let badge = egui::Rect::from_min_size(
            pos - egui::vec2(3.0, 1.0),
            galley.size() + egui::vec2(6.0, 2.0),
        );
        ui.painter()
            .rect_filled(badge, 3.0, egui::Color32::from_black_alpha(180));
        ui.painter().galley(pos, galley, egui::Color32::WHITE);
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mixer: &mut Mixer) {
        self.sync_from_mixer(mixer);
        egui::ScrollArea::vertical().id_salt("palette_editor").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Palette Editor");
                if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).clicked() {
                    if let Some(draft) = self.undo.pop() { self.redo.push(self.draft.clone()); self.restore(draft, mixer); }
                }
                if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).clicked() {
                    if let Some(draft) = self.redo.pop() { self.undo.push(self.draft.clone()); self.restore(draft, mixer); }
                }
            });
            if let Some(error) = &self.error { ui.colored_label(ui.visuals().error_fg_color, error); }
            ui.horizontal_wrapped(|ui| {
                ui.label("Name");
                let previous = self.draft.clone();
                if ui.text_edit_singleline(&mut self.draft.name).changed() { self.commit(previous, mixer); }
                if ui.button("New draft").clicked() {
                    let previous = self.draft.clone();
                    self.draft = Draft { name: "Untitled".into(), colors: Vec::new(), locked: Vec::new(), names: Vec::new(), id: None };
                    self.commit(previous, mixer);
                }
                if ui.button("Save palette").clicked() { self.save(); }
                if ui.button("Duplicate as draft").clicked() {
                    let previous = self.draft.clone(); self.draft.id = None; self.draft.name.push_str(" copy"); self.commit(previous, mixer);
                }
            });
            ui.label(if self.draft.id.is_some() { "Active saved palette (changes are unsaved until Save)." } else { "Active unsaved draft." });
            ui.checkbox(&mut self.lock_history, "Lock colors added from history");
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_label("Generator").selected_text(format!("{:?}", self.options.mode)).show_ui(ui, |ui| {
                    for (mode, label) in GenerationMode::ALL { ui.selectable_value(&mut self.options.mode, mode, label); }
                });
                ui.add(egui::DragValue::new(&mut self.options.count).range(1..=64).prefix("Colors "));
                ui.add(egui::DragValue::new(&mut self.options.seed).prefix("Seed "));
                if ui.button("Generate / regenerate unlocked").clicked() { self.generate(mixer); }
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.use_base, "Base color");
                ui.add_enabled(self.use_base, egui::TextEdit::singleline(&mut self.base).desired_width(100.0));
                ui.checkbox(&mut self.use_contrast, "Minimum contrast vs base (or white)");
                ui.add_enabled(self.use_contrast, egui::DragValue::new(&mut self.contrast).range(1.0..=21.0).speed(0.1));
            });
            ui.horizontal_wrapped(|ui| {
                for (label, range, max) in [("Hue °", &mut self.options.hue, 360.0), ("Lightness", &mut self.options.lightness, 1.0), ("Chroma", &mut self.options.chroma, 0.4)] {
                    ui.label(label); ui.add(egui::DragValue::new(&mut range[0]).range(0.0..=max).speed(max / 500.0)); ui.label("–"); ui.add(egui::DragValue::new(&mut range[1]).range(0.0..=max).speed(max / 500.0));
                }
            });
            ui.small("OKLCH colors are mapped to sRGB; locked swatches are preserved on regeneration.");
            ui.separator();
            if ui.add_enabled(self.draft.colors.len() < 64, egui::Button::new("Add swatch")).clicked() { self.add_history(Color::new(255,255,255,255), mixer); }
            ui.separator();
            ui.heading("Saved palettes");
            ui.horizontal(|ui| { ui.label("Search"); ui.text_edit_singleline(&mut self.search); });
            let mut open = None;
            let mut delete = None;
            for palette in &self.saved {
                if !palette.name.to_lowercase().contains(&self.search.to_lowercase()) { continue; }
                ui.horizontal_wrapped(|ui| {
                    if ui.button(&palette.name).clicked() { open = Some(palette.clone()); }
                    ui.label(format!("{} colors", palette.colors.len()));
                    if ui.button("Delete").clicked() { delete = Some(palette.id.clone()); }
                });
            }
            if let Some(palette) = open { self.open_palette(&palette, mixer); }
            if let Some(id) = delete { self.error = Some(format!("Delete palette? Confirm below: {id}")); ui.memory_mut(|m| m.data.insert_temp(ui.id().with("delete_palette"), id)); }
            if let Some(id) = ui.memory(|m| m.data.get_temp::<String>(ui.id().with("delete_palette"))) {
                ui.horizontal(|ui| {
                    ui.label("Permanently delete selected saved palette?");
                    if ui.button("Confirm delete").clicked() {
                        if let Some(repo) = &mut self.repository { match repo.delete(&id) { Ok(()) => { if self.draft.id.as_deref() == Some(&id) { self.draft.id = None; } self.error = None; self.refresh(); }, Err(e) => self.error = Some(e.to_string()) } }
                        ui.memory_mut(|m| m.data.remove::<String>(ui.id().with("delete_palette")));
                    }
                    if ui.button("Cancel delete").clicked() { ui.memory_mut(|m| m.data.remove::<String>(ui.id().with("delete_palette"))); }
                });
            }
            ui.separator(); ui.heading("Generation history (drafts)");
            let mut reopen = None;
            for record in &self.generations {
                if ui.button(format!("{} · {} colors", record.description, record.palette.colors.len())).clicked() { reopen = Some(record.palette.clone()); }
            }
            if let Some(mut palette) = reopen { palette.id.clear(); self.open_palette(&palette, mixer); self.draft.id = None; }
        });
    }
}

#[derive(Clone, Copy)]
pub struct PaletteDrag {
    pub index: usize,
    pub color: Color,
}

enum Action {
    Set(usize, Color),
    Lock(usize, bool),
    Move(usize, usize),
    MoveEarlier(usize),
    MoveLater(usize),
    Rename(usize, String),
    Edit(usize),
    Delete(usize),
    Apply(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Headless harness for the pinned working palette.
    struct Pinned {
        ctx: egui::Context,
        editor: PaletteEditor,
        mixer: Mixer,
        size: egui::Vec2,
        /// Height of the pinned bottom panel under test.
        panel_height: f32,
        painted: Vec<(String, egui::Rect)>,
        /// Clip rect of each painted text, in the same order as `painted`.
        clips: Vec<egui::Rect>,
        strokes: Vec<(egui::Rect, f32)>,
        dir: PathBuf,
    }

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "swatch-board-palette-actions-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn hex(color: Color) -> String {
        color.format(Format::HexRgba)
    }

    impl Pinned {
        fn new(
            tag: &str,
            size: egui::Vec2,
            colors: Vec<Color>,
            names: Vec<String>,
            locked: Vec<bool>,
        ) -> Self {
            let dir = temp_dir(tag);
            let repo = PaletteRepository::open(dir.join("library.sqlite3")).unwrap();
            let editor = PaletteEditor {
                draft: Draft {
                    name: "Working".into(),
                    colors: colors.clone(),
                    names: names.clone(),
                    locked: locked.clone(),
                    id: None,
                },
                undo: vec![],
                redo: vec![],
                repository: Some(repo),
                saved: vec![],
                generations: vec![],
                options: GenerationOptions::default(),
                use_base: false,
                base: "#247AC3FF".into(),
                use_contrast: false,
                contrast: 4.5,
                search: String::new(),
                name_edits: names,
                edit: colors.iter().map(|c| hex(*c)).collect(),
                error: None,
                lock_history: false,
                status: None,
                pending_hex_focus: None,
                selected: None,
            };
            let mut mixer = Mixer::default();
            mixer.replace_palette(colors);
            Self {
                ctx: egui::Context::default(),
                editor,
                mixer,
                size,
                panel_height: 145.0,
                painted: vec![],
                clips: vec![],
                strokes: vec![],
                dir,
            }
        }

        fn set_font_size(&self, size: f32) {
            let mut style = (*self.ctx.style()).clone();
            for key in style.text_styles.keys().cloned().collect::<Vec<_>>() {
                let mut id = style.text_styles[&key].clone();
                id.size = size;
                style.text_styles.insert(key, id);
            }
            self.ctx.set_style(style);
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            let Self {
                ctx,
                editor,
                mixer,
                size,
                panel_height,
                painted,
                clips,
                strokes,
                ..
            } = self;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, *size)),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::TopBottomPanel::bottom("active_palette")
                        .resizable(true)
                        .default_height(*panel_height)
                        .min_height(130.0)
                        .max_height(220.0)
                        .show(ctx, |ui| {
                            mixer.palette_ui(ui);
                            editor.sync_from_mixer(mixer);
                            editor.pinned_ui(ui, mixer);
                        });
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.label("Central");
                    });
                },
            );
            painted.clear();
            clips.clear();
            strokes.clear();
            for shape in output.shapes {
                match &shape.shape {
                    egui::Shape::Text(text) => {
                        painted.push((
                            text.galley.text().to_owned(),
                            egui::Rect::from_min_size(text.pos, text.galley.size()),
                        ));
                        clips.push(shape.clip_rect);
                    }
                    egui::Shape::Rect(rect) if rect.stroke.width > 0.0 => {
                        strokes.push((rect.rect, rect.stroke.width));
                    }
                    _ => {}
                }
            }
        }

        /// Render the palette editor tab, where Undo/Redo live.
        fn frame_editor(&mut self, events: Vec<egui::Event>) {
            let Self {
                ctx,
                editor,
                mixer,
                size,
                painted,
                clips,
                ..
            } = self;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, *size)),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        editor.ui(ui, mixer);
                    });
                },
            );
            painted.clear();
            clips.clear();
            for shape in output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    painted.push((
                        text.galley.text().to_owned(),
                        egui::Rect::from_min_size(text.pos, text.galley.size()),
                    ));
                    clips.push(shape.clip_rect);
                }
            }
        }

        /// The clip rect that applies to the last painted occurrence of `label`.
        fn clip_of(&self, label: &str) -> egui::Rect {
            let index = self
                .painted
                .iter()
                .rposition(|(text, _)| text == label)
                .unwrap_or_else(|| {
                    panic!(
                        "missing painted text {label:?}; painted: {:?}",
                        self.painted.iter().map(|(t, _)| t).collect::<Vec<_>>()
                    )
                });
            self.clips[index]
        }

        /// True when the whole painted text lies inside its clip rect, i.e. it
        /// is actually on screen and not cut off by a panel or scroll area.
        fn text_fully_visible(&self, label: &str) -> bool {
            let index = self
                .painted
                .iter()
                .rposition(|(text, _)| text == label)
                .unwrap_or_else(|| {
                    panic!(
                        "missing painted text {label:?}; painted: {:?}",
                        self.painted.iter().map(|(t, _)| t).collect::<Vec<_>>()
                    )
                });
            self.clips[index].contains_rect(self.painted[index].1)
        }

        fn text_rect(&self, label: &str) -> egui::Rect {
            self.painted
                .iter()
                .rev()
                .find(|(text, _)| text == label)
                .unwrap_or_else(|| {
                    panic!(
                        "missing painted text {label:?}; painted: {:?}",
                        self.painted.iter().map(|(t, _)| t).collect::<Vec<_>>()
                    )
                })
                .1
        }

        fn has_text(&self, label: &str) -> bool {
            self.painted.iter().any(|(text, _)| text == label)
        }

        fn click(&mut self, label: &str) {
            let pos = self.text_rect(label).center();
            self.click_at(pos);
        }

        /// Click a label painted in the palette editor tab.
        fn click_editor(&mut self, label: &str) {
            let pos = self.text_rect(label).center();
            self.click_at_editor(pos);
        }

        fn click_at(&mut self, pos: egui::Pos2) {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            // One more frame so that feedback painted in response to the click
            // is visible to the assertions.
            self.frame(vec![]);
        }

        fn click_at_editor(&mut self, pos: egui::Pos2) {
            self.frame_editor(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            self.frame_editor(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            self.frame_editor(vec![]);
        }

        fn key(&mut self, key: egui::Key) {
            self.frame(vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            self.frame(vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
        }

        /// Tab until the widget showing `label` holds keyboard focus, then
        /// press Space to activate it. This mirrors the proven helper in
        /// `mixer.rs` and proves the action is reachable without a pointer.
        fn activate_with_keyboard(&mut self, label: &str) {
            // The panel re-lays out between frames, so the target position is
            // re-read on every step instead of being cached.
            for _ in 0..120 {
                let target = self.text_rect(label).center();
                let focused = self
                    .ctx
                    .memory(|memory| memory.focused())
                    .and_then(|id| self.ctx.read_response(id));
                if focused.is_some_and(|response| response.rect.contains(target)) {
                    self.key(egui::Key::Space);
                    return;
                }
                self.key(egui::Key::Tab);
            }
            panic!("keyboard could not reach {label:?}");
        }

        /// Click into the text field showing `label` and replace its content
        /// with `text`, then confirm the edit with Enter.
        fn retype_field(&mut self, label: &str, text: &str) {
            let pos = self.text_rect(label).center();
            self.click_at(pos);
            self.frame(vec![
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Text(text.into()),
            ]);
            self.key(egui::Key::Enter);
        }
    }

    impl Drop for Pinned {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn working_palette() -> Pinned {
        Pinned::new(
            "working",
            egui::vec2(960.0, 640.0),
            vec![
                Color::new(224, 108, 117, 255),
                Color::new(152, 195, 121, 255),
                Color::new(97, 175, 239, 255),
            ],
            vec!["Rose".into(), "Sage".into(), "Sky".into()],
            vec![false, false, false],
        )
    }

    #[test]
    fn swatch_actions_are_always_painted_at_minimum_size() {
        let mut app = working_palette();
        app.set_font_size(14.0);
        app.frame(vec![]);
        for label in ["Lock", "Edit", "Delete", "Earlier", "Later"] {
            assert!(
                app.has_text(label),
                "action {label:?} must be painted without hover; painted: {:?}",
                app.painted.iter().map(|(t, _)| t).collect::<Vec<_>>()
            );
        }
        // The actions must be inside the window, i.e. not clipped away.
        for label in ["Lock", "Edit", "Delete", "Earlier", "Later"] {
            let rect = app.text_rect(label);
            assert!(
                rect.min.x >= 0.0 && rect.max.x <= 960.0 && rect.max.y <= 640.0,
                "action {label:?} painted outside the 960x640 window: {rect:?}"
            );
        }
    }

    #[test]
    fn swatch_actions_remain_visible_in_a_540x360_window_with_more_swatches() {
        let mut app = Pinned::new(
            "minimum",
            egui::vec2(540.0, 360.0),
            vec![
                Color::new(224, 108, 117, 255),
                Color::new(152, 195, 121, 255),
                Color::new(97, 175, 239, 255),
                Color::new(229, 192, 123, 255),
                Color::new(198, 120, 221, 255),
            ],
            vec![
                "Rose".into(),
                "Sage".into(),
                "Sky".into(),
                "Sand".into(),
                "Plum".into(),
            ],
            vec![false, false, false, false, false],
        );
        app.set_font_size(14.0);
        app.frame(vec![]);
        for label in ["Lock", "Edit", "Delete", "Earlier", "Later"] {
            let rect = app.text_rect(label);
            assert!(
                rect.min.x >= 0.0
                    && rect.max.x <= 540.0
                    && rect.min.y >= 0.0
                    && rect.max.y <= 360.0,
                "action {label:?} must stay inside the 540x360 window; painted at {rect:?}"
            );
        }
        // The swatch itself must still be visible, not pushed out of the panel.
        let swatch = app.text_rect("#E06C75FF");
        assert!(
            swatch.max.y <= 360.0,
            "the first swatch must remain visible at minimum size: {swatch:?}"
        );
    }

    #[test]
    fn a_keyboard_focused_action_button_has_a_visible_focus_ring() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.key(egui::Key::Tab);
        app.key(egui::Key::Tab);
        app.frame(vec![]);
        let lock = app.text_rect("Lock");
        let ringed = app
            .strokes
            .iter()
            .any(|(rect, width)| *width >= 2.0 && rect.contains(lock.center()));
        assert!(
            ringed,
            "the focused action button must have a visible focus ring; strokes: {:?}",
            app.strokes
        );
    }

    #[test]
    fn the_selected_swatch_has_a_visible_selection_outline() {
        let mut app = working_palette();
        app.frame(vec![]);
        // The color tile is painted directly above the swatch's HEX field.
        let hex = app.text_rect("#E06C75FF");
        let tile_center = egui::Pos2::new(hex.center().x, hex.top() - 24.0);
        let outlined = app
            .strokes
            .iter()
            .any(|(rect, width)| *width >= 2.0 && rect.contains(tile_center));
        assert!(
            outlined,
            "the selected swatch must have a visible outline; strokes: {:?}",
            app.strokes
        );
    }

    #[test]
    fn pointer_lock_button_locks_and_unlocks_the_selected_swatch() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.click("Lock");
        assert!(app.editor.draft.locked[0]);
        assert!(
            app.has_text("Locked"),
            "locked state must be stated in words"
        );
        app.click("Unlock");
        assert!(!app.editor.draft.locked[0]);
    }

    #[test]
    fn keyboard_space_activates_the_lock_button() {
        let mut app = working_palette();
        app.frame(vec![]);
        // Tab from the panel's first control to the swatch action buttons.
        app.key(egui::Key::Tab);
        app.key(egui::Key::Tab);
        app.key(egui::Key::Space);
        assert!(
            app.editor.draft.locked[0],
            "Space on the focused action button must lock the swatch"
        );
    }

    #[test]
    fn locked_swatch_refuses_delete_and_explains_why() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.click("Lock");
        app.frame(vec![]);
        app.click("Delete");
        assert_eq!(
            app.editor.draft.colors.len(),
            3,
            "a locked swatch must not be deleted"
        );
        assert!(
            app.painted
                .iter()
                .any(|(text, _)| text.contains("Unlock it before deleting")),
            "the refusal must be explained in words; painted: {:?}",
            app.painted.iter().map(|(t, _)| t).collect::<Vec<_>>()
        );
    }

    #[test]
    fn locked_swatch_refuses_history_replacement_and_explains_why() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.click("Lock");
        app.frame(vec![]);
        // The color tile sits just above the HEX field of the same swatch.
        let hex = app.text_rect("#E06C75FF");
        let tile = egui::Pos2::new(hex.center().x, hex.top() - 25.0);
        app.frame(vec![
            egui::Event::PointerMoved(tile),
            egui::Event::PointerButton {
                pos: tile,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        egui::DragAndDrop::set_payload(&app.ctx, Color::new(1, 2, 3, 255));
        app.frame(vec![
            egui::Event::PointerMoved(tile),
            egui::Event::PointerButton {
                pos: tile,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        app.frame(vec![]);
        assert_eq!(
            app.editor.draft.colors[0],
            Color::new(224, 108, 117, 255),
            "a locked swatch must keep its color when history is dropped on it"
        );
        assert!(
            app.painted
                .iter()
                .any(|(text, _)| text.contains("Unlock it before replacing")),
            "the refusal must be explained in words; painted: {:?}",
            app.painted.iter().map(|(t, _)| t).collect::<Vec<_>>()
        );
    }

    #[test]
    fn swatch_actions_are_not_clipped_by_the_pinned_panel_at_minimum_size() {
        let mut app = Pinned::new(
            "clipped",
            egui::vec2(540.0, 360.0),
            vec![
                Color::new(224, 108, 117, 255),
                Color::new(152, 195, 121, 255),
                Color::new(97, 175, 239, 255),
                Color::new(229, 192, 123, 255),
                Color::new(198, 120, 221, 255),
            ],
            vec![
                "Rose".into(),
                "Sage".into(),
                "Sky".into(),
                "Sand".into(),
                "Plum".into(),
            ],
            vec![false, false, false, false, false],
        );
        app.set_font_size(14.0);
        // The pinned panel is user-resizable; check its smallest allowed height
        // as well as the default, because the action bar adds a row of height.
        for panel_height in [130.0_f32, 145.0, 220.0] {
            app.panel_height = panel_height;
            app.frame(vec![]);
            for label in ["Lock", "Edit", "Delete", "Earlier", "Later"] {
                assert!(
                    app.text_fully_visible(label),
                    "action {label:?} must be fully painted, not clipped by the pinned \
                     panel at height {panel_height}; painted at {:?} with clip {:?}",
                    app.text_rect(label),
                    app.clip_of(label)
                );
            }
        }
    }

    #[test]
    fn the_edit_action_focuses_the_hex_field_for_keyboard_editing() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.click("Edit");
        // Replace the value using the keyboard only: no further pointer use.
        app.frame(vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            egui::Event::Text("#3A86FFFF".into()),
        ]);
        assert_eq!(
            app.editor.edit[0], "#3A86FFFF",
            "Edit must hand keyboard control to the HEX field"
        );
        let changed = Color::new(58, 134, 255, 255);
        app.key(egui::Key::Enter);
        assert_eq!(
            app.editor.draft.colors[0], changed,
            "the edited HEX value must be applied to the swatch"
        );
    }

    #[test]
    fn locked_state_and_order_survive_save_and_reopen() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.click("Lock");
        app.frame(vec![]);
        app.click("Later");
        assert_eq!(
            app.editor.draft.names,
            vec!["Sage".to_string(), "Rose".to_string(), "Sky".to_string()]
        );
        app.editor.save();
        let saved = app.editor.saved.last().cloned().expect("palette saved");
        assert_eq!(
            saved.locked,
            vec![false, true, false],
            "the saved palette must keep the lock on the moved swatch"
        );
        assert_eq!(
            saved.swatch_names,
            vec!["Sage".to_string(), "Rose".to_string(), "Sky".to_string()],
            "the saved palette must keep the new order"
        );
        // Reopen the saved palette from storage and confirm the state survives.
        let reopened = app.editor.saved.last().cloned().expect("palette saved");
        app.editor.open_palette(&reopened, &mut app.mixer);
        assert_eq!(
            app.editor.draft.locked,
            vec![false, true, false],
            "reopening must restore the lock state"
        );
        assert_eq!(
            app.editor.draft.colors[1],
            Color::new(224, 108, 117, 255),
            "reopening must restore the locked swatch's color"
        );
    }

    #[test]
    fn keyboard_can_reorder_a_swatch_without_dragging() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.activate_with_keyboard("Later");
        assert_eq!(
            app.editor.draft.names,
            vec!["Sage".to_string(), "Rose".to_string(), "Sky".to_string()],
            "the keyboard must offer a real reorder alternative to dragging"
        );
    }

    #[test]
    fn keyboard_can_delete_an_unlocked_swatch() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.activate_with_keyboard("Delete");
        assert_eq!(
            app.editor.draft.colors.len(),
            2,
            "an unlocked swatch must be deletable from the keyboard"
        );
    }

    #[test]
    fn typing_in_a_swatch_name_field_is_not_stolen_by_a_button() {
        let mut app = working_palette();
        app.frame(vec![]);
        // Put the caret in the name field and type a space.
        let name = app.text_rect("Rose").center();
        app.click_at(name);
        app.frame(vec![egui::Event::Text(" ".into())]);
        assert_eq!(
            app.editor.name_edits[0].replace(' ', ""),
            "Rose",
            "the typed space must reach the focused text field instead of being \
             swallowed by a shortcut"
        );
        assert_eq!(
            app.editor.name_edits[0].len(),
            5,
            "exactly one space must be inserted into the name field"
        );
        assert!(
            !app.editor.draft.locked[0],
            "typing a space must not trigger the lock button"
        );
    }

    #[test]
    fn hex_editing_still_applies_a_changed_color_and_supports_undo() {
        let mut app = working_palette();
        app.frame(vec![]);
        let original = app.editor.draft.colors[0];
        let changed = Color::new(58, 134, 255, 255);
        assert_ne!(
            original, changed,
            "the edited value must differ from the default"
        );
        app.retype_field("#E06C75FF", &hex(changed));
        assert_eq!(
            app.editor.draft.colors[0], changed,
            "an edited HEX value must be applied to the swatch"
        );
        app.frame_editor(vec![]);
        app.click_editor("Undo");
        assert_eq!(
            app.editor.draft.colors[0], original,
            "undo must restore the previous swatch color"
        );
    }

    #[test]
    fn later_button_reorders_and_keeps_lock_with_its_swatch() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.click("Lock");
        app.frame(vec![]);
        app.click("Later");
        assert_eq!(
            app.editor.draft.names,
            vec!["Sage".to_string(), "Rose".to_string(), "Sky".to_string()],
            "Later must move the selected swatch one place later"
        );
        assert!(app.editor.draft.locked[1]);
        assert_eq!(app.editor.draft.colors[1], Color::new(224, 108, 117, 255));
    }

    #[test]
    fn locked_swatch_keeps_name_and_color_through_generation() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.click("Lock");
        app.frame(vec![]);
        app.editor.generate(&mut app.mixer);
        let locked = app
            .editor
            .draft
            .locked
            .iter()
            .position(|locked| *locked)
            .expect("the locked swatch must survive generation");
        assert_eq!(app.editor.draft.names[locked], "Rose");
        assert_eq!(
            app.editor.draft.colors[locked],
            Color::new(224, 108, 117, 255),
            "a locked swatch must keep its color through generation"
        );
    }

    #[test]
    fn undo_and_redo_restore_the_lock_state() {
        let mut app = working_palette();
        app.frame(vec![]);
        app.click("Lock");
        assert!(app.editor.draft.locked[0]);
        app.frame_editor(vec![]);
        app.click_editor("Undo");
        assert!(
            !app.editor.draft.locked[0],
            "undo must restore the previous lock state"
        );
        app.frame_editor(vec![]);
        app.click_editor("Redo");
        assert!(app.editor.draft.locked[0]);
    }

    #[test]
    fn locked_generation_save_reopen_and_mixer_share_active_colors() {
        let dir = std::env::temp_dir().join(format!(
            "swatch-board-palette-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = PaletteRepository::open(dir.join("library.sqlite3")).unwrap();
        let mut editor = PaletteEditor {
            draft: Draft {
                name: "Example".into(),
                colors: vec![],
                locked: vec![],
                names: vec![],
                id: None,
            },
            undo: vec![],
            redo: vec![],
            repository: Some(repo),
            saved: vec![],
            generations: vec![],
            options: GenerationOptions::default(),
            use_base: false,
            base: String::new(),
            use_contrast: false,
            contrast: 4.5,
            search: String::new(),
            name_edits: vec![],
            edit: vec![],
            error: None,
            lock_history: true,
            status: None,
            pending_hex_focus: None,
            selected: None,
        };
        let mut mixer = Mixer::default();
        let red = Color::new(255, 0, 0, 255);
        editor.add_history(red, &mut mixer);
        editor.generate(&mut mixer);
        assert_eq!(editor.draft.colors[0], red);
        assert!(editor.draft.locked[0]);
        assert_eq!(editor.draft.colors, mixer.colors());
        assert_eq!(editor.generations.len(), 1);
        editor.save();
        let saved = editor.saved[0].clone();
        let previous = editor.draft.clone();
        editor.draft.colors.clear();
        editor.draft.locked.clear();
        editor.commit(previous, &mut mixer);
        editor.open_palette(&saved, &mut mixer);
        assert_eq!(editor.draft.colors, mixer.colors());
        assert_eq!(editor.draft.colors[0], red);
        assert!(editor.draft.locked[0]);
        let before = editor.draft.colors.clone();
        let before_locks = editor.draft.locked.clone();
        let ctx = egui::Context::default();
        let mut frame = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| editor.pinned_ui(ui, &mut mixer));
                },
            );
        };
        frame(vec![]);
        let start = egui::pos2(50.0, 48.0);
        let end = egui::pos2(160.0, 48.0);
        frame(vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        frame(vec![egui::Event::PointerMoved(end)]);
        frame(vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        // A history drag onto the locked, moved swatch must not replace it.
        frame(vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        egui::DragAndDrop::set_payload(&ctx, Color::new(1, 2, 3, 255));
        frame(vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        assert_eq!(editor.draft.colors[0], before[1]);
        assert_eq!(editor.draft.colors[1], before[0]);
        assert_eq!(editor.draft.locked[1], before_locks[0]);
        assert_eq!(editor.draft.colors, mixer.colors());
        drop(editor);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
