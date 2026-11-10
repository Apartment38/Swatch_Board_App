//! Isolated normal-layer magnifier experiment. NOT production or a Phase 1A pass.
#[path = "../../rust/picker/src/capture.rs"]
mod capture;
mod guard;
#[path = "../../rust/picker/src/portal.rs"]
mod portal;

use guard::{Guard, Rect, Ticket};
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
    io::Write,
    sync::mpsc,
    time::{Duration, Instant},
};
use swatch_board_picker_core::{PixelPoint, place_lens, render_magnifier};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

fn monotonic_ns() -> Result<u64> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes one initialized timespec through a valid pointer.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let secs = u64::try_from(ts.tv_sec)?;
    let nanos = u64::try_from(ts.tv_nsec)?;
    if nanos >= 1_000_000_000 {
        return Err("invalid monotonic nanoseconds".into());
    }
    secs.checked_mul(1_000_000_000)
        .and_then(|s| s.checked_add(nanos))
        .ok_or_else(|| "monotonic timestamp overflow".into())
}

#[cfg(test)]
mod clock_tests {
    #[test]
    fn native_clock_read_is_positive_and_nondecreasing() {
        let a = super::monotonic_ns().unwrap();
        let b = super::monotonic_ns().unwrap();
        assert!(a > 0 && b >= a);
    }
}

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const LEFT: u32 = 272;
const RIGHT: u32 = 273;
const SIDE: u32 = 160;
const RADIUS: u32 = 5;
const ZOOM: u32 = 12;
// Explicit flush makes logs usable even when the parent redirects stdout to a file.
macro_rules! log { ($($arg:tt)*) => {{ println!($($arg)*); let _ = std::io::stdout().flush(); }} }

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--run"] {
        log!(
            "Usage: phase1a_clean_overlay_probe --run\nHUMAN ScreenCast chooser; PREPARE 3s, then 12s active. Separate full pointer shields, no keyboard objects or Escape. Left/right complete release selects/cancels. NOT production or Phase1A acceptance."
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
    let worker_feed = feed.clone();
    let worker =
        std::thread::spawn(move || capture::run(pipewire_fd, streams, worker_feed, stop_rx, None));
    let result = (|| -> Result<()> {
        let first = feed.wait_for_frame(Duration::from_secs(5))?;
        log!(
            "CAPTURE_READY stream={} image={}x{} portal_origin={:?} portal_size={:?} pts={:?}",
            first.stream_id,
            first.frame.width,
            first.frame.height,
            first.stream_position,
            first.stream_logical_size,
            first.presentation_time_ns
        );
        log!(
            "PREPARE 3s: activate native fixture NOW; no shield yet. No keyboard binding. Active timeout=12s."
        );
        std::thread::sleep(Duration::from_secs(3));
        run_overlay(feed, first, &worker)
    })();
    let _ = stop_tx.send(());
    let joined = worker.join();
    let closed = session.close().await;
    log!("CAPTURE_CLEANUP worker={joined:?} session_close={closed:?}");
    result?;
    match joined {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err("capture worker panicked".into()),
    }
    closed?;
    Ok(())
}

struct Shield {
    output: wl_output::WlOutput,
    layer: LayerSurface,
    viewport: wp_viewport::WpViewport,
    origin: (i32, i32),
    size: (i32, i32),
    configured: bool,
}
struct Lens {
    layer: LayerSurface,
    viewport: wp_viewport::WpViewport,
    configured: bool,
    buffer_side: u32,
}
#[derive(Clone, Debug)]
struct Feedback {
    ticket: Ticket,
    source: Option<PixelPoint>,
    video_pts: Option<u64>,
}
struct App {
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    shm: Shm,
    compositor: CompositorState,
    shell: LayerShell,
    viewporter: wp_viewporter::WpViewporter,
    presentation: wp_presentation::WpPresentation,
    clock_id: Option<u32>,
    shields: Vec<Shield>,
    lens: Option<Lens>,
    selected_output: usize,
    first: capture::CapturedFrame,
    pointers: Vec<(wl_seat::WlSeat, wl_pointer::WlPointer)>,
    active: Option<(u32, wl_surface::WlSurface)>,
    position: (f64, f64),
    pool: SlotPool,
    shield_buffers: Vec<Buffer>,
    lens_buffers: Vec<Buffer>,
    pressed: BTreeSet<(u32, u32)>,
    selected: bool,
    guard: Guard,
    pending: Option<(u64, Instant)>,
    hide_started: Option<Instant>,
    last_submit: Instant,
    quit: bool,
    failure: Option<String>,
}

fn run_overlay(
    feed: capture::CaptureFeed,
    first: capture::CapturedFrame,
    worker: &std::thread::JoinHandle<std::result::Result<(), String>>,
) -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let mut app = App {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        pool: SlotPool::new(1024 * 1024, &shm)?,
        shell: LayerShell::bind(&globals, &qh)?,
        viewporter: globals.bind(&qh, 1..=1, ())?,
        presentation: globals.bind(&qh, 1..=1, ())?,
        clock_id: None,
        compositor,
        shm,
        shields: vec![],
        lens: None,
        selected_output: 0,
        first,
        pointers: vec![],
        active: None,
        position: (0., 0.),
        shield_buffers: vec![],
        lens_buffers: vec![],
        pressed: BTreeSet::new(),
        selected: false,
        guard: Guard::default(),
        pending: None,
        hide_started: None,
        last_submit: Instant::now() - Duration::from_secs(1),
        quit: false,
        failure: None,
    };
    let result = (|| -> Result<()> {
        queue.roundtrip(&mut app)?;
        queue.roundtrip(&mut app)?;
        if app.clock_id != Some(1) {
            return Err("wp_presentation must announce Linux CLOCK_MONOTONIC (1)".into());
        }
        app.create_surfaces(&qh)?;
        let mut event_loop: EventLoop<App> = EventLoop::try_new()?;
        WaylandSource::new(conn.clone(), queue).insert(event_loop.handle())?;
        let start = Instant::now();
        while !app.quit && start.elapsed() < Duration::from_secs(12) {
            event_loop.dispatch(Duration::from_millis(16), &mut app)?;
            if app.quit {
                break;
            }
            if let Some(error) = feed.failure() {
                return Err(error.into());
            }
            if worker.is_finished() {
                return Err("capture worker stopped during probe".into());
            }
            app.tick(&feed, &qh)?;
            conn.flush()?;
        }
        if !app.pressed.is_empty() {
            log!(
                "WARNING bounded timeout/error with buttons held: click safety NOT established held={:?}",
                app.pressed
            );
        }
        log!(
            "END reason={} selected={} history={}",
            if app.quit {
                "release_or_error"
            } else {
                "12s_timeout"
            },
            app.selected,
            app.guard.history.len()
        );
        if let Some(error) = &app.failure {
            return Err(error.clone().into());
        }
        Ok(())
    })();
    app.cleanup();
    let flushed = conn.flush();
    log!(
        "WAYLAND_CLEANUP flush={flushed:?} result={:?}",
        result.as_ref().err().map(|e| e.to_string())
    );
    result
}

impl App {
    fn fail(&mut self, message: impl Into<String>) {
        let message = message.into();
        log!("FAIL_CLOSED {message}");
        self.failure = Some(message);
        self.quit = true;
    }
    fn create_surfaces(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        let outputs: Vec<_> = self.outputs.outputs().collect();
        if outputs.is_empty() {
            return Err("no outputs".into());
        }
        let mut matches = vec![];
        for (index, output) in outputs.into_iter().enumerate() {
            let info = self.outputs.info(&output).ok_or("missing output info")?;
            let origin = info
                .logical_position
                .ok_or("logical output origin required")?;
            let size = info
                .logical_size
                .filter(|s| s.0 > 0 && s.1 > 0)
                .ok_or("logical output size required")?;
            if origin == self.first.stream_position {
                if info.transform != wl_output::Transform::Normal {
                    return Err("transformed selected output unsupported".into());
                }
                if size != self.first.stream_logical_size {
                    return Err("portal/Wayland selected geometry mismatch".into());
                }
                matches.push(index);
            }
            log!(
                "OUTPUT name={:?} origin={origin:?} logical={size:?}",
                info.name
            );
            let surface = self.compositor.create_surface(qh);
            let viewport = self.viewporter.get_viewport(&surface, qh, ());
            viewport.set_destination(size.0, size.1);
            let layer = self.shell.create_layer_surface(
                qh,
                surface,
                Layer::Overlay,
                Some("swatch-clean-overlay-shield-TEST"),
                Some(&output),
            );
            layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            layer.set_size(size.0 as u32, size.1 as u32);
            layer.set_exclusive_zone(-1);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.wl_surface().set_input_region(None); // NULL = full infinite input region.
            layer.commit();
            self.shields.push(Shield {
                output,
                layer,
                viewport,
                origin,
                size,
                configured: false,
            });
        }
        let [selected] = matches.as_slice() else {
            return Err("selected stream matches zero or ambiguous outputs".into());
        };
        self.selected_output = *selected;
        let s = &self.shields[*selected];
        let (w, h) = (self.first.frame.width, self.first.frame.height);
        if w == 0 || h == 0 || w > 16384 || h > 16384 {
            return Err("unsupported capture dimensions".into());
        }
        let sx = w as f64 / s.size.0 as f64;
        let sy = h as f64 / s.size.1 as f64;
        if (sx - sy).abs() > 0.01 || !(0.5..=8.).contains(&sx) {
            return Err("nonuniform/unsupported capture scale".into());
        }
        let buffer_side = (SIDE as f64 * sx).ceil() as u32;
        if !((RADIUS * 2 + 1) * ZOOM + 8..=2048).contains(&buffer_side) {
            return Err("physical lens buffer cannot fit magnifier".into());
        }
        let surface = self.compositor.create_surface(qh);
        let viewport = self.viewporter.get_viewport(&surface, qh, ());
        viewport.set_destination(SIDE as i32, SIDE as i32);
        let layer = self.shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("swatch-clean-overlay-lens-TEST"),
            Some(&s.output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer.set_size(SIDE, SIDE);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        let empty = Region::new(&self.compositor)?;
        layer.wl_surface().set_input_region(Some(empty.wl_region()));
        layer.commit();
        self.lens = Some(Lens {
            layer,
            viewport,
            configured: false,
            buffer_side,
        });
        log!(
            "LENS_REQUESTED normal_layer=true cursor_role=false keyboard=None empty_input=true buffer={buffer_side}"
        );
        Ok(())
    }
    fn tick(&mut self, feed: &capture::CaptureFeed, qh: &QueueHandle<Self>) -> Result<()> {
        if self
            .pending
            .is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(2))
        {
            return Err("presentation feedback timeout".into());
        }
        if self
            .hide_started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(3))
        {
            return Err("post-hide clean video timeout".into());
        }
        if !self.shields.iter().all(|s| s.configured)
            || !self.lens.as_ref().is_some_and(|l| l.configured)
        {
            return Ok(());
        }
        let frames = feed.snapshot();
        // This spike deliberately supports exactly one authorized output/stream.
        if frames.len() != 1 {
            return Err("exactly one capture stream required for this spike".into());
        }
        let f = &frames[0];
        if f.stream_id != self.first.stream_id
            || f.stream_position != self.first.stream_position
            || f.stream_logical_size != self.first.stream_logical_size
            || (f.frame.width, f.frame.height) != (self.first.frame.width, self.first.frame.height)
        {
            return Err("capture geometry/identity changed".into());
        }
        let pts = f
            .presentation_time_ns
            .ok_or("video PTS missing; fail closed")?;
        if f.received_at
            .elapsed()
            .map_or(true, |age| age > Duration::from_secs(2))
        {
            return Err("stale capture image".into());
        }
        if self.guard.hiding() {
            if !self.guard.clean_frame(Some(pts))? {
                return Ok(());
            }
            log!("GUARD_CLEAN_FRAME pts_ns={pts}");
            self.hide_started = None;
        }
        let Some((_, surface)) = &self.active else {
            return Ok(());
        }; // NEVER cursor metadata fallback.
        let s = &self.shields[self.selected_output];
        if surface != s.layer.wl_surface() {
            return Err("pointer is on unselected output".into());
        }
        let point = logical_pixel(self.position, s.size, (f.frame.width, f.frame.height))
            .ok_or("invalid shield pointer position")?;
        let sx = s.size.0 as f64 / f.frame.width as f64;
        let sy = s.size.1 as f64 / f.frame.height as f64;
        // Include every physical pixel read by render_magnifier; pad for rounding/filter edges.
        let x0 = ((point.x.saturating_sub(RADIUS) as f64 * sx).floor() as i32 - 2) + s.origin.0;
        let y0 = ((point.y.saturating_sub(RADIUS) as f64 * sy).floor() as i32 - 2) + s.origin.1;
        let x1 = (((point.x + RADIUS + 1).min(f.frame.width) as f64 * sx).ceil() as i32 + 2)
            + s.origin.0;
        let y1 = (((point.y + RADIUS + 1).min(f.frame.height) as f64 * sy).ceil() as i32 + 2)
            + s.origin.1;
        let protected = Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        };
        if self.guard.contaminated(protected) {
            // A transparent BUFFER keeps the layer mapped. Unmapping is not this barrier.
            let ticket = self.guard.hide()?;
            self.hide_started = Some(Instant::now());
            self.submit(
                qh,
                Feedback {
                    ticket,
                    source: None,
                    video_pts: Some(pts),
                },
                None,
                None,
            )?;
            log!(
                "GUARD_HIDE_SUBMIT commit={} generation={} mapped=true transparent_buffer=true",
                ticket.commit,
                ticket.generation
            );
            return Ok(());
        }
        if self.pending.is_some() || self.last_submit.elapsed() < Duration::from_millis(100) {
            return Ok(());
        }
        let logical_point = PixelPoint {
            x: self.position.0.floor() as u32,
            y: self.position.1.floor() as u32,
        };
        let gap = (RADIUS as f64 * sx.max(sy)).ceil() as u32 + 8;
        let pos = place_lens(
            (s.size.0 as u32, s.size.1 as u32),
            logical_point,
            (SIDE, SIDE),
            gap,
        )
        .ok_or("cannot place lens outside source neighborhood")?;
        let rect = Rect {
            x: s.origin.0 + pos.x as i32,
            y: s.origin.1 + pos.y as i32,
            w: SIDE as i32,
            h: SIDE as i32,
        };
        if rect.intersects(protected) {
            return Err("new lens intersects protected neighborhood".into());
        }
        let color = f.frame.pixel(point).ok_or("source pixel missing")?;
        let color = [color.r, color.g, color.b];
        let magnifier = render_magnifier(&f.frame, point, RADIUS, ZOOM)
            .ok_or("magnifier core rejected source")?;
        let ticket = self.guard.visible(rect, color)?;
        self.submit(
            qh,
            Feedback {
                ticket,
                source: Some(point),
                video_pts: Some(pts),
            },
            Some(pos),
            Some(&magnifier),
        )?;
        self.last_submit = Instant::now();
        log!(
            "LENS_SUBMIT commit={} rect=({},{},{},{}) source=({},{}) color=#{:02X}{:02X}{:02X} pts_ns={} generation={}",
            ticket.commit,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            point.x,
            point.y,
            color[0],
            color[1],
            color[2],
            pts,
            ticket.generation
        );
        Ok(())
    }
    fn submit(
        &mut self,
        qh: &QueueHandle<Self>,
        data: Feedback,
        pos: Option<PixelPoint>,
        magnifier: Option<&swatch_board_picker_core::Magnifier>,
    ) -> Result<()> {
        let lens = self.lens.as_ref().ok_or("lens missing")?;
        // Released buffers can be dropped; active slots are never overwritten.
        self.lens_buffers
            .retain(|b| b.canvas(&mut self.pool).is_none());
        if self.lens_buffers.len() >= 16 {
            return Err("active lens buffer bound exhausted".into());
        }
        let side = lens.buffer_side as usize;
        let (buffer, canvas) = self.pool.create_buffer(
            side as i32,
            side as i32,
            (side * 4) as i32,
            wl_shm::Format::Argb8888,
        )?;
        canvas.fill(0);
        if let Some(m) = magnifier {
            // Opaque background; real core pixels are drawn without interpolation in buffer space.
            for pixel in canvas.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[24, 24, 24, 255]);
            }
            let ox = (side - m.width as usize) / 2;
            let oy = (side - m.height as usize) / 2;
            for y in 0..m.height as usize {
                for x in 0..m.width as usize {
                    let c = m.pixels[y * m.width as usize + x];
                    let i = ((y + oy) * side + x + ox) * 4;
                    canvas[i..i + 4].copy_from_slice(&[c.b, c.g, c.r, 255]);
                }
            }
            // Thin center-pixel outline; do not overwrite its actual center color.
            let cx = ox + RADIUS as usize * ZOOM as usize;
            let cy = oy + RADIUS as usize * ZOOM as usize;
            for y in 0..ZOOM as usize {
                for x in 0..ZOOM as usize {
                    if x == 0 || y == 0 || x == ZOOM as usize - 1 || y == ZOOM as usize - 1 {
                        let i = ((cy + y) * side + cx + x) * 4;
                        canvas[i..i + 4].copy_from_slice(&[255; 4]);
                    }
                }
            }
        }
        if let Some(p) = pos {
            lens.layer.set_margin(p.y as i32, 0, 0, p.x as i32);
        }
        buffer.attach_to(lens.layer.wl_surface())?;
        lens.layer
            .wl_surface()
            .damage_buffer(0, 0, side as i32, side as i32);
        self.presentation
            .feedback(lens.layer.wl_surface(), qh, data.clone());
        lens.layer.commit();
        self.pending = Some((data.ticket.commit, Instant::now()));
        self.lens_buffers.push(buffer);
        Ok(())
    }
    fn cleanup(&mut self) {
        if !self.pressed.is_empty() {
            log!(
                "WARNING CLEANUP held_buttons={:?}; bounded exit is not proof of click safety",
                self.pressed
            );
        }
        for (_, p) in self.pointers.drain(..) {
            if p.version() >= 3 {
                p.release();
            }
        }
        if let Some(lens) = self.lens.take() {
            lens.layer.wl_surface().attach(None, 0, 0);
            lens.layer.commit();
            lens.viewport.destroy();
            lens.layer.wl_surface().destroy();
        }
        let count = self.shields.len();
        for s in self.shields.drain(..) {
            s.layer.wl_surface().attach(None, 0, 0);
            s.layer.commit();
            s.viewport.destroy();
            s.layer.wl_surface().destroy();
        }
        self.shield_buffers.clear();
        self.lens_buffers.clear();
        log!(
            "CLEANUP shields={count} lens_destroyed=true pointers_released=true keyboard_objects=0"
        );
    }
}

fn logical_pixel(p: (f64, f64), logical: (i32, i32), physical: (u32, u32)) -> Option<PixelPoint> {
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
    (x < physical.0 && y < physical.1).then_some(PixelPoint { x, y })
}
impl Dispatch<wl_pointer::WlPointer, ()> for App {
    fn event(
        app: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = pointer.id().protocol_id();
        match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                if app.active.as_ref().is_some_and(|(other, _)| *other != id) {
                    app.fail("multiple active pointers unsupported");
                    return;
                }
                app.active = Some((id, surface));
                app.position = (surface_x, surface_y);
                log!("POINTER_ENTER id={id} local=({surface_x},{surface_y}) source=wl_pointer");
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if app.active.as_ref().is_some_and(|(active, _)| *active == id) {
                    app.position = (surface_x, surface_y);
                }
            }
            wl_pointer::Event::Leave { .. } => {
                if app.active.as_ref().is_some_and(|(active, _)| *active == id) {
                    app.active = None;
                }
            }
            wl_pointer::Event::Button { button, state, .. } => {
                let key = (id, button);
                match state {
                    WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                        app.pressed.insert(key);
                    }
                    WEnum::Value(wl_pointer::ButtonState::Released) => {
                        let paired = app.pressed.remove(&key);
                        if paired && [LEFT, RIGHT].contains(&button) && !app.selected {
                            app.selected = true;
                            if button == RIGHT {
                                log!("SELECTED_RELEASE button={button} action=cancel");
                            } else if app.active.as_ref().is_some_and(|(_, surf)| {
                                surf == app.shields[app.selected_output].layer.wl_surface()
                            }) {
                                if let Some(c) = app.guard.sample() {
                                    log!(
                                        "SELECTED_RELEASE button={button} action=select color=#{:02X}{:02X}{:02X}",
                                        c[0],
                                        c[1],
                                        c[2]
                                    );
                                } else {
                                    log!(
                                        "SELECTED_RELEASE button={button} action=select no_sample fail"
                                    );
                                }
                            } else {
                                log!(
                                    "SELECTED_RELEASE button={button} action=select no_sample fail inactive_or_unselected_pointer"
                                );
                            }
                        }
                        // Never destroy on Press; consume every observed held-button release.
                        if app.selected && app.pressed.is_empty() {
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
impl Dispatch<wp_presentation::WpPresentation, ()> for App {
    fn event(
        app: &mut Self,
        _: &wp_presentation::WpPresentation,
        event: wp_presentation::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_presentation::Event::ClockId { clk_id } = event {
            app.clock_id = Some(clk_id);
            log!("PRESENTATION_CLOCK clock_id={clk_id} required_CLOCK_MONOTONIC=1");
            if clk_id != 1 {
                app.fail("incomparable presentation clock");
            }
        }
    }
}
impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, Feedback> for App {
    fn event(
        app: &mut Self,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        data: &Feedback,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                ..
            } => {
                if app.clock_id != Some(1) {
                    app.fail("presentation clock unverified");
                    return;
                }
                let seconds = ((tv_sec_hi as u64) << 32) | tv_sec_lo as u64;
                let Some(ns) = seconds
                    .checked_mul(1_000_000_000)
                    .and_then(|s| s.checked_add(tv_nsec as u64))
                else {
                    app.fail("presentation timestamp overflow");
                    return;
                };
                if tv_nsec >= 1_000_000_000 {
                    app.fail("invalid presentation nanoseconds");
                    return;
                }
                if app.pending.is_some_and(|(c, _)| c == data.ticket.commit) {
                    app.pending = None;
                }
                let receipt_ns = match monotonic_ns() {
                    Ok(t) => t,
                    Err(e) => {
                        app.fail(e.to_string());
                        return;
                    }
                };
                match app
                    .guard
                    .presented_after_receipt(data.ticket, ns, receipt_ns)
                {
                    Ok(true) => {
                        if let Some(c) = data.ticket.color {
                            log!(
                                "LENS_PRESENTED commit={} color=#{:02X}{:02X}{:02X} source={:?} video_pts={:?} presented_ns={ns} generation={}",
                                data.ticket.commit,
                                c[0],
                                c[1],
                                c[2],
                                data.source,
                                data.video_pts,
                                data.ticket.generation
                            );
                        } else {
                            log!(
                                "GUARD_HIDE_PRESENTED barrier_ns={receipt_ns} presented_ns={ns} commit={} generation={}",
                                data.ticket.commit,
                                data.ticket.generation
                            );
                        }
                    }
                    Ok(false) => log!(
                        "PRESENTATION_IGNORED commit={} stale_generation_or_hidden=true",
                        data.ticket.commit
                    ),
                    Err(e) => app.fail(e),
                }
            }
            wp_presentation_feedback::Event::Discarded => {
                if app.pending.is_some_and(|(c, _)| c == data.ticket.commit) {
                    app.pending = None;
                }
                log!(
                    "PRESENTATION_DISCARDED commit={} hidden={}",
                    data.ticket.commit,
                    data.ticket.color.is_none()
                );
                // A discarded hide is never sufficient to clear contamination.
                if data.ticket.color.is_none() {
                    app.fail("hide presentation discarded");
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
        if cap == Capability::Pointer {
            let p = seat.get_pointer(qh, ());
            self.pointers.push((seat, p));
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Pointer {
            self.fail("pointer capability removed");
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {
        self.fail("seat removed");
    }
}
impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.fail("layer closed");
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        c: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(lens) = self.lens.as_mut().filter(|l| l.layer == *layer) {
            if (c.new_size.0 != 0 && c.new_size.0 != SIDE)
                || (c.new_size.1 != 0 && c.new_size.1 != SIDE)
            {
                self.fail("lens configure geometry mismatch");
            } else {
                lens.configured = true;
            }
            return;
        }
        let Some(index) = self.shields.iter().position(|s| s.layer == *layer) else {
            self.fail("unknown configured layer");
            return;
        };
        let s = &self.shields[index];
        if (c.new_size.0 != 0 && c.new_size.0 != s.size.0 as u32)
            || (c.new_size.1 != 0 && c.new_size.1 != s.size.1 as u32)
        {
            self.fail("shield does not cover output");
            return;
        }
        if s.configured {
            return;
        }
        match self.pool.create_buffer(1, 1, 4, wl_shm::Format::Argb8888) {
            Ok((b, canvas)) => {
                canvas.fill(0);
                if let Err(e) = b.attach_to(layer.wl_surface()) {
                    self.fail(e.to_string());
                    return;
                }
                layer.wl_surface().damage_buffer(0, 0, 1, 1);
                layer.commit();
                self.shield_buffers.push(b);
                self.shields[index].configured = true;
                log!("SHIELD_MAPPED output={index} transparent=true full_input=true keyboard=None");
            }
            Err(e) => self.fail(e.to_string()),
        }
    }
}
impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if !self.shields.is_empty() {
            self.fail("output added");
        }
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if !self.shields.is_empty() {
            self.fail("output changed");
        }
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.fail("output removed");
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
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {} // NEVER presentation authority.
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
wayland_client::delegate_noop!(App: ignore wp_viewporter::WpViewporter);
wayland_client::delegate_noop!(App: ignore wp_viewport::WpViewport);
