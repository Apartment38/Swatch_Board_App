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
}

pub struct PaletteEditor {
    draft: Draft,
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
            },
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
        if self.draft != previous {
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
        let previous = self.draft.clone();
        self.draft = Draft {
            name: palette.name.clone(),
            colors: palette.colors.iter().copied().map(from_rgba).collect(),
            locked: palette.locked.clone(),
            names: palette.swatch_names.clone(),
            id: saved.then(|| palette.id.clone()),
        };
        self.commit(previous, mixer);
    }
    pub fn snapshot(&self) -> Value {
        json!({"name":self.draft.name,"id":self.draft.id,"swatches":self.draft.colors.iter().enumerate().map(|(i,c)| json!({"color":c.format(Format::HexRgba),"name":self.draft.names[i],"locked":self.draft.locked[i]})).collect::<Vec<_>>(),"saved":self.saved.iter().map(|p|json!({"id":p.id,"name":p.name})).collect::<Vec<_>>(),"generations":self.generations.iter().map(|g|json!({"id":g.id,"label":format!("{} · {} colors",g.description,g.palette.colors.len())})).collect::<Vec<_>>(),"can_undo":!self.undo.is_empty(),"can_redo":!self.redo.is_empty(),"lock_history":self.lock_history,"error":self.error,"status":self.status})
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
                self.draft.id = Some(saved.id);
                self.draft.name = saved.name;
                self.refresh();
            }
            "palette_duplicate" => {
                let p = self.draft.clone();
                self.draft.id = None;
                self.draft.name.push_str(" copy");
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
        let mut colors = generate(options)?;
        if self.draft.locked.iter().filter(|&&x| x).count() > options.count {
            return Err("Palette size is smaller than the number of locked swatches.".into());
        }
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
            if self.draft.locked[i] {
                colors[i] = self.draft.colors[i];
                locks[i] = true;
            }
        }
        for i in options.count..self.draft.colors.len() {
            if self.draft.locked[i] {
                let j = locks
                    .iter()
                    .position(|x| !x)
                    .ok_or("Too many locked swatches")?;
                colors[j] = self.draft.colors[i];
                locks[j] = true;
                names[j] = self.draft.names[i].clone();
            }
        }
        Ok((colors, locks, names))
    }

    fn generate(&mut self, options: GenerationOptions, mixer: &mut Mixer) -> Result<(), String> {
        let (colors, locks, names) = self.generated_colors(&options)?;
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
    fn editor() -> PaletteEditor {
        PaletteEditor {
            draft: Draft {
                name: "Test".into(),
                colors: vec![],
                locked: vec![],
                names: vec![],
                id: None,
            },
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
}
