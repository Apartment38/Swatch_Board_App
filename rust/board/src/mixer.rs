use serde_json::{Value, json};
use swatch_board_color::{
    Color, Format,
    mixing::{MixingSpace, mix},
};

const FORMATS: [(Format, &str); 5] = [
    (Format::HexRgba, "HEX"),
    (Format::Rgb, "RGB"),
    (Format::Rgba, "RGBA"),
    (Format::Hsl, "HSL"),
    (Format::Hsv, "HSV"),
];

#[derive(Clone, PartialEq)]
struct State {
    current: Color,
    hsv: [f32; 3],
    sources: Vec<(Color, f64)>,
    space: MixingSpace,
    palette: Vec<Color>,
}

pub struct Mixer {
    state: State,
    undo: Vec<State>,
    redo: Vec<State>,
    error: Option<String>,
}

impl Default for Mixer {
    fn default() -> Self {
        let current = Color::new(36, 122, 195, 255);
        Self {
            state: State {
                current,
                hsv: hsv(current),
                sources: vec![
                    (Color::new(255, 0, 0, 255), 1.0),
                    (Color::new(0, 0, 255, 255), 1.0),
                ],
                space: MixingSpace::Srgb,
                palette: Vec::new(),
            },
            undo: Vec::new(),
            redo: Vec::new(),
            error: None,
        }
    }
}

impl Mixer {
    pub fn colors(&self) -> &[Color] {
        &self.state.palette
    }
    pub fn current(&self) -> Color {
        self.state.current
    }
    pub fn replace_palette(&mut self, colors: Vec<Color>) {
        if self.state.palette != colors {
            self.state.palette = colors;
            self.undo.clear();
            self.redo.clear();
        }
    }
    fn commit(&mut self, previous: State) {
        if self.state != previous {
            if self.undo.len() == 100 {
                self.undo.remove(0);
            }
            self.undo.push(previous);
            self.redo.clear();
        }
    }
    pub fn select(&mut self, color: Color) {
        let previous = self.state.clone();
        let mut channels = hsv(color);
        if channels[1] == 0.0 {
            channels[0] = self.state.hsv[0];
        }
        self.state.current = color;
        self.state.hsv = channels;
        self.commit(previous);
        self.error = None;
    }
    pub fn snapshot(&self, format: Format) -> Value {
        let c = self.state.current;
        json!({"current": c.format(Format::HexRgba), "hsv": [self.state.hsv[0]*360.0, self.state.hsv[1]*100.0, self.state.hsv[2]*100.0], "rgba": [c.r,c.g,c.b,c.a], "fields": FORMATS.iter().map(|(f,_)| c.format(*f)).collect::<Vec<_>>(), "cmyk": c.format(Format::CmykApprox), "output": c.format(format), "sources": self.state.sources.iter().map(|(c,w)| json!({"color":c.format(Format::HexRgba),"weight":w})).collect::<Vec<_>>(), "space": if self.state.space == MixingSpace::Srgb {"srgb"} else {"oklch"}, "can_undo": !self.undo.is_empty(), "can_redo": !self.redo.is_empty(), "error": self.error })
    }
    pub fn command(&mut self, request: &Value) -> Result<(), String> {
        let result = self.execute(request);
        self.error = result.as_ref().err().cloned();
        result
    }
    fn execute(&mut self, r: &Value) -> Result<(), String> {
        let op = string(r, "op")?;
        match op {
            "mixer_select" => self.select(color(r, "color")?),
            "mixer_apply" => {
                let index = index(r, "index", FORMATS.len())?;
                let text = string(r, "text")?.trim();
                let valid = match index {
                    0 => text.starts_with('#'),
                    1 => text.starts_with("rgb("),
                    2 => text.starts_with("rgba("),
                    3 => text.starts_with("hsl("),
                    _ => text.starts_with("hsv("),
                };
                let mut c = if valid {
                    text.parse::<Color>().ok()
                } else {
                    None
                }
                .ok_or_else(|| {
                    format!(
                        "Invalid {} value; current color is unchanged.",
                        FORMATS[index].1
                    )
                })?;
                if matches!(FORMATS[index].0, Format::Rgb | Format::Hsl | Format::Hsv) {
                    c.a = self.current().a;
                }
                self.select(c);
            }
            "mixer_hsv" => {
                let h = number(r, "h", 360.0)? as f32 / 360.0;
                let s = number(r, "s", 100.0)? as f32 / 100.0;
                let v = number(r, "v", 100.0)? as f32 / 100.0;
                let previous = self.state.clone();
                self.state.hsv = [h, s, v];
                self.state.current = from_hsv([h, s, v], self.current().a);
                self.commit(previous);
            }
            "mixer_rgba" => {
                let channel = |name| {
                    r.get(name)
                        .and_then(Value::as_u64)
                        .filter(|v| *v <= 255)
                        .map(|v| v as u8)
                        .ok_or_else(|| format!("Invalid {name}"))
                };
                let c = Color::new(channel("r")?, channel("g")?, channel("b")?, channel("a")?);
                let previous = self.state.clone();
                if (c.r, c.g, c.b) != (self.current().r, self.current().g, self.current().b) {
                    self.state.hsv = hsv(c);
                }
                self.state.current = c;
                self.commit(previous);
            }
            "mixer_undo" => {
                if let Some(s) = self.undo.pop() {
                    self.redo.push(std::mem::replace(&mut self.state, s));
                }
            }
            "mixer_redo" => {
                if let Some(s) = self.redo.pop() {
                    self.undo.push(std::mem::replace(&mut self.state, s));
                }
            }
            "mixer_source_add" => {
                let p = self.state.clone();
                self.state.sources.push((self.current(), 1.0));
                let result = self.blend();
                self.commit(p);
                result?;
            }
            "mixer_source_remove" => {
                let i = index(r, "index", self.state.sources.len())?;
                if self.state.sources.len() <= 2 {
                    return Err("At least two sources are required.".into());
                }
                let p = self.state.clone();
                self.state.sources.remove(i);
                let result = self.blend();
                self.commit(p);
                result?;
            }
            "mixer_source" => {
                let i = index(r, "index", self.state.sources.len())?;
                let c = color(r, "color")?;
                let w = number(r, "weight", 1.0)?;
                let p = self.state.clone();
                self.state.sources[i] = (c, w);
                let result = self.blend();
                self.commit(p);
                result?;
            }
            "mixer_space" => {
                let space = match string(r, "space")? {
                    "srgb" => MixingSpace::Srgb,
                    "oklch" => MixingSpace::Oklch,
                    _ => return Err("Invalid mixing space".into()),
                };
                let p = self.state.clone();
                self.state.space = space;
                let result = self.blend();
                self.commit(p);
                result?;
            }
            "mixer_mix" => {
                let p = self.state.clone();
                self.blend()?;
                self.commit(p);
            }
            _ => return Err(format!("Unknown mixer operation: {op}")),
        }
        Ok(())
    }
    fn blend(&mut self) -> Result<(), String> {
        let c = mix(&self.state.sources, self.state.space).map_err(|e| e.to_string())?;
        self.state.current = c;
        self.state.hsv = hsv(c);
        Ok(())
    }
}

fn string<'a>(r: &'a Value, key: &str) -> Result<&'a str, String> {
    r.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Invalid {key}"))
}
fn color(r: &Value, key: &str) -> Result<Color, String> {
    string(r, key)?
        .parse()
        .map_err(|_| format!("Invalid {key} color"))
}
fn index(r: &Value, key: &str, len: usize) -> Result<usize, String> {
    r.get(key)
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .filter(|&n| n < len)
        .ok_or_else(|| format!("Invalid {key}"))
}
fn number(r: &Value, key: &str, max: f64) -> Result<f64, String> {
    r.get(key)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && (0.0..=max).contains(v))
        .ok_or_else(|| format!("Invalid {key}"))
}

fn hsv(c: Color) -> [f32; 3] {
    let [r, g, b] = [c.r, c.g, c.b].map(|x| x as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    [h, if max == 0.0 { 0.0 } else { d / max }, max]
}
fn from_hsv([h, s, v]: [f32; 3], alpha: u8) -> Color {
    let h = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::new(
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
        alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_hsv_and_undo() {
        let mut m = Mixer::default();
        m.select(Color::new(255, 0, 0, 42));
        m.command(&json!({"op":"mixer_apply","index":1,"text":"rgb(0, 255, 0)"}))
            .unwrap();
        assert_eq!(m.current(), Color::new(0, 255, 0, 42));
        m.command(&json!({"op":"mixer_hsv","h":240,"s":100,"v":100}))
            .unwrap();
        assert_eq!(m.current(), Color::new(0, 0, 255, 42));
        m.command(&json!({"op":"mixer_undo"})).unwrap();
        assert_eq!(m.current(), Color::new(0, 255, 0, 42));
        assert!(
            m.command(&json!({"op":"mixer_apply","index":99,"text":"#fff"}))
                .is_err()
        );
    }
}
