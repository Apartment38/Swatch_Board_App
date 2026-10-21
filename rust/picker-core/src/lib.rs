//! Pure pixel-coordinate and magnifier rendering primitives for Phase 1A.
//!
//! This crate deliberately contains no capture or compositor backend. A desktop
//! picker must only be enabled once live portal/PipeWire frames and compositor-owned
//! selection are connected and validated together.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelPoint {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputTransform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

/// Geometry for one captured output. `logical_origin` may be negative and
/// `scale` maps logical desktop units to untransformed stream pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputGeometry {
    pub logical_origin: Point,
    pub logical_size: Point,
    pub scale: f64,
    pub stream_width: u32,
    pub stream_height: u32,
    pub transform: OutputTransform,
}

impl OutputGeometry {
    /// Map a desktop logical point to the output's captured physical pixel.
    /// The caller must first choose the output containing the point.
    pub fn desktop_to_stream(&self, point: Point) -> Option<PixelPoint> {
        if !self.scale.is_finite()
            || self.scale <= 0.0
            || self.stream_width == 0
            || self.stream_height == 0
        {
            return None;
        }

        let lx = point.x - self.logical_origin.x;
        let ly = point.y - self.logical_origin.y;
        if lx < 0.0 || ly < 0.0 || lx >= self.logical_size.x || ly >= self.logical_size.y {
            return None;
        }

        let (raw_w, raw_h) = match self.transform {
            OutputTransform::Rotate90
            | OutputTransform::Rotate270
            | OutputTransform::Flipped90
            | OutputTransform::Flipped270 => (self.stream_height as f64, self.stream_width as f64),
            _ => (self.stream_width as f64, self.stream_height as f64),
        };
        let x = (lx * self.scale).floor();
        let y = (ly * self.scale).floor();
        if x < 0.0 || y < 0.0 || x >= raw_w || y >= raw_h {
            return None;
        }

        let (tx, ty) = match self.transform {
            OutputTransform::Normal => (x, y),
            OutputTransform::Rotate90 => (raw_h - 1.0 - y, x),
            OutputTransform::Rotate180 => (raw_w - 1.0 - x, raw_h - 1.0 - y),
            OutputTransform::Rotate270 => (y, raw_w - 1.0 - x),
            OutputTransform::Flipped => (raw_w - 1.0 - x, y),
            OutputTransform::Flipped90 => (raw_h - 1.0 - y, raw_w - 1.0 - x),
            OutputTransform::Flipped180 => (x, raw_h - 1.0 - y),
            OutputTransform::Flipped270 => (y, x),
        };
        let px = tx as u32;
        let py = ty as u32;
        (px < self.stream_width && py < self.stream_height).then_some(PixelPoint { x: px, y: py })
    }
}

/// A tightly packed RGBA frame. PipeWire adapters must normalize the negotiated
/// pixel format and stride before constructing this value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Rgba8>,
}

impl RgbaFrame {
    pub fn new(width: u32, height: u32, pixels: Vec<Rgba8>) -> Option<Self> {
        let expected = (width as usize).checked_mul(height as usize)?;
        (width > 0 && height > 0 && pixels.len() == expected).then_some(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn pixel(&self, point: PixelPoint) -> Option<Rgba8> {
        if point.x >= self.width || point.y >= self.height {
            return None;
        }
        self.pixels
            .get((point.y as usize) * self.width as usize + point.x as usize)
            .copied()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Magnifier {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Rgba8>,
    pub center: PixelPoint,
}

/// Build a nearest-neighbor lens centered on `source_center`. No interpolation
/// is used; each source pixel is drawn as a square `zoom` pixels wide/high.
pub fn render_magnifier(
    frame: &RgbaFrame,
    source_center: PixelPoint,
    radius: u32,
    zoom: u32,
) -> Option<Magnifier> {
    if zoom == 0 || frame.pixel(source_center).is_none() {
        return None;
    }
    let side = radius.checked_mul(2)?.checked_add(1)?;
    let extent = side.checked_mul(zoom)?;
    let len = (extent as usize).checked_mul(extent as usize)?;
    let mut pixels = vec![
        Rgba8 {
            r: 0,
            g: 0,
            b: 0,
            a: 0
        };
        len
    ];

    for sy in 0..side {
        for sx in 0..side {
            let src_x = source_center.x as i64 + sx as i64 - radius as i64;
            let src_y = source_center.y as i64 + sy as i64 - radius as i64;
            let color = (if src_x >= 0 && src_y >= 0 {
                frame.pixel(PixelPoint {
                    x: src_x as u32,
                    y: src_y as u32,
                })
            } else {
                None
            })
            .unwrap_or(Rgba8 {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            });
            for oy in 0..zoom {
                for ox in 0..zoom {
                    let dx = sx * zoom + ox;
                    let dy = sy * zoom + oy;
                    pixels[(dy as usize) * extent as usize + dx as usize] = color;
                }
            }
        }
    }
    let center = PixelPoint {
        x: radius * zoom + zoom / 2,
        y: radius * zoom + zoom / 2,
    };
    Some(Magnifier {
        width: extent,
        height: extent,
        pixels,
        center,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(transform: OutputTransform, origin: Point) -> OutputGeometry {
        OutputGeometry {
            logical_origin: origin,
            logical_size: Point { x: 4.0, y: 3.0 },
            scale: 1.0,
            stream_width: if matches!(
                transform,
                OutputTransform::Rotate90
                    | OutputTransform::Rotate270
                    | OutputTransform::Flipped90
                    | OutputTransform::Flipped270
            ) {
                3
            } else {
                4
            },
            stream_height: if matches!(
                transform,
                OutputTransform::Rotate90
                    | OutputTransform::Rotate270
                    | OutputTransform::Flipped90
                    | OutputTransform::Flipped270
            ) {
                4
            } else {
                3
            },
            transform,
        }
    }

    #[test]
    fn maps_negative_origin_and_fractional_scale() {
        let output = OutputGeometry {
            logical_origin: Point {
                x: -1920.0,
                y: -100.0,
            },
            logical_size: Point {
                x: 1280.0,
                y: 720.0,
            },
            scale: 1.5,
            stream_width: 1920,
            stream_height: 1080,
            transform: OutputTransform::Normal,
        };
        assert_eq!(
            output.desktop_to_stream(Point {
                x: -1919.0,
                y: -99.0
            }),
            Some(PixelPoint { x: 1, y: 1 })
        );
        assert_eq!(
            output.desktop_to_stream(Point {
                x: -1920.0,
                y: -100.0
            }),
            Some(PixelPoint { x: 0, y: 0 })
        );
        assert_eq!(output.desktop_to_stream(Point { x: 0.0, y: 0.0 }), None);
    }

    #[test]
    fn maps_rotated_stream_geometry() {
        let output = geometry(OutputTransform::Rotate90, Point { x: 0.0, y: 0.0 });
        assert_eq!(
            output.desktop_to_stream(Point { x: 0.0, y: 0.0 }),
            Some(PixelPoint { x: 2, y: 0 })
        );
        assert_eq!(
            output.desktop_to_stream(Point { x: 3.0, y: 2.0 }),
            Some(PixelPoint { x: 0, y: 3 })
        );
    }

    #[test]
    fn magnifier_expands_source_pixels_without_interpolation() {
        let red = Rgba8 {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        let green = Rgba8 {
            r: 0,
            g: 255,
            b: 0,
            a: 255,
        };
        let blue = Rgba8 {
            r: 0,
            g: 0,
            b: 255,
            a: 255,
        };
        let frame = RgbaFrame::new(3, 1, vec![red, green, blue]).unwrap();
        let magnifier = render_magnifier(&frame, PixelPoint { x: 1, y: 0 }, 1, 2).unwrap();
        assert_eq!((magnifier.width, magnifier.height), (6, 6));
        assert_eq!(
            magnifier.pixels[0],
            Rgba8 {
                r: 0,
                g: 0,
                b: 0,
                a: 255
            }
        );
        assert_eq!(magnifier.pixels[2 * 6], red);
        assert_eq!(magnifier.pixels[2 * 6 + 2], green);
        assert_eq!(magnifier.pixels[2 * 6 + 4], blue);
        assert_eq!(magnifier.center, PixelPoint { x: 3, y: 3 });
    }

    #[test]
    fn rejects_invalid_geometry_and_frame_coordinates() {
        let mut output = geometry(OutputTransform::Normal, Point { x: 0.0, y: 0.0 });
        output.scale = f64::NAN;
        assert_eq!(output.desktop_to_stream(Point { x: 0.0, y: 0.0 }), None);
        let frame = RgbaFrame::new(
            1,
            1,
            vec![Rgba8 {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            }],
        )
        .unwrap();
        assert_eq!(frame.pixel(PixelPoint { x: 1, y: 0 }), None);
        assert!(RgbaFrame::new(2, 2, vec![]).is_none());
    }

    #[test]
    fn maps_required_display_scales_to_exact_source_pixels() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let physical_width = (8.0 * scale) as u32;
            let physical_height = (6.0 * scale) as u32;
            let output = OutputGeometry {
                logical_origin: Point { x: 0.0, y: 0.0 },
                logical_size: Point { x: 8.0, y: 6.0 },
                scale,
                stream_width: physical_width,
                stream_height: physical_height,
                transform: OutputTransform::Normal,
            };
            assert_eq!(
                output.desktop_to_stream(Point { x: 3.9, y: 2.9 }),
                Some(PixelPoint {
                    x: (3.9_f64 * scale).floor() as u32,
                    y: (2.9_f64 * scale).floor() as u32,
                }),
                "scale {scale}"
            );
            assert_eq!(
                output.desktop_to_stream(Point { x: 8.0, y: 0.0 }),
                None,
                "right edge is outside at scale {scale}"
            );
        }
    }

    #[test]
    fn maps_all_output_transforms_with_negative_origin() {
        let origin = Point { x: -4.0, y: -3.0 };
        let expected = [
            (OutputTransform::Normal, PixelPoint { x: 0, y: 0 }),
            (OutputTransform::Rotate90, PixelPoint { x: 2, y: 0 }),
            (OutputTransform::Rotate180, PixelPoint { x: 3, y: 2 }),
            (OutputTransform::Rotate270, PixelPoint { x: 0, y: 3 }),
            (OutputTransform::Flipped, PixelPoint { x: 3, y: 0 }),
            (OutputTransform::Flipped90, PixelPoint { x: 2, y: 3 }),
            (OutputTransform::Flipped180, PixelPoint { x: 0, y: 2 }),
            (OutputTransform::Flipped270, PixelPoint { x: 0, y: 0 }),
        ];
        for (transform, expected_top_left) in expected {
            let output = geometry(transform, origin);
            assert_eq!(
                output.desktop_to_stream(origin),
                Some(expected_top_left),
                "transform {transform:?}"
            );
        }
    }

    #[test]
    fn samples_checkerboard_and_gradient_without_interpolation() {
        let pixels = (0..4)
            .flat_map(|y| {
                (0..4).map(move |x| {
                    if (x + y) % 2 == 0 {
                        Rgba8 {
                            r: 255,
                            g: 255,
                            b: 255,
                            a: 255,
                        }
                    } else {
                        Rgba8 {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        }
                    }
                })
            })
            .collect();
        let checkerboard = RgbaFrame::new(4, 4, pixels).unwrap();
        let lens = render_magnifier(&checkerboard, PixelPoint { x: 1, y: 1 }, 1, 3).unwrap();
        assert_eq!(
            lens.pixels[lens.center.y as usize * lens.width as usize + lens.center.x as usize],
            checkerboard.pixel(PixelPoint { x: 1, y: 1 }).unwrap()
        );
        assert_eq!(
            lens.pixels[0],
            checkerboard.pixel(PixelPoint { x: 0, y: 0 }).unwrap()
        );

        let gradient = RgbaFrame::new(
            4,
            1,
            (0..4)
                .map(|value| Rgba8 {
                    r: (value * 64) as u8,
                    g: 0,
                    b: 255 - (value * 64) as u8,
                    a: 255,
                })
                .collect(),
        )
        .unwrap();
        for x in 0..gradient.width {
            let lens = render_magnifier(&gradient, PixelPoint { x, y: 0 }, 0, 4).unwrap();
            assert!(
                lens.pixels
                    .iter()
                    .all(|pixel| *pixel == gradient.pixel(PixelPoint { x, y: 0 }).unwrap())
            );
        }
    }
}
