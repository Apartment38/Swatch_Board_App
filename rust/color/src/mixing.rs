//! Weighted mixing in encoded sRGB or perceptual OKLCH.
//!
//! Alpha is averaged independently (straight alpha), not premultiplied. In OKLCH,
//! hue follows the shortest arc relative to the first chromatic, positive-weight
//! source; achromatic sources contribute to lightness and chroma but not hue.
//! The resulting linear RGB channels are clipped to the sRGB gamut before encoding.

use std::f64::consts::{PI, TAU};

use crate::{Color, quantize};

/// The color space in which components are averaged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MixingSpace {
    /// Average gamma-encoded sRGB channels directly.
    Srgb,
    /// Average OKLCH lightness, chroma and shortest-arc hue.
    Oklch,
}

/// Mix at least two colors with finite, nonnegative weights and a finite positive sum.
///
/// Zero-weight colors have no influence, including on the OKLCH hue reference.
/// Channels and alpha are rounded to the nearest 8-bit value.
pub fn mix(sources: &[(Color, f64)], space: MixingSpace) -> Result<Color, &'static str> {
    if sources.len() < 2 {
        return Err("at least two sources are required");
    }
    let mut sum = 0.0;
    for &(_, weight) in sources {
        if !weight.is_finite() || weight < 0.0 {
            return Err("weights must be finite and nonnegative");
        }
        sum += weight;
    }
    if !sum.is_finite() || sum <= 0.0 {
        return Err("weight sum must be finite and positive");
    }

    let mut alpha = 0.0;
    let mut components = [0.0; 3];
    let mut reference = None;
    let mut hue_offset = 0.0;
    let mut hue_weight = 0.0;
    for &(color, weight) in sources {
        if weight == 0.0 {
            continue;
        }
        let weight = weight / sum;
        if weight == 0.0 {
            continue;
        }
        alpha += f64::from(color.a) / 255.0 * weight;
        match space {
            MixingSpace::Srgb => {
                components[0] += f64::from(color.r) / 255.0 * weight;
                components[1] += f64::from(color.g) / 255.0 * weight;
                components[2] += f64::from(color.b) / 255.0 * weight;
            }
            MixingSpace::Oklch => {
                let [l, c, h] = to_oklch(color);
                components[0] += l * weight;
                components[1] += c * weight;
                // Conversion roundoff gives neutral sRGB grays tiny chroma.
                if c > 1e-7 {
                    let base = *reference.get_or_insert(h);
                    hue_offset += ((h - base + PI).rem_euclid(TAU) - PI) * weight;
                    hue_weight += weight;
                }
            }
        }
    }
    let rgb = match space {
        MixingSpace::Srgb => components,
        MixingSpace::Oklch => {
            let hue = reference.map_or(0.0, |base| base + hue_offset / hue_weight);
            from_oklch(components[0], components[1], hue)
        }
    };
    Ok(Color {
        r: quantize(rgb[0]),
        g: quantize(rgb[1]),
        b: quantize(rgb[2]),
        a: quantize(alpha),
    })
}

pub(crate) fn decode(channel: u8) -> f64 {
    let v = f64::from(channel) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn encode(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

pub(crate) fn to_oklch(color: Color) -> [f64; 3] {
    let (r, g, b) = (decode(color.r), decode(color.g), decode(color.b));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    let lightness = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
    let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    let b = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    [lightness, a.hypot(b), b.atan2(a)]
}

pub(crate) fn from_oklch(l: f64, c: f64, h: f64) -> [f64; 3] {
    let a = c * h.cos();
    let b = c * h.sin();
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        encode(4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_),
        encode(-1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_),
        encode(-0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    const BLUE: Color = Color {
        r: 0,
        g: 0,
        b: 255,
        a: 0,
    };

    #[test]
    fn rejects_invalid_sources_and_weights() {
        assert!(mix(&[], MixingSpace::Srgb).is_err());
        assert!(mix(&[(RED, 1.0)], MixingSpace::Srgb).is_err());
        for weights in [
            [-1.0, 2.0],
            [f64::NAN, 1.0],
            [f64::INFINITY, 1.0],
            [0.0, 0.0],
            [f64::MAX, f64::MAX],
        ] {
            assert!(mix(&[(RED, weights[0]), (BLUE, weights[1])], MixingSpace::Oklch).is_err());
        }
    }

    #[test]
    fn zero_weights_preserve_endpoints() {
        for space in [MixingSpace::Srgb, MixingSpace::Oklch] {
            assert_eq!(mix(&[(BLUE, 0.0), (RED, 3.0)], space), Ok(RED));
            assert_eq!(mix(&[(BLUE, 2.0), (RED, 0.0)], space), Ok(BLUE));
        }
    }

    #[test]
    fn srgb_averages_encoded_channels_and_straight_alpha() {
        assert_eq!(
            mix(&[(RED, 1.0), (BLUE, 1.0)], MixingSpace::Srgb),
            Ok(Color {
                r: 128,
                g: 0,
                b: 128,
                a: 128
            })
        );
    }

    #[test]
    fn oklch_differs_from_encoded_srgb() {
        let sources = [(RED, 1.0), (BLUE, 1.0)];
        let perceptual = mix(&sources, MixingSpace::Oklch).unwrap();
        assert_ne!(perceptual, mix(&sources, MixingSpace::Srgb).unwrap());
        assert_eq!(perceptual.a, 128);
    }

    #[test]
    fn achromatic_sources_do_not_change_hue() {
        let gray = Color {
            r: 128,
            g: 128,
            b: 128,
            a: 255,
        };
        let mixed = mix(&[(gray, 1.0), (RED, 1.0)], MixingSpace::Oklch).unwrap();
        let hue = to_oklch(mixed)[2];
        assert!((hue - to_oklch(RED)[2]).abs() < 0.03);
        let neutral = mix(
            &[
                (gray, 1.0),
                (
                    Color {
                        r: 255,
                        g: 255,
                        b: 255,
                        a: 255,
                    },
                    1.0,
                ),
            ],
            MixingSpace::Oklch,
        )
        .unwrap();
        assert_eq!(neutral.r, neutral.g);
        assert_eq!(neutral.g, neutral.b);
    }

    #[test]
    fn hue_wrap_uses_short_arc() {
        let color = |h: f64| {
            let rgb = from_oklch(0.7, 0.12, h);
            Color {
                r: quantize(rgb[0]),
                g: quantize(rgb[1]),
                b: quantize(rgb[2]),
                a: 255,
            }
        };
        let mixed = mix(&[(color(0.1), 1.0), (color(-0.1), 1.0)], MixingSpace::Oklch).unwrap();
        assert!(to_oklch(mixed)[2].abs() < 0.03);
    }
}
