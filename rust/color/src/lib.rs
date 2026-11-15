//! Canonical 8-bit sRGB colors and locale-independent text conversion.
//!
//! Hex and RGB formats preserve alpha only when their alpha-bearing variant is
//! selected. HSL, HSV, and CMYK describe opaque colors; formatting a translucent
//! color in one of these formats intentionally omits its alpha. CMYK conversion
//! is a device-independent approximation, not an ICC-profile conversion.

pub mod generation;
pub mod mixing;

use std::{error::Error, fmt, str::FromStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    HexRgb,
    HexRgba,
    Rgb,
    Rgba,
    Hsl,
    Hsv,
    /// Approximate, unprofiled CMYK; do not present it as color-managed output.
    CmykApprox,
}

impl FromStr for Format {
    type Err = ParseColorError;

    /// Parse a stable lowercase format name used in configuration.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "hex" => Ok(Self::HexRgb),
            "hex-rgba" => Ok(Self::HexRgba),
            "rgb" => Ok(Self::Rgb),
            "rgba" => Ok(Self::Rgba),
            "hsl" => Ok(Self::Hsl),
            "hsv" => Ok(Self::Hsv),
            "cmyk" => Ok(Self::CmykApprox),
            _ => Err(ParseColorError),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseColorError;

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid color")
    }
}
impl Error for ParseColorError {}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Format with fixed precision, a decimal point, and no locale dependence.
    pub fn format(self, format: Format) -> String {
        match format {
            Format::HexRgb => format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b),
            Format::HexRgba => format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a),
            Format::Rgb => format!("rgb({}, {}, {})", self.r, self.g, self.b),
            Format::Rgba => format!(
                "rgba({}, {}, {}, {:.3})",
                self.r,
                self.g,
                self.b,
                f64::from(self.a) / 255.0
            ),
            Format::Hsl | Format::Hsv => {
                let (h, s, v, l, sl) = self.hsv_hsl();
                match format {
                    Format::Hsl => format!("hsl({h:.2}, {:.2}%, {:.2}%)", sl * 100.0, l * 100.0),
                    _ => format!("hsv({h:.2}, {:.2}%, {:.2}%)", s * 100.0, v * 100.0),
                }
            }
            Format::CmykApprox => {
                let r = f64::from(self.r) / 255.0;
                let g = f64::from(self.g) / 255.0;
                let b = f64::from(self.b) / 255.0;
                let k = 1.0 - r.max(g).max(b);
                let (c, m, y) = if k == 1.0 {
                    (0.0, 0.0, 0.0)
                } else {
                    (
                        (1.0 - r - k) / (1.0 - k),
                        (1.0 - g - k) / (1.0 - k),
                        (1.0 - b - k) / (1.0 - k),
                    )
                };
                format!(
                    "cmyk({:.2}%, {:.2}%, {:.2}%, {:.2}%)",
                    c * 100.0,
                    m * 100.0,
                    y * 100.0,
                    k * 100.0
                )
            }
        }
    }

    fn hsv_hsl(self) -> (f64, f64, f64, f64, f64) {
        let r = f64::from(self.r) / 255.0;
        let g = f64::from(self.g) / 255.0;
        let b = f64::from(self.b) / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        let l = (max + min) / 2.0;
        let hsv_s = if max == 0.0 { 0.0 } else { d / max };
        let hsl_s = if d == 0.0 {
            0.0
        } else {
            d / (1.0 - (2.0 * l - 1.0).abs())
        };
        (h, hsv_s, max, l, hsl_s)
    }
}

fn number(value: &str, max: f64) -> Result<f64, ParseColorError> {
    let n: f64 = value.trim().parse().map_err(|_| ParseColorError)?;
    if !n.is_finite() || !(0.0..=max).contains(&n) {
        return Err(ParseColorError);
    }
    Ok(n)
}

fn percentage(value: &str) -> Result<f64, ParseColorError> {
    Ok(number(
        value.trim().strip_suffix('%').ok_or(ParseColorError)?,
        100.0,
    )? / 100.0)
}

fn byte(value: &str) -> Result<u8, ParseColorError> {
    // RGB channels are decimal integers, not floats, percentages, or signed values.
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseColorError);
    }
    value.parse().map_err(|_| ParseColorError)
}

fn quantize(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn from_hue(h: f64, chroma: f64, offset: f64) -> Color {
    let sector = (h % 360.0) / 60.0;
    let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (r, g, b) = if sector < 1.0 {
        (chroma, x, 0.0)
    } else if sector < 2.0 {
        (x, chroma, 0.0)
    } else if sector < 3.0 {
        (0.0, chroma, x)
    } else if sector < 4.0 {
        (0.0, x, chroma)
    } else if sector < 5.0 {
        (x, 0.0, chroma)
    } else {
        (chroma, 0.0, x)
    };
    Color::new(
        quantize(r + offset),
        quantize(g + offset),
        quantize(b + offset),
        255,
    )
}

impl FromStr for Color {
    type Err = ParseColorError;

    /// Parse #RRGGBB, #RRGGBBAA or comma-separated rgb/rgba/hsl/hsv/cmyk.
    /// RGB channels are 0–255 integers; alpha is 0–1; hue is 0–360 degrees
    /// (optionally suffixed `deg`); HSL/HSV/CMYK components require `%`.
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        if let Some(hex) = input.strip_prefix('#') {
            if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(ParseColorError);
            }
            let channel = |i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ParseColorError);
            return Ok(Self::new(
                channel(0)?,
                channel(2)?,
                channel(4)?,
                if hex.len() == 8 { channel(6)? } else { 255 },
            ));
        }
        let (name, args) = input.split_once('(').ok_or(ParseColorError)?;
        let args = args.strip_suffix(')').ok_or(ParseColorError)?;
        let mut parts = args.split(',');
        let parts = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        );
        match (name.trim(), parts) {
            ("rgb", (Some(r), Some(g), Some(b), None, None)) => {
                Ok(Self::new(byte(r)?, byte(g)?, byte(b)?, 255))
            }
            ("rgba", (Some(r), Some(g), Some(b), Some(a), None)) => Ok(Self::new(
                byte(r)?,
                byte(g)?,
                byte(b)?,
                quantize(number(a, 1.0)?),
            )),
            ("hsl" | "hsv", (Some(h), Some(s), Some(third), None, None)) => {
                let h = h.trim();
                let h = number(h.strip_suffix("deg").unwrap_or(h), 360.0)?;
                let s = percentage(s)?;
                let third = percentage(third)?;
                if name.trim() == "hsl" {
                    let chroma = (1.0 - (2.0 * third - 1.0).abs()) * s;
                    Ok(from_hue(h, chroma, third - chroma / 2.0))
                } else {
                    let chroma = third * s;
                    Ok(from_hue(h, chroma, third - chroma))
                }
            }
            ("cmyk", (Some(c), Some(m), Some(y), Some(k), None)) => {
                let (c, m, y, k) = (
                    percentage(c)?,
                    percentage(m)?,
                    percentage(y)?,
                    percentage(k)?,
                );
                Ok(Self::new(
                    quantize((1.0 - c) * (1.0 - k)),
                    quantize((1.0 - m) * (1.0 - k)),
                    quantize((1.0 - y) * (1.0 - k)),
                    255,
                ))
            }
            _ => Err(ParseColorError),
        }
    }
}
