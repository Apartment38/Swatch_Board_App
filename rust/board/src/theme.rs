use std::collections::BTreeMap;
use std::path::PathBuf;

use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};
use swatch_board_color::{Color, Format};
use swatch_board_storage::{SavedTheme, ThemeRepository};
use swatch_board_theme_core::{self as core, Plugin, PluginMetadata, RenderedFile};

use crate::palette::{PaletteDrag, PaletteEditor};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
enum Assignment {
    Override(String),
    Link(String),
    Default,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Project {
    version: u32,
    id: String,
    name: String,
    plugin_id: String,
    plugin_version: String,
    palette_id: Option<String>,
    scene: String,
    assignments: BTreeMap<String, Assignment>,
    dirty: bool,
}
enum PendingProject {
    New(usize),
    Open(SavedTheme),
}

pub struct ThemeEditor {
    plugins: Vec<PluginMetadata>,
    plugin: Option<Plugin>,
    project: Option<Project>,
    repo: Option<ThemeRepository>,
    saved: Vec<SavedTheme>,
    undo: Vec<Project>,
    redo: Vec<Project>,
    selected: Option<String>,
    edits: BTreeMap<String, String>,
    output_dir: String,
    preview: Vec<RenderedFile>,
    overwrite: bool,
    error: Option<String>,
    textures: BTreeMap<String, egui::TextureHandle>,
    pending_project: Option<PendingProject>,
}

impl ThemeEditor {
    pub fn new() -> Self {
        let (repo, error) = match ThemeRepository::open_default() {
            Ok(repo) => (Some(repo), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let mut this = Self {
            plugins: Vec::new(),
            plugin: None,
            project: None,
            repo,
            saved: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            selected: None,
            edits: BTreeMap::new(),
            output_dir: String::new(),
            preview: Vec::new(),
            overwrite: false,
            error,
            textures: BTreeMap::new(),
            pending_project: None,
        };
        if let Some(repo) = &this.repo {
            match repo.load_draft() {
                Ok(Some(json)) => match serde_json::from_str::<Project>(&json) {
                    Ok(p) if p.version == 1 => this.project = Some(p),
                    Ok(_) => this.error = Some("Unsupported theme draft version".into()),
                    Err(e) => this.error = Some(format!("Cannot read theme draft: {e}")),
                },
                Ok(None) => (),
                Err(e) => this.error = Some(e.to_string()),
            }
        }
        this.reload();
        this.refresh();
        this
    }

    fn refresh(&mut self) {
        if let Some(repo) = &self.repo {
            match repo.list() {
                Ok(saved) => self.saved = saved,
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    fn reload(&mut self) {
        let (plugins, errors) = core::discover();
        self.plugins = plugins;
        self.plugin = None;
        self.textures.clear();
        if let Some(project) = &self.project {
            if let Some(meta) = self.plugins.iter().find(|p| {
                p.manifest.id == project.plugin_id && p.manifest.version == project.plugin_version
            }) {
                match core::load(&meta.path) {
                    Ok(plugin) => self.plugin = Some(plugin),
                    Err(e) => self.error = Some(e),
                }
            } else {
                self.error = Some(format!(
                    "Plugin {} version {} is missing or incompatible; project retained",
                    project.plugin_id, project.plugin_version
                ));
            }
        }
        if !errors.is_empty() {
            self.error = Some(errors.join("\n"));
        }
        self.preview.clear();
    }
    fn persist(&mut self) {
        if let (Some(repo), Some(project)) = (&mut self.repo, &self.project) {
            match serde_json::to_string(project) {
                Ok(json) => {
                    if let Err(e) = repo.save_draft(&json) {
                        self.error = Some(e.to_string());
                    }
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    fn changed(&mut self, previous: Project) {
        if let Some(project) = &mut self.project {
            if *project != previous {
                project.dirty = true;
                if self.undo.len() == 100 {
                    self.undo.remove(0);
                }
                self.undo.push(previous);
                self.redo.clear();
                self.preview.clear();
                self.persist();
            }
        }
    }
    fn choose(&mut self, index: usize, palette: &PaletteEditor) {
        let meta = &self.plugins[index];
        match core::load(&meta.path) {
            Ok(plugin) => {
                let (id, _, _) = palette.theme_palette();
                let scene = plugin
                    .scenes
                    .first()
                    .map(|s| s.id.clone())
                    .unwrap_or_default();
                self.project = Some(Project {
                    version: 1,
                    id: String::new(),
                    name: format!("{} theme", plugin.manifest.name),
                    plugin_id: plugin.manifest.id.clone(),
                    plugin_version: plugin.manifest.version.clone(),
                    palette_id: id.map(str::to_owned),
                    scene,
                    assignments: BTreeMap::new(),
                    dirty: true,
                });
                self.plugin = Some(plugin);
                self.selected = None;
                self.undo.clear();
                self.redo.clear();
                self.edits.clear();
                self.textures.clear();
                self.preview.clear();
                self.error = None;
                self.persist();
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn resolved(&self, palette: &PaletteEditor) -> Result<BTreeMap<String, String>, String> {
        let project = self.project.as_ref().ok_or("Select a plugin first")?;
        let plugin = self
            .plugin
            .as_ref()
            .ok_or("The selected plugin is unavailable")?;
        let (id, _, colors) = palette.theme_palette();
        let mut values = BTreeMap::new();
        for (role, assignment) in &project.assignments {
            let value = match assignment {
                Assignment::Default => continue,
                Assignment::Override(value) => value.clone(),
                Assignment::Link(name) => {
                    if id.is_none() || project.palette_id.as_deref() != id {
                        return Err(
                            "Open the project's saved source palette before resolving links".into(),
                        );
                    }
                    colors
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, c)| c.format(Format::HexRgba))
                        .ok_or_else(|| format!("Linked palette swatch '{name}' is missing"))?
                }
            };
            values.insert(role.clone(), value);
        }
        plugin.resolve(&values)
    }
    fn save(&mut self) {
        let (Some(repo), Some(project)) = (&mut self.repo, &mut self.project) else {
            self.error = Some("Theme storage unavailable".into());
            return;
        };
        if project.name.trim().is_empty() {
            self.error = Some("Project name cannot be empty".into());
            return;
        }
        let mut saved = project.clone();
        saved.dirty = false;
        let payload = match serde_json::to_string(&saved) {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        let record = SavedTheme {
            id: saved.id.clone(),
            name: saved.name.clone(),
            plugin_id: saved.plugin_id.clone(),
            plugin_version: saved.plugin_version.clone(),
            palette_id: saved.palette_id.clone(),
            scene: saved.scene.clone(),
            payload,
            dirty: false,
        };
        match repo.save(&record) {
            Ok(id) => {
                project.id = id;
                project.dirty = false;
                self.error = None;
                self.persist();
                self.refresh();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn open(&mut self, record: &SavedTheme) {
        match serde_json::from_str::<Project>(&record.payload) {
            Ok(mut p) if p.version == 1 => {
                p.id = record.id.clone();
                p.dirty = false;
                self.project = Some(p);
                self.undo.clear();
                self.redo.clear();
                self.edits.clear();
                self.error = None;
                self.reload();
                self.persist();
            }
            Ok(_) => self.error = Some("Unsupported project version".into()),
            Err(e) => self.error = Some(format!("Invalid project: {e}")),
        }
    }
    fn assign(&mut self, role: &str, assignment: Assignment) {
        if let Some(project) = &mut self.project {
            let previous = project.clone();
            project.assignments.insert(role.to_owned(), assignment);
            self.edits.remove(role);
            self.changed(previous);
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, palette: &PaletteEditor, format: Format) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Theme Generator");
            if ui.button("Reload Plugins").clicked() {
                self.error = None;
                self.reload();
            }
            if ui
                .add_enabled(!self.undo.is_empty(), egui::Button::new("Undo"))
                .clicked()
            {
                if let (Some(prev), Some(project)) = (self.undo.pop(), &mut self.project) {
                    self.redo.push(std::mem::replace(project, prev));
                    self.preview.clear();
                    self.edits.clear();
                    self.persist();
                }
            }
            if ui
                .add_enabled(!self.redo.is_empty(), egui::Button::new("Redo"))
                .clicked()
            {
                if let (Some(next), Some(project)) = (self.redo.pop(), &mut self.project) {
                    self.undo.push(std::mem::replace(project, next));
                    self.preview.clear();
                    self.edits.clear();
                    self.persist();
                }
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let mut choose = None;
        let label = self
            .project
            .as_ref()
            .map(|p| p.plugin_id.as_str())
            .unwrap_or("Select plugin");
        egui::ComboBox::from_label("New project for plugin")
            .selected_text(label)
            .show_ui(ui, |ui| {
                for (i, p) in self.plugins.iter().enumerate() {
                    if ui.selectable_label(false, &p.manifest.name).clicked() {
                        choose = Some(i);
                    }
                }
            });
        if let Some(i) = choose {
            self.pending_project = Some(PendingProject::New(i));
        }
        let mut reopen = None;
        egui::ComboBox::from_label("Saved projects")
            .selected_text("Open a project")
            .show_ui(ui, |ui| {
                for record in &self.saved {
                    if ui.selectable_label(false, &record.name).clicked() {
                        reopen = Some(record.clone());
                    }
                }
            });
        if let Some(record) = reopen {
            self.pending_project = Some(PendingProject::Open(record));
        }
        if self.pending_project.is_some() {
            let dirty = self.project.as_ref().is_some_and(|p| p.dirty);
            let mut proceed = !dirty;
            if dirty {
                ui.horizontal(|ui| {
                    ui.label("Discard the current unsaved theme edits?");
                    if ui.button("Discard and continue").clicked() {
                        proceed = true;
                    }
                    if ui.button("Keep editing").clicked() {
                        self.pending_project = None;
                    }
                });
            }
            if proceed {
                match self.pending_project.take() {
                    Some(PendingProject::New(i)) if i < self.plugins.len() => {
                        self.choose(i, palette)
                    }
                    Some(PendingProject::Open(record)) => self.open(&record),
                    _ => {}
                }
            }
        }
        if self.project.is_none() {
            ui.label("Select a plugin to create a theme. Reload discovers installed packages without rebuilding.");
            return;
        }
        let (palette_id, palette_name, palette_colors) = palette.theme_palette();
        ui.horizontal_wrapped(|ui| {
            if let Some(project) = &mut self.project {
                let previous = project.clone();
                ui.label("Project name");
                ui.text_edit_singleline(&mut project.name);
                if ui.button("Use active palette").clicked() {
                    project.palette_id = palette_id.map(str::to_owned);
                }
                ui.label(format!(
                    "Active palette: {palette_name}{}",
                    if palette_id.is_none() {
                        " (unsaved)"
                    } else {
                        ""
                    }
                ));
                ui.label(if project.dirty {
                    "Unsaved changes (draft recovered on restart)"
                } else {
                    "Saved"
                });
                self.changed(previous);
            }
            if ui.button("Save project").clicked() {
                self.save();
            }
        });
        let values = match self.resolved(palette) {
            Ok(v) => v,
            Err(e) => {
                ui.colored_label(ui.visuals().error_fg_color, e);
                BTreeMap::new()
            }
        };
        let Some(plugin) = &self.plugin else {
            ui.label("Plugin unavailable. Reload after restoring its files; edits are retained.");
            return;
        };
        let roles = plugin.roles.clone();
        let scenes = plugin.scenes.clone();
        for scene in &scenes {
            for asset in scene.nodes.iter().filter_map(|node| node.asset.as_ref()) {
                if !self.textures.contains_key(asset) {
                    match plugin.asset(asset).and_then(|bytes| {
                        image::load_from_memory(&bytes).map_err(|e| e.to_string())
                    }) {
                        Ok(image) => {
                            let rgba = image.to_rgba8();
                            let size = [rgba.width() as usize, rgba.height() as usize];
                            let texture = ui.ctx().load_texture(
                                asset,
                                egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
                                egui::TextureOptions::LINEAR,
                            );
                            self.textures.insert(asset.clone(), texture);
                        }
                        Err(error) => self.error = Some(format!("Cannot load {asset}: {error}")),
                    }
                }
            }
        }
        let project = self.project.as_ref().unwrap().clone();
        let mut pending: Option<(String, Assignment)> = None;
        let mut scene_change = None;
        ui.columns(2, |columns| {
            columns[0].heading("Preview");
            if let Some(scene) = scenes.iter().find(|s| s.id == project.scene) {
                let width = columns[0].available_width().max(50.0);
                let scale = (width / scene.width).min(1.5);
                let (rect, _) = columns[0].allocate_exact_size(
                    Vec2::new(scene.width * scale, scene.height * scale),
                    Sense::hover(),
                );
                let painter = columns[0].painter_at(rect);
                let mut drop_targets = Vec::new();
                for node in &scene.nodes {
                    let r = Rect::from_min_size(
                        rect.min + Vec2::new(node.rect[0] * scale, node.rect[1] * scale),
                        Vec2::new(node.rect[2] * scale, node.rect[3] * scale),
                    );
                    let role_color = |id: &Option<String>, fallback: Color32| {
                        id.as_ref()
                            .and_then(|id| values.get(id))
                            .and_then(|v| v.parse::<Color>().ok())
                            .map(egui_color)
                            .unwrap_or(fallback)
                    };
                    let text_only = matches!(node.kind.as_str(), "text" | "syntax");
                    let fill = if text_only {
                        Color32::TRANSPARENT
                    } else {
                        role_color(&node.role, Color32::TRANSPARENT)
                    };
                    let border = role_color(&node.border_role, Color32::TRANSPARENT);
                    painter.rect_filled(r, 2.0, fill);
                    if border != Color32::TRANSPARENT {
                        painter.rect_stroke(
                            r,
                            2.0,
                            Stroke::new(1.0_f32, border),
                            StrokeKind::Inside,
                        );
                    }
                    if let Some(texture) = node
                        .asset
                        .as_ref()
                        .and_then(|asset| self.textures.get(asset))
                    {
                        painter.image(
                            texture.id(),
                            r,
                            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    if let Some(text) = &node.text {
                        let text_role = if text_only && node.text_role.is_none() {
                            &node.role
                        } else {
                            &node.text_role
                        };
                        painter.text(
                            r.left_center() + Vec2::new(5.0 * scale, 0.0),
                            egui::Align2::LEFT_CENTER,
                            text,
                            FontId::proportional(node.font_size * scale),
                            role_color(text_role, Color32::WHITE),
                        );
                    }
                    // Text-only and border-only nodes still expose an editable semantic role.
                    let interactive_role = if text_only {
                        node.text_role.as_ref().or(node.role.as_ref())
                    } else {
                        node.role.as_ref().or(node.text_role.as_ref())
                    }
                    .or(node.border_role.as_ref());
                    if node.enabled && (interactive_role.is_some() || node.target.is_some()) {
                        let response = columns[0].interact(
                            r,
                            columns[0].id().with((&scene.id, &node.id)),
                            Sense::click(),
                        );
                        if let Some(role) = interactive_role {
                            drop_targets.push((role.clone(), response.clone()));
                        }
                        if response.hovered() {
                            painter.rect_stroke(
                                r,
                                0.0,
                                Stroke::new(2.0_f32, Color32::LIGHT_BLUE),
                                StrokeKind::Inside,
                            );
                        }
                        if response.clicked() {
                            if let Some(role) = interactive_role {
                                self.selected = Some(role.clone());
                            }
                            if let Some(target) = &node.target {
                                scene_change = Some(target.clone());
                            }
                        }
                        if self
                            .selected
                            .as_ref()
                            .is_some_and(|s| interactive_role == Some(s))
                        {
                            painter.rect_stroke(
                                r,
                                0.0,
                                Stroke::new(2.0_f32, Color32::YELLOW),
                                StrokeKind::Inside,
                            );
                        }
                    }
                }
                // Overlapping backgrounds must not consume a payload before the topmost node.
                for (role, response) in drop_targets.into_iter().rev() {
                    if let Some(color) = dropped_color(&response) {
                        pending = Some((role, Assignment::Override(color.format(Format::HexRgba))));
                        break;
                    }
                }
            } else {
                columns[0].label("This project's preview scene is unavailable.");
            }
            columns[1].heading("Roles");
            egui::ScrollArea::vertical()
                .id_salt("theme_roles")
                .max_height(450.0)
                .show(&mut columns[1], |ui| {
                    for role in &roles {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        self.selected.as_deref() == Some(&role.id),
                                        &role.title,
                                    )
                                    .clicked()
                                {
                                    self.selected = Some(role.id.clone());
                                }
                                if let Some(value) = values.get(&role.id) {
                                    if let Ok(color) = value.parse::<Color>() {
                                        let response =
                                            crate::mixer::swatch(ui, color, Vec2::new(32.0, 22.0));
                                        if let Some(color) = dropped_color(&response) {
                                            pending = Some((
                                                role.id.clone(),
                                                Assignment::Override(color.format(Format::HexRgba)),
                                            ));
                                        }
                                        if response.hovered() {
                                            ui.painter().rect_stroke(
                                                response.rect,
                                                0.0,
                                                Stroke::new(2.0_f32, Color32::LIGHT_BLUE),
                                                StrokeKind::Inside,
                                            );
                                        }
                                    }
                                }
                            });
                            if !role.help.is_empty() {
                                ui.small(&role.help);
                            }
                            let current = values
                                .get(&role.id)
                                .cloned()
                                .unwrap_or_else(|| role.default.clone());
                            let edit_id = ui.make_persistent_id(("role_color", &role.id));
                            if matches!(
                                project.assignments.get(&role.id),
                                Some(Assignment::Link(_))
                            ) && !ui.memory(|memory| memory.has_focus(edit_id))
                            {
                                // Linked values are live; do not retain a stale editable snapshot.
                                // Preserve text while the user is actively entering an override.
                                self.edits.remove(&role.id);
                            }
                            let edit = self.edits.entry(role.id.clone()).or_insert_with(|| {
                                current
                                    .parse::<Color>()
                                    .map(|c| c.format(format))
                                    .unwrap_or(current)
                            });
                            let response = ui.add(egui::TextEdit::singleline(edit).id(edit_id));
                            if response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                match edit.parse::<Color>() {
                                    Ok(color) => {
                                        self.error = None;
                                        pending = Some((
                                            role.id.clone(),
                                            Assignment::Override(color.format(Format::HexRgba)),
                                        ))
                                    }
                                    Err(_) => {
                                        self.error =
                                            Some(format!("Invalid color for {}", role.title))
                                    }
                                }
                            }
                            let current_link = match project.assignments.get(&role.id) {
                                Some(Assignment::Link(name)) => name.as_str(),
                                Some(Assignment::Override(_)) => "Override",
                                Some(Assignment::Default) | None => "Default",
                            };
                            egui::ComboBox::from_id_salt(("role_link", &role.id))
                                .selected_text(current_link)
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(current_link == "Default", "Default")
                                        .clicked()
                                    {
                                        pending = Some((role.id.clone(), Assignment::Default));
                                    }
                                    for (name, _) in &palette_colors {
                                        if palette_id.is_some()
                                            && project.palette_id.as_deref() == palette_id
                                            && (role.links.is_empty() || role.links.contains(name))
                                            && ui
                                                .selectable_label(current_link == name, name)
                                                .clicked()
                                        {
                                            pending = Some((
                                                role.id.clone(),
                                                Assignment::Link(name.clone()),
                                            ));
                                        }
                                    }
                                });
                        });
                    }
                });
        });
        if let Some((role, assignment)) = pending {
            self.assign(&role, assignment);
        }
        if let Some(scene) = scene_change {
            if let Some(p) = &mut self.project {
                let previous = p.clone();
                p.scene = scene;
                self.changed(previous);
            }
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label("Export directory");
            ui.text_edit_singleline(&mut self.output_dir);
            if ui.button("Preview export").clicked() {
                match self
                    .resolved(palette)
                    .and_then(|v| self.plugin.as_ref().unwrap().render(&v))
                {
                    Ok(files) => {
                        self.preview = files;
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            ui.checkbox(&mut self.overwrite, "Confirm overwrite existing files");
            if ui
                .add_enabled(
                    !self.preview.is_empty() && !self.output_dir.trim().is_empty(),
                    egui::Button::new("Export previewed files"),
                )
                .clicked()
            {
                let current = self
                    .resolved(palette)
                    .and_then(|v| self.plugin.as_ref().unwrap().render(&v));
                match current {
                    Ok(files)
                        if files
                            .iter()
                            .zip(&self.preview)
                            .all(|(a, b)| a.path == b.path && a.content == b.content)
                            && files.len() == self.preview.len() =>
                    {
                        match core::export(
                            &PathBuf::from(&self.output_dir),
                            &self.preview,
                            self.overwrite,
                        ) {
                            Ok(()) => {
                                self.error = None;
                            }
                            Err(e) => self.error = Some(e),
                        }
                    }
                    Ok(_) => {
                        self.preview.clear();
                        self.error = Some("Palette changed; preview export again.".into());
                    }
                    Err(e) => {
                        self.preview.clear();
                        self.error = Some(e);
                    }
                }
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("theme_export_preview")
            .max_height(180.0)
            .show(ui, |ui| {
                for file in &self.preview {
                    ui.collapsing(&file.path, |ui| {
                        ui.monospace(&file.content);
                    });
                }
            });
    }
}

fn egui_color(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}
fn dropped_color(response: &egui::Response) -> Option<Color> {
    // egui clears a taken payload even if its type does not match.
    // Inspect first so palette drags survive the history-color type check.
    if response.dnd_hover_payload::<Color>().is_some() {
        response.dnd_release_payload::<Color>().map(|c| *c)
    } else if response.dnd_hover_payload::<PaletteDrag>().is_some() {
        response
            .dnd_release_payload::<PaletteDrag>()
            .map(|p| p.color)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    /// Run constructors/discovery in a child so parallel tests never change global XDG state.
    fn isolated(name: &str, run: impl FnOnce(&std::path::Path)) {
        if let Some(root) = std::env::var_os("SWATCH_THEME_ACCEPTANCE_ROOT") {
            run(std::path::Path::new(&root));
            return;
        }
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("theme-{name}-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("theme::tests::{name}"), "--nocapture"])
            .env("SWATCH_THEME_ACCEPTANCE_ROOT", &root)
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_DATA_DIRS", root.join("system"))
            .output()
            .unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn install_package(root: &std::path::Path) {
        let package = root.join("data/swatch-board/plugins/runtime-fixture");
        fs::create_dir_all(package.join("templates")).unwrap();
        for (path, value) in [
            (
                "manifest.json",
                serde_json::json!({
                    "id":"runtime-fixture", "name":"Runtime fixture", "version":"1.0",
                    "api_version":1, "target":"Example", "target_versions":["1"],
                    "outputs":[{"path":"example.txt", "template":"templates/example.j2", "format":"text"}]
                }),
            ),
            (
                "roles.json",
                serde_json::json!([
                    {"id":"surface", "title":"Surface", "default":"#112233"},
                    {"id":"ink", "title":"Ink", "default":"#445566"}
                ]),
            ),
            (
                "scenes.json",
                serde_json::json!([
                    {"id":"home", "title":"Home", "width":400, "height":200, "nodes":[
                        {"id":"background", "kind":"window", "rect":[0,0,400,200], "role":"surface"},
                        {"id":"open", "kind":"tab", "rect":[20,20,160,40], "text":"Open drawer", "target":"drawer"}
                    ]},
                    {"id":"drawer", "title":"Drawer", "width":400, "height":200, "nodes":[
                        {"id":"background", "kind":"window", "rect":[0,0,400,200], "role":"surface"},
                        {"id":"ink-label", "kind":"text", "rect":[20,20,180,40], "text":"Hidden title", "text_role":"ink"},
                        {"id":"close", "kind":"tab", "rect":[20,100,160,40], "text":"Close drawer", "target":"home"}
                    ]}
                ]),
            ),
        ] {
            fs::write(
                package.join(path),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .unwrap();
        }
        fs::write(
            package.join("templates/example.j2"),
            "ink={{ roles.ink }}\nsurface={{ roles.surface }}\n",
        )
        .unwrap();
    }

    struct Headless {
        ctx: egui::Context,
        editor: ThemeEditor,
        palette: PaletteEditor,
        text: Vec<(String, Rect)>,
        palette_mode: bool,
        pinned_on_theme: bool,
        mixer: crate::mixer::Mixer,
        painted_textures: Vec<egui::TextureId>,
    }
    impl Headless {
        fn new() -> Self {
            Self {
                ctx: egui::Context::default(),
                editor: ThemeEditor::new(),
                palette: PaletteEditor::new(),
                text: Vec::new(),
                palette_mode: false,
                pinned_on_theme: false,
                mixer: crate::mixer::Mixer::default(),
                painted_textures: Vec::new(),
            }
        }
        fn frame(&mut self, events: Vec<egui::Event>) {
            let output = self.ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400.0, 1100.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    if self.palette_mode || self.pinned_on_theme {
                        let panel = egui::TopBottomPanel::bottom("test_palette");
                        let panel = if self.pinned_on_theme {
                            panel
                                .default_height(145.0)
                                .min_height(130.0)
                                .max_height(220.0)
                        } else {
                            panel
                        };
                        panel.show(ctx, |ui| {
                            self.palette.pinned_ui(ui, &mut self.mixer);
                        });
                    }
                    egui::CentralPanel::default().show(ctx, |ui| {
                        if self.palette_mode {
                            self.palette.ui(ui, &mut self.mixer);
                        } else {
                            self.editor.ui(ui, &self.palette, Format::HexRgba);
                        }
                    });
                },
            );
            fn collect(shape: egui::epaint::Shape, text: &mut Vec<(String, Rect)>) {
                match shape {
                    egui::epaint::Shape::Text(t) => text.push((
                        t.galley.text().to_owned(),
                        Rect::from_min_size(t.pos, t.galley.size()),
                    )),
                    egui::epaint::Shape::Vec(shapes) => {
                        for shape in shapes {
                            collect(shape, text);
                        }
                    }
                    _ => {}
                }
            }
            self.text.clear();
            self.painted_textures.clear();
            for shape in output.shapes {
                if let egui::epaint::Shape::Mesh(mesh) = &shape.shape {
                    self.painted_textures.push(mesh.texture_id);
                }
                collect(shape.shape, &mut self.text);
            }
        }
        fn palette_tab(&mut self, active: bool) {
            self.palette_mode = active;
            self.frame(vec![]);
            self.frame(vec![]);
        }
        fn edit(&mut self, current: &str, replacement: &str) {
            self.click(current);
            self.replace_focused(replacement);
            self.enter();
        }
        fn link_ink(&mut self, name: &str) {
            // Ink is the second role; rect() deliberately chooses the last matching label.
            self.click("Default");
            self.click(name);
            assert_eq!(
                self.editor.project.as_ref().unwrap().assignments.get("ink"),
                Some(&Assignment::Link(name.into()))
            );
        }
        fn rect(&self, label: &str) -> Rect {
            self.text
                .iter()
                .rev()
                .find(|(text, _)| text == label)
                .unwrap_or_else(|| panic!("Missing {label:?}; visible: {:?}", self.text))
                .1
        }
        fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }
        }
        fn click_at(&mut self, pos: egui::Pos2) {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                Self::pointer(pos, true),
            ]);
            self.frame(vec![Self::pointer(pos, false)]);
            self.frame(vec![]);
        }
        fn click(&mut self, label: &str) {
            self.click_at(self.rect(label).center());
        }
        fn replace_focused(&mut self, text: &str) {
            self.frame(vec![
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::CTRL,
                },
                egui::Event::Text(text.to_owned()),
            ]);
        }
        fn enter(&mut self) {
            self.frame(vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            self.frame(vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        fn drop_on(&mut self, label: &str, color: Color) {
            let pos = self.rect(label).center();
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                Self::pointer(pos, true),
            ]);
            egui::DragAndDrop::set_payload(&self.ctx, color);
            self.frame(vec![Self::pointer(pos, false)]);
            self.frame(vec![]);
        }
    }

    fn linked_app(root: &std::path::Path) -> Headless {
        install_package(root);
        let mut repo = swatch_board_storage::PaletteRepository::open_default().unwrap();
        let saved = repo
            .save_named(
                None,
                "Named source",
                &[swatch_board_storage::Rgba8 {
                    r: 171,
                    g: 205,
                    b: 239,
                    a: 255,
                }],
                &[false],
                &["Accent".into()],
            )
            .unwrap();
        let mut app = Headless::new();
        app.palette_tab(true);
        app.click("Named source");
        assert_eq!(app.palette.theme_palette().0, Some(saved.id.as_str()));
        app.palette_tab(false);
        app.click("Select plugin");
        app.click("Runtime fixture");
        app.link_ink("Accent");
        app
    }

    #[test]
    fn palette_payload_is_not_consumed_by_history_type_probe() {
        isolated(
            "palette_payload_is_not_consumed_by_history_type_probe",
            |root| {
                let mut app = linked_app(root);
                app.click("Open drawer");
                let target = app.rect("Hidden title").center();
                app.frame(vec![
                    egui::Event::PointerMoved(target),
                    Headless::pointer(target, true),
                ]);
                egui::DragAndDrop::set_payload(
                    &app.ctx,
                    PaletteDrag {
                        index: 0,
                        color: Color::new(224, 160, 64, 255),
                    },
                );
                app.frame(vec![Headless::pointer(target, false)]);
                app.frame(vec![]);
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments.get("ink"),
                    Some(&Assignment::Override("#E0A040FF".into()))
                );
            },
        );
    }

    #[test]
    fn pinned_palette_starts_real_drag_into_theme_preview() {
        isolated(
            "pinned_palette_starts_real_drag_into_theme_preview",
            |root| {
                let mut app = linked_app(root);
                app.pinned_on_theme = true;
                app.editor
                    .assign("ink", Assignment::Override("#A344CC".into()));
                app.frame(vec![]);
                app.click("Open drawer");
                // Let the newly visible bottom panel settle before measuring drag coordinates.
                app.frame(vec![]);
                app.frame(vec![]);
                let source = app.rect("#ABCDEFFF").center() - egui::vec2(0.0, 40.0);
                let target = app.rect("Hidden title").center();
                app.frame(vec![egui::Event::PointerMoved(source)]);
                app.frame(vec![Headless::pointer(source, true)]);
                app.frame(vec![egui::Event::PointerMoved(
                    source + egui::vec2(15.0, -5.0),
                )]);
                app.frame(vec![egui::Event::PointerMoved(target)]);
                assert!(
                    egui::DragAndDrop::has_payload_of_type::<PaletteDrag>(&app.ctx),
                    "a real pinned-swatch drag must create the palette payload; source={source:?}, target={target:?}, text={:?}",
                    app.text
                );
                app.frame(vec![Headless::pointer(target, false)]);
                app.frame(vec![]);
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments.get("ink"),
                    Some(&Assignment::Override("#ABCDEFFF".into()))
                );
            },
        );
    }

    #[test]
    fn named_palette_link_propagates_and_survives_theme_undo_redo_and_reopen() {
        isolated(
            "named_palette_link_propagates_and_survives_theme_undo_redo_and_reopen",
            |root| {
                let mut app = linked_app(root);
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#ABCDEF");
                app.click("Undo");
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#445566");
                app.click("Redo");
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#ABCDEF");
                app.click("Save project");
                let saved_id = app.editor.project.as_ref().unwrap().id.clone();
                app.click("Preview export");
                assert_eq!(
                    app.editor.preview[0].content,
                    "ink=#ABCDEF\nsurface=#112233\n"
                );

                app.palette_tab(true);
                app.edit("#ABCDEFFF", "#102030");
                app.click("Save palette");
                let source_id = app.palette.theme_palette().0.unwrap().to_owned();
                let stored = swatch_board_storage::PaletteRepository::open_default()
                    .unwrap()
                    .list()
                    .unwrap()
                    .into_iter()
                    .find(|palette| palette.id == source_id)
                    .unwrap();
                assert_eq!(stored.swatch_names, vec!["Accent"]);
                assert_eq!(
                    stored.colors,
                    vec![swatch_board_storage::Rgba8 {
                        r: 16,
                        g: 32,
                        b: 48,
                        a: 255,
                    }]
                );
                app.palette_tab(false);
                // Links follow named swatches, not snapshots of their color at assignment time.
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#102030");
                assert_eq!(
                    app.editor.edits["ink"], "#102030FF",
                    "the linked role's editable text must follow the displayed swatch"
                );
                assert_eq!(ThemeEditor::new().project, app.editor.project);
                // A preview from before propagation must never write stale bytes.
                let export = root.join("linked-export");
                fs::create_dir(&export).unwrap();
                app.editor.output_dir = export.to_str().unwrap().into();
                app.frame(vec![]);
                app.click("Export previewed files");
                assert_eq!(
                    app.editor.error.as_deref(),
                    Some("Palette changed; preview export again.")
                );
                assert!(app.editor.preview.is_empty());
                assert!(!export.join("example.txt").exists());
                app.click("Preview export");
                assert_eq!(
                    app.editor.preview[0].content,
                    "ink=#102030\nsurface=#112233\n"
                );
                app.click("Export previewed files");
                assert!(app.editor.error.is_none(), "{:?}", app.editor.error);
                assert_eq!(
                    fs::read_to_string(export.join("example.txt")).unwrap(),
                    "ink=#102030\nsurface=#112233\n"
                );
                app.click("Open a project");
                app.click("Runtime fixture theme");
                assert_eq!(app.editor.project.as_ref().unwrap().id, saved_id);
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#102030");
            },
        );
    }

    #[test]
    fn links_require_saved_source_identity_and_explicit_palette_rebinding_is_undoable() {
        isolated(
            "links_require_saved_source_identity_and_explicit_palette_rebinding_is_undoable",
            |root| {
                let mut app = linked_app(root);
                let source = app.editor.project.as_ref().unwrap().palette_id.clone();
                app.palette_tab(true);
                app.click("Duplicate as draft");
                assert!(app.palette.theme_palette().0.is_none());
                app.palette_tab(false);
                assert_eq!(
                    app.editor.resolved(&app.palette).unwrap_err(),
                    "Open the project's saved source palette before resolving links"
                );
                // Identical names/colors in an unsaved draft are not the saved source.
                app.click("Use active palette");
                assert!(app.editor.project.as_ref().unwrap().palette_id.is_none());
                assert!(app.editor.resolved(&app.palette).is_err());
                app.click("Undo");
                assert_eq!(app.editor.project.as_ref().unwrap().palette_id, source);
                app.palette_tab(true);
                app.click("Save palette");
                let duplicate = app.palette.theme_palette().0.unwrap().to_owned();
                assert_ne!(Some(duplicate.clone()), source);
                app.palette_tab(false);
                assert!(app.editor.resolved(&app.palette).is_err());
                app.click("Use active palette");
                assert_eq!(
                    app.editor.project.as_ref().unwrap().palette_id,
                    Some(duplicate)
                );
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#ABCDEF");
                app.click("Undo");
                assert_eq!(app.editor.project.as_ref().unwrap().palette_id, source);
                assert!(app.editor.resolved(&app.palette).is_err());
                app.click("Redo");
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#ABCDEF");
                app.palette_tab(true);
                app.click("Named source");
                app.palette_tab(false);
                assert!(app.editor.resolved(&app.palette).is_err());
                app.click("Undo");
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#ABCDEF");
            },
        );
    }

    #[test]
    fn linked_swatch_rename_and_removal_fail_closed_without_losing_assignment() {
        isolated(
            "linked_swatch_rename_and_removal_fail_closed_without_losing_assignment",
            |root| {
                let mut app = linked_app(root);
                let assignments = app.editor.project.as_ref().unwrap().assignments.clone();
                app.palette_tab(true);
                app.edit("Accent", "Renamed");
                app.click("Save palette");
                app.palette_tab(false);
                assert_eq!(
                    app.editor.resolved(&app.palette).unwrap_err(),
                    "Linked palette swatch 'Accent' is missing"
                );
                app.click("Preview export");
                assert!(app.editor.preview.is_empty());
                assert!(
                    app.editor
                        .error
                        .as_ref()
                        .unwrap()
                        .contains("'Accent' is missing")
                );
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments,
                    assignments
                );
                app.palette_tab(true);
                app.click("Undo");
                app.palette_tab(false);
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#ABCDEF");

                app.palette_tab(true);
                // Hover the pinned swatch, then use its actual Delete overlay.
                let color_rect = app.rect("#ABCDEFFF");
                app.frame(vec![egui::Event::PointerMoved(egui::pos2(
                    color_rect.center().x,
                    color_rect.top() - 25.0,
                ))]);
                let delete = app
                    .text
                    .iter()
                    .filter(|(label, _)| label == "Delete")
                    .max_by(|a, b| a.1.center().y.total_cmp(&b.1.center().y))
                    .unwrap()
                    .1;
                app.click_at(delete.center());
                assert!(app.palette.theme_palette().2.is_empty());
                app.palette_tab(false);
                assert_eq!(
                    app.editor.resolved(&app.palette).unwrap_err(),
                    "Linked palette swatch 'Accent' is missing"
                );
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments,
                    assignments
                );
                app.palette_tab(true);
                app.click("Undo");
                app.palette_tab(false);
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#ABCDEF");
            },
        );
    }

    #[test]
    fn override_remains_independent_of_source_changes_and_source_deletion() {
        isolated(
            "override_remains_independent_of_source_changes_and_source_deletion",
            |root| {
                let mut app = linked_app(root);
                app.edit("#ABCDEFFF", "#556677");
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments["ink"],
                    Assignment::Override("#556677FF".into())
                );
                app.click("Undo");
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#ABCDEF");
                app.click("Redo");
                app.palette_tab(true);
                app.edit("#ABCDEFFF", "#998877");
                app.edit("Accent", "Different name");
                app.click("Save palette");
                app.palette_tab(false);
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#556677");
                app.palette_tab(true);
                app.click("Delete");
                app.click("Confirm delete");
                assert!(app.palette.theme_palette().0.is_none());
                app.palette_tab(false);
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#556677");
                app.click("Undo");
                assert_eq!(
                    app.editor.resolved(&app.palette).unwrap_err(),
                    "Open the project's saved source palette before resolving links"
                );
                app.click("Redo");
                assert_eq!(app.editor.resolved(&app.palette).unwrap()["ink"], "#556677");
            },
        );
    }

    #[test]
    fn runtime_png_asset_is_painted_and_reload_recovers_from_corrupt_asset() {
        isolated(
            "runtime_png_asset_is_painted_and_reload_recovers_from_corrupt_asset",
            |root| {
                install_package(root);
                let package = root.join("data/swatch-board/plugins/runtime-fixture");
                fs::create_dir(package.join("assets")).unwrap();
                let asset = package.join("assets/pixel.png");
                let pixel = image::RgbaImage::from_pixel(2, 3, image::Rgba([20, 80, 160, 255]));
                pixel.save(&asset).unwrap();
                let scene_path = package.join("scenes.json");
                let mut scenes: serde_json::Value =
                    serde_json::from_slice(&fs::read(&scene_path).unwrap()).unwrap();
                scenes[0]["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({
                        "id":"picture", "kind":"image", "rect":[220,20,100,100],
                        "asset":"assets/pixel.png"
                    }));
                fs::write(&scene_path, serde_json::to_vec(&scenes).unwrap()).unwrap();
                let mut app = Headless::new();
                app.frame(vec![]);
                app.click("Select plugin");
                app.click("Runtime fixture");
                assert!(app.editor.error.is_none(), "{:?}", app.editor.error);
                let texture = &app.editor.textures["assets/pixel.png"];
                assert_eq!(texture.size(), [2, 3]);
                assert!(app.painted_textures.contains(&texture.id()));
                let project = app.editor.project.clone();

                fs::write(&asset, b"not a PNG").unwrap();
                app.click("Reload Plugins");
                assert!(app.editor.textures.is_empty());
                assert!(app.editor.error.as_ref().unwrap().contains("invalid PNG"));
                assert_eq!(app.editor.project, project);
                pixel.save(&asset).unwrap();
                app.click("Reload Plugins");
                assert!(app.editor.error.is_none(), "{:?}", app.editor.error);
                let texture = &app.editor.textures["assets/pixel.png"];
                assert_eq!(texture.size(), [2, 3]);
                assert!(app.painted_textures.contains(&texture.id()));
                assert_eq!(app.editor.project, project);
            },
        );
    }

    #[test]
    fn external_plugin_hidden_scene_text_role_accepts_selection_and_drop() {
        isolated(
            "external_plugin_hidden_scene_text_role_accepts_selection_and_drop",
            |root| {
                let mut app = Headless::new();
                app.frame(vec![]);
                assert!(
                    !app.editor
                        .plugins
                        .iter()
                        .any(|p| p.manifest.id == "runtime-fixture")
                );
                install_package(root);
                app.click("Reload Plugins");
                app.click("Select plugin");
                app.click("Runtime fixture");
                assert_eq!(
                    app.editor.project.as_ref().unwrap().plugin_id,
                    "runtime-fixture"
                );
                assert!(!app.text.iter().any(|(s, _)| s == "Hidden title"));
                app.click("Open drawer");
                assert_eq!(app.editor.project.as_ref().unwrap().scene, "drawer");
                app.click("Hidden title");
                assert_eq!(app.editor.selected.as_deref(), Some("ink"));
                let color = Color::new(171, 205, 239, 255);
                app.drop_on("Hidden title", color);
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments.get("ink"),
                    Some(&Assignment::Override(color.format(Format::HexRgba))),
                    "assignments: {:?}",
                    app.editor.project.as_ref().unwrap().assignments
                );
                app.click("Save project");
                assert!(app.editor.error.is_none(), "{:?}", app.editor.error);
                let id = app.editor.project.as_ref().unwrap().id.clone();
                // Read through the same repository, then recreate the editor from its persisted draft.
                let record = app
                    .editor
                    .repo
                    .as_ref()
                    .unwrap()
                    .load(&id)
                    .unwrap()
                    .unwrap();
                let saved: Project = serde_json::from_str(&record.payload).unwrap();
                assert_eq!(saved.scene, "drawer");
                assert_eq!(
                    saved.assignments,
                    app.editor.project.as_ref().unwrap().assignments
                );
                let restored = ThemeEditor::new();
                assert_eq!(restored.project, app.editor.project);
                app.click("Undo");
                assert!(
                    !app.editor
                        .project
                        .as_ref()
                        .unwrap()
                        .assignments
                        .contains_key("ink")
                );
                app.click("Redo");
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments,
                    saved.assignments
                );
                app.click("Close drawer");
                assert_eq!(app.editor.project.as_ref().unwrap().scene, "home");
                app.click("Preview export");
                assert!(app.editor.error.is_none(), "{:?}", app.editor.error);
                assert_eq!(app.editor.preview.len(), 1);
                assert!(app.editor.preview[0].content.contains("#ABCDEF"));
                let export = root.join("export");
                fs::create_dir(&export).unwrap();
                let directory_label = app.rect("Export directory");
                app.click_at(egui::pos2(
                    directory_label.right() + 80.0,
                    directory_label.center().y,
                ));
                app.replace_focused(export.to_str().unwrap());
                assert_eq!(app.editor.output_dir, export.to_str().unwrap());
                app.click("Export previewed files");
                assert!(app.editor.error.is_none(), "{:?}", app.editor.error);
                assert_eq!(
                    fs::read_to_string(export.join("example.txt")).unwrap(),
                    app.editor.preview[0].content
                );
                fs::write(export.join("example.txt"), "existing user file").unwrap();
                app.click("Export previewed files");
                assert!(app.editor.error.is_some());
                assert_eq!(
                    fs::read_to_string(export.join("example.txt")).unwrap(),
                    "existing user file"
                );
                app.click("Confirm overwrite existing files");
                app.click("Export previewed files");
                assert!(app.editor.error.is_none(), "{:?}", app.editor.error);
                assert_eq!(
                    fs::read_to_string(export.join("example.txt")).unwrap(),
                    app.editor.preview[0].content
                );
                app.click("Open a project");
                app.click("Runtime fixture theme");
                app.click("Keep editing");
                assert_eq!(app.editor.project.as_ref().unwrap().scene, "home");
                app.click("Open a project");
                app.click("Runtime fixture theme");
                app.click("Discard and continue");
                assert_eq!(app.editor.project.as_ref().unwrap().scene, "drawer");
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments,
                    saved.assignments
                );
            },
        );
    }

    #[test]
    fn correcting_invalid_role_text_clears_error_and_persists_override() {
        isolated(
            "correcting_invalid_role_text_clears_error_and_persists_override",
            |root| {
                install_package(root);
                let mut app = Headless::new();
                app.frame(vec![]);
                app.click("Select plugin");
                app.click("Runtime fixture");
                let undo_count = app.editor.undo.len();
                app.click("#445566FF");
                app.replace_focused("not a color");
                app.enter();
                assert!(
                    app.editor
                        .error
                        .as_ref()
                        .unwrap()
                        .contains("Invalid color for Ink")
                );
                assert!(app.editor.project.as_ref().unwrap().assignments.is_empty());
                assert_eq!(app.editor.undo.len(), undo_count);
                app.click("not a color");
                app.replace_focused("#102030");
                app.enter();
                assert_eq!(
                    app.editor.project.as_ref().unwrap().assignments.get("ink"),
                    Some(&Assignment::Override("#102030FF".into()))
                );
                assert!(app.editor.error.is_none(), "{:?}", app.editor.error);
                assert_eq!(ThemeEditor::new().project, app.editor.project);
            },
        );
    }

    #[test]
    fn new_project_clears_previous_role_selection_after_discard_confirmation() {
        isolated(
            "new_project_clears_previous_role_selection_after_discard_confirmation",
            |root| {
                install_package(root);
                let mut app = Headless::new();
                app.frame(vec![]);
                app.click("Select plugin");
                app.click("Runtime fixture");
                app.click("Open drawer");
                app.click("Hidden title");
                assert_eq!(app.editor.selected.as_deref(), Some("ink"));
                app.click("runtime-fixture");
                app.click("Runtime fixture");
                app.click("Keep editing");
                assert_eq!(app.editor.selected.as_deref(), Some("ink"));
                assert_eq!(app.editor.project.as_ref().unwrap().scene, "drawer");
                app.click("runtime-fixture");
                app.click("Runtime fixture");
                app.click("Discard and continue");
                assert_eq!(app.editor.project.as_ref().unwrap().scene, "home");
                assert_eq!(app.editor.selected, None);
            },
        );
    }
}
