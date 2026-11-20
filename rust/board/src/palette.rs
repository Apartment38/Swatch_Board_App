use crate::mixer::Mixer;
use serde_json::{Value, json};
use swatch_board_color::{
    Color, Format,
    generation::{GenerationMode, GenerationOptions, generate},
};
use swatch_board_storage::{GenerationRecord, PaletteRepository, Rgba8, SavedPalette};

type GeneratedColors = (Vec<Color>, Vec<bool>, Vec<String>);

#[derive(Clone, PartialEq)]
struct Draft {
    name: String,
    colors: Vec<Color>,
    locked: Vec<bool>,
    names: Vec<String>,
    id: Option<String>,
    lineage: u64,
}

impl Draft {
    fn same_content(&self, other: &Self) -> bool {
        self.name == other.name
            && self.colors == other.colors
            && self.locked == other.locked
            && self.names == other.names
            && self.id == other.id
    }
}

pub struct PaletteEditor {
    draft: Draft,
    next_lineage: u64,
    undo: Vec<Draft>,
    redo: Vec<Draft>,
    repository: Option<PaletteRepository>,
    saved: Vec<SavedPalette>,
    generations: Vec<GenerationRecord>,
    error: Option<String>,
    lock_history: bool,
    status: Option<String>,
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
            Ok(r) => (Some(r), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let mut p = Self {
            draft: Draft {
                name: "Untitled".into(),
                colors: vec![],
                locked: vec![],
                names: vec![],
                id: None,
                lineage: 0,
            },
            next_lineage: 1,
            undo: vec![],
            redo: vec![],
            repository,
            saved: vec![],
            generations: vec![],
            error,
            lock_history: false,
            status: None,
        };
        p.refresh();
        p
    }
    fn next_lineage(&mut self) -> u64 {
        let lineage = self.next_lineage;
        self.next_lineage += 1;
        lineage
    }
    fn refresh(&mut self) {
        if let Some(repo) = &self.repository {
            match (repo.list(), repo.generations()) {
                (Ok(s), Ok(g)) => {
                    self.saved = s;
                    self.generations = g;
                }
                (Err(e), _) | (_, Err(e)) => self.error = Some(e.to_string()),
            }
        }
    }
    fn fresh_name(names: &[String]) -> String {
        (1..)
            .map(|n| format!("Swatch {n}"))
            .find(|n| !names.contains(n))
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
    fn commit(&mut self, previous: Draft, mixer: &mut Mixer) {
        if !self.draft.same_content(&previous) {
            if self.undo.len() == 100 {
                self.undo.remove(0);
            }
            self.undo.push(previous);
            self.redo.clear();
            mixer.replace_palette(self.draft.colors.clone());
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
        }
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
        self.error = None;
    }
    fn open_palette(&mut self, palette: &SavedPalette, mixer: &mut Mixer, saved: bool) {
        let name = if palette.name.trim().is_empty() {
            let existing = self.draft.name.trim();
            if !existing.is_empty() && existing.chars().count() <= 256 {
                self.draft.name.clone()
            } else {
                "Untitled".into()
            }
        } else {
            palette.name.clone()
        };
        let colors: Vec<_> = palette.colors.iter().copied().map(from_rgba).collect();
        let mut locked = palette.locked.clone();
        locked.resize(colors.len(), false);
        let mut names = palette.swatch_names.clone();
        names.truncate(colors.len());
        while names.len() < colors.len() {
            names.push(Self::fresh_name(&names));
        }
        let mut candidate = Draft {
            name,
            colors,
            locked,
            names,
            id: saved.then(|| palette.id.clone()),
            lineage: self.draft.lineage,
        };
        if candidate.same_content(&self.draft) {
            return;
        }
        let previous = self.draft.clone();
        candidate.lineage = self.next_lineage();
        self.draft = candidate;
        self.commit(previous, mixer);
    }
    pub fn snapshot(&self) -> Value {
        json!({
            "name": self.draft.name,
            "id": self.draft.id,
            "swatches": self.draft.colors.iter().enumerate().map(|(i, c)| {
                let fallback = format!("Swatch {}", i + 1);
                let name = self.draft.names.get(i).map(String::as_str).unwrap_or(&fallback);
                let locked = self.draft.locked.get(i).copied().unwrap_or(false);
                json!({
                    "color": c.format(Format::HexRgba),
                    "name": name,
                    "locked": locked,
                })
            }).collect::<Vec<_>>(),
            "saved": self.saved.iter().map(|p| json!({"id": p.id, "name": p.name})).collect::<Vec<_>>(),
            "generations": self.generations.iter().map(|g| json!({"id": g.id, "label": format!("{} · {} colors", g.description, g.palette.colors.len())})).collect::<Vec<_>>(),
            "can_undo": !self.undo.is_empty(),
            "can_redo": !self.redo.is_empty(),
            "lock_history": self.lock_history,
            "error": self.error,
            "status": self.status,
        })
    }
    pub fn command(&mut self, r: &Value, mixer: &mut Mixer) -> Result<(), String> {
        self.error = None;
        self.status = None;
        let result = self.execute(r, mixer);
        if let Err(e) = &result {
            self.error = Some(e.clone());
        }
        result
    }
    fn execute(&mut self, r: &Value, mixer: &mut Mixer) -> Result<(), String> {
        let op = string(r, "op")?;
        match op {
            "palette_add_current" => {
                if self.draft.colors.len() >= 64 {
                    return Err("A palette can contain at most 64 colors.".into());
                }
                let p = self.draft.clone();
                self.draft.colors.push(mixer.current());
                self.draft.names.push(Self::fresh_name(&self.draft.names));
                self.draft.locked.push(false);
                self.commit(p, mixer);
            }
            "palette_lock_history" => self.lock_history = boolean(r, "locked")?,
            "palette_lock" => {
                let i = index(r, "index", self.draft.colors.len())?;
                let locked = boolean(r, "locked")?;
                let p = self.draft.clone();
                self.draft.locked[i] = locked;
                self.commit(p, mixer);
                self.status = Some(format!(
                    "Swatch {} {}.",
                    i + 1,
                    if locked { "locked" } else { "unlocked" }
                ));
            }
            "palette_edit" => {
                let i = index(r, "index", self.draft.colors.len())?;
                let color = parse_color(string(r, "color")?)?;
                let name = string(r, "name")?;
                if name.trim().is_empty()
                    || self
                        .draft
                        .names
                        .iter()
                        .enumerate()
                        .any(|(j, n)| j != i && n == name)
                {
                    return Err("Swatch names must be nonempty and unique.".into());
                }
                if self.draft.locked[i] && color != self.draft.colors[i] {
                    return Err(format!(
                        "Swatch {} is locked. Unlock it before editing.",
                        i + 1
                    ));
                }
                let p = self.draft.clone();
                self.draft.colors[i] = color;
                self.draft.names[i] = name.to_owned();
                self.commit(p, mixer);
                self.status = Some(format!("Swatch {} updated.", i + 1));
            }
            "palette_delete_swatch" => {
                let i = index(r, "index", self.draft.colors.len())?;
                if self.draft.locked[i] {
                    return Err(format!(
                        "Swatch {} is locked. Unlock it before deleting.",
                        i + 1
                    ));
                }
                let p = self.draft.clone();
                self.draft.colors.remove(i);
                self.draft.locked.remove(i);
                let name = self.draft.names.remove(i);
                self.commit(p, mixer);
                self.status = Some(format!("{name} deleted."));
            }
            "palette_move" => {
                let from = index(r, "from", self.draft.colors.len())?;
                let to = index(r, "to", self.draft.colors.len())?;
                let p = self.draft.clone();
                let c = self.draft.colors.remove(from);
                let n = self.draft.names.remove(from);
                let l = self.draft.locked.remove(from);
                self.draft.colors.insert(to, c);
                self.draft.names.insert(to, n);
                self.draft.locked.insert(to, l);
                self.commit(p, mixer);
                self.status = Some(format!("Swatch moved to position {}.", to + 1));
            }
            "palette_undo" => {
                if let Some(d) = self.undo.pop() {
                    self.redo.push(std::mem::replace(&mut self.draft, d));
                    mixer.replace_palette(self.draft.colors.clone());
                }
            }
            "palette_redo" => {
                if let Some(d) = self.redo.pop() {
                    self.undo.push(std::mem::replace(&mut self.draft, d));
                    mixer.replace_palette(self.draft.colors.clone());
                }
            }
            "palette_name" => {
                let name = string(r, "name")?;
                let p = self.draft.clone();
                self.draft.name = name.to_owned();
                self.commit(p, mixer);
            }
            "palette_new" => {
                let p = self.draft.clone();
                self.draft = Draft {
                    name: "Untitled".into(),
                    colors: vec![],
                    locked: vec![],
                    names: vec![],
                    id: None,
                    lineage: self.next_lineage(),
                };
                self.commit(p, mixer);
            }
            "palette_save" => {
                let repo = self
                    .repository
                    .as_mut()
                    .ok_or("Palette storage unavailable.")?;
                let colors: Vec<_> = self.draft.colors.iter().copied().map(to_rgba).collect();
                let saved = repo
                    .save_named(
                        self.draft.id.as_deref(),
                        &self.draft.name,
                        &colors,
                        &self.draft.locked,
                        &self.draft.names,
                    )
                    .map_err(|e| e.to_string())?;
                let lineage = self.draft.lineage;
                self.draft.id = Some(saved.id.clone());
                self.draft.name = saved.name;
                for snapshot in self.undo.iter_mut().chain(self.redo.iter_mut()) {
                    if snapshot.lineage == lineage && snapshot.id.is_none() {
                        snapshot.id = Some(saved.id.clone());
                    }
                }
                self.refresh();
            }
            "palette_duplicate" => {
                let p = self.draft.clone();
                self.draft.id = None;
                self.draft.name.push_str(" copy");
                self.draft.lineage = self.next_lineage();
                self.commit(p, mixer);
            }
            "palette_open" => {
                let id = string(r, "id")?;
                let p = self
                    .saved
                    .iter()
                    .find(|p| p.id == id)
                    .cloned()
                    .ok_or("Palette not found")?;
                self.open_palette(&p, mixer, true);
            }
            "palette_delete" => {
                let id = string(r, "id")?;
                self.repository
                    .as_mut()
                    .ok_or("Palette storage unavailable.")?
                    .delete(id)
                    .map_err(|e| e.to_string())?;
                if self.draft.id.as_deref() == Some(id) {
                    self.draft.id = None;
                }
                for snapshot in self.undo.iter_mut().chain(self.redo.iter_mut()) {
                    if snapshot.id.as_deref() == Some(id) {
                        snapshot.id = None;
                    }
                }
                self.refresh();
            }
            "palette_generation" => {
                let id = string(r, "id")?;
                let p = self
                    .generations
                    .iter()
                    .find(|g| g.id == id)
                    .map(|g| g.palette.clone())
                    .ok_or("Generation not found")?;
                self.open_palette(&p, mixer, false);
            }
            "palette_generate" => {
                let options = options(r)?;
                self.generate(options, mixer)?;
            }
            _ => return Err(format!("Unknown palette operation: {op}")),
        }
        Ok(())
    }
    pub fn preview_generation(&self, request: &Value) -> Result<Vec<String>, String> {
        let options = options(request)?;
        let (colors, _, _) = self.generated_colors(&options)?;
        Ok(colors.iter().map(|c| c.format(Format::HexRgba)).collect())
    }

    fn generated_colors(&self, options: &GenerationOptions) -> Result<GeneratedColors, String> {
        let locked_count = self
            .draft
            .colors
            .iter()
            .zip(&self.draft.locked)
            .filter(|(_, locked)| **locked)
            .count();
        if locked_count > options.count {
            return Err("Palette size is smaller than the number of locked swatches.".into());
        }
        let mut colors = if locked_count == options.count {
            vec![Color::new(0, 0, 0, 255); options.count]
        } else {
            generate(options)?
        };
        let mut locks = vec![false; options.count];
        let mut names: Vec<_> = self
            .draft
            .names
            .iter()
            .take(options.count)
            .cloned()
            .collect();
        while names.len() < options.count {
            names.push(Self::fresh_name(&names));
        }
        for i in 0..self.draft.colors.len().min(options.count) {
            if self.draft.locked.get(i).copied().unwrap_or(false) {
                colors[i] = self.draft.colors[i];
                locks[i] = true;
            }
        }
        for i in options.count..self.draft.colors.len() {
            if self.draft.locked.get(i).copied().unwrap_or(false) {
                let j = locks
                    .iter()
                    .position(|x| !x)
                    .ok_or("Too many locked swatches")?;
                colors[j] = self.draft.colors[i];
                locks[j] = true;
                if let Some(name) = self.draft.names.get(i) {
                    names[j] = name.clone();
                }
            }
        }
        Ok((colors, locks, names))
    }

    fn generate(&mut self, options: GenerationOptions, mixer: &mut Mixer) -> Result<(), String> {
        let (colors, locks, names) = self.generated_colors(&options)?;
        if colors == self.draft.colors && locks == self.draft.locked && names == self.draft.names {
            return Ok(());
        }
        let values: Vec<_> = colors.iter().copied().map(to_rgba).collect();
        self.repository
            .as_mut()
            .ok_or("Storage unavailable; generation was not recorded.")?
            .record_generation(
                &format!("{:?} · seed {}", options.mode, options.seed),
                &values,
                &locks,
            )
            .map_err(|e| e.to_string())?;
        let previous = self.draft.clone();
        self.draft.colors = colors;
        self.draft.names = names;
        self.draft.locked = locks;
        self.commit(previous, mixer);
        self.refresh();
        Ok(())
    }
}
fn string<'a>(r: &'a Value, key: &str) -> Result<&'a str, String> {
    r.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Invalid {key}"))
}
fn boolean(r: &Value, key: &str) -> Result<bool, String> {
    r.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("Invalid {key}"))
}
fn index(r: &Value, key: &str, len: usize) -> Result<usize, String> {
    r.get(key)
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .filter(|&n| n < len)
        .ok_or_else(|| format!("Invalid {key}"))
}
fn parse_color(s: &str) -> Result<Color, String> {
    s.parse()
        .map_err(|_| "Invalid color; swatch unchanged.".into())
}
fn number(r: &Value, key: &str) -> Result<f64, String> {
    r.get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("Invalid {key}"))
}
fn options(r: &Value) -> Result<GenerationOptions, String> {
    let mode = match string(r, "mode")? {
        "complementary" => GenerationMode::Complementary,
        "split-complementary" => GenerationMode::SplitComplementary,
        "analogous" => GenerationMode::Analogous,
        "triadic" => GenerationMode::Triadic,
        "tetradic" => GenerationMode::Tetradic,
        "monochromatic" => GenerationMode::Monochromatic,
        "perceptual-contrast" => GenerationMode::PerceptualContrast,
        "random" => GenerationMode::Random,
        _ => return Err("Invalid generation mode".into()),
    };
    let count = r
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (1..=64).contains(n))
        .ok_or("Invalid count")?;
    let seed = match r.get("seed") {
        Some(Value::String(value)) => value.parse::<u64>().map_err(|_| "Invalid seed")?,
        Some(value) => value.as_u64().ok_or("Invalid seed")?,
        None => return Err("Invalid seed".into()),
    };
    let base = match r.get("base") {
        Some(Value::Null) => None,
        Some(Value::String(s)) => Some(parse_color(s)?),
        _ => return Err("Invalid base".into()),
    };
    let contrast = match r.get("contrast") {
        Some(Value::Null) => None,
        Some(v) => Some(
            v.as_f64()
                .filter(|n| n.is_finite())
                .ok_or("Invalid contrast")?,
        ),
        _ => return Err("Invalid contrast".into()),
    };
    Ok(GenerationOptions {
        mode,
        count,
        seed,
        base,
        hue: [number(r, "hue_min")?, number(r, "hue_max")?],
        lightness: [number(r, "lightness_min")?, number(r, "lightness_max")?],
        chroma: [number(r, "chroma_min")?, number(r, "chroma_max")?],
        minimum_contrast: contrast,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDb {
        root: PathBuf,
    }

    impl TempDb {
        fn new() -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "swatch-board-palette-test-{}-{id}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn editor(&self) -> PaletteEditor {
            let repo = PaletteRepository::open(self.root.join("library.sqlite3")).unwrap();
            let mut p = PaletteEditor {
                draft: Draft {
                    name: "Untitled".into(),
                    colors: vec![],
                    locked: vec![],
                    names: vec![],
                    id: None,
                    lineage: 0,
                },
                next_lineage: 1,
                undo: vec![],
                redo: vec![],
                repository: Some(repo),
                saved: vec![],
                generations: vec![],
                error: None,
                lock_history: false,
                status: None,
            };
            p.refresh();
            p
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn editor() -> PaletteEditor {
        PaletteEditor {
            draft: Draft {
                name: "Test".into(),
                colors: vec![],
                locked: vec![],
                names: vec![],
                id: None,
                lineage: 0,
            },
            next_lineage: 1,
            undo: vec![],
            redo: vec![],
            repository: None,
            saved: vec![],
            generations: vec![],
            error: None,
            lock_history: false,
            status: None,
        }
    }
    #[test]
    fn generation_preview_preserves_locks_and_draft() {
        let mut p = editor();
        let mut m = Mixer::default();
        p.add_history(Color::new(1, 2, 3, 255), &mut m);
        p.command(
            &json!({"op":"palette_lock","index":0,"locked":true}),
            &mut m,
        )
        .unwrap();
        let before = p.draft.clone();
        let request = json!({"mode":"triadic", "count":3, "seed":"9", "base":"#3388CCFF",
            "hue_min":0.0, "hue_max":360.0, "lightness_min":0.0, "lightness_max":1.0,
            "chroma_min":0.0, "chroma_max":0.4, "contrast":null});
        let preview = p.preview_generation(&request).unwrap();
        let (generated, _, _) = p.generated_colors(&options(&request).unwrap()).unwrap();
        assert_eq!(
            preview,
            generated
                .iter()
                .map(|c| c.format(Format::HexRgba))
                .collect::<Vec<_>>()
        );
        assert_eq!(preview[0], "#010203FF");
        assert_eq!(p.draft.colors, before.colors);
        assert_eq!(p.draft.locked, before.locked);
        assert_eq!(p.draft.names, before.names);
    }
    #[test]
    fn locks_reorder_and_undo() {
        let mut p = editor();
        let mut m = Mixer::default();
        p.add_history(Color::new(1, 2, 3, 4), &mut m);
        p.add_history(Color::new(5, 6, 7, 8), &mut m);
        p.command(
            &json!({"op":"palette_lock","index":0,"locked":true}),
            &mut m,
        )
        .unwrap();
        assert!(
            p.command(&json!({"op":"palette_delete_swatch","index":0}), &mut m)
                .is_err()
        );
        p.command(&json!({"op":"palette_move","from":0,"to":1}), &mut m)
            .unwrap();
        assert!(p.draft.locked[1]);
        assert_eq!(p.draft.names[1], "Swatch 1");
        p.command(&json!({"op":"palette_undo"}), &mut m).unwrap();
        assert_eq!(m.colors()[0], Color::new(1, 2, 3, 4));
        assert!(p.draft.locked[0]);
        assert!(
            p.command(&json!({"op":"palette_move","from":100,"to":0}), &mut m)
                .is_err()
        );
    }

    #[test]
    fn palette_edits_do_not_wipe_mixer_undo_or_clobber_palette_on_mixer_undo() {
        let mut p = editor();
        let mut m = Mixer::default();
        let red = Color::new(255, 0, 0, 255);
        let green = Color::new(0, 255, 0, 255);
        m.select(red);
        m.select(green);
        m.command(&json!({"op":"mixer_undo"})).unwrap();
        assert_eq!(m.current(), red);
        assert_eq!(m.snapshot(Format::HexRgba)["can_undo"], json!(true));
        assert_eq!(m.snapshot(Format::HexRgba)["can_redo"], json!(true));

        p.command(&json!({"op":"palette_add_current"}), &mut m)
            .unwrap();
        p.add_history(Color::new(0, 0, 255, 255), &mut m);
        p.command(&json!({"op":"palette_undo"}), &mut m).unwrap();
        p.command(&json!({"op":"palette_redo"}), &mut m).unwrap();

        assert_eq!(m.snapshot(Format::HexRgba)["can_undo"], json!(true));
        assert_eq!(m.snapshot(Format::HexRgba)["can_redo"], json!(true));

        m.command(&json!({"op":"mixer_redo"})).unwrap();
        p.sync_from_mixer(&m);
        assert_eq!(m.current(), green);
        assert_eq!(p.draft.colors, vec![red, Color::new(0, 0, 255, 255)]);
    }

    #[test]
    fn loading_generation_keeps_valid_draft_name_and_allows_save() {
        let db = TempDb::new();
        let mut p = db.editor();
        let mut m = Mixer::default();

        let request = json!({
            "op": "palette_generate",
            "mode": "triadic",
            "count": 3,
            "seed": "7",
            "base": "#3388CCFF",
            "hue_min": 0.0,
            "hue_max": 360.0,
            "lightness_min": 0.0,
            "lightness_max": 1.0,
            "chroma_min": 0.0,
            "chroma_max": 0.4,
            "contrast": null
        });
        p.command(&request, &mut m).unwrap();
        let gen_id = p.generations[0].id.clone();

        // With an existing valid draft name, loading a generation preserves it.
        p.command(&json!({"op":"palette_name","name":"Ocean"}), &mut m)
            .unwrap();
        p.command(&json!({"op":"palette_generation","id":gen_id}), &mut m)
            .unwrap();
        assert_eq!(p.draft.name, "Ocean");
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 1);
        assert_eq!(p.saved[0].name, "Ocean");

        // With an empty draft name, loading a generation falls back to "Untitled".
        p.command(&json!({"op":"palette_name","name":"   "}), &mut m)
            .unwrap();
        p.command(&json!({"op":"palette_generation","id":gen_id}), &mut m)
            .unwrap();
        assert_eq!(p.draft.name, "Untitled");
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 2);
        assert_eq!(p.saved[0].name, "Untitled");
    }

    #[test]
    fn palette_delete_clears_id_from_undo_and_redo() {
        let db = TempDb::new();
        let mut p = db.editor();
        let mut m = Mixer::default();

        p.add_history(Color::new(10, 20, 30, 255), &mut m);
        p.command(&json!({"op":"palette_name","name":"One"}), &mut m)
            .unwrap();
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        let saved_id = p.draft.id.clone().unwrap();

        p.add_history(Color::new(40, 50, 60, 255), &mut m);
        p.add_history(Color::new(70, 80, 90, 255), &mut m);
        // Undo once so both undo and redo contain snapshots referencing saved_id.
        p.command(&json!({"op":"palette_undo"}), &mut m).unwrap();
        assert!(!p.undo.is_empty());
        assert!(!p.redo.is_empty());

        p.command(&json!({"op":"palette_delete","id":saved_id}), &mut m)
            .unwrap();
        assert_eq!(p.draft.id, None);
        assert!(p.undo.iter().all(|d| d.id.is_none()));
        assert!(p.redo.iter().all(|d| d.id.is_none()));

        // Undoing and saving should succeed as a new saved palette, not fail with NotFound.
        p.command(&json!({"op":"palette_undo"}), &mut m).unwrap();
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 1);

        // Redoing and saving updates the newly saved palette cleanly.
        p.command(&json!({"op":"palette_redo"}), &mut m).unwrap();
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 1);
    }

    #[test]
    fn palette_save_undo_save_updates_single_palette_and_new_starts_fresh_lineage() {
        let db = TempDb::new();
        let mut p = db.editor();
        let mut m = Mixer::default();

        p.add_history(Color::new(1, 2, 3, 255), &mut m);
        p.add_history(Color::new(4, 5, 6, 255), &mut m);
        p.command(&json!({"op":"palette_name","name":"Brand"}), &mut m)
            .unwrap();
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        let first_id = p.draft.id.clone().unwrap();
        assert_eq!(p.saved.len(), 1);

        // Undoing back to before the rename/second swatch keeps the bound palette ID.
        p.command(&json!({"op":"palette_undo"}), &mut m).unwrap();
        assert_eq!(p.draft.id.as_deref(), Some(first_id.as_str()));
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 1);
        assert_eq!(p.saved[0].id, first_id);

        // Starting a new palette creates a fresh lineage that does not overwrite the first.
        p.command(&json!({"op":"palette_new"}), &mut m).unwrap();
        assert_eq!(p.draft.id, None);
        p.add_history(Color::new(9, 8, 7, 255), &mut m);
        p.command(&json!({"op":"palette_name","name":"Second"}), &mut m)
            .unwrap();
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 2);
        let second_id = p.draft.id.clone().unwrap();
        assert_ne!(second_id, first_id);

        // Duplicate also starts a fresh lineage.
        p.command(&json!({"op":"palette_duplicate"}), &mut m)
            .unwrap();
        assert_eq!(p.draft.id, None);
        p.command(&json!({"op":"palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 3);
    }

    #[test]
    fn snapshot_does_not_panic_on_mismatched_names_or_locks() {
        let mut p = editor();
        p.draft.colors = vec![Color::new(1, 2, 3, 255), Color::new(4, 5, 6, 255)];
        p.draft.names = vec!["Only one".into()];
        p.draft.locked = vec![true];
        let snap = p.snapshot();
        let swatches = snap["swatches"].as_array().unwrap();
        assert_eq!(swatches.len(), 2);
        assert_eq!(swatches[0]["name"], json!("Only one"));
        assert_eq!(swatches[0]["locked"], json!(true));
        assert_eq!(swatches[1]["name"], json!("Swatch 2"));
        assert_eq!(swatches[1]["locked"], json!(false));
    }

    #[test]
    fn generation_checks_locked_count_first_and_skips_noop_records() {
        let db = TempDb::new();
        let mut p = db.editor();
        let mut m = Mixer::default();

        p.add_history(Color::new(10, 20, 30, 255), &mut m);
        p.add_history(Color::new(40, 50, 60, 255), &mut m);
        p.command(
            &json!({"op":"palette_lock","index":0,"locked":true}),
            &mut m,
        )
        .unwrap();
        p.command(
            &json!({"op":"palette_lock","index":1,"locked":true}),
            &mut m,
        )
        .unwrap();

        // Even with invalid OKLCH bounds, locked_count > count returns the lock error first.
        let too_small = json!({
            "op": "palette_generate",
            "mode": "triadic",
            "count": 1,
            "seed": "1",
            "base": "#3388CCFF",
            "hue_min": 300.0,
            "hue_max": 100.0,
            "lightness_min": 0.9,
            "lightness_max": 0.1,
            "chroma_min": 0.0,
            "chroma_max": 0.4,
            "contrast": null
        });
        let err = p.command(&too_small, &mut m).unwrap_err();
        assert_eq!(
            err,
            "Palette size is smaller than the number of locked swatches."
        );
        assert!(p.generations.is_empty());

        // When locked_count == count, unused bounds do not fail and no spurious generation is written.
        let all_locked = json!({
            "op": "palette_generate",
            "mode": "triadic",
            "count": 2,
            "seed": "1",
            "base": "#808080FF",
            "hue_min": 300.0,
            "hue_max": 100.0,
            "lightness_min": 0.9,
            "lightness_max": 0.1,
            "chroma_min": 0.0,
            "chroma_max": 0.4,
            "contrast": 21.0
        });
        let preview = p.preview_generation(&all_locked).unwrap();
        assert_eq!(preview, vec!["#0A141EFF", "#28323CFF"]);
        p.command(&all_locked, &mut m).unwrap();
        assert_eq!(
            p.draft.colors,
            vec![Color::new(10, 20, 30, 255), Color::new(40, 50, 60, 255)]
        );
        assert!(p.generations.is_empty());
    }

    #[test]
    fn noop_open_palette_preserves_lineage_so_save_undo_save_does_not_duplicate() {
        let db = TempDb::new();
        let mut p = db.editor();
        let mut m = Mixer::default();

        let request = json!({
            "op": "palette_generate",
            "mode": "triadic",
            "count": 3,
            "seed": "42",
            "base": "#3388CCFF",
            "hue_min": 0.0,
            "hue_max": 360.0,
            "lightness_min": 0.0,
            "lightness_max": 1.0,
            "chroma_min": 0.0,
            "chroma_max": 0.4,
            "contrast": null
        });
        p.command(&request, &mut m).unwrap();
        let gen_id = p.generations[0].id.clone();

        p.command(&json!({"op": "palette_name", "name": "Ocean"}), &mut m)
            .unwrap();
        let lineage_before = p.draft.lineage;
        let undo_before = p.undo.len();
        let redo_before = p.redo.len();

        // Reloading the identical generation is a content no-op and must not fork lineage.
        p.command(&json!({"op": "palette_generation", "id": gen_id}), &mut m)
            .unwrap();
        assert_eq!(p.draft.lineage, lineage_before);
        assert_eq!(p.undo.len(), undo_before);
        assert_eq!(p.redo.len(), redo_before);

        p.command(&json!({"op": "palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 1);
        let saved_id = p.saved[0].id.clone();

        // Reopening the already-open saved palette is also a strict no-op.
        let lineage_saved = p.draft.lineage;
        p.command(&json!({"op": "palette_open", "id": saved_id}), &mut m)
            .unwrap();
        assert_eq!(p.draft.lineage, lineage_saved);

        p.command(&json!({"op": "palette_undo"}), &mut m).unwrap();
        assert_eq!(p.draft.id.as_deref(), Some(saved_id.as_str()));

        p.command(&json!({"op": "palette_save"}), &mut m).unwrap();
        assert_eq!(p.saved.len(), 1);
        assert_eq!(p.saved[0].id, saved_id);
    }
}
