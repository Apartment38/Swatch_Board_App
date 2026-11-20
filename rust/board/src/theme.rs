use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use swatch_board_color::{Color, Format};
use swatch_board_storage::{SavedTheme, ThemeRepository};
use swatch_board_theme_core::{self as core, Plugin, PluginMetadata, RenderedFile};

use crate::palette::PaletteEditor;

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

impl Project {
    fn same_content(&self, other: &Self) -> bool {
        fn effective(
            map: &BTreeMap<String, Assignment>,
        ) -> impl Iterator<Item = (&String, &Assignment)> {
            map.iter()
                .filter(|(_, v)| !matches!(v, Assignment::Default))
        }
        self.version == other.version
            && self.name == other.name
            && self.plugin_id == other.plugin_id
            && self.plugin_version == other.plugin_version
            && self.palette_id == other.palette_id
            && self.scene == other.scene
            && effective(&self.assignments).eq(effective(&other.assignments))
    }
}

type PreviewCache = Option<(BTreeMap<String, String>, Vec<RenderedFile>)>;

pub struct ThemeEditor {
    plugins: Vec<PluginMetadata>,
    plugin: Option<Plugin>,
    project: Option<Project>,
    saved_state: Option<Project>,
    repo: Option<ThemeRepository>,
    saved: Vec<SavedTheme>,
    undo: Vec<Project>,
    redo: Vec<Project>,
    selected: Option<String>,
    preview: Vec<RenderedFile>,
    preview_cache: RefCell<PreviewCache>,
    error: Option<String>,
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
            saved_state: None,
            repo,
            saved: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            selected: None,
            preview: Vec::new(),
            preview_cache: RefCell::new(None),
            error,
        };
        if let Some(repo) = &this.repo {
            match repo.load_draft() {
                Ok(Some(json)) => match serde_json::from_str::<Project>(&json) {
                    Ok(p) if p.version == 1 => this.project = Some(p),
                    Ok(_) => this.push_error("Unsupported theme draft version".into()),
                    Err(e) => this.push_error(format!("Cannot read theme draft: {e}")),
                },
                Ok(None) => (),
                Err(e) => this.push_error(e.to_string()),
            }
        }
        this.reload();
        this.refresh();
        this.sync_saved_state();
        this
    }

    fn push_error(&mut self, message: String) {
        match &mut self.error {
            Some(existing) if !existing.is_empty() => {
                existing.push('\n');
                existing.push_str(&message);
            }
            slot => *slot = Some(message),
        }
    }
    fn update_dirty(&mut self) {
        if let Some(project) = &mut self.project {
            project.dirty = self
                .saved_state
                .as_ref()
                .is_none_or(|saved| !project.same_content(saved));
        }
    }
    fn sync_saved_state(&mut self) {
        let previous_dirty = self.project.as_ref().map(|p| p.dirty);
        self.saved_state = self
            .project
            .as_ref()
            .filter(|project| !project.id.is_empty())
            .and_then(|project| self.saved.iter().find(|r| r.id == project.id))
            .and_then(|record| {
                serde_json::from_str::<Project>(&record.payload)
                    .ok()
                    .filter(|saved| saved.version == 1)
                    .map(|mut saved| {
                        saved.id = record.id.clone();
                        saved.dirty = false;
                        saved
                    })
            });
        self.update_dirty();
        if self.project.as_ref().map(|p| p.dirty) != previous_dirty {
            self.persist();
        }
    }
    fn refresh(&mut self) {
        if let Some(repo) = &self.repo {
            match repo.list() {
                Ok(saved) => self.saved = saved,
                Err(e) => self.push_error(e.to_string()),
            }
        }
    }
    fn reload(&mut self) {
        let (plugins, errors) = core::discover();
        self.plugins = plugins;
        self.preview_cache.replace(None);
        self.plugin = None;
        let mut scene_updated = false;
        let mut load_error = None;
        if let Some(project) = &mut self.project {
            if let Some(meta) = self.plugins.iter().find(|p| {
                p.manifest.id == project.plugin_id && p.manifest.version == project.plugin_version
            }) {
                match core::load(&meta.path) {
                    Ok(plugin) => {
                        if !plugin.scenes.iter().any(|s| s.id == project.scene) {
                            if let Some(first) = plugin.scenes.first() {
                                project.scene = first.id.clone();
                                scene_updated = true;
                            }
                        }
                        self.plugin = Some(plugin);
                    }
                    Err(e) => load_error = Some(e),
                }
            } else {
                load_error = Some(format!(
                    "Plugin {} version {} is missing or incompatible; project retained",
                    project.plugin_id, project.plugin_version
                ));
            }
        }
        if scene_updated {
            self.update_dirty();
            self.persist();
        }
        if let Some(e) = load_error {
            self.push_error(e);
        }
        if !errors.is_empty() {
            self.push_error(errors.join("\n"));
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
        if let Some(project) = &self.project {
            if !project.same_content(&previous) {
                if self.undo.len() == 100 {
                    self.undo.remove(0);
                }
                self.undo.push(previous);
                self.redo.clear();
                self.preview.clear();
                self.update_dirty();
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
                self.saved_state = None;
                self.preview_cache.replace(None);
                self.plugin = Some(plugin);
                self.selected = None;
                self.undo.clear();
                self.redo.clear();
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
        let Some(repo) = &mut self.repo else {
            self.error = Some("Theme storage unavailable".into());
            return;
        };
        let Some(project) = &mut self.project else {
            self.error = Some("Select a plugin first".into());
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
                project.id = id.clone();
                project.dirty = false;
                let snapshot = project.clone();
                for entry in self.undo.iter_mut().chain(self.redo.iter_mut()) {
                    entry.id = id.clone();
                }
                self.saved_state = Some(snapshot);
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
                self.saved_state = Some(p.clone());
                self.project = Some(p);
                self.undo.clear();
                self.redo.clear();
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
            match assignment {
                Assignment::Default => {
                    if matches!(
                        project.assignments.get(role),
                        None | Some(Assignment::Default)
                    ) {
                        return;
                    }
                    project.assignments.remove(role);
                }
                other => {
                    project.assignments.insert(role.to_owned(), other);
                }
            }
            self.changed(previous);
        }
    }
    pub fn snapshot(&self, palette: &PaletteEditor, format: Format) -> Value {
        let resolved = self.project.as_ref().map(|_| self.resolved(palette));
        let values = resolved.as_ref().and_then(|r| r.as_ref().ok());
        let rendered = values.and_then(|v| {
            self.plugin.as_ref().map(|p| {
                let mut cache = self.preview_cache.borrow_mut();
                if let Some((key, files)) = cache.as_ref() {
                    if key == v {
                        return Ok(files.clone());
                    }
                }
                match p.render(v) {
                    Ok(files) => {
                        *cache = Some((v.clone(), files.clone()));
                        Ok(files)
                    }
                    Err(err) => {
                        *cache = None;
                        Err(err)
                    }
                }
            })
        });
        let (palette_id, palette_name, swatches) = palette.theme_palette();
        let mut scene_error = None;
        let scene = self.plugin.as_ref().and_then(|plugin| {
            let project = self.project.as_ref()?;
            let scene = plugin
                .scenes
                .iter()
                .find(|s| s.id == project.scene)
                .or_else(|| plugin.scenes.first())?;
            let elements: Vec<Value> = scene.nodes.iter().map(|node| {
                let color = |id: Option<&String>, fallback: &str| {
                    id.and_then(|id| values.and_then(|v| v.get(id)))
                        .and_then(|v| v.parse::<Color>().ok())
                        .map(|c| c.format(Format::HexRgba))
                        .unwrap_or_else(|| fallback.to_owned())
                };
                let text_only = matches!(node.kind.as_str(), "text" | "syntax");
                let role = if text_only {
                    node.text_role.as_ref().or(node.role.as_ref())
                } else {
                    node.role.as_ref().or(node.text_role.as_ref())
                }.or(node.border_role.as_ref());
                let asset = node.asset.as_ref().and_then(|path| match plugin.asset(path) {
                    Ok(bytes) => Some(bytes),
                    Err(e) => { scene_error = Some(format!("Cannot load {path}: {e}")); None }
                });
                json!({
                    "id": node.id, "kind": node.kind, "rect": node.rect,
                    "fill": if text_only { "#00000000".to_owned() } else { color(node.role.as_ref(), "#00000000") },
                    "border": color(node.border_role.as_ref(), "#00000000"),
                    "text": node.text, "text_color": color(node.text_role.as_ref().or(if text_only { node.role.as_ref() } else { None }), "#FFFFFFFF"),
                    "font_size": node.font_size, "role": role, "target": node.target,
                    "enabled": node.enabled, "asset": asset
                })
            }).collect();
            Some(json!({"width": scene.width, "height": scene.height, "elements": elements}))
        });
        let roles: Vec<Value> = self
            .plugin
            .as_ref()
            .map(|plugin| {
                plugin
                    .roles
                    .iter()
                    .map(|role| {
                        let assignment = self
                            .project
                            .as_ref()
                            .and_then(|p| p.assignments.get(&role.id));
                        let (kind, link) = match assignment {
                            Some(Assignment::Override(_)) => ("override", None),
                            Some(Assignment::Link(name)) => ("link", Some(name.as_str())),
                            _ => ("default", None),
                        };
                        let value = values
                            .and_then(|v| v.get(&role.id))
                            .unwrap_or(&role.default);
                        let parsed = value.parse::<Color>().ok();
                        json!({"id": role.id, "name": role.title, "help": role.help,
                "color": parsed.map(|c| c.format(Format::HexRgba)),
                "value": parsed.map(|c| c.format(format)).unwrap_or_else(|| value.clone()),
                "assignment": kind, "link": link, "links": role.links})
                    })
                    .collect()
            })
            .unwrap_or_default();
        let error = self
            .error
            .as_ref()
            .or(scene_error.as_ref())
            .or_else(|| rendered.as_ref().and_then(|r| r.as_ref().err()))
            .or_else(|| resolved.as_ref().and_then(|r| r.as_ref().err()));
        json!({
            "plugins": self.plugins.iter().map(|p| json!({"id":p.manifest.id,"name":p.manifest.name})).collect::<Vec<_>>(),
            "saved": self.saved.iter().map(|p| json!({"id":p.id,"name":p.name})).collect::<Vec<_>>(),
            "project": self.project.as_ref().map(|p| json!({"name":p.name,"dirty":p.dirty,"plugin_id":p.plugin_id,"scene":p.scene,"palette_id":p.palette_id})),
            "scenes": self.plugin.as_ref().map(|p| p.scenes.iter().map(|s| json!({"id":s.id,"name":s.title})).collect::<Vec<_>>()).unwrap_or_default(),
            "roles": roles, "palette_swatches": swatches.iter().map(|(name,color)| json!({"name":name,"color":color.format(Format::HexRgba)})).collect::<Vec<_>>(),
            "palette_id": palette_id, "palette_name": palette_name,
            "selected": self.selected, "preview": rendered.as_ref().and_then(|r| r.as_ref().ok()), "scene": scene,
            "can_undo": !self.undo.is_empty(), "can_redo": !self.redo.is_empty(), "error": error
        })
    }

    pub fn command(&mut self, request: &Value, palette: &PaletteEditor) -> Result<(), String> {
        self.error = None;
        let result = self.command_inner(request, palette);
        if let Err(error) = &result {
            self.error = Some(error.clone());
        }
        result
    }

    fn command_inner(&mut self, request: &Value, palette: &PaletteEditor) -> Result<(), String> {
        let field = |name: &str| {
            request
                .get(name)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("Missing {name}"))
        };
        let op = field("op")?;
        match op {
            "theme_reload" => {
                self.reload();
                self.refresh();
                self.sync_saved_state();
            }
            "theme_choose" | "theme_open" => {
                let id = field("id")?;
                if self.project.as_ref().is_some_and(|p| p.dirty)
                    && request.get("discard").and_then(Value::as_bool) != Some(true)
                {
                    return Err("Discard the current unsaved theme edits?".into());
                }
                if op == "theme_choose" {
                    let index = self
                        .plugins
                        .iter()
                        .position(|p| p.manifest.id == id)
                        .ok_or("Unknown plugin")?;
                    self.choose(index, palette);
                } else {
                    let record = self
                        .saved
                        .iter()
                        .find(|p| p.id == id)
                        .cloned()
                        .ok_or("Unknown project")?;
                    self.open(&record);
                    self.selected = None;
                }
            }
            "theme_name" | "theme_scene" | "theme_palette_bind" => {
                if op == "theme_scene" {
                    let id = field("id")?;
                    if !self
                        .plugin
                        .as_ref()
                        .is_some_and(|p| p.scenes.iter().any(|s| s.id == id))
                    {
                        return Err("Unknown scene".into());
                    }
                }
                let project = self.project.as_mut().ok_or("Select a plugin first")?;
                let previous = project.clone();
                match op {
                    "theme_name" => project.name = field("name")?.to_owned(),
                    "theme_scene" => project.scene = field("id")?.to_owned(),
                    _ => {
                        let id = palette
                            .theme_palette()
                            .0
                            .ok_or("Save the active palette before binding it to a theme")?;
                        project.palette_id = Some(id.to_owned());
                    }
                }
                self.changed(previous);
            }
            "theme_save" => self.save(),
            "theme_select" | "theme_assign" | "theme_default" | "theme_link" => {
                let id = field("role")?;
                let plugin = self
                    .plugin
                    .as_ref()
                    .ok_or("The selected plugin is unavailable")?;
                let role = plugin
                    .roles
                    .iter()
                    .find(|r| r.id == id)
                    .ok_or("Unknown role")?;
                if self.project.is_none() {
                    return Err("Select a plugin first".into());
                }
                match op {
                    "theme_select" => self.selected = Some(id.to_owned()),
                    "theme_default" => self.assign(id, Assignment::Default),
                    "theme_assign" => {
                        let color = field("color")?
                            .parse::<Color>()
                            .map_err(|_| format!("Invalid color for {}", role.title))?;
                        let value = color.format(Format::HexRgba);
                        let mut input = BTreeMap::new();
                        input.insert(id.to_owned(), value.clone());
                        plugin.resolve(&input)?;
                        self.assign(id, Assignment::Override(value));
                    }
                    _ => {
                        let name = field("name")?;
                        if name.trim().is_empty() {
                            let current_link =
                                self.project
                                    .as_ref()
                                    .and_then(|p| match p.assignments.get(id) {
                                        Some(Assignment::Link(linked)) => Some(linked.clone()),
                                        _ => None,
                                    });
                            if let Some(linked_name) = current_link {
                                let (palette_id, _, swatches) = palette.theme_palette();
                                let replacement = if palette_id.is_some()
                                    && self.project.as_ref().and_then(|p| p.palette_id.as_deref())
                                        == palette_id
                                {
                                    swatches.iter().find(|(n, _)| n == &linked_name).and_then(
                                        |(_, c)| {
                                            let value = c.format(Format::HexRgba);
                                            let mut input = BTreeMap::new();
                                            input.insert(id.to_owned(), value.clone());
                                            let resolved_link = plugin.resolve(&input).ok()?;
                                            let resolved_default =
                                                plugin.resolve(&BTreeMap::new()).ok()?;
                                            if resolved_link.get(id) != resolved_default.get(id) {
                                                Some(Assignment::Override(value))
                                            } else {
                                                None
                                            }
                                        },
                                    )
                                } else {
                                    None
                                };
                                self.assign(id, replacement.unwrap_or(Assignment::Default));
                            }
                        } else {
                            let (palette_id, _, swatches) = palette.theme_palette();
                            if palette_id.is_none()
                                || self.project.as_ref().and_then(|p| p.palette_id.as_deref())
                                    != palette_id
                            {
                                return Err(
                                    "Open the project's saved source palette before resolving links"
                                        .into(),
                                );
                            }
                            let Some((_, swatch_color)) = swatches.iter().find(|(n, _)| n == name)
                            else {
                                return Err("Unknown or unavailable palette swatch".into());
                            };
                            if !role.links.is_empty() && !role.links.iter().any(|n| n == name) {
                                return Err("Unknown or unavailable palette swatch".into());
                            }
                            let value = swatch_color.format(Format::HexRgba);
                            let mut input = BTreeMap::new();
                            input.insert(id.to_owned(), value);
                            plugin.resolve(&input)?;
                            self.assign(id, Assignment::Link(name.to_owned()));
                        }
                    }
                }
            }
            "theme_undo" | "theme_redo" => {
                let is_undo = op == "theme_undo";
                let has_entry = if is_undo {
                    !self.undo.is_empty()
                } else {
                    !self.redo.is_empty()
                };
                if !has_entry {
                    return Err("Nothing to undo or redo".into());
                }
                let project = self.project.as_mut().ok_or("Select a plugin first")?;
                let mut next = if is_undo {
                    self.undo.pop().unwrap()
                } else {
                    self.redo.pop().unwrap()
                };
                if !project.id.is_empty() {
                    next.id = project.id.clone();
                }
                let old = std::mem::replace(project, next);
                if is_undo {
                    self.redo.push(old);
                } else {
                    self.undo.push(old);
                }
                self.update_dirty();
                self.preview.clear();
                self.persist();
            }
            "theme_preview" => {
                let values = self.resolved(palette)?;
                let rendered = self
                    .plugin
                    .as_ref()
                    .ok_or("The selected plugin is unavailable")?
                    .render(&values);
                match rendered {
                    Ok(files) => {
                        self.preview_cache.replace(Some((values, files.clone())));
                        self.preview = files;
                    }
                    Err(err) => {
                        self.preview_cache.replace(None);
                        self.preview.clear();
                        return Err(err);
                    }
                }
            }
            "theme_export" => {
                let directory = field("directory")?;
                if directory.trim().is_empty() {
                    return Err("Select an export directory first".into());
                }
                let values = self.resolved(palette)?;
                let files = self
                    .plugin
                    .as_ref()
                    .ok_or("The selected plugin is unavailable")?
                    .render(&values)?;
                core::export(
                    &PathBuf::from(directory),
                    &files,
                    request.get("overwrite").and_then(Value::as_bool) == Some(true),
                )?;
            }
            _ => return Err(format!("Unknown theme operation {op}")),
        }
        match &self.error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
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

    #[test]
    fn commands_preserve_project_scene_export_and_discard_guard() {
        isolated(
            "commands_preserve_project_scene_export_and_discard_guard",
            |root| {
                install_package(root);
                let palette = PaletteEditor::new();
                let mut editor = ThemeEditor::new();
                let cmd = |editor: &mut ThemeEditor, value| editor.command(&value, &palette);
                cmd(
                    &mut editor,
                    json!({"op":"theme_choose","id":"runtime-fixture"}),
                )
                .unwrap();
                assert!(
                    cmd(
                        &mut editor,
                        json!({"op":"theme_choose","id":"runtime-fixture"})
                    )
                    .is_err()
                );
                assert!(cmd(&mut editor, json!({"op":"theme_scene","id":"missing"})).is_err());
                cmd(&mut editor, json!({"op":"theme_scene","id":"drawer"})).unwrap();
                let state = editor.snapshot(&palette, Format::HexRgba);
                assert_eq!(state["scene"]["elements"][1]["role"], "ink");
                assert_eq!(state["scene"]["elements"][2]["target"], "home");
                assert!(
                    cmd(
                        &mut editor,
                        json!({"op":"theme_assign","role":"missing","color":"#abcdef"})
                    )
                    .is_err()
                );
                assert!(
                    cmd(
                        &mut editor,
                        json!({"op":"theme_assign","role":"ink","color":"invalid"})
                    )
                    .is_err()
                );
                cmd(
                    &mut editor,
                    json!({"op":"theme_assign","role":"ink","color":"#abcdef"}),
                )
                .unwrap();
                assert_eq!(editor.resolved(&palette).unwrap()["ink"], "#ABCDEF");
                cmd(&mut editor, json!({"op":"theme_undo"})).unwrap();
                assert_eq!(editor.resolved(&palette).unwrap()["ink"], "#445566");
                cmd(&mut editor, json!({"op":"theme_redo"})).unwrap();
                cmd(&mut editor, json!({"op":"theme_save"})).unwrap();
                let id = editor.project.as_ref().unwrap().id.clone();
                assert_eq!(ThemeEditor::new().project, editor.project);
                cmd(&mut editor, json!({"op":"theme_preview"})).unwrap();
                let output = root.join("export");
                fs::create_dir(&output).unwrap();
                let request = json!({"op":"theme_export","directory":output.to_str().unwrap(),"overwrite":false});
                cmd(&mut editor, request.clone()).unwrap();
                assert_eq!(
                    fs::read_to_string(output.join("example.txt")).unwrap(),
                    "ink=#ABCDEF\nsurface=#112233\n"
                );
                assert!(cmd(&mut editor, request).is_err());
                cmd(&mut editor, json!({"op":"theme_scene","id":"home"})).unwrap();
                assert!(cmd(&mut editor, json!({"op":"theme_open","id":id})).is_err());
                cmd(
                    &mut editor,
                    json!({"op":"theme_open","id":id,"discard":true}),
                )
                .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().scene, "drawer");
            },
        );
    }

    #[test]
    fn named_links_follow_palette_and_export_current_values() {
        isolated(
            "named_links_follow_palette_and_export_current_values",
            |root| {
                install_package(root);
                let mut repo = swatch_board_storage::PaletteRepository::open_default().unwrap();
                let saved = repo
                    .save_named(
                        None,
                        "Source",
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
                let mut palette = PaletteEditor::new();
                let mut mixer = crate::mixer::Mixer::default();
                palette
                    .command(&json!({"op":"palette_open","id":saved.id}), &mut mixer)
                    .unwrap();
                let mut editor = ThemeEditor::new();
                editor
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(
                        &json!({"op":"theme_link","role":"ink","name":"Accent"}),
                        &palette,
                    )
                    .unwrap();
                assert_eq!(editor.resolved(&palette).unwrap()["ink"], "#ABCDEF");
                editor
                    .command(&json!({"op":"theme_preview"}), &palette)
                    .unwrap();
                palette
                    .command(
                        &json!({"op":"palette_edit","index":0,"name":"Accent","color":"#102030"}),
                        &mut mixer,
                    )
                    .unwrap();
                assert_eq!(editor.resolved(&palette).unwrap()["ink"], "#102030");
                assert!(
                    editor.snapshot(&palette, Format::HexRgba)["preview"][0]["content"]
                        .as_str()
                        .unwrap()
                        .contains("#102030")
                );
                let output = root.join("export");
                fs::create_dir(&output).unwrap();
                assert!(editor.command(&json!({"op":"theme_export","directory":output.to_str().unwrap(),"overwrite":false}), &palette).is_ok());
                assert!(
                    fs::read_to_string(output.join("example.txt"))
                        .unwrap()
                        .contains("#102030")
                );
                palette
                    .command(
                        &json!({"op":"palette_edit","index":0,"name":"Renamed","color":"#102030"}),
                        &mut mixer,
                    )
                    .unwrap();
                assert!(
                    editor
                        .resolved(&palette)
                        .unwrap_err()
                        .contains("'Accent' is missing")
                );
                assert_eq!(
                    editor.project.as_ref().unwrap().assignments["ink"],
                    Assignment::Link("Accent".into())
                );
                editor
                    .command(&json!({"op":"theme_undo"}), &palette)
                    .unwrap();
                assert_eq!(editor.resolved(&palette).unwrap()["ink"], "#445566");
                editor
                    .command(&json!({"op":"theme_redo"}), &palette)
                    .unwrap();
                assert!(editor.resolved(&palette).is_err());
            },
        );
    }

    #[test]
    fn undo_redo_after_save_preserves_id_and_tracks_dirty_state() {
        isolated(
            "undo_redo_after_save_preserves_id_and_tracks_dirty_state",
            |root| {
                install_package(root);
                let palette = PaletteEditor::new();
                let mut editor = ThemeEditor::new();
                editor
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(
                        &json!({"op":"theme_assign","role":"ink","color":"#abcdef"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_save"}), &palette)
                    .unwrap();
                let saved_id = editor.project.as_ref().unwrap().id.clone();
                assert!(!saved_id.is_empty());
                assert!(!editor.project.as_ref().unwrap().dirty);
                assert_eq!(editor.saved.len(), 1);

                // Undo after save must keep Project.id and set dirty = true (discard guard active).
                editor
                    .command(&json!({"op":"theme_undo"}), &palette)
                    .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().id, saved_id);
                assert!(editor.project.as_ref().unwrap().dirty);
                assert!(
                    editor
                        .command(
                            &json!({"op":"theme_choose","id":"runtime-fixture"}),
                            &palette,
                        )
                        .is_err()
                );

                // Saving after undo must update the existing project rather than create a duplicate.
                editor
                    .command(&json!({"op":"theme_save"}), &palette)
                    .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().id, saved_id);
                assert!(!editor.project.as_ref().unwrap().dirty);
                assert_eq!(editor.saved.len(), 1);

                // Redo moves away from the newly saved state -> dirty = true.
                editor
                    .command(&json!({"op":"theme_redo"}), &palette)
                    .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().id, saved_id);
                assert!(editor.project.as_ref().unwrap().dirty);

                // Save the redone state, then undo back to the previously saved state:
                // dirty must be true (never stale false from the undo entry).
                editor
                    .command(&json!({"op":"theme_save"}), &palette)
                    .unwrap();
                assert!(!editor.project.as_ref().unwrap().dirty);
                assert_eq!(editor.saved.len(), 1);
                editor
                    .command(&json!({"op":"theme_undo"}), &palette)
                    .unwrap();
                assert!(editor.project.as_ref().unwrap().dirty);
                assert!(
                    editor
                        .command(
                            &json!({"op":"theme_choose","id":"runtime-fixture"}),
                            &palette,
                        )
                        .is_err()
                );

                // Redoing back to the saved state clears dirty and allows switching without discard.
                editor
                    .command(&json!({"op":"theme_redo"}), &palette)
                    .unwrap();
                assert!(!editor.project.as_ref().unwrap().dirty);
                editor
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();
            },
        );
    }

    #[test]
    fn theme_link_validates_alpha_and_supports_unlinking() {
        isolated(
            "theme_link_validates_alpha_and_supports_unlinking",
            |root| {
                install_package(root);
                // Add an alpha-capable role to the fixture package.
                let package = root.join("data/swatch-board/plugins/runtime-fixture");
                fs::write(
                    package.join("roles.json"),
                    serde_json::to_vec_pretty(&json!([
                        {"id":"surface", "title":"Surface", "default":"#112233", "alpha":false},
                        {"id":"ink", "title":"Ink", "default":"#445566", "alpha":false},
                        {"id":"tint", "title":"Tint", "default":"#44556680", "alpha":true}
                    ]))
                    .unwrap(),
                )
                .unwrap();

                let mut repo = swatch_board_storage::PaletteRepository::open_default().unwrap();
                let saved = repo
                    .save_named(
                        None,
                        "Source",
                        &[
                            swatch_board_storage::Rgba8 {
                                r: 0xAB,
                                g: 0xCD,
                                b: 0xEF,
                                a: 0xFF,
                            },
                            swatch_board_storage::Rgba8 {
                                r: 0xAB,
                                g: 0xCD,
                                b: 0xEF,
                                a: 0x80,
                            },
                            swatch_board_storage::Rgba8 {
                                r: 0x44,
                                g: 0x55,
                                b: 0x66,
                                a: 0xFF,
                            },
                        ],
                        &[false, false, false],
                        &["Accent".into(), "Glass".into(), "DefaultInk".into()],
                    )
                    .unwrap();
                let mut palette = PaletteEditor::new();
                let mut mixer = crate::mixer::Mixer::default();
                palette
                    .command(&json!({"op":"palette_open","id":saved.id}), &mut mixer)
                    .unwrap();

                let mut editor = ThemeEditor::new();
                editor
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();

                // Translucent swatch rejected for opaque role without corrupting assignments/preview.
                let err = editor
                    .command(
                        &json!({"op":"theme_link","role":"ink","name":"Glass"}),
                        &palette,
                    )
                    .unwrap_err();
                assert!(err.contains("nonopaque"), "{err}");
                assert!(
                    !editor
                        .project
                        .as_ref()
                        .unwrap()
                        .assignments
                        .contains_key("ink")
                );
                assert!(editor.resolved(&palette).is_ok());

                // Translucent swatch accepted for alpha: true role.
                editor
                    .command(
                        &json!({"op":"theme_link","role":"tint","name":"Glass"}),
                        &palette,
                    )
                    .unwrap();
                assert_eq!(editor.resolved(&palette).unwrap()["tint"], "#ABCDEF80");

                // Unlinking a linked role whose value differs from default converts to Override.
                editor
                    .command(
                        &json!({"op":"theme_link","role":"ink","name":"Accent"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_link","role":"ink","name":""}), &palette)
                    .unwrap();
                assert!(matches!(
                    editor.project.as_ref().unwrap().assignments.get("ink"),
                    Some(Assignment::Override(_))
                ));
                palette
                    .command(
                        &json!({"op":"palette_edit","index":0,"name":"Accent","color":"#102030"}),
                        &mut mixer,
                    )
                    .unwrap();
                assert_eq!(editor.resolved(&palette).unwrap()["ink"], "#ABCDEF");

                // Unlinking an already non-linked role is a clean no-op.
                editor
                    .command(&json!({"op":"theme_save"}), &palette)
                    .unwrap();
                let undo_len = editor.undo.len();
                editor
                    .command(
                        &json!({"op":"theme_link","role":"ink","name":"   "}),
                        &palette,
                    )
                    .unwrap();
                assert!(!editor.project.as_ref().unwrap().dirty);
                assert_eq!(editor.undo.len(), undo_len);

                // Unlinking a link whose color equals the role default removes the assignment.
                editor
                    .command(
                        &json!({"op":"theme_link","role":"ink","name":"DefaultInk"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_link","role":"ink","name":""}), &palette)
                    .unwrap();
                assert!(
                    !editor
                        .project
                        .as_ref()
                        .unwrap()
                        .assignments
                        .contains_key("ink")
                );

                // Unlinking a broken link removes the assignment cleanly without error.
                editor
                    .command(
                        &json!({"op":"theme_link","role":"ink","name":"Accent"}),
                        &palette,
                    )
                    .unwrap();
                palette
                    .command(
                        &json!({"op":"palette_edit","index":0,"name":"Renamed","color":"#102030"}),
                        &mut mixer,
                    )
                    .unwrap();
                assert!(editor.resolved(&palette).is_err());
                editor
                    .command(&json!({"op":"theme_link","role":"ink","name":""}), &palette)
                    .unwrap();
                assert!(
                    !editor
                        .project
                        .as_ref()
                        .unwrap()
                        .assignments
                        .contains_key("ink")
                );
                assert_eq!(editor.resolved(&palette).unwrap()["ink"], "#445566");
            },
        );
    }

    #[test]
    fn theme_palette_bind_rejects_unsaved_palette() {
        isolated("theme_palette_bind_rejects_unsaved_palette", |root| {
            install_package(root);
            let mut repo = swatch_board_storage::PaletteRepository::open_default().unwrap();
            let saved = repo
                .save_named(
                    None,
                    "Bound",
                    &[swatch_board_storage::Rgba8 {
                        r: 1,
                        g: 2,
                        b: 3,
                        a: 255,
                    }],
                    &[false],
                    &["Primary".into()],
                )
                .unwrap();
            let mut palette = PaletteEditor::new();
            let mut mixer = crate::mixer::Mixer::default();
            palette
                .command(&json!({"op":"palette_open","id":saved.id}), &mut mixer)
                .unwrap();

            let mut editor = ThemeEditor::new();
            editor
                .command(
                    &json!({"op":"theme_choose","id":"runtime-fixture"}),
                    &palette,
                )
                .unwrap();
            editor
                .command(&json!({"op":"theme_save"}), &palette)
                .unwrap();
            assert_eq!(
                editor.project.as_ref().unwrap().palette_id.as_deref(),
                Some(saved.id.as_str())
            );

            // Switch to an unsaved palette and attempt to bind it.
            let unsaved_palette = PaletteEditor::new();
            assert!(unsaved_palette.theme_palette().0.is_none());
            let err = editor
                .command(&json!({"op":"theme_palette_bind"}), &unsaved_palette)
                .unwrap_err();
            assert!(err.contains("Save the active palette"), "{err}");
            assert_eq!(
                editor.project.as_ref().unwrap().palette_id.as_deref(),
                Some(saved.id.as_str())
            );
            assert!(!editor.project.as_ref().unwrap().dirty);
        });
    }

    #[test]
    fn preview_cache_does_not_cache_render_errors_and_syncs_with_preview() {
        isolated(
            "preview_cache_does_not_cache_render_errors_and_syncs_with_preview",
            |root| {
                install_package(root);
                let package = root.join("data/swatch-board/plugins/runtime-fixture");
                fs::write(
                    package.join("templates/example.j2"),
                    "ink={{ roles.undefined_role }}\n",
                )
                .unwrap();

                let palette = PaletteEditor::new();
                let mut editor = ThemeEditor::new();
                editor
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();

                let state = editor.snapshot(&palette, Format::HexRgba);
                assert!(state["error"].is_string());
                assert!(state["preview"].is_null());
                assert!(editor.preview_cache.borrow().is_none());

                assert!(
                    editor
                        .command(&json!({"op":"theme_preview"}), &palette)
                        .is_err()
                );
                assert!(editor.preview_cache.borrow().is_none());

                // Fix template, reload, and confirm theme_preview populates preview_cache.
                fs::write(
                    package.join("templates/example.j2"),
                    "ink={{ roles.ink }}\nsurface={{ roles.surface }}\n",
                )
                .unwrap();
                editor
                    .command(&json!({"op":"theme_reload"}), &palette)
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_preview"}), &palette)
                    .unwrap();
                assert!(editor.preview_cache.borrow().is_some());
                let ok_state = editor.snapshot(&palette, Format::HexRgba);
                assert!(ok_state["error"].is_null());
                assert!(
                    ok_state["preview"][0]["content"]
                        .as_str()
                        .unwrap()
                        .contains("ink=#445566")
                );
            },
        );
    }

    #[test]
    fn theme_reload_falls_back_removed_scene_preserves_errors_and_reloads_saved_themes() {
        isolated(
            "theme_reload_falls_back_removed_scene_preserves_errors_and_reloads_saved_themes",
            |root| {
                install_package(root);
                let palette = PaletteEditor::new();
                let mut editor = ThemeEditor::new();
                editor
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_scene","id":"drawer"}), &palette)
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_save"}), &palette)
                    .unwrap();
                assert_eq!(editor.saved.len(), 1);

                // Externally add a second saved theme to storage.
                let mut external_repo = ThemeRepository::open_default().unwrap();
                external_repo
                    .save(&SavedTheme {
                        id: String::new(),
                        name: "External Theme".into(),
                        plugin_id: "runtime-fixture".into(),
                        plugin_version: "1.0".into(),
                        palette_id: None,
                        scene: "home".into(),
                        payload: serde_json::to_string(editor.project.as_ref().unwrap()).unwrap(),
                        dirty: false,
                    })
                    .unwrap();

                // Remove the active "drawer" scene from the plugin on disk.
                let package = root.join("data/swatch-board/plugins/runtime-fixture");
                fs::write(
                    package.join("scenes.json"),
                    serde_json::to_vec_pretty(&json!([
                        {"id":"home", "title":"Home", "width":400, "height":200, "nodes":[
                            {"id":"background", "kind":"window", "rect":[0,0,400,200], "role":"surface"}
                        ]}
                    ]))
                    .unwrap(),
                )
                .unwrap();

                editor
                    .command(&json!({"op":"theme_reload"}), &palette)
                    .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().scene, "home");
                let state = editor.snapshot(&palette, Format::HexRgba);
                assert!(!state["scene"].is_null());
                assert_eq!(state["saved"].as_array().unwrap().len(), 2);

                // Remove active plugin AND add an unrelated broken plugin; both errors must appear.
                fs::remove_dir_all(&package).unwrap();
                let broken = root.join("data/swatch-board/plugins/broken-plugin");
                fs::create_dir_all(&broken).unwrap();
                fs::write(broken.join("manifest.json"), "{invalid json").unwrap();

                let err = editor
                    .command(&json!({"op":"theme_reload"}), &palette)
                    .unwrap_err();
                assert!(err.contains("runtime-fixture"), "{err}");
                assert!(err.contains("broken-plugin"), "{err}");
            },
        );
    }

    #[test]
    fn theme_default_noop_and_save_error_messages() {
        isolated("theme_default_noop_and_save_error_messages", |root| {
            install_package(root);
            let palette = PaletteEditor::new();
            let mut editor = ThemeEditor::new();

            // Clicking Save with no open project must not blame storage.
            let err = editor
                .command(&json!({"op":"theme_save"}), &palette)
                .unwrap_err();
            assert_eq!(err, "Select a plugin first");

            editor
                .command(
                    &json!({"op":"theme_choose","id":"runtime-fixture"}),
                    &palette,
                )
                .unwrap();
            editor
                .command(&json!({"op":"theme_save"}), &palette)
                .unwrap();
            let saved_id = editor.project.as_ref().unwrap().id.clone();
            editor
                .command(&json!({"op":"theme_open","id":saved_id}), &palette)
                .unwrap();
            assert!(!editor.project.as_ref().unwrap().dirty);
            assert!(editor.undo.is_empty());

            // Resetting an already-default role is a true no-op.
            editor
                .command(&json!({"op":"theme_default","role":"ink"}), &palette)
                .unwrap();
            assert!(!editor.project.as_ref().unwrap().dirty);
            assert!(editor.undo.is_empty());
            assert!(editor.project.as_ref().unwrap().assignments.is_empty());

            // Overriding then resetting removes the key instead of storing Assignment::Default.
            editor
                .command(
                    &json!({"op":"theme_assign","role":"ink","color":"#abcdef"}),
                    &palette,
                )
                .unwrap();
            assert!(editor.project.as_ref().unwrap().dirty);
            editor
                .command(&json!({"op":"theme_default","role":"ink"}), &palette)
                .unwrap();
            assert!(editor.project.as_ref().unwrap().assignments.is_empty());
            assert!(!editor.project.as_ref().unwrap().dirty);

            // Missing storage still reports "Theme storage unavailable".
            editor.repo = None;
            let storage_err = editor
                .command(&json!({"op":"theme_save"}), &palette)
                .unwrap_err();
            assert_eq!(storage_err, "Theme storage unavailable");
        });
    }

    #[test]
    fn draft_restore_and_reload_reconcile_saved_state_and_dirty() {
        isolated(
            "draft_restore_and_reload_reconcile_saved_state_and_dirty",
            |root| {
                install_package(root);
                let palette = PaletteEditor::new();
                let mut editor = ThemeEditor::new();
                editor
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_save"}), &palette)
                    .unwrap();
                let saved_id = editor.project.as_ref().unwrap().id.clone();
                assert!(!editor.project.as_ref().unwrap().dirty);

                // Simulate deleted DB row + restart: keep the clean draft (dirty: false)
                // while removing the corresponding theme_projects row.
                let clean_draft = ThemeRepository::open_default()
                    .unwrap()
                    .load_draft()
                    .unwrap()
                    .unwrap();
                assert!(!serde_json::from_str::<Project>(&clean_draft).unwrap().dirty);
                drop(editor);
                let db_dir = root.join("data/swatch-board");
                for name in [
                    "library.sqlite3",
                    "library.sqlite3-wal",
                    "library.sqlite3-shm",
                ] {
                    let path = db_dir.join(name);
                    if path.exists() {
                        fs::remove_file(path).unwrap();
                    }
                }
                ThemeRepository::open_default()
                    .unwrap()
                    .save_draft(&clean_draft)
                    .unwrap();

                let mut restored = ThemeEditor::new();
                assert_eq!(restored.project.as_ref().unwrap().id, saved_id);
                assert!(restored.saved_state.is_none());
                assert!(restored.project.as_ref().unwrap().dirty);
                let persisted_draft: Project = serde_json::from_str(
                    &ThemeRepository::open_default()
                        .unwrap()
                        .load_draft()
                        .unwrap()
                        .unwrap(),
                )
                .unwrap();
                assert!(persisted_draft.dirty);
                assert!(
                    restored
                        .command(
                            &json!({"op":"theme_choose","id":"runtime-fixture"}),
                            &palette,
                        )
                        .is_err()
                );

                // Re-save to establish a clean saved record, then externally modify the DB row.
                restored
                    .command(&json!({"op":"theme_save"}), &palette)
                    .unwrap();
                let saved_id = restored.project.as_ref().unwrap().id.clone();
                assert!(!restored.project.as_ref().unwrap().dirty);

                let mut external_repo = ThemeRepository::open_default().unwrap();
                let mut modified_project = restored.project.as_ref().unwrap().clone();
                modified_project.name = "Externally Renamed".into();
                modified_project.dirty = false;
                external_repo
                    .save(&SavedTheme {
                        id: saved_id.clone(),
                        name: modified_project.name.clone(),
                        plugin_id: modified_project.plugin_id.clone(),
                        plugin_version: modified_project.plugin_version.clone(),
                        palette_id: modified_project.palette_id.clone(),
                        scene: modified_project.scene.clone(),
                        payload: serde_json::to_string(&modified_project).unwrap(),
                        dirty: false,
                    })
                    .unwrap();

                restored
                    .command(&json!({"op":"theme_reload"}), &palette)
                    .unwrap();
                assert_eq!(
                    restored.saved_state.as_ref().unwrap().name,
                    "Externally Renamed"
                );
                assert!(restored.project.as_ref().unwrap().dirty);
                assert!(
                    restored
                        .command(
                            &json!({"op":"theme_choose","id":"runtime-fixture"}),
                            &palette,
                        )
                        .is_err()
                );

                // Externally restore DB row to match the open project -> dirty becomes false.
                let mut identical_project = restored.project.as_ref().unwrap().clone();
                identical_project.dirty = false;
                external_repo
                    .save(&SavedTheme {
                        id: saved_id,
                        name: identical_project.name.clone(),
                        plugin_id: identical_project.plugin_id.clone(),
                        plugin_version: identical_project.plugin_version.clone(),
                        palette_id: identical_project.palette_id.clone(),
                        scene: identical_project.scene.clone(),
                        payload: serde_json::to_string(&identical_project).unwrap(),
                        dirty: false,
                    })
                    .unwrap();

                restored
                    .command(&json!({"op":"theme_reload"}), &palette)
                    .unwrap();
                assert!(!restored.project.as_ref().unwrap().dirty);
                restored
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();
            },
        );
    }

    #[test]
    fn scene_fallback_updates_dirty_across_reload_open_and_restart() {
        isolated(
            "scene_fallback_updates_dirty_across_reload_open_and_restart",
            |root| {
                install_package(root);
                let palette = PaletteEditor::new();
                let mut editor = ThemeEditor::new();
                editor
                    .command(
                        &json!({"op":"theme_choose","id":"runtime-fixture"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_scene","id":"drawer"}), &palette)
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_save"}), &palette)
                    .unwrap();
                let saved_id = editor.project.as_ref().unwrap().id.clone();
                assert!(!editor.project.as_ref().unwrap().dirty);
                let clean_drawer_draft = ThemeRepository::open_default()
                    .unwrap()
                    .load_draft()
                    .unwrap()
                    .unwrap();

                // Remove the saved "drawer" scene from scenes.json on disk.
                let package = root.join("data/swatch-board/plugins/runtime-fixture");
                fs::write(
                    package.join("scenes.json"),
                    serde_json::to_vec_pretty(&json!([
                        {"id":"home", "title":"Home", "width":400, "height":200, "nodes":[
                            {"id":"background", "kind":"window", "rect":[0,0,400,200], "role":"surface"}
                        ]}
                    ]))
                    .unwrap(),
                )
                .unwrap();

                // 1. theme_reload fallback marks project dirty and stays consistent across edit + undo.
                editor
                    .command(&json!({"op":"theme_reload"}), &palette)
                    .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().scene, "home");
                assert!(editor.project.as_ref().unwrap().dirty);
                assert!(
                    editor
                        .command(
                            &json!({"op":"theme_choose","id":"runtime-fixture"}),
                            &palette,
                        )
                        .is_err()
                );
                editor
                    .command(
                        &json!({"op":"theme_assign","role":"ink","color":"#abcdef"}),
                        &palette,
                    )
                    .unwrap();
                assert!(editor.project.as_ref().unwrap().dirty);
                editor
                    .command(&json!({"op":"theme_undo"}), &palette)
                    .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().scene, "home");
                assert!(editor.project.as_ref().unwrap().dirty);

                // 2. theme_open fallback marks project dirty and stays consistent across edit + undo.
                editor
                    .command(
                        &json!({"op":"theme_open","id":saved_id,"discard":true}),
                        &palette,
                    )
                    .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().scene, "home");
                assert!(editor.project.as_ref().unwrap().dirty);
                assert!(
                    editor
                        .command(
                            &json!({"op":"theme_choose","id":"runtime-fixture"}),
                            &palette,
                        )
                        .is_err()
                );
                editor
                    .command(
                        &json!({"op":"theme_assign","role":"ink","color":"#abcdef"}),
                        &palette,
                    )
                    .unwrap();
                editor
                    .command(&json!({"op":"theme_undo"}), &palette)
                    .unwrap();
                assert_eq!(editor.project.as_ref().unwrap().scene, "home");
                assert!(editor.project.as_ref().unwrap().dirty);

                // 3. ThemeEditor::new() draft restore fallback marks project dirty and stays consistent across edit + undo.
                ThemeRepository::open_default()
                    .unwrap()
                    .save_draft(&clean_drawer_draft)
                    .unwrap();
                let mut restarted = ThemeEditor::new();
                assert_eq!(restarted.project.as_ref().unwrap().scene, "home");
                assert!(restarted.project.as_ref().unwrap().dirty);
                assert!(
                    restarted
                        .command(
                            &json!({"op":"theme_choose","id":"runtime-fixture"}),
                            &palette,
                        )
                        .is_err()
                );
                restarted
                    .command(
                        &json!({"op":"theme_assign","role":"ink","color":"#abcdef"}),
                        &palette,
                    )
                    .unwrap();
                restarted
                    .command(&json!({"op":"theme_undo"}), &palette)
                    .unwrap();
                assert_eq!(restarted.project.as_ref().unwrap().scene, "home");
                assert!(restarted.project.as_ref().unwrap().dirty);
            },
        );
    }
}
