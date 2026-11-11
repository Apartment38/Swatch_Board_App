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
    edit: Vec<String>,
    error: Option<String>,
    lock_history: bool,
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
            search: String::new(),
            edit: Vec::new(),
            error,
            lock_history: false,
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

    fn sync_fields(&mut self) {
        self.edit = self
            .draft
            .colors
            .iter()
            .map(|c| c.format(Format::HexRgba))
            .collect();
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
        self.draft.locked.push(self.lock_history);
        self.commit(previous, mixer);
    }

    fn open_palette(&mut self, palette: &SavedPalette, mixer: &mut Mixer) {
        let previous = self.draft.clone();
        self.draft = Draft {
            name: palette.name.clone(),
            colors: palette.colors.iter().copied().map(from_rgba).collect(),
            locked: palette.locked.clone(),
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
        let locked: Vec<(Color, bool)> = self
            .draft
            .colors
            .iter()
            .copied()
            .zip(self.draft.locked.iter().copied())
            .enumerate()
            .filter_map(|(i, pair)| (pair.1 && i >= options.count).then_some(pair))
            .collect();
        let mut colors = generated;
        let mut locks = vec![false; options.count];
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
        for (color, _) in locked {
            if let Some(i) = locks.iter().position(|locked| !locked) {
                colors[i] = color;
                locks[i] = true;
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
        match repo.save(
            self.draft.id.as_deref(),
            &self.draft.name,
            &colors,
            &self.draft.locked,
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

    pub fn pinned_ui(&mut self, ui: &mut egui::Ui, mixer: &mut Mixer) {
        let mut action = None;
        egui::ScrollArea::horizontal()
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
                                response.dnd_set_drag_payload(i);
                                if response.dnd_hover_payload::<usize>().is_some() {
                                    if let Some(source) = response.dnd_release_payload::<usize>() {
                                        if *source != i {
                                            action = Some(Action::Move(*source, i));
                                        }
                                    }
                                } else if response.dnd_hover_payload::<Color>().is_some() {
                                    if let Some(color) = response.dnd_release_payload::<Color>() {
                                        if !self.draft.locked[i] {
                                            action = Some(Action::Set(i, *color));
                                        }
                                    }
                                }
                                let hovered = ui.input(|input| {
                                    input
                                        .pointer
                                        .hover_pos()
                                        .is_some_and(|pos| rect.contains(pos))
                                });
                                if hovered {
                                    let lock_rect = egui::Rect::from_min_size(
                                        rect.min + egui::vec2(2.0, 2.0),
                                        egui::vec2(55.0, 22.0),
                                    );
                                    let delete_rect = egui::Rect::from_min_size(
                                        rect.min + egui::vec2(59.0, 2.0),
                                        egui::vec2(43.0, 22.0),
                                    );
                                    if ui
                                        .put(
                                            lock_rect,
                                            egui::Button::new(if self.draft.locked[i] {
                                                "Unlock"
                                            } else {
                                                "Lock"
                                            }),
                                        )
                                        .clicked()
                                    {
                                        action = Some(Action::Lock(i, !self.draft.locked[i]));
                                    }
                                    if ui
                                        .add_enabled_ui(!self.draft.locked[i], |ui| {
                                            ui.put(delete_rect, egui::Button::new("Delete"))
                                        })
                                        .inner
                                        .clicked()
                                    {
                                        action = Some(Action::Remove(i));
                                    }
                                } else if self.draft.locked[i] {
                                    ui.painter().text(
                                        rect.left_top() + egui::vec2(4.0, 3.0),
                                        egui::Align2::LEFT_TOP,
                                        "Locked",
                                        egui::FontId::proportional(12.0),
                                        egui::Color32::WHITE,
                                    );
                                }
                                let edit = ui.add_enabled(
                                    !self.draft.locked[i],
                                    egui::TextEdit::singleline(&mut self.edit[i])
                                        .desired_width(104.0),
                                );
                                if edit.lost_focus() && action.is_none() && !self.draft.locked[i] {
                                    action = Some(Action::Apply(i));
                                }
                            });
                        });
                    }
                });
            });
        if let Some(action) = action {
            let previous = self.draft.clone();
            match action {
                Action::Set(i, c) => self.draft.colors[i] = c,
                Action::Lock(i, value) => self.draft.locked[i] = value,
                Action::Move(from, to) => {
                    let color = self.draft.colors.remove(from);
                    let locked = self.draft.locked.remove(from);
                    self.draft.colors.insert(to, color);
                    self.draft.locked.insert(to, locked);
                }
                Action::Remove(i) => {
                    self.draft.colors.remove(i);
                    self.draft.locked.remove(i);
                }
                Action::Apply(i) => match self.edit[i].parse::<Color>() {
                    Ok(c) => {
                        self.draft.colors[i] = c;
                        self.error = None;
                    }
                    Err(_) => self.error = Some("Invalid color; swatch unchanged.".into()),
                },
            }
            self.commit(previous, mixer);
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
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
                    self.draft = Draft { name: "Untitled".into(), colors: Vec::new(), locked: Vec::new(), id: None };
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

enum Action {
    Set(usize, Color),
    Lock(usize, bool),
    Move(usize, usize),
    Remove(usize),
    Apply(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

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
            edit: vec![],
            error: None,
            lock_history: true,
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
