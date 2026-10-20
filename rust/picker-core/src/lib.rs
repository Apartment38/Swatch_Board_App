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
        (px < self.stream_width && py < self.stream_height)
            .then_some(PixelPoint { x: px, y: py })
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
        self.pixels.get((point.y as usize) * self.width as usize + point.x as usize).copied()
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
    let mut pixels = vec![Rgba8 { r: 0, g: 0, b: 0, a: 0 }; len];

    for sy in 0..side {
        for sx in 0..side {
            let src_x = source_center.x as i64 + sx as i64 - radius as i64;
            let src_y = source_center.y as i64 + sy as i64 - radius as i64;
            let color = (if src_x >= 0 && src_y >= 0 {
                frame.pixel(PixelPoint { x: src_x as u32, y: src_y as u32 })
            } else {
                None
            })
            .unwrap_or(Rgba8 { r: 0, g: 0, b: 0, a: 255 });
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
    Some(Magnifier { width: extent, height: extent, pixels, center })
}
