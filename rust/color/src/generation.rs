//! Seeded palette generation in OKLCH.
//!
//! Bounds and harmony relationships apply to candidate OKLCH coordinates before
//! conversion. Conversion clips linear RGB channels to the sRGB gamut and rounds
//! to 8 bits, so the resulting colors can differ from those coordinates. Output
//! colors are opaque. Contrast is measured on the resulting encoded sRGB colors;
//! alpha on the reference color is ignored.

use crate::{
    Color,
    mixing::{decode, from_oklch, to_oklch},
    quantize,
};

/// The relationship between generated colors and the base hue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationMode {
    Complementary,
    SplitComplementary,
    Analogous,
    Triadic,
    Tetradic,
    Monochromatic,
    PerceptualContrast,
    Random,
}

impl GenerationMode {
    pub const ALL: [(Self, &'static str); 8] = [
        (Self::Complementary, "Complementary"),
        (Self::SplitComplementary, "Split complementary"),
        (Self::Analogous, "Analogous"),
        (Self::Triadic, "Triadic"),
        (Self::Tetradic, "Tetradic"),
        (Self::Monochromatic, "Monochromatic"),
        (Self::PerceptualContrast, "Perceptual contrast"),
        (Self::Random, "Random"),
    ];
}

/// Bounds are inclusive, and hue is a non-wrapping interval in degrees.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationOptions {
    pub count: usize,
    pub base: Option<Color>,
    pub mode: GenerationMode,
    pub seed: u64,
    pub hue: [f64; 2],
    pub lightness: [f64; 2],
    pub chroma: [f64; 2],
    pub minimum_contrast: Option<f64>,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            count: 5,
            base: None,
            mode: GenerationMode::Random,
            seed: 0,
            hue: [0.0, 360.0],
            lightness: [0.0, 1.0],
            chroma: [0.0, 0.4],
            minimum_contrast: None,
        }
    }
}

/// WCAG contrast ratio of two opaque sRGB colors (alpha is ignored).
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    fn luminance(c: Color) -> f64 {
        0.2126 * decode(c.r) + 0.7152 * decode(c.g) + 0.0722 * decode(c.b)
    }
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

// SplitMix64, including a fixed conversion to a uniformly distributed [0, 1).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / 9007199254740992.0)
    }
    fn between(&mut self, bounds: [f64; 2]) -> f64 {
        bounds[0] + (bounds[1] - bounds[0]) * self.next()
    }
}

/// Generate a complete palette or return an error without partial results.
///
/// Harmony modes use the base hue plus fixed offsets; monochromatic uses just
/// the base hue. If no base is supplied, or it is achromatic, a seeded hue is
/// chosen within the hue bounds. Perceptual contrast requires candidate OKLCH
/// lightness to differ by at least 0.2 from the base (white when absent).
/// Contrast constraints use the base, or white when absent. Candidate search is
/// bounded, so very narrow feasible contrast regions can also return an error.
pub fn generate(options: &GenerationOptions) -> Result<Vec<Color>, String> {
    fn valid(bounds: [f64; 2], max: f64) -> bool {
        bounds[0].is_finite()
            && bounds[1].is_finite()
            && bounds[0] >= 0.0
            && bounds[0] <= bounds[1]
            && bounds[1] <= max
    }
    if !(1..=64).contains(&options.count) {
        return Err("count must be between 1 and 64".into());
    }
    if !valid(options.hue, 360.0) || !valid(options.lightness, 1.0) || !valid(options.chroma, 0.4) {
        return Err("invalid OKLCH bounds".into());
    }
    if options
        .minimum_contrast
        .is_some_and(|v| !v.is_finite() || !(1.0..=21.0).contains(&v))
    {
        return Err("minimum contrast must be finite and between 1 and 21".into());
    }
    let white = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    let reference = options.base.unwrap_or(white);
    let [base_l, base_c, base_h] = to_oklch(reference);
    let mut rng = Rng(options.seed);
    let hue = if options.base.is_some() && base_c > 0.0001 {
        base_h.to_degrees().rem_euclid(360.0)
    } else {
        rng.between(options.hue).rem_euclid(360.0)
    };
    let offsets: &[f64] = match options.mode {
        GenerationMode::Complementary => &[0.0, 180.0],
        GenerationMode::SplitComplementary => &[0.0, 150.0, 210.0],
        GenerationMode::Analogous => &[-30.0, 0.0, 30.0],
        GenerationMode::Triadic => &[0.0, 120.0, 240.0],
        GenerationMode::Tetradic => &[0.0, 90.0, 180.0, 270.0],
        GenerationMode::Monochromatic => &[0.0],
        GenerationMode::PerceptualContrast | GenerationMode::Random => &[],
    };
    // 360 and 0 represent the same hue, including for singleton [360, 360].
    let in_hue = |h: f64| {
        (h >= options.hue[0] && h <= options.hue[1]) || (h == 0.0 && options.hue[1] == 360.0)
    };
    let hues: Vec<f64> = offsets
        .iter()
        .map(|offset| (hue + offset).rem_euclid(360.0))
        .filter(|&h| in_hue(h))
        .collect();
    if !offsets.is_empty() && hues.is_empty() {
        return Err("no harmony hue lies within the hue bounds".into());
    }
    let mut result = Vec::with_capacity(options.count);
    for index in 0..options.count {
        let mut accepted = None;
        for _ in 0..4096 {
            let h = if hues.is_empty() {
                rng.between(options.hue)
            } else {
                hues[index % hues.len()]
            };
            let l = rng.between(options.lightness);
            let c = rng.between(options.chroma);
            if options.mode == GenerationMode::PerceptualContrast && (l - base_l).abs() < 0.2 {
                continue;
            }
            let rgb = from_oklch(l, c, h.to_radians());
            let color = Color {
                r: quantize(rgb[0]),
                g: quantize(rgb[1]),
                b: quantize(rgb[2]),
                a: 255,
            };
            if options
                .minimum_contrast
                .is_none_or(|minimum| contrast_ratio(color, reference) >= minimum)
            {
                accepted = Some(color);
                break;
            }
        }
        match accepted {
            Some(color) => result.push(color),
            None => {
                return Err(
                    "unable to satisfy palette constraints within the candidate limit".into(),
                );
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_are_deterministic_and_produce_requested_count() {
        for (mode, _) in GenerationMode::ALL {
            let options = GenerationOptions {
                mode,
                seed: 42,
                ..Default::default()
            };
            let colors = generate(&options).unwrap();
            assert_eq!(colors.len(), options.count);
            assert_eq!(colors, generate(&options).unwrap());
            assert!(colors.iter().all(|color| color.a == 255));
        }
        let options = GenerationOptions::default();
        assert_ne!(
            generate(&options).unwrap(),
            generate(&GenerationOptions { seed: 1, ..options }).unwrap()
        );
    }

    #[test]
    fn validates_bounds_and_impossible_contrast() {
        let options = GenerationOptions {
            count: 0,
            ..Default::default()
        };
        assert!(generate(&options).is_err());
        assert!(
            generate(&GenerationOptions {
                count: 65,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            generate(&GenerationOptions {
                hue: [100.0, 20.0],
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            generate(&GenerationOptions {
                chroma: [0.0, f64::NAN],
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            generate(&GenerationOptions {
                lightness: [1.0, 1.0],
                chroma: [0.0, 0.0],
                minimum_contrast: Some(2.0),
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn contrast_is_checked_after_mapping() {
        let options = GenerationOptions {
            minimum_contrast: Some(4.5),
            seed: 9,
            ..Default::default()
        };
        let white = Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        assert!(
            generate(&options)
                .unwrap()
                .into_iter()
                .all(|c| contrast_ratio(c, white) >= 4.5)
        );
    }
}
