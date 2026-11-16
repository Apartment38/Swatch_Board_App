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

pub struct ThemeEditor {
    plugins: Vec<PluginMetadata>,
    plugin: Option<Plugin>,
    project: Option<Project>,
    repo: Option<ThemeRepository>,
    saved: Vec<SavedTheme>,
    undo: Vec<Project>,
    redo: Vec<Project>,
    selected: Option<String>,
    preview: Vec<RenderedFile>,
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
            repo,
            saved: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            selected: None,
            preview: Vec::new(),
            error,
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
            self.changed(previous);
        }
    }
    pub fn snapshot(&self, palette: &PaletteEditor, format: Format) -> Value {
        let resolved = self.project.as_ref().map(|_| self.resolved(palette));
        let values = resolved.as_ref().and_then(|r| r.as_ref().ok());
        let (palette_id, palette_name, swatches) = palette.theme_palette();
        let mut scene_error = None;
        let scene = self.plugin.as_ref().and_then(|plugin| {
            let project = self.project.as_ref()?;
            let scene = plugin.scenes.iter().find(|s| s.id == project.scene)?;
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
            .or_else(|| resolved.as_ref().and_then(|r| r.as_ref().err()));
        json!({
            "plugins": self.plugins.iter().map(|p| json!({"id":p.manifest.id,"name":p.manifest.name})).collect::<Vec<_>>(),
            "saved": self.saved.iter().map(|p| json!({"id":p.id,"name":p.name})).collect::<Vec<_>>(),
            "project": self.project.as_ref().map(|p| json!({"name":p.name,"dirty":p.dirty,"plugin_id":p.plugin_id,"scene":p.scene,"palette_id":p.palette_id})),
            "scenes": self.plugin.as_ref().map(|p| p.scenes.iter().map(|s| json!({"id":s.id,"name":s.title})).collect::<Vec<_>>()).unwrap_or_default(),
            "roles": roles, "palette_swatches": swatches.iter().map(|(name,color)| json!({"name":name,"color":color.format(Format::HexRgba)})).collect::<Vec<_>>(),
            "palette_id": palette_id, "palette_name": palette_name,
            "selected": self.selected, "preview": self.preview, "scene": scene,
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
            "theme_reload" => self.reload(),
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
                    _ => project.palette_id = palette.theme_palette().0.map(str::to_owned),
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
                        if !swatches.iter().any(|(n, _)| n == name)
                            || (!role.links.is_empty() && !role.links.iter().any(|n| n == name))
                        {
                            return Err("Unknown or unavailable palette swatch".into());
                        }
                        self.assign(id, Assignment::Link(name.to_owned()));
                    }
                }
            }
            "theme_undo" | "theme_redo" => {
                let source = if op == "theme_undo" {
                    &mut self.undo
                } else {
                    &mut self.redo
                };
                let next = source.pop().ok_or("Nothing to undo or redo")?;
                let project = self.project.as_mut().ok_or("Select a plugin first")?;
                let old = std::mem::replace(project, next);
                if op == "theme_undo" {
                    self.redo.push(old);
                } else {
                    self.undo.push(old);
                }
                self.preview.clear();
                self.persist();
            }
            "theme_preview" => {
                let values = self.resolved(palette)?;
                self.preview = self
                    .plugin
                    .as_ref()
                    .ok_or("The selected plugin is unavailable")?
                    .render(&values)?;
            }
            "theme_export" => {
                let directory = field("directory")?;
                if directory.trim().is_empty() || self.preview.is_empty() {
                    return Err("Preview files and select an export directory first".into());
                }
                let current = self.resolved(palette).and_then(|v| {
                    self.plugin
                        .as_ref()
                        .ok_or_else(|| "The selected plugin is unavailable".to_owned())?
                        .render(&v)
                });
                match current {
                    Ok(files)
                        if files.len() == self.preview.len()
                            && files
                                .iter()
                                .zip(&self.preview)
                                .all(|(a, b)| a.path == b.path && a.content == b.content) =>
                    {
                        core::export(
                            &PathBuf::from(directory),
                            &self.preview,
                            request.get("overwrite").and_then(Value::as_bool) == Some(true),
                        )?;
                    }
                    Ok(_) => {
                        self.preview.clear();
                        return Err("Palette changed; preview export again.".into());
                    }
                    Err(e) => {
                        self.preview.clear();
                        return Err(e);
                    }
                }
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
    fn named_links_follow_palette_and_stale_preview_is_rejected() {
        isolated(
            "named_links_follow_palette_and_stale_preview_is_rejected",
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
                let output = root.join("export");
                fs::create_dir(&output).unwrap();
                assert!(editor.command(&json!({"op":"theme_export","directory":output.to_str().unwrap(),"overwrite":false}), &palette).is_err());
                assert!(editor.preview.is_empty());
                assert!(!output.join("example.txt").exists());
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
}
