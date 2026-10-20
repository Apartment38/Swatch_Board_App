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

const LENS_SIZE: u32 = 150;
const SURFACE_SIZE: u32 = 200;
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

    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Overlay,
        Some("colorboard-picker"),
        None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
    layer.set_margin(16, 16, 0, 0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(SURFACE_SIZE, SURFACE_SIZE);

    // An explicitly empty input region keeps all pointer events with KWin.
    let input_region = Region::new(&compositor)?;
    layer
        .wl_surface()
        .set_input_region(Some(input_region.wl_region()));
    layer.commit();

    let pool = SlotPool::new((SURFACE_SIZE * SURFACE_SIZE * 4) as usize, &shm)?;
    let mut app = Overlay {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        buffers: Vec::new(),
        layer,
        frame: first_frame,
        frames,
        stop,
        ready: Some(ready),
        configured: false,
        quit: false,
    };

    while !app.quit {
        event_queue.blocking_dispatch(&mut app)?;
    }
    Ok(())
}

struct Overlay {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    buffers: Vec<Buffer>,
    layer: LayerSurface,
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
        let width = SURFACE_SIZE;
        let height = SURFACE_SIZE;
        let stride = (width * 4) as i32;
        if !self
            .buffers
            .iter()
            .any(|buffer| !buffer.slot().has_active_buffers())
            && self.buffers.len() < 3
        {
            let Ok((buffer, _canvas)) = self.pool.create_buffer(
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
            self.layer
                .wl_surface()
                .frame(qh, FrameCallbackData(self.layer.wl_surface().clone()));
            self.layer.commit();
            return;
        };
        let Some(canvas) = self.buffers[buffer_index].canvas(&mut self.pool) else {
            self.layer
                .wl_surface()
                .frame(qh, FrameCallbackData(self.layer.wl_surface().clone()));
            self.layer.commit();
            return;
        };

        // Opaque dark backing helps the pixel grid remain legible.
        for pixel in canvas.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[24, 24, 24, 255]);
        }
        let lens_ready = self
            .frame
            .pointer
            .and_then(|pointer| render_magnifier(&self.frame.frame, pointer, RADIUS, ZOOM));
        if let Some(lens) = &lens_ready {
            paint_lens(canvas, lens);
            let center_index =
                lens.center.y as usize * lens.width as usize + lens.center.x as usize;
            if let Some(color) = lens.pixels.get(center_index) {
                paint_color_value(canvas, *color);
            }
        }

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        self.layer
            .wl_surface()
            .frame(qh, FrameCallbackData(self.layer.wl_surface().clone()));
        if self.buffers[buffer_index]
            .attach_to(self.layer.wl_surface())
            .is_err()
        {
            self.quit = true;
            return;
        }
        self.layer.commit();
        if lens_ready.is_some() {
            if let Some(ready) = self.ready.take() {
                let _ = ready.send(());
            }
        }
    }
}

fn paint_lens(canvas: &mut [u8], lens: &Magnifier) {
    let origin = ((SURFACE_SIZE - LENS_SIZE) / 2) as usize;
    for y in 0..lens.height as usize {
        for x in 0..lens.width as usize {
            let color = lens.pixels[y * lens.width as usize + x];
            let cx = lens.center.x as usize;
            let cy = lens.center.y as usize;
            let left = cx - ZOOM as usize / 2;
            let top = cy - ZOOM as usize / 2;
            let right = left + ZOOM as usize;
            let bottom = top + ZOOM as usize;
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
            let index = ((origin + y) * SURFACE_SIZE as usize + origin + x) * 4;
            // ARGB8888 in wl_shm is stored BGRA on little-endian systems.
            canvas[index..index + 4].copy_from_slice(&[color.b, color.g, color.r, color.a]);
        }
    }
}

fn paint_color_value(canvas: &mut [u8], color: Rgba8) {
    let value = format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    let scale = 2usize;
    let advance = 4 * scale;
    let start_x = (SURFACE_SIZE as usize - (value.len() * advance - scale)) / 2;
    let start_y = SURFACE_SIZE as usize - 16;
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
                        let offset = (y * SURFACE_SIZE as usize + x) * 4;
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
            if (width.get(), height.get()) != (SURFACE_SIZE, SURFACE_SIZE) {
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
