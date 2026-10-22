//! Disposable native shield + actual pointer cursor experiment. Not production code.
#[path = "../../rust/picker/src/capture.rs"]
mod capture;
#[path = "../../rust/picker/src/portal.rs"]
mod portal;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
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
    seat::{Capability, SeatHandler, SeatState},
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
use std::{
    collections::BTreeSet,
    error::Error,
    sync::mpsc,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
    globals::registry_queue_init,
    protocol::{wl_callback, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const LEFT: u32 = 272;
const RIGHT: u32 = 273;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--inspect"] {
        return inspect();
    }
    if args != ["--run"] && args != ["--run", "--large-cursor"] {
        println!(
            "Usage: phase1a_shield_probe --inspect | --run\n--run opens a HUMAN ScreenCast chooser, then waits 3 seconds before shielding ALL outputs for up to 12 seconds. No Escape binding. Left/right RELEASE selects/cancels. TEST cursor, not a magnifier."
        );
        return Ok(());
    }
    let portal::CaptureSession {
        portal: _portal,
        session,
        streams,
        pipewire_fd,
    } = portal::begin_capture().await?;
    let feed = capture::CaptureFeed::new();
    let (stop_tx, stop_rx) = mpsc::channel();
    let capture_feed = feed.clone();
    let worker =
        std::thread::spawn(move || capture::run(pipewire_fd, streams, capture_feed, stop_rx));
    // All paths after successful portal setup converge on explicit cleanup.
    let result = (|| -> Result<()> {
        let first = feed.wait_for_frame(Duration::from_secs(5))?;
        println!(
            "CAPTURE_READY stream={} image={}x{} portal_origin={:?} portal_size={:?}",
            first.stream_id,
            first.frame.width,
            first.frame.height,
            first.stream_position,
            first.stream_logical_size
        );
        println!(
            "PREPARE 3s: activate the native fixture NOW; shield has not been created. Timeout starts after mapping requests. No keyboard is bound."
        );
        std::thread::sleep(Duration::from_secs(3));
        run_shield(feed, if args.len() == 2 { 512 } else { 80 })
    })();
    let _ = stop_tx.send(());
    let joined = worker.join();
    let closed = session.close().await;
    println!("CAPTURE_CLEANUP worker={joined:?} session_close={closed:?}");
    result?;
    match joined {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err("capture worker panicked".into()),
    }
    closed?;
    Ok(())
}

fn inspect() -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let (globals, _) = registry_queue_init::<App>(&conn)?;
    globals.contents().with_list(|items| {
        for g in items {
            println!("GLOBAL {} {} v{}", g.name, g.interface, g.version);
        }
    });
    Ok(())
}

struct Shield {
    layer: LayerSurface,
    viewport: wp_viewport::WpViewport,
    origin: (i32, i32),
    size: (i32, i32),
    configured: bool,
}
struct App {
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    shm: Shm,
    compositor: CompositorState,
    shell: LayerShell,
    viewporter: wp_viewporter::WpViewporter,
    presentation: Option<wp_presentation::WpPresentation>,
    shields: Vec<Shield>,
    pointers: Vec<(wl_seat::WlSeat, wl_pointer::WlPointer)>,
    cursor: wl_surface::WlSurface,
    cursor_side: u32,
    pool: SlotPool,
    buffers: Vec<Buffer>,
    pressed: BTreeSet<(u32, u32)>,
    selected: Option<u32>,
    active_surface: Option<wl_surface::WlSurface>,
    position: (f64, f64),
    quit: bool,
    failure: Option<String>,
    feedback_presented: usize,
    feedback_discarded: usize,
}

fn run_shield(feed: capture::CaptureFeed, cursor_side: u32) -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let cursor = compositor.create_surface(&qh);
    let mut app = App {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        pool: SlotPool::new((cursor_side * cursor_side * 4 + 4) as usize, &shm)?,
        shell: LayerShell::bind(&globals, &qh)?,
        viewporter: globals.bind(&qh, 1..=1, ())?,
        presentation: globals.bind(&qh, 1..=1, ()).ok(),
        shm,
        compositor,
        cursor,
        cursor_side,
        shields: vec![],
        pointers: vec![],
        buffers: vec![],
        pressed: BTreeSet::new(),
        selected: None,
        active_surface: None,
        position: (0., 0.),
        quit: false,
        failure: None,
        feedback_presented: 0,
        feedback_discarded: 0,
    };
    // Cleanup must also run after a dispatch/render error.
    let result = (|| -> Result<()> {
        queue.roundtrip(&mut app)?;
        queue.roundtrip(&mut app)?;
        app.create_shields(&qh)?;
        let mut event_loop: EventLoop<App> = EventLoop::try_new()?;
        WaylandSource::new(conn.clone(), queue).insert(event_loop.handle())?;
        let start = Instant::now();
        let mut diagnostic = Instant::now() - Duration::from_secs(1);
        while !app.quit && start.elapsed() < Duration::from_secs(12) {
            event_loop.dispatch(Duration::from_millis(16), &mut app)?;
            if diagnostic.elapsed() >= Duration::from_millis(200) {
                app.sample(&feed);
                diagnostic = Instant::now();
            }
            if let Some(error) = feed.failure() {
                app.failure = Some(error);
                app.quit = true;
            }
            conn.flush()?;
        }
        app.sample(&feed);
        println!(
            "END reason={} selected_released_button={:?} still_pressed={:?} cursor_feedback_presented={} discarded={}",
            if app.quit {
                "release_or_error"
            } else {
                "12s_timeout"
            },
            app.selected,
            app.pressed,
            app.feedback_presented,
            app.feedback_discarded
        );
        if !app.pressed.is_empty() {
            println!(
                "WARNING timeout/error with buttons held: click safety NOT established; timeout is bounded, not a safe selection."
            );
        }
        if let Some(e) = &app.failure {
            return Err(e.clone().into());
        }
        Ok(())
    })();
    app.cleanup();
    let flushed = conn.flush();
    println!(
        "WAYLAND_CLEANUP destroy requests queued; flush={flushed:?}; connection drops on return"
    );
    result
}

impl App {
    fn create_shields(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        let outputs: Vec<_> = self.outputs.outputs().collect();
        if outputs.is_empty() {
            return Err("no Wayland outputs".into());
        }
        for output in outputs {
            let info = self.outputs.info(&output).ok_or("output info absent")?;
            let origin = info
                .logical_position
                .ok_or("xdg-output logical position required")?;
            let size = info
                .logical_size
                .filter(|s| s.0 > 0 && s.1 > 0)
                .ok_or("xdg-output logical size required; refusing integer-scale fallback")?;
            println!(
                "OUTPUT name={:?} origin={origin:?} logical={size:?} integer_scale={} modes={:?}",
                info.name, info.scale_factor, info.modes
            );
            let surface = self.compositor.create_surface(qh);
            let viewport = self.viewporter.get_viewport(&surface, qh, ());
            viewport.set_destination(size.0, size.1);
            let layer = self.shell.create_layer_surface(
                qh,
                surface,
                Layer::Overlay,
                Some("swatch-native-shield-TEST"),
                Some(&output),
            );
            layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            layer.set_size(size.0 as u32, size.1 as u32);
            layer.set_exclusive_zone(-1); // Cover panels too; do not reserve space.
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            // NULL means the infinite/default FULL input region, NOT empty.
            layer.wl_surface().set_input_region(None);
            layer.commit();
            self.shields.push(Shield {
                layer,
                viewport,
                origin,
                size,
                configured: false,
            });
        }
        println!(
            "SHIELD_REQUESTED outputs={} keyboard_interactivity=None keyboard_objects=0 timeout=12s cursor_sentinel=80x80 hotspot=0,0",
            self.shields.len()
        );
        Ok(())
    }
    fn show_cursor(
        &mut self,
        pointer: &wl_pointer::WlPointer,
        serial: u32,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let side = self.cursor_side;
        let (buffer, canvas) = self.pool.create_buffer(
            side as i32,
            side as i32,
            (side * 4) as i32,
            wl_shm::Format::Argb8888,
        )?;
        paint_sentinel(canvas, side as usize);
        self.cursor.set_buffer_scale(1);
        // Only Enter's serial authorizes this seat's actual cursor role.
        pointer.set_cursor(serial, Some(&self.cursor), 0, 0);
        buffer.attach_to(&self.cursor)?;
        self.cursor.damage_buffer(0, 0, side as i32, side as i32);
        self.cursor.frame(qh, ());
        if let Some(p) = &self.presentation {
            p.feedback(&self.cursor, qh, ());
        }
        self.cursor.commit();
        self.buffers.push(buffer);
        println!(
            "CURSOR_SET enter_serial={serial} buffer_side={side} pointer={} actual_wl_pointer_cursor=true sentinel_at_offset_40_40=#FF00FF feedback_requested={}",
            pointer.id().protocol_id(),
            self.presentation.is_some()
        );
        Ok(())
    }
    fn sample(&self, feed: &capture::CaptureFeed) {
        let frames = feed.snapshot();
        for f in frames {
            let matches: Vec<_> = self
                .shields
                .iter()
                .filter(|s| s.origin == f.stream_position)
                .collect();
            let [s] = matches.as_slice() else {
                println!(
                    "SAMPLE_UNMATCHED stream={} origin={:?} matches={}",
                    f.stream_id,
                    f.stream_position,
                    matches.len()
                );
                continue;
            };
            if f.stream_logical_size != s.size {
                println!(
                    "GEOMETRY_MISMATCH portal={:?} Wayland={:?}; Wayland authoritative",
                    f.stream_logical_size, s.size
                );
            }
            let physical = (f.frame.width, f.frame.height);
            let cursor_age = f
                .cursor_updated_at
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_millis());
            let image_age = f.received_at.elapsed().ok().map(|d| d.as_millis());
            // Prefer independently observed wl_pointer local position. Metadata is also logged.
            let local = self
                .active_surface
                .as_ref()
                .filter(|surf| *surf == s.layer.wl_surface())
                .map(|_| self.position);
            let fallback = f.pointer.map(|p| {
                (
                    p.x as f64 * s.size.0 as f64 / physical.0 as f64,
                    p.y as f64 * s.size.1 as f64 / physical.1 as f64,
                )
            });
            let Some(pos) = local.or(fallback) else {
                println!(
                    "SAMPLE_NO_POINTER stream={} image_age_ms={image_age:?}",
                    f.stream_id
                );
                continue;
            };
            for offset in [0., 40.] {
                let point = logical_pixel((pos.0 + offset, pos.1 + offset), s.size, physical);
                let color = point.and_then(|(x, y)| {
                    f.frame
                        .pixels
                        .get(y as usize * physical.0 as usize + x as usize)
                });
                println!(
                    "SAMPLE stream={} pointer_logical={pos:?} pointer_source={} meta_pointer={:?} offset={offset} physical={point:?} rgba={color:?} image={}x{} image_age_ms={image_age:?} cursor_age_ms={cursor_age:?} pts={:?}",
                    f.stream_id,
                    if local.is_some() {
                        "wl_pointer"
                    } else {
                        "metadata"
                    },
                    f.pointer,
                    physical.0,
                    physical.1,
                    f.presentation_time_ns
                );
            }
        }
    }
    fn cleanup(&mut self) {
        // Do not use a button serial to hide the cursor. Unmapping/destroying its role
        // surface and releasing pointers lets the compositor restore the next client.
        self.cursor.attach(None, 0, 0);
        self.cursor.commit();
        for (_, p) in self.pointers.drain(..) {
            if p.version() >= 3 {
                p.release();
            }
        }
        let count = self.shields.len();
        for s in self.shields.drain(..) {
            s.layer.wl_surface().attach(None, 0, 0);
            s.layer.commit();
            s.viewport.destroy();
            drop(s.layer);
        }
        self.cursor.destroy();
        self.buffers.clear();
        println!(
            "CLEANUP layers_dropped={count} cursor_surface_destroyed=true pointers_released=true keyboard_objects=0"
        );
    }
}

fn logical_pixel(p: (f64, f64), logical: (i32, i32), physical: (u32, u32)) -> Option<(u32, u32)> {
    if !p.0.is_finite()
        || !p.1.is_finite()
        || logical.0 <= 0
        || logical.1 <= 0
        || p.0 < 0.
        || p.1 < 0.
        || p.0 >= logical.0 as f64
        || p.1 >= logical.1 as f64
    {
        return None;
    }
    let x = (p.0 * physical.0 as f64 / logical.0 as f64).floor() as u32;
    let y = (p.1 * physical.1 as f64 / logical.1 as f64).floor() as u32;
    (x < physical.0 && y < physical.1).then_some((x, y))
}
fn paint_sentinel(canvas: &mut [u8], side: usize) {
    canvas.fill(0);
    for y in 16..80 {
        for x in 16..80 {
            let color = if (x / 8 + y / 8) % 2 == 0 {
                [255, 0, 255, 255]
            } else {
                [80, 0, 80, 255]
            };
            let i = (y * side + x) * 4;
            canvas[i..i + 4].copy_from_slice(&color);
        }
    }
    // Pixel-font TEST label (white, 2x) near the top, outside offset 40,40.
    for (n, glyph) in [
        [7u8, 2, 2, 2, 2],
        [7, 4, 6, 4, 7],
        [7, 4, 7, 1, 7],
        [7, 2, 2, 2, 2],
    ]
    .iter()
    .enumerate()
    {
        for (y, bits) in glyph.iter().enumerate() {
            for x in 0..3 {
                if bits & (1 << (2 - x)) != 0 {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let i = ((19 + y * 2 + dy) * side + 25 + n * 8 + x * 2 + dx) * 4;
                            canvas[i..i + 4].copy_from_slice(&[255; 4]);
                        }
                    }
                }
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for App {
    fn event(
        app: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        println!("POINTER id={} event={event:?}", pointer.id().protocol_id());
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                app.active_surface = Some(surface);
                app.position = (surface_x, surface_y);
                if let Err(e) = app.show_cursor(pointer, serial, qh) {
                    app.failure = Some(e.to_string());
                    app.quit = true;
                }
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => app.position = (surface_x, surface_y),
            wl_pointer::Event::Leave { .. } => app.active_surface = None,
            wl_pointer::Event::Button { button, state, .. } => {
                let key = (pointer.id().protocol_id(), button);
                match state {
                    WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                        app.pressed.insert(key);
                    }
                    WEnum::Value(wl_pointer::ButtonState::Released) => {
                        let paired = app.pressed.remove(&key);
                        if paired && [LEFT, RIGHT].contains(&button) && app.selected.is_none() {
                            app.selected = Some(button);
                            println!(
                                "SELECTED_RELEASE button={button} action={} (observed press+release, not yet proof of no clickthrough)",
                                if button == LEFT { "select" } else { "cancel" }
                            );
                        }
                        // Keep shield mapped until every observed held button is released.
                        if app.selected.is_some() && app.pressed.is_empty() {
                            app.quit = true;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        println!("SEAT capability={cap:?} (only Pointer is bound)");
        if cap == Capability::Pointer {
            let p = seat.get_pointer(qh, ());
            self.pointers.push((seat, p));
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Pointer && self.pointers.iter().any(|(s, _)| s == &seat) {
            self.failure = Some("pointer capability removed".into());
            self.quit = true;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {
        self.failure = Some("seat removed".into());
        self.quit = true;
    }
}
impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.failure = Some("layer closed".into());
        self.quit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        c: LayerSurfaceConfigure,
        _: u32,
    ) {
        let Some(s) = self.shields.iter_mut().find(|s| s.layer == *layer) else {
            return;
        };
        println!(
            "SHIELD_CONFIGURE origin={:?} size={:?}",
            s.origin, c.new_size
        );
        if (c.new_size.0 != 0 && c.new_size.0 != s.size.0 as u32)
            || (c.new_size.1 != 0 && c.new_size.1 != s.size.1 as u32)
        {
            self.failure = Some("configured shield does not cover output".into());
            self.quit = true;
            return;
        }
        match self.pool.create_buffer(1, 1, 4, wl_shm::Format::Argb8888) {
            Ok((b, canvas)) => {
                canvas.fill(0);
                if let Err(e) = b.attach_to(layer.wl_surface()) {
                    self.failure = Some(e.to_string());
                    self.quit = true;
                    return;
                }
                layer.wl_surface().damage_buffer(0, 0, 1, 1);
                layer.commit();
                self.buffers.push(b);
                s.configured = true;
                println!("SHIELD_MAPPED transparent=true full_input=true keyboard=None");
            }
            Err(e) => {
                self.failure = Some(e.to_string());
                self.quit = true;
            }
        }
    }
}
impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if !self.shields.is_empty() {
            self.failure = Some("output added during probe".into());
            self.quit = true;
        }
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if !self.shields.is_empty() {
            self.failure = Some("output configuration changed during probe".into());
            self.quit = true;
        }
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.failure = Some("output removed".into());
        self.quit = true;
    }
}
impl CompositorHandler for App {
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
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
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
impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}
delegate_registry!(App);
smithay_client_toolkit::delegate_dispatch2!(App);
impl Dispatch<wl_callback::WlCallback, ()> for App {
    fn event(
        _app: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        println!("CURSOR_FRAME_CALLBACK {event:?}; not presentation proof");
    }
}
impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, ()> for App {
    fn event(
        app: &mut Self,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        println!("CURSOR_PRESENTATION {event:?}");
        match event {
            wp_presentation_feedback::Event::Presented { .. } => app.feedback_presented += 1,
            wp_presentation_feedback::Event::Discarded => app.feedback_discarded += 1,
            _ => {}
        }
    }
}
wayland_client::delegate_noop!(App: ignore wp_presentation::WpPresentation);
wayland_client::delegate_noop!(App: ignore wp_viewporter::WpViewporter);
wayland_client::delegate_noop!(App: ignore wp_viewport::WpViewport);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_uses_wayland_fractional_geometry() {
        assert_eq!(
            logical_pixel((40., 40.), (1646, 1029), (2880, 1800)),
            Some((69, 69))
        );
        assert_eq!(
            logical_pixel((1645.99, 1028.99), (1646, 1029), (2880, 1800)),
            Some((2879, 1799))
        );
        assert_eq!(
            logical_pixel((1646., 40.), (1646, 1029), (2880, 1800)),
            None
        );
        assert_eq!(logical_pixel((-1., 40.), (1646, 1029), (2880, 1800)), None);
    }
    #[test]
    fn large_cursor_buffer_preserves_sentinel_coordinates() {
        let mut pixels = vec![0; 512 * 512 * 4];
        paint_sentinel(&mut pixels, 512);
        assert_eq!(
            &pixels[(40 * 512 + 40) * 4..(40 * 512 + 40) * 4 + 4],
            &[255, 0, 255, 255]
        );
        assert_eq!(
            &pixels[(300 * 512 + 300) * 4..(300 * 512 + 300) * 4 + 4],
            &[0, 0, 0, 0]
        );
    }
    #[test]
    fn sentinel_center_magenta_and_outside_transparent() {
        let mut pixels = vec![0; 80 * 80 * 4];
        paint_sentinel(&mut pixels, 80);
        assert_eq!(
            &pixels[(40 * 80 + 40) * 4..(40 * 80 + 40) * 4 + 4],
            &[255, 0, 255, 255]
        );
        assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
    }
}
