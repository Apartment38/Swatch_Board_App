use std::{
    error::Error,
    sync::mpsc,
    time::{Duration, SystemTime},
};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        protocols::wp::{
            presentation_time::client::{wp_presentation, wp_presentation_feedback},
            viewporter::client::{wp_viewport, wp_viewporter},
        },
    },
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
use swatch_board_picker_core::{Magnifier, PixelPoint, Rgba8, place_lens, render_magnifier};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
};

use crate::{
    capture::{CaptureFeed, CapturedFrame},
    options::PickerOptions,
};

#[derive(Clone, Debug)]
pub struct LensSample {
    pub stream_id: u32,
    pub source_point: PixelPoint,
    pub color: Rgba8,
    pub presentation_time_ns: Option<u64>,
    pub received_at: SystemTime,
}

const MIN_SURFACE_SIZE: u32 = 200;
const POINTER_GAP: i32 = 16;

/// Display a live source-pixel lens on a non-interactive layer surface.
/// The lens follows PipeWire cursor metadata on the matched output.
pub fn run(
    first_frame: CapturedFrame,
    frames: CaptureFeed,
    sample: tokio::sync::watch::Sender<Option<LensSample>>,
    ready: tokio::sync::oneshot::Sender<()>,
    stop: mpsc::Receiver<()>,
    options: PickerOptions,
) -> Result<(), String> {
    run_inner(first_frame, frames, sample, ready, stop, options).map_err(|error| error.to_string())
}

fn run_inner(
    first_frame: CapturedFrame,
    frames: CaptureFeed,
    sample: tokio::sync::watch::Sender<Option<LensSample>>,
    ready: tokio::sync::oneshot::Sender<()>,
    stop: mpsc::Receiver<()>,
    options: PickerOptions,
) -> Result<(), Box<dyn Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let layer_shell = LayerShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let viewporter: wp_viewporter::WpViewporter = globals.bind(&qh, 1..=1, ())?;
    let presentation: wp_presentation::WpPresentation = globals.bind(&qh, 1..=1, ())?;

    let mut app = Overlay {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        compositor,
        layer_shell,
        viewporter,
        presentation,
        viewport: None,
        output: None,
        buffer_size: MIN_SURFACE_SIZE,
        failure: None,
        pool: None,
        buffers: Vec::new(),
        layer: None,
        output_logical_size: (0, 0),
        surface_size: MIN_SURFACE_SIZE,
        zoom: options.zoom,
        options,
        last_diagnostic: std::time::Instant::now(),
        frame: first_frame,
        frames,
        sample,
        stop,
        ready: Some(ready),
        configured: false,
        quit: false,
    };

    event_queue.roundtrip(&mut app)?;
    app.configure_output(&qh)?;
    let mut event_loop: EventLoop<Overlay> = EventLoop::try_new()?;
    WaylandSource::new(conn.clone(), event_queue).insert(event_loop.handle())?;
    // Bound dispatch even if the compositor stops issuing frame callbacks.
    // Shutdown and new-output cursor updates must not depend on surface visibility.
    while !app.quit {
        event_loop.dispatch(Duration::from_millis(16), &mut app)?;
        app.update_frame(&qh);
        if app.configured && !app.quit {
            app.draw(&qh);
        }
        conn.flush()?;
    }
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    Ok(())
}

struct Overlay {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    compositor: CompositorState,
    layer_shell: LayerShell,
    viewporter: wp_viewporter::WpViewporter,
    presentation: wp_presentation::WpPresentation,
    viewport: Option<wp_viewport::WpViewport>,
    output: Option<wl_output::WlOutput>,
    buffer_size: u32,
    failure: Option<String>,
    pool: Option<SlotPool>,
    buffers: Vec<Buffer>,
    layer: Option<LayerSurface>,
    output_logical_size: (u32, u32),
    surface_size: u32,
    zoom: u32,
    options: PickerOptions,
    last_diagnostic: std::time::Instant,
    frame: CapturedFrame,
    frames: CaptureFeed,
    sample: tokio::sync::watch::Sender<Option<LensSample>>,
    stop: mpsc::Receiver<()>,
    ready: Option<tokio::sync::oneshot::Sender<()>>,
    configured: bool,
    quit: bool,
}

impl Overlay {
    fn configure_output(&mut self, qh: &QueueHandle<Self>) -> Result<(), Box<dyn Error>> {
        let portal_position = self.frame.stream_position;
        let matches: Vec<_> = self
            .output_state
            .outputs()
            .filter_map(|output| {
                let info = self.output_state.info(&output)?;
                let position = info.logical_position.unwrap_or(info.location);
                let size = info.logical_size?;
                (position == portal_position && size.0 > 0 && size.1 > 0)
                    .then_some((output, (size.0 as u32, size.1 as u32)))
            })
            .collect();
        let [(output, logical_size)] = matches.as_slice() else {
            return Err(format!(
                "portal stream {} at {:?} matched {} Wayland outputs",
                self.frame.stream_id,
                portal_position,
                matches.len()
            )
            .into());
        };
        if self.frame.stream_logical_size != (logical_size.0 as i32, logical_size.1 as i32) {
            eprintln!(
                "Portal logical size {:?} differs from output {:?}; using Wayland geometry",
                self.frame.stream_logical_size, logical_size
            );
        }
        let scale = (self.frame.frame.width as f64 / logical_size.0 as f64
            + self.frame.frame.height as f64 / logical_size.1 as f64)
            / 2.0;
        // Render in physical pixels, then set the exact logical destination.
        // A scale-1 buffer enlarged by the compositor blurs the source-pixel grid.
        let surface_size = MIN_SURFACE_SIZE.max(
            (((self.options.radius * 2 + 1) * self.options.zoom + 64) as f64 / scale).ceil() as u32,
        );
        let buffer_size = (surface_size as f64 * scale).round() as u32;
        if buffer_size < (self.options.radius * 2 + 1) * self.options.zoom + 32 {
            return Err("output scale cannot fit a physical-pixel magnifier".into());
        }
        if let Some(viewport) = self.viewport.take() {
            viewport.destroy();
        }
        self.layer.take();
        self.buffers.clear();
        self.pool = Some(SlotPool::new(
            (buffer_size * buffer_size * 4) as usize,
            &self.shm,
        )?);
        let surface = self.compositor.create_surface(qh);
        let viewport = self.viewporter.get_viewport(&surface, qh, ());
        viewport.set_destination(surface_size as i32, surface_size as i32);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("swatch-board-picker"),
            Some(output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_size(surface_size, surface_size);
        let input_region = Region::new(&self.compositor)?;
        layer
            .wl_surface()
            .set_input_region(Some(input_region.wl_region()));
        layer.commit();
        self.layer = Some(layer);
        self.viewport = Some(viewport);
        self.output = Some(output.clone());
        self.output_logical_size = *logical_size;
        self.surface_size = surface_size;
        self.buffer_size = buffer_size;
        self.zoom = self.options.zoom;
        self.configured = false;
        self.sample.send_replace(None);
        eprintln!(
            "Magnifier output: stream={} origin={:?} logical={:?} frame={}x{} surface={} buffer={} scale={scale:.4}",
            self.frame.stream_id,
            portal_position,
            logical_size,
            self.frame.frame.width,
            self.frame.frame.height,
            surface_size,
            buffer_size
        );
        Ok(())
    }

    fn update_frame(&mut self, qh: &QueueHandle<Self>) {
        if !matches!(self.stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            self.quit = true;
            return;
        }
        if let Some(error) = self.frames.failure() {
            self.failure = Some(error);
            self.quit = true;
            return;
        }
        let active = self
            .frames
            .snapshot()
            .into_iter()
            .filter(|frame| frame.pointer.is_some())
            .max_by_key(|frame| frame.cursor_updated_at);
        if let Some(frame) = active {
            let changed_output = frame.stream_id != self.frame.stream_id;
            let resized = (frame.frame.width, frame.frame.height)
                != (self.frame.frame.width, self.frame.frame.height);
            self.frame = frame;
            if changed_output || resized {
                if let Err(error) = self.configure_output(qh) {
                    self.failure = Some(error.to_string());
                    self.quit = true;
                }
            }
        } else {
            self.frame.pointer = None;
            self.sample.send_replace(None);
        }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let Some(layer) = self.layer.as_ref() else {
            return;
        };
        let width = self.buffer_size;
        let height = self.buffer_size;
        let stride = (width * 4) as i32;
        if !self
            .buffers
            .iter()
            .any(|buffer| !buffer.slot().has_active_buffers())
            && self.buffers.len() < 3
        {
            let Some((buffer, _canvas)) = renderer_operation(
                self.pool.as_mut().expect("overlay pool").create_buffer(
                    width as i32,
                    height as i32,
                    stride,
                    wl_shm::Format::Argb8888,
                ),
                "create magnifier buffer",
                &mut self.failure,
                &mut self.quit,
            ) else {
                return;
            };
            self.buffers.push(buffer);
        }
        let Some(buffer_index) = self
            .buffers
            .iter()
            .position(|buffer| !buffer.slot().has_active_buffers())
        else {
            layer.commit();
            return;
        };
        let Some(canvas) =
            self.buffers[buffer_index].canvas(self.pool.as_mut().expect("overlay pool"))
        else {
            layer.commit();
            return;
        };

        // Clear the movable lens surface to transparency outside the pixel grid.
        for pixel in canvas.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0, 0, 0, 0]);
        }
        let mut submitted_sample = None;
        let lens_ready = self.frame.pointer.and_then(|pointer| {
            render_magnifier(&self.frame.frame, pointer, self.options.radius, self.zoom)
        });
        if let Some(lens) = &lens_ready {
            let pointer = self.frame.pointer.expect("lens requires cursor position");
            let x_scale = self.output_logical_size.0 as f64 / self.frame.frame.width as f64;
            let y_scale = self.output_logical_size.1 as f64 / self.frame.frame.height as f64;
            let Some(logical_pointer) = source_to_logical(
                pointer,
                (self.frame.frame.width, self.frame.frame.height),
                self.output_logical_size,
            ) else {
                self.failure = Some("invalid source cursor or output dimensions".into());
                self.quit = true;
                return;
            };
            let protected_radius = (self.options.radius as f64 * x_scale.max(y_scale)).ceil()
                as u32
                + POINTER_GAP as u32;
            let Some(position) = place_lens(
                self.output_logical_size,
                logical_pointer,
                (self.surface_size, self.surface_size),
                protected_radius,
            ) else {
                self.failure = Some(
                    "output too small to keep magnifier outside the sampled neighborhood".into(),
                );
                self.quit = true;
                return;
            };
            layer.set_margin(position.y as i32, 0, 0, position.x as i32);

            paint_lens(canvas, lens, width, self.zoom);
            let center_index =
                lens.center.y as usize * lens.width as usize + lens.center.x as usize;
            if let Some(color) = lens.pixels.get(center_index) {
                submitted_sample = Some(LensSample {
                    stream_id: self.frame.stream_id,
                    source_point: pointer,
                    color: *color,
                    presentation_time_ns: self.frame.presentation_time_ns,
                    received_at: self.frame.received_at,
                });
                paint_color_value(canvas, *color, width);
                paint_text(
                    canvas,
                    &format!("X{} Y{}", pointer.x, pointer.y),
                    width,
                    width as usize - 30,
                );
                if self.options.diagnostics
                    && self.last_diagnostic.elapsed() >= Duration::from_millis(200)
                {
                    eprintln!(
                        "Lens candidate: stream={} source=({}, {}) RGB=#{:02X}{:02X}{:02X} receipt_age={}ms cursor_age={:?}ms pts={:?}",
                        self.frame.stream_id,
                        pointer.x,
                        pointer.y,
                        color.r,
                        color.g,
                        color.b,
                        self.frame
                            .received_at
                            .elapsed()
                            .unwrap_or_default()
                            .as_millis(),
                        self.frame
                            .cursor_updated_at
                            .and_then(|time| time.elapsed().ok())
                            .map(|age| age.as_millis()),
                        self.frame.presentation_time_ns
                    );
                    self.last_diagnostic = std::time::Instant::now();
                }
            }
        }

        layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        if renderer_operation(
            self.buffers[buffer_index].attach_to(layer.wl_surface()),
            "attach magnifier buffer",
            &mut self.failure,
            &mut self.quit,
        )
        .is_none()
        {
            return;
        }
        self.presentation.feedback(
            layer.wl_surface(),
            qh,
            SubmittedSample {
                surface: layer.wl_surface().clone(),
                sample: submitted_sample,
            },
        );
        layer.commit();
    }
}

fn validate_configured_size(size: (u32, u32), expected: u32) -> Result<(), String> {
    // A zero axis lets the client choose that axis; it does not waive
    // validation of the other, compositor-specified dimension.
    if (size.0 != 0 && size.0 != expected) || (size.1 != 0 && size.1 != expected) {
        return Err(format!(
            "unexpected size {size:?}, expected {expected}x{expected}"
        ));
    }
    Ok(())
}

fn renderer_operation<T, E: std::fmt::Display>(
    result: Result<T, E>,
    context: &str,
    failure: &mut Option<String>,
    quit: &mut bool,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            *failure = Some(format!("{context}: {error}"));
            *quit = true;
            None
        }
    }
}

fn source_to_logical(
    pointer: PixelPoint,
    physical: (u32, u32),
    logical: (u32, u32),
) -> Option<PixelPoint> {
    if physical.0 == 0
        || physical.1 == 0
        || logical.0 == 0
        || logical.1 == 0
        || pointer.x >= physical.0
        || pointer.y >= physical.1
    {
        return None;
    }
    Some(PixelPoint {
        x: (u64::from(pointer.x) * u64::from(logical.0) / u64::from(physical.0)) as u32,
        y: (u64::from(pointer.y) * u64::from(logical.1) / u64::from(physical.1)) as u32,
    })
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
            let border = if zoom < 6 { 1 } else { 2 };
            let is_center = x >= left
                && x < right
                && y >= top
                && y < bottom
                && (x < left + border
                    || x + border >= right
                    || y < top + border
                    || y + border >= bottom);
            let color = if is_center {
                let outer = x == left || x + 1 == right || y == top || y + 1 == bottom;
                // Small blocks only have room for one outline; contrast it with
                // the source instead of painting over the entire sampled pixel.
                let level = if zoom < 6 {
                    if u32::from(color.r) + u32::from(color.g) + u32::from(color.b) >= 384 {
                        0
                    } else {
                        255
                    }
                } else if outer {
                    255
                } else {
                    0
                };
                Rgba8 {
                    r: level,
                    g: level,
                    b: level,
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
    paint_text(
        canvas,
        &format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b),
        surface_size,
        surface_size as usize - 16,
    );
}

fn paint_text(canvas: &mut [u8], value: &str, surface_size: u32, start_y: usize) {
    let scale = 2usize;
    let advance = 4 * scale;
    let text_width = value.len() * advance;
    let Some(start_x) = (surface_size as usize)
        .checked_sub(text_width)
        .map(|width| width / 2)
    else {
        return;
    };
    if start_y + 5 * scale >= surface_size as usize {
        return;
    }
    // An opaque backplate keeps text legible over both white and black content.
    for y in start_y.saturating_sub(2)..(start_y + 5 * scale + 2).min(surface_size as usize) {
        for x in start_x.saturating_sub(2)..(start_x + text_width + 2).min(surface_size as usize) {
            let offset = (y * surface_size as usize + x) * 4;
            canvas[offset..offset + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }
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
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
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
        let _ = qh;
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
    fn update_output(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self.output.as_ref() != Some(&output) {
            return;
        }
        let Some(info) = self.output_state.info(&output) else {
            return;
        };
        let position = info.logical_position.unwrap_or(info.location);
        if position != self.frame.stream_position {
            self.failure = Some(
                "captured output origin changed; cancel and request fresh portal geometry".into(),
            );
            self.quit = true;
            return;
        }
        if info.logical_size
            != Some((
                self.output_logical_size.0 as i32,
                self.output_logical_size.1 as i32,
            ))
        {
            if let Err(error) = self.configure_output(qh) {
                self.failure = Some(error.to_string());
                self.quit = true;
            }
        }
    }
    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self.output.as_ref() == Some(&output) {
            self.failure = Some("captured output was removed".into());
            self.quit = true;
        }
    }
}

impl LayerShellHandler for Overlay {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if self.layer.as_ref().map(|current| current.wl_surface()) != Some(layer.wl_surface()) {
            return;
        }
        self.failure = Some("compositor closed the magnifier layer".into());
        self.quit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        if self.layer.as_ref().map(|current| current.wl_surface()) != Some(layer.wl_surface()) {
            return;
        }
        if renderer_operation(
            validate_configured_size(configure.new_size, self.surface_size),
            "configure magnifier dimensions",
            &mut self.failure,
            &mut self.quit,
        )
        .is_none()
        {
            return;
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

struct SubmittedSample {
    surface: wl_surface::WlSurface,
    sample: Option<LensSample>,
}

impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, SubmittedSample> for Overlay {
    fn event(
        state: &mut Self,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        submitted: &SubmittedSample,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, wp_presentation_feedback::Event::Presented { .. })
            && state
                .layer
                .as_ref()
                .is_some_and(|layer| layer.wl_surface() == &submitted.surface)
        {
            // Publish only compositor-confirmed displayed content, never queued
            // or discarded commits. KWin selection begins after this feedback.
            state.sample.send_replace(submitted.sample.clone());
            if submitted.sample.is_some() {
                if let Some(ready) = state.ready.take() {
                    let _ = ready.send(());
                }
            }
        }
    }
}
wayland_client::delegate_noop!(Overlay: ignore wp_presentation::WpPresentation);
wayland_client::delegate_noop!(Overlay: ignore wp_viewporter::WpViewporter);
wayland_client::delegate_noop!(Overlay: ignore wp_viewport::WpViewport);

#[cfg(test)]
mod tests {
    use super::*;
    use swatch_board_picker_core::RgbaFrame;

    #[test]
    fn configure_validates_each_nonzero_dimension_independently() {
        for size in [(201, 200), (200, 201), (201, 0), (0, 201)] {
            let mut failure = None;
            let mut quit = false;
            assert!(
                renderer_operation(
                    validate_configured_size(size, 200),
                    "configure magnifier dimensions",
                    &mut failure,
                    &mut quit
                )
                .is_none()
            );
            assert!(quit);
            assert!(failure.unwrap().contains("unexpected size"));
        }
        for size in [(0, 0), (200, 0), (0, 200), (200, 200)] {
            assert!(validate_configured_size(size, 200).is_ok());
        }
    }

    #[test]
    fn renderer_errors_record_concrete_failure_before_quitting() {
        for context in [
            "create magnifier buffer",
            "attach magnifier buffer",
            "configure magnifier dimensions",
        ] {
            let mut failure = None;
            let mut quit = false;
            let result = renderer_operation::<(), _>(
                Err("specific cause"),
                context,
                &mut failure,
                &mut quit,
            );
            assert!(result.is_none());
            assert!(quit);
            assert_eq!(failure, Some(format!("{context}: specific cause")));
        }
        let mut failure = None;
        let mut quit = false;
        assert_eq!(
            renderer_operation::<_, &str>(Ok(42), "ok", &mut failure, &mut quit),
            Some(42)
        );
        assert!(!quit);
        assert!(failure.is_none());
    }

    #[test]
    fn source_pointer_maps_all_physical_edges_inside_logical_output() {
        for (physical, logical) in [
            ((1920, 1080), (1920, 1080)),
            ((1920, 1080), (1536, 864)),
            ((1920, 1080), (1280, 720)),
            ((1920, 1080), (1097, 617)),
            ((1920, 1080), (960, 540)),
            ((2880, 1800), (1646, 1029)),
        ] {
            for x in 0..physical.0 {
                for y in [0, physical.1 - 1] {
                    let mapped = source_to_logical(PixelPoint { x, y }, physical, logical).unwrap();
                    assert_eq!(
                        mapped.x,
                        (u64::from(x) * u64::from(logical.0) / u64::from(physical.0)) as u32
                    );
                    assert!(
                        mapped.y < logical.1,
                        "{physical:?} -> {logical:?}: {mapped:?}"
                    );
                }
            }
            for y in 0..physical.1 {
                for x in [0, physical.0 - 1] {
                    let mapped = source_to_logical(PixelPoint { x, y }, physical, logical).unwrap();
                    assert_eq!(
                        mapped.y,
                        (u64::from(y) * u64::from(logical.1) / u64::from(physical.1)) as u32
                    );
                    assert!(
                        mapped.x < logical.0,
                        "{physical:?} -> {logical:?}: {mapped:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn source_mapping_rejects_invalid_geometry_and_avoids_overflow() {
        for (pointer, physical, logical) in [
            (PixelPoint { x: 0, y: 0 }, (0, 10), (10, 10)),
            (PixelPoint { x: 0, y: 0 }, (10, 0), (10, 10)),
            (PixelPoint { x: 0, y: 0 }, (10, 10), (0, 10)),
            (PixelPoint { x: 0, y: 0 }, (10, 10), (10, 0)),
            (PixelPoint { x: 10, y: 0 }, (10, 10), (5, 5)),
            (PixelPoint { x: 0, y: 10 }, (10, 10), (5, 5)),
        ] {
            assert!(source_to_logical(pointer, physical, logical).is_none());
        }
        let point = PixelPoint {
            x: u32::MAX - 1,
            y: u32::MAX - 1,
        };
        assert_eq!(
            source_to_logical(point, (u32::MAX, u32::MAX), (u32::MAX, u32::MAX)),
            Some(point)
        );
    }

    #[test]
    fn painted_center_retains_source_interior_and_visible_marker_at_supported_zooms() {
        for zoom in [4, 5, 10] {
            for level in [0, 255] {
                let color = Rgba8 {
                    r: level,
                    g: level,
                    b: level,
                    a: 255,
                };
                let frame = RgbaFrame::new(3, 3, vec![color; 9]).unwrap();
                let lens = render_magnifier(&frame, PixelPoint { x: 1, y: 1 }, 1, zoom).unwrap();
                let mut canvas = vec![0; 80 * 80 * 4];
                paint_lens(&mut canvas, &lens, 80, zoom);
                let origin = ((80 - lens.width) / 2) as usize;
                let left = origin + lens.center.x as usize - zoom as usize / 2;
                let top = origin + lens.center.y as usize - zoom as usize / 2;
                let mut contrasting = false;
                for y in top..top + zoom as usize {
                    for x in left..left + zoom as usize {
                        let offset = (y * 80 + x) * 4;
                        let pixel = &canvas[offset..offset + 4];
                        let border = if zoom < 6 { 1 } else { 2 };
                        if x >= left + border
                            && x + border < left + zoom as usize
                            && y >= top + border
                            && y + border < top + zoom as usize
                        {
                            assert_eq!(pixel, &[level, level, level, 255]);
                        }
                        contrasting |= pixel != [level, level, level, 255];
                    }
                }
                let center =
                    ((origin + lens.center.y as usize) * 80 + origin + lens.center.x as usize) * 4;
                assert_eq!(
                    &canvas[center..center + 4],
                    &[level, level, level, 255],
                    "zoom {zoom}, source {level}"
                );
                assert!(
                    contrasting,
                    "zoom {zoom}, source {level} needs a visible marker"
                );
            }
        }
    }

    #[test]
    fn center_marker_contrasts_on_a_white_source() {
        let frame = RgbaFrame::new(
            3,
            3,
            vec![
                Rgba8 {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255
                };
                9
            ],
        )
        .unwrap();
        let lens = render_magnifier(&frame, PixelPoint { x: 1, y: 1 }, 1, 10).unwrap();
        let mut canvas = vec![0; 80 * 80 * 4];
        paint_lens(&mut canvas, &lens, 80, 10);
        assert!(
            canvas.chunks_exact(4).any(|pixel| pixel == [0, 0, 0, 255]),
            "white source requires a contrasting black center outline"
        );
        let center = 40 * 80 * 4 + 40 * 4;
        assert_eq!(&canvas[center..center + 4], &[255, 255, 255, 255]);
    }
}
