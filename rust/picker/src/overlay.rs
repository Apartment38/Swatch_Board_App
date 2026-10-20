use std::{error::Error, num::NonZeroU32, sync::mpsc};

use colorboard_picker_core::{Magnifier, Rgba8, render_magnifier};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData, Region},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{
        Shm, ShmHandler,
        slot::{Buffer, SlotPool},
    },
};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
};

use crate::capture::CapturedFrame;

const MIN_SURFACE_SIZE: u32 = 200;
const RADIUS: u32 = 7;
const ZOOM: u32 = 10;

/// Display a live source-pixel lens on a non-interactive layer surface.
/// The current prototype anchors the lens in the upper-right corner; source
/// sampling follows PipeWire cursor metadata.
pub fn run(
    first_frame: CapturedFrame,
    frames: mpsc::Receiver<CapturedFrame>,
    ready: tokio::sync::oneshot::Sender<()>,
    stop: mpsc::Receiver<()>,
) -> Result<(), String> {
    run_inner(first_frame, frames, ready, stop).map_err(|error| error.to_string())
}

fn run_inner(
    first_frame: CapturedFrame,
    frames: mpsc::Receiver<CapturedFrame>,
    ready: tokio::sync::oneshot::Sender<()>,
    stop: mpsc::Receiver<()>,
) -> Result<(), Box<dyn Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let layer_shell = LayerShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;

    let mut app = Overlay {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool: None,
        buffers: Vec::new(),
        layer: None,
        output_logical_size: (0, 0),
        surface_size: MIN_SURFACE_SIZE,
        zoom: ZOOM,
        frame: first_frame,
        frames,
        stop,
        ready: Some(ready),
        configured: false,
        quit: false,
    };

    event_queue.roundtrip(&mut app)?;
    let portal_position = app.frame.stream_position;
    let mut matching_outputs = app.output_state.outputs().filter_map(|output| {
        let info = app.output_state.info(&output)?;
        let position = info.logical_position.unwrap_or(info.location);
        let size = info.logical_size?;
        (position == portal_position && size.0 > 0 && size.1 > 0)
            .then_some((output, (size.0 as u32, size.1 as u32)))
    });
    let Some((output, output_logical_size)) = matching_outputs.next() else {
        return Err(format!(
            "no Wayland output matched portal stream origin {:?}",
            portal_position
        )
        .into());
    };
    if matching_outputs.next().is_some() {
        return Err(format!(
            "portal stream origin {:?} matched multiple Wayland outputs",
            portal_position
        )
        .into());
    }

    let scale_x = output_logical_size.0 as f64 / app.frame.frame.width as f64;
    let scale_y = output_logical_size.1 as f64 / app.frame.frame.height as f64;
    if app.frame.stream_logical_size != (output_logical_size.0 as i32, output_logical_size.1 as i32)
    {
        eprintln!(
            "Portal stream logical size {:?} differs from Wayland output size {:?}; using Wayland output geometry",
            app.frame.stream_logical_size, output_logical_size
        );
    }
    let zoom = (ZOOM as f64 * (scale_x + scale_y) / 2.0).round().max(1.0) as u32;
    let lens_extent = (RADIUS * 2 + 1) * zoom;
    let surface_size = MIN_SURFACE_SIZE.max(lens_extent + 50);

    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Overlay,
        Some("colorboard-picker"),
        Some(&output),
    );
    layer.set_anchor(Anchor::TOP | Anchor::LEFT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(surface_size, surface_size);

    // An explicitly empty input region keeps all pointer events with KWin.
    let input_region = Region::new(&compositor)?;
    layer
        .wl_surface()
        .set_input_region(Some(input_region.wl_region()));
    layer.commit();
    app.layer = Some(layer);
    app.output_logical_size = output_logical_size;
    app.surface_size = surface_size;
    app.zoom = zoom;
    app.pool = Some(SlotPool::new(
        (surface_size * surface_size * 4) as usize,
        &app.shm,
    )?);

    while !app.quit {
        event_queue.blocking_dispatch(&mut app)?;
    }
    Ok(())
}

struct Overlay {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: Option<SlotPool>,
    buffers: Vec<Buffer>,
    layer: Option<LayerSurface>,
    output_logical_size: (u32, u32),
    surface_size: u32,
    zoom: u32,
    frame: CapturedFrame,
    frames: mpsc::Receiver<CapturedFrame>,
    stop: mpsc::Receiver<()>,
    ready: Option<tokio::sync::oneshot::Sender<()>>,
    configured: bool,
    quit: bool,
}

impl Overlay {
    fn update_frame(&mut self) {
        while let Ok(frame) = self.frames.try_recv() {
            if frame.stream_id == self.frame.stream_id {
                self.frame = frame;
            }
        }
        if self.stop.try_recv().is_ok() {
            self.quit = true;
        }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        self.update_frame();
        let Some(layer) = self.layer.as_ref() else {
            return;
        };
        let width = self.surface_size;
        let height = self.surface_size;
        let stride = (width * 4) as i32;
        if !self
            .buffers
            .iter()
            .any(|buffer| !buffer.slot().has_active_buffers())
            && self.buffers.len() < 3
        {
            let Ok((buffer, _canvas)) = self.pool.as_mut().expect("overlay pool").create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            ) else {
                self.quit = true;
                return;
            };
            self.buffers.push(buffer);
        }
        let Some(buffer_index) = self
            .buffers
            .iter()
            .position(|buffer| !buffer.slot().has_active_buffers())
        else {
            layer
                .wl_surface()
                .frame(qh, FrameCallbackData(layer.wl_surface().clone()));
            layer.commit();
            return;
        };
        let Some(canvas) =
            self.buffers[buffer_index].canvas(self.pool.as_mut().expect("overlay pool"))
        else {
            layer
                .wl_surface()
                .frame(qh, FrameCallbackData(layer.wl_surface().clone()));
            layer.commit();
            return;
        };

        // Clear the movable lens surface to transparency outside the pixel grid.
        for pixel in canvas.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0, 0, 0, 0]);
        }
        let lens_ready = self
            .frame
            .pointer
            .and_then(|pointer| render_magnifier(&self.frame.frame, pointer, RADIUS, self.zoom));
        if let Some(lens) = &lens_ready {
            let pointer = self.frame.pointer.expect("lens requires cursor position");
            let x_scale = self.output_logical_size.0 as f64 / self.frame.frame.width as f64;
            let y_scale = self.output_logical_size.1 as f64 / self.frame.frame.height as f64;
            let cursor_x = (pointer.x as f64 * x_scale).round() as i32;
            let cursor_y = (pointer.y as f64 * y_scale).round() as i32;
            let center = self.surface_size as i32 / 2;
            let offset = 16;
            let surface_left = if cursor_x + offset + center <= self.output_logical_size.0 as i32 {
                cursor_x + offset - center
            } else {
                cursor_x - offset - center
            }
            .clamp(0, (self.output_logical_size.0 as i32 - width as i32).max(0));
            let surface_top = (cursor_y - center).clamp(
                0,
                (self.output_logical_size.1 as i32 - height as i32).max(0),
            );
            layer.set_margin(surface_top, 0, 0, surface_left);

            paint_lens(canvas, lens, width, self.zoom);
            let center_index =
                lens.center.y as usize * lens.width as usize + lens.center.x as usize;
            if let Some(color) = lens.pixels.get(center_index) {
                paint_color_value(canvas, *color, width);
            }
        }

        layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        layer
            .wl_surface()
            .frame(qh, FrameCallbackData(layer.wl_surface().clone()));
        if self.buffers[buffer_index]
            .attach_to(layer.wl_surface())
            .is_err()
        {
            self.quit = true;
            return;
        }
        layer.commit();
        if lens_ready.is_some() {
            if let Some(ready) = self.ready.take() {
                let _ = ready.send(());
            }
        }
    }
}

fn paint_lens(canvas: &mut [u8], lens: &Magnifier, surface_size: u32, zoom: u32) {
    let origin = ((surface_size - lens.width) / 2) as usize;
    for y in 0..lens.height as usize {
        for x in 0..lens.width as usize {
            let color = lens.pixels[y * lens.width as usize + x];
            let cx = lens.center.x as usize;
            let cy = lens.center.y as usize;
            let left = cx - zoom as usize / 2;
            let top = cy - zoom as usize / 2;
            let right = left + zoom as usize;
            let bottom = top + zoom as usize;
            let is_center = x >= left
                && x < right
                && y >= top
                && y < bottom
                && (x < left + 2 || x + 2 >= right || y < top + 2 || y + 2 >= bottom);
            let color = if is_center {
                Rgba8 {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                }
            } else {
                color
            };
            let index = ((origin + y) * surface_size as usize + origin + x) * 4;
            // ARGB8888 in wl_shm is stored BGRA on little-endian systems.
            canvas[index..index + 4].copy_from_slice(&[color.b, color.g, color.r, color.a]);
        }
    }
}

fn paint_color_value(canvas: &mut [u8], color: Rgba8, surface_size: u32) {
    let value = format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    let scale = 2usize;
    let advance = 4 * scale;
    let start_x = (surface_size as usize - (value.len() * advance - scale)) / 2;
    let start_y = surface_size as usize - 16;
    for (index, character) in value.chars().enumerate() {
        for (row, bits) in glyph(character).iter().enumerate() {
            for column in 0..3 {
                if bits & (1 << (2 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = start_x + index * advance + column * scale + dx;
                        let y = start_y + row * scale + dy;
                        let offset = (y * surface_size as usize + x) * 4;
                        canvas[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
                    }
                }
            }
        }
    }
}

fn glyph(character: char) -> [u8; 5] {
    match character {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        _ => [0; 5],
    }
}

impl CompositorHandler for Overlay {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        if self.configured {
            self.draw(qh);
        }
    }
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Overlay {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for Overlay {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.quit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let (Some(width), Some(height)) = (
            NonZeroU32::new(configure.new_size.0),
            NonZeroU32::new(configure.new_size.1),
        ) {
            if (width.get(), height.get()) != (self.surface_size, self.surface_size) {
                self.quit = true;
                return;
            }
        }
        self.configured = true;
        self.draw(qh);
    }
}

impl ShmHandler for Overlay {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(Overlay);
impl ProvidesRegistryState for Overlay {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}
smithay_client_toolkit::delegate_dispatch2!(Overlay);
