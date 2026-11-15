use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState, pointer::cursor_shape::CursorShapeManager},
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use smithay_client_toolkit::reexports::protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use std::error::Error;
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};
mod handlers;
mod palette;

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;
type Entry = (String, String, String, String);

struct Options {
    limit: u32,
    columns: usize,
    rows: usize,
}
fn options() -> Result<Options> {
    let mut o = Options {
        limit: 24,
        columns: 4,
        rows: 3,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value: u32 = args.next().ok_or("missing argument value")?.parse()?;
        match arg.as_str() {
            "--display-limit" if (1..=10000).contains(&value) => o.limit = value,
            "--columns" if (1..=12).contains(&value) => o.columns = value as usize,
            "--rows" if (1..=12).contains(&value) => o.rows = value as usize,
            _ => return Err(format!("invalid argument: {arg}").into()),
        }
    }
    Ok(o)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("swatch-board-history: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let opts = options()?;
    let bus = zbus::blocking::Connection::session()?;
    let entries: Vec<Entry> =
        swatch_board_ipc::SwatchBoardProxyBlocking::new(&bus)?.get_history(opts.limit)?;
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shell = LayerShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let cursor = compositor.create_surface(&qh);
    let cursor_theme = wayland_cursor::CursorTheme::load(&conn, shm.wl_shm().clone(), 24)?;
    let cursor_shapes = CursorShapeManager::bind(&globals, &qh).ok();
    let mut app = App {
        palette: palette::Palette::load(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        pool: SlotPool::new(1024 * 1024, &shm)?,
        shm,
        cursor,
        cursor_theme,
        cursor_shapes,
        shape_devices: Vec::new(),
        compositor,
        shell,
        shields: Vec::new(),
        pointers: Vec::new(),
        keyboards: Vec::new(),
        active: None,
        selected_output: None,
        selected: 0,
        offset: 0,
        entries,
        opts,
        bus,
        status: String::new(),
        pressed: None,
        exit: false,
        started: false,
    };
    queue.roundtrip(&mut app)?;
    queue.roundtrip(&mut app)?;
    let outputs: Vec<_> = app.outputs.outputs().collect();
    if outputs.is_empty() {
        return Err("no Wayland outputs".into());
    }
    for output in outputs {
        app.add_output(&qh, output);
    }
    queue.roundtrip(&mut app)?;
    queue.roundtrip(&mut app)?;
    if app.selected_output.is_none() {
        let result = std::process::Command::new("kscreen-doctor")
            .arg("-j")
            .output()
            .map_err(|e| format!("cannot query KDE primary output: {e}"))?;
        if !result.status.success() {
            return Err("kscreen-doctor could not query KDE output configuration".into());
        }
        let primary = primary_output(&result.stdout)?;
        app.selected_output = app.shields.iter().position(|shield| {
            app.outputs
                .info(&shield.output)
                .and_then(|info| info.name)
                .as_deref()
                == Some(primary.as_str())
        });
        if app.selected_output.is_none() {
            return Err(
                format!("KDE primary output {primary:?} has no matching Wayland output").into(),
            );
        }
    }
    app.started = true;
    app.activate();
    while !app.exit {
        queue.blocking_dispatch(&mut app)?;
    }
    Ok(())
}

fn primary_output(data: &[u8]) -> Result<String> {
    let config: serde_json::Value = serde_json::from_slice(data)?;
    let outputs = config["outputs"]
        .as_array()
        .ok_or("KDE output configuration lacks outputs")?;
    let mut primary = outputs.iter().filter(|output| {
        output["enabled"].as_bool() == Some(true) && output["priority"].as_u64() == Some(1)
    });
    let name = primary
        .next()
        .and_then(|output| output["name"].as_str())
        .ok_or("KDE did not identify a primary output")?;
    if primary.next().is_some() {
        return Err("KDE reported multiple primary outputs".into());
    }
    Ok(name.to_owned())
}

struct Shield {
    output: wl_output::WlOutput,
    layer: LayerSurface,
    width: u32,
    height: u32,
    configured: bool,
}
struct App {
    palette: palette::Palette,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    shm: Shm,
    compositor: CompositorState,
    shell: LayerShell,
    pool: SlotPool,
    cursor: wl_surface::WlSurface,
    cursor_theme: wayland_cursor::CursorTheme,
    cursor_shapes: Option<CursorShapeManager>,
    shape_devices: Vec<(wl_pointer::WlPointer, WpCursorShapeDeviceV1)>,
    shields: Vec<Shield>,
    pointers: Vec<(wl_seat::WlSeat, wl_pointer::WlPointer)>,
    keyboards: Vec<(wl_seat::WlSeat, wl_keyboard::WlKeyboard)>,
    active: Option<usize>,
    selected_output: Option<usize>,
    selected: usize,
    offset: usize,
    entries: Vec<Entry>,
    opts: Options,
    bus: zbus::blocking::Connection,
    status: String,
    pressed: Option<Option<usize>>,
    exit: bool,
    started: bool,
}
impl App {
    fn show_cursor(&mut self, pointer: &wl_pointer::WlPointer, serial: u32) {
        if let Some((_, device)) = self.shape_devices.iter().find(|(p, _)| p == pointer) {
            device.set_shape(serial, Shape::Default);
            return;
        }
        if let Some(cursor) = self.cursor_theme.get_cursor("default") {
            let image = &cursor[0];
            let (x, y) = image.hotspot();
            pointer.set_cursor(serial, Some(&self.cursor), x as i32, y as i32);
            self.cursor.attach(Some(image), 0, 0);
            self.cursor.damage_buffer(0, 0, i32::MAX, i32::MAX);
            self.cursor.commit();
            return;
        }
        let Ok((buffer, canvas)) = self
            .pool
            .create_buffer(24, 24, 96, wl_shm::Format::Argb8888)
        else {
            self.exit = true;
            return;
        };
        canvas.fill(0);
        for y in 0..19 {
            for x in 0..12 {
                if x <= y / 2 && (y < 15 || (4..=7).contains(&x)) {
                    let border = x == 0
                        || x == y / 2
                        || y == 14
                        || (y >= 15 && (x == 4 || x == 7 || y == 18));
                    let color: u32 = if border { 0xff000000 } else { 0xffffffff };
                    let offset = ((y * 24 + x) * 4) as usize;
                    canvas[offset..offset + 4].copy_from_slice(&color.to_le_bytes());
                }
            }
        }
        if buffer.attach_to(&self.cursor).is_err() {
            self.exit = true;
            return;
        }
        pointer.set_cursor(serial, Some(&self.cursor), 0, 0);
        self.cursor.damage_buffer(0, 0, 24, 24);
        self.cursor.commit();
    }
    fn add_output(&mut self, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let surface = self.compositor.create_surface(qh);
        let layer = self.shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("swatch-board-history"),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();
        self.shields.push(Shield {
            output,
            layer,
            width: 0,
            height: 0,
            configured: false,
        });
    }
    fn activate(&mut self) {
        if let Some(i) = self.selected_output {
            self.shields[i]
                .layer
                .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
            self.shields[i].layer.commit();
        }
        self.redraw_all();
    }
    fn geometry(&self, i: usize) -> (i32, i32, i32, i32, i32, i32) {
        let s = &self.shields[i];
        popup_geometry(
            s.width as i32,
            s.height as i32,
            self.opts.columns,
            self.opts.rows,
        )
    }

    fn redraw_all(&mut self) {
        for i in 0..self.shields.len() {
            self.draw(i);
        }
    }
    fn draw(&mut self, i: usize) {
        if !self.shields[i].configured {
            return;
        }
        let (width, height) = (self.shields[i].width as i32, self.shields[i].height as i32);
        if width <= 0 || height <= 0 {
            return;
        }
        let (x, y, w, h, cw, ch) = self.geometry(i);
        let Ok((buffer, canvas)) =
            self.pool
                .create_buffer(width, height, width * 4, wl_shm::Format::Argb8888)
        else {
            self.exit = true;
            return;
        };
        canvas.fill(0);
        if self.started && self.selected_output == Some(i) {
            popup_frame(canvas, (width, height), x, y, w, h, &self.palette);
            text(
                canvas,
                width,
                height,
                x + 8,
                y + 8,
                "Recent colors",
                self.palette.window_text,
            );
            if self.entries.is_empty() {
                text(
                    canvas,
                    width,
                    height,
                    x + 8,
                    y + 32,
                    "Pick a color to start your history",
                    self.palette.window_text,
                );
            }
            for n in 0..self.opts.columns * self.opts.rows {
                if cw < 12 {
                    break;
                }
                let index = self.offset + n;
                let Some(entry) = self.entries.get(index) else {
                    break;
                };
                let xx = x + 8 + (n % self.opts.columns) as i32 * cw;
                let yy = y + 28 + (n / self.opts.columns) as i32 * ch;
                let (background, foreground) = if index == self.selected {
                    (self.palette.selection, self.palette.selection_text)
                } else {
                    (self.palette.view, self.palette.view_text)
                };
                rect(canvas, (width, height), xx, yy, cw - 4, ch - 4, background);
                if index == self.selected {
                    // Separate the focus accent from a matching swatch, using
                    // the scheme's contrasting selection foreground.
                    for (dx, dy, w, h) in [
                        (1, 1, cw - 6, 1),
                        (1, ch - 6, cw - 6, 1),
                        (1, 1, 1, ch - 6),
                        (cw - 6, 1, 1, ch - 6),
                    ] {
                        rect(canvas, (width, height), xx + dx, yy + dy, w, h, foreground);
                    }
                }
                let color = u32::from_str_radix(
                    entry.1.trim_start_matches('#').get(..6).unwrap_or("000000"),
                    16,
                )
                .unwrap_or(0);
                rect(
                    canvas,
                    (width, height),
                    xx + 3,
                    yy + 3,
                    cw - 10,
                    ch - 10,
                    self.palette.border,
                );
                rect(
                    canvas,
                    (width, height),
                    xx + 4,
                    yy + 4,
                    cw - 12,
                    ch - 12,
                    0xff000000 | color,
                );
                // Keep the code inside a padded strip, centered with the same
                // font metrics used to render it. The complete tile is square.
                let band_height = 25.min(ch - 12);
                let band_y = yy + ch - 8 - band_height;
                rect(
                    canvas,
                    (width, height),
                    xx + 4,
                    band_y,
                    cw - 12,
                    band_height,
                    background,
                );
                centered_text(
                    canvas,
                    (width, height),
                    (xx + 7, band_y + 4, cw - 18, band_height - 8),
                    &entry.1,
                    foreground,
                );
            }
            let status = if self.status.is_empty() {
                "Click or Enter to copy · Esc to close"
            } else {
                &self.status
            };
            text(
                canvas,
                width,
                height,
                x + 8,
                y + h - 24,
                status,
                self.palette.window_text,
            );
        }
        let surface = self.shields[i].layer.wl_surface();
        if buffer.attach_to(surface).is_err() {
            self.exit = true;
            return;
        }
        surface.damage_buffer(0, 0, width, height);
        self.shields[i].layer.commit();
    }
    fn ensure_visible(&mut self) {
        let page = self.opts.columns * self.opts.rows;
        if self.selected < self.offset {
            self.offset = self.selected / self.opts.columns * self.opts.columns;
        }
        if self.selected >= self.offset + page {
            self.offset =
                (self.selected / self.opts.columns + 1 - self.opts.rows) * self.opts.columns;
        }
    }
    fn copy(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            let result = swatch_board_ipc::SwatchBoardProxyBlocking::new(&self.bus)
                .and_then(|proxy| proxy.copy_history(entry.0.as_str()));
            match result {
                Ok(_) => self.exit = true,
                Err(_) => {
                    self.status = "COPY FAILED".into();
                    self.selected = index;
                    self.ensure_visible();
                    self.redraw_all();
                }
            }
        }
    }
    fn hit(&self, i: usize, p: (f64, f64)) -> Option<usize> {
        if self.selected_output != Some(i) {
            return None;
        }
        let (x, y, w, h, cw, ch) = self.geometry(i);
        let (px, py) = (p.0 as i32, p.1 as i32);
        if px < x || py < y || px >= x + w || py >= y + h {
            return None;
        }
        let Some((col, row)) = grid_hit(x, y, cw, ch, self.opts.columns, self.opts.rows, px, py)
        else {
            return Some(usize::MAX);
        };
        let n = self.offset + row as usize * self.opts.columns + col as usize;
        Some(if n < self.entries.len() {
            n
        } else {
            usize::MAX
        })
    }
    fn key(&mut self, key: u32) {
        if key == 1 {
            self.exit = true;
            return;
        }
        if key == 28 || key == 96 {
            self.copy(self.selected);
            return;
        }
        if self.entries.is_empty() {
            return;
        }
        let n = self.entries.len();
        self.selected = match key {
            105 => self.selected.saturating_sub(1),
            106 => (self.selected + 1).min(n - 1),
            103 => self.selected.saturating_sub(self.opts.columns),
            108 => (self.selected + self.opts.columns).min(n - 1),
            102 => 0,
            107 => n - 1,
            104 => self
                .selected
                .saturating_sub(self.opts.columns * self.opts.rows),
            109 => (self.selected + self.opts.columns * self.opts.rows).min(n - 1),
            _ => return,
        };
        self.ensure_visible();
        self.redraw_all();
    }
    fn scroll(&mut self, delta: f64) {
        if self.entries.is_empty() || delta == 0.0 {
            return;
        }
        let max = ((self.entries.len() - 1) / self.opts.columns).saturating_sub(self.opts.rows - 1)
            * self.opts.columns;
        if delta > 0.0 {
            self.offset = (self.offset + self.opts.columns).min(max);
        } else {
            self.offset = self.offset.saturating_sub(self.opts.columns);
        }
        self.selected = self.selected.clamp(
            self.offset,
            (self.offset + self.opts.columns * self.opts.rows - 1).min(self.entries.len() - 1),
        );
        self.redraw_all();
    }
}

// Reserve title/footer space before fitting the square grid to either axis.
fn popup_geometry(
    width: i32,
    height: i32,
    columns: usize,
    rows: usize,
) -> (i32, i32, i32, i32, i32, i32) {
    let cell = ((width - 16).max(0) / columns as i32)
        .min((height - 64).max(0) / rows as i32)
        .min(108);
    let w = (cell * columns as i32 + 16).min(width);
    let h = (cell * rows as i32 + 64).min(height);
    ((width - w) / 2, (height - h) / 2, w, h, cell, cell)
}
fn grid_hit(
    x: i32,
    y: i32,
    cw: i32,
    ch: i32,
    columns: usize,
    rows: usize,
    px: i32,
    py: i32,
) -> Option<(i32, i32)> {
    let (dx, dy) = (px - x - 8, py - y - 28);
    if dx < 0 || dy < 0 || cw < 12 {
        return None;
    }
    let (col, row) = (dx / cw, dy / ch);
    if col >= columns as i32 || row >= rows as i32 || dx % cw >= cw - 4 || dy % ch >= ch - 4 {
        return None;
    }
    Some((col, row))
}

fn popup_frame(
    c: &mut [u8],
    size: (i32, i32),
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    palette: &palette::Palette,
) {
    let (width, height) = size;
    // Layer surfaces have no compositor decoration: paint a soft, premultiplied
    // shadow into the transparent shield to lift the popup above the desktop.
    for yy in (y - 12).max(0)..(y + h + 20).min(height) {
        for xx in (x - 16).max(0)..(x + w + 16).min(width) {
            let dx = (x - xx).max(xx - (x + w - 1)).max(0) as f32;
            let dy = (y + 4 - yy).max(yy - (y + h + 3)).max(0) as f32;
            let alpha = (30.0 * (-(dx * dx + dy * dy) / 50.0).exp()).round() as u32;
            let p = ((yy * width + xx) * 4) as usize;
            c[p..p + 4].copy_from_slice(&(alpha << 24).to_le_bytes());
        }
    }
    rect(c, size, x, y, w, h, palette.border);
    rect(c, size, x + 1, y + 1, w - 2, h - 2, palette.window);
}

fn rect(c: &mut [u8], (width, height): (i32, i32), x: i32, y: i32, w: i32, h: i32, color: u32) {
    for yy in y.max(0)..(y + h).min(height) {
        for xx in x.max(0)..(x + w).min(width) {
            let p = ((yy * width + xx) * 4) as usize;
            c[p..p + 4].copy_from_slice(&color.to_le_bytes());
        }
    }
}
fn system_font() -> Option<&'static ab_glyph::FontArc> {
    static FONT: std::sync::OnceLock<Option<ab_glyph::FontArc>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        // Fontconfig resolves installed system fonts, including user overrides.
        let output = std::process::Command::new("fc-match")
            .args(["-s", "-f", "%{file}\n", "sans-serif"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|path| ab_glyph::FontArc::try_from_vec(std::fs::read(path).ok()?).ok())
    })
    .as_ref()
}
fn centered_text(c: &mut [u8], size: (i32, i32), area: (i32, i32, i32, i32), s: &str, color: u32) {
    use ab_glyph::{Font, ScaleFont};
    let (x, y, width, height) = area;
    if width <= 0 || height <= 0 {
        return;
    }
    let Some(font) = system_font() else {
        let text_width = s.chars().count() as i32 * 6;
        if text_width <= width && height >= 7 {
            bitmap_text(
                c,
                size.0,
                size.1,
                x + (width - text_width) / 2,
                y + (height - 7) / 2,
                s,
                color,
            );
        }
        return;
    };
    let scaled = font.as_scaled(17.0);
    let advance: f32 = s
        .chars()
        .map(|ch| scaled.h_advance(scaled.glyph_id(ch)))
        .sum();
    let scale = 17.0_f32
        .min(height as f32)
        .min(17.0 * width as f32 / advance.max(1.0));
    let text_width = advance * scale / 17.0;
    text_at_scale(
        c,
        size.0,
        size.1,
        x + ((width as f32 - text_width) / 2.0).round() as i32,
        y + ((height as f32 - scale) / 2.0).round() as i32,
        s,
        color,
        scale,
    );
}
fn text(c: &mut [u8], w: i32, h: i32, x: i32, y: i32, s: &str, color: u32) {
    text_at_scale(c, w, h, x, y, s, color, 17.0);
}
fn text_at_scale(c: &mut [u8], w: i32, h: i32, x: i32, y: i32, s: &str, color: u32, scale: f32) {
    use ab_glyph::{Font, ScaleFont};
    let Some(font) = system_font() else {
        bitmap_text(c, w, h, x, y, s, color);
        return;
    };
    let scaled = font.as_scaled(scale);
    let mut cursor = x as f32;
    for ch in s.chars() {
        let id = scaled.glyph_id(ch);
        let glyph =
            id.with_scale_and_position(scale, ab_glyph::point(cursor, y as f32 + scaled.ascent()));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= w || py >= h {
                    return;
                }
                let offset = ((py * w + px) * 4) as usize;
                let old = u32::from_le_bytes(c[offset..offset + 4].try_into().unwrap());
                let mut blended = 0xff000000;
                for shift in [0, 8, 16] {
                    let a = ((old >> shift) & 255) as f32;
                    let b = ((color >> shift) & 255) as f32;
                    blended |= ((a + (b - a) * coverage).round() as u32) << shift;
                }
                c[offset..offset + 4].copy_from_slice(&blended.to_le_bytes());
            });
        }
        cursor += scaled.h_advance(id);
    }
}
fn bitmap_text(c: &mut [u8], w: i32, h: i32, mut x: i32, y: i32, s: &str, color: u32) {
    for ch in s.to_ascii_uppercase().chars() {
        if x + 6 >= w {
            break;
        }
        for (row, b) in glyph(ch).iter().enumerate() {
            for col in 0..5 {
                if b & (1 << (4 - col)) != 0 {
                    rect(c, (w, h), x + col, y + row as i32, 1, 1, color);
                }
            }
        }
        x += 6;
    }
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '#' => [10, 31, 10, 10, 31, 10, 0],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::{grid_hit, popup_geometry, primary_output};

    #[test]
    fn square_grid_fits_each_screen_axis_and_configured_dimensions() {
        for (width, height) in [(1920, 1080), (640, 480), (320, 240), (128, 96)] {
            for columns in 1..=12 {
                for rows in 1..=12 {
                    let (x, y, w, h, cw, ch) = popup_geometry(width, height, columns, rows);
                    assert_eq!(cw, ch);
                    assert!(x >= 0 && y >= 0 && x + w <= width && y + h <= height);
                    assert!(8 + cw * columns as i32 <= w);
                    assert!(28 + ch * rows as i32 <= h - 24);
                }
            }
        }
    }

    #[test]
    fn hit_regions_match_tiles_and_exclude_gutters_and_footer() {
        let (x, y, _, _, cw, ch) = popup_geometry(640, 480, 4, 3);
        assert_eq!(grid_hit(x, y, cw, ch, 4, 3, x + 8, y + 28), Some((0, 0)));
        assert_eq!(
            grid_hit(
                x,
                y,
                cw,
                ch,
                4,
                3,
                x + 8 + 3 * cw + 10,
                y + 28 + 2 * ch + 10
            ),
            Some((3, 2))
        );
        assert_eq!(grid_hit(x, y, cw, ch, 4, 3, x + 8 + cw - 1, y + 38), None);
        assert_eq!(grid_hit(x, y, cw, ch, 4, 3, x + 18, y + 28 + 3 * ch), None);
    }

    #[test]
    fn primary_output_uses_enabled_priority_one_and_rejects_ambiguity() {
        let config = br#"{"outputs":[{"name":"HDMI-A-1","enabled":true,"priority":2},{"name":"eDP-1","enabled":true,"priority":1}]}"#;
        assert_eq!(primary_output(config).unwrap(), "eDP-1");
        assert!(
            primary_output(br#"{"outputs":[{"name":"eDP-1","enabled":false,"priority":1}]}"#)
                .is_err()
        );
        assert!(primary_output(br#"{"outputs":[{"name":"A","enabled":true,"priority":1},{"name":"B","enabled":true,"priority":1}]}"#).is_err());
        assert!(primary_output(b"invalid").is_err());
    }
}
