//! Native test-only fixture, based on SCTK 0.21.1's simple_window example.
//! Buffer coordinates equal surface-local logical coordinates (buffer scale 1).
//! No capture, fake input, permission handling, or shipped picker functionality.
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::{calloop::EventLoop, calloop_wayland_source::WaylandSource},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
    shm::{
        Shm, ShmHandler,
        slot::{Buffer, SlotPool},
    },
};
use std::{
    error::Error,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

const WIDTH: u32 = 900;
const HEIGHT: u32 = 650;
const PERIOD: Duration = Duration::from_millis(250);
const COLORS: [u32; 5] = [0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffffff, 0xff000000];

// Fixed coordinates intentionally do not scale on resize: logs always describe exact targets.
fn pixel(x: u32, y: u32, phase: bool) -> u32 {
    if (40..860).contains(&x) && (40..260).contains(&y) {
        return COLORS[((x - 40) / 164) as usize];
    }
    if (40..440).contains(&x) && (320..600).contains(&y) {
        return if ((x - 40) / 20 + (y - 320) / 20) % 2 == 0 {
            0xffffffff
        } else {
            0xff000000
        };
    }
    if (500..860).contains(&x) && (320..600).contains(&y) {
        return if phase { 0xffff00ff } else { 0xff00ffff };
    }
    // Distinct opaque gray background: cannot be confused with transparent picker overlay.
    0xff303030
}
fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn requested_output(args: &[String]) -> Result<Option<String>, &'static str> {
    match args {
        [] => Ok(None),
        [flag, name] if flag == "--fullscreen-output" && !name.is_empty() => Ok(Some(name.clone())),
        _ => Err("Usage: phase1a_fixture [--fullscreen-output EXACT_OUTPUT_NAME]"),
    }
}

#[cfg(test)]
mod fixture_cli_tests {
    use super::*;
    #[test]
    fn fullscreen_output_is_explicit_and_preserves_exact_name() {
        assert_eq!(requested_output(&[]).unwrap(), None);
        assert_eq!(
            requested_output(&["--fullscreen-output".into(), "Virtual-1".into()]).unwrap(),
            Some("Virtual-1".into())
        );
        assert!(requested_output(&["--fullscreen-output".into()]).is_err());
        assert!(requested_output(&["--fullscreen-output".into(), "".into()]).is_err());
        assert!(requested_output(&["--unknown".into()]).is_err());
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let fullscreen_output = requested_output(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let connection = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init(&connection)?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<Fixture> = EventLoop::try_new()?;
    WaylandSource::new(connection.clone(), queue).insert(event_loop.handle())?;
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shell = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let window = shell.create_window(
        compositor.create_surface(&qh),
        WindowDecorations::RequestServer,
        &qh,
    );
    window.set_title("Swatch Board Phase 1A Fixture");
    window.set_app_id("swatch-board-phase1a-fixture");
    if fullscreen_output.is_none() {
        window.set_min_size(Some((WIDTH, HEIGHT)));
    }
    let pool = SlotPool::new((WIDTH * HEIGHT * 4) as usize, &shm)?;
    window.commit();
    let mut fixture = Fixture {
        fullscreen_output,
        registry: RegistryState::new(&globals),
        seats: SeatState::new(&globals, &qh),
        outputs: OutputState::new(&globals, &qh),
        shm,
        pool,
        window,
        buffer: None,
        keyboard: None,
        pointer: None,
        focused: false,
        configured: false,
        exit: false,
        width: WIDTH,
        height: HEIGHT,
        started: Instant::now(),
        last_draw: Instant::now(),
        events: 0,
        presses: 0,
        releases: 0,
        motions: 0,
        frame: 0,
        keyboard_enters: 0,
        keyboard_leaves: 0,
    };
    println!(
        "FIXTURE start unix_ms={} title='Swatch Board Phase 1A Fixture' app_id=swatch-board-phase1a-fixture desired=900x650 opaque=true buffer_scale=1",
        unix_ms()
    );
    println!(
        "LAYOUT surface-local logical coordinates, half-open bounds; global origin must be measured externally (decorations excluded)"
    );
    for (index, name) in ["red", "green", "blue", "white", "black"]
        .iter()
        .enumerate()
    {
        let left = 40 + index * 164;
        println!(
            "TARGET {name} rect=[{left},40,{},260) center=({},150) argb={:08X}",
            left + 164,
            left + 82,
            COLORS[index]
        );
    }
    println!(
        "TARGET checker rect=[40,320,440,600) cell=20 white_center=(50,330) black_center=(70,330)"
    );
    println!(
        "TARGET animation rect=[500,320,860,600) center=(680,460) cyan=FF00FFFF magenta=FFFF00FF alternates_every_ms=250"
    );
    // Timed dispatch keeps commits progressing even when picker covers the fixture.
    // Commit logs prove client updates, not presentation; FRAME_CALLBACK logs provide compositor feedback.
    while !fixture.exit {
        event_loop.dispatch(Duration::from_millis(16), &mut fixture)?;
        if let Some(name) = fixture.fullscreen_output.as_ref() {
            let output = fixture
                .outputs
                .outputs()
                .find(|o| fixture.outputs.info(o).and_then(|i| i.name).as_ref() == Some(name));
            if let Some(output) = output {
                println!("FULLSCREEN_REQUEST output={name}");
                fixture.window.set_fullscreen(Some(&output));
                fixture.window.commit();
                fixture.fullscreen_output = None;
            } else if fixture.started.elapsed() > Duration::from_secs(5) {
                return Err(format!("Requested output not found: {name}").into());
            }
        }
        if fixture.configured && fixture.last_draw.elapsed() >= PERIOD && !fixture.exit {
            fixture.draw(&qh);
        }
    }
    fixture.log("shutdown");
    Ok(())
}

struct Fixture {
    fullscreen_output: Option<String>,
    registry: RegistryState,
    seats: SeatState,
    outputs: OutputState,
    shm: Shm,
    pool: SlotPool,
    window: Window,
    buffer: Option<Buffer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    focused: bool,
    configured: bool,
    exit: bool,
    width: u32,
    height: u32,
    started: Instant,
    last_draw: Instant,
    events: u64,
    presses: u64,
    releases: u64,
    motions: u64,
    frame: u64,
    keyboard_enters: u64,
    keyboard_leaves: u64,
}
impl Fixture {
    fn log(&mut self, message: &str) {
        self.events += 1;
        println!(
            "EVENT n={} unix_ms={} elapsed_ms={} {message} focused={} presses={} releases={} motions={} keyboard_enters={} keyboard_leaves={}",
            self.events,
            unix_ms(),
            self.started.elapsed().as_millis(),
            self.focused,
            self.presses,
            self.releases,
            self.motions,
            self.keyboard_enters,
            self.keyboard_leaves
        );
    }
    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let phase = (self.started.elapsed().as_millis() / PERIOD.as_millis()) % 2 != 0;
        let width = self.width;
        let height = self.height;
        let buffer = self.buffer.get_or_insert_with(|| {
            self.pool
                .create_buffer(
                    width as i32,
                    height as i32,
                    width as i32 * 4,
                    wl_shm::Format::Argb8888,
                )
                .expect("fixture buffer")
                .0
        });
        let canvas = match self.pool.canvas(buffer) {
            Some(canvas) => canvas,
            None => {
                let (next, canvas) = self
                    .pool
                    .create_buffer(
                        width as i32,
                        height as i32,
                        width as i32 * 4,
                        wl_shm::Format::Argb8888,
                    )
                    .expect("fixture double buffer");
                *buffer = next;
                canvas
            }
        };
        for (index, bytes) in canvas.chunks_exact_mut(4).enumerate() {
            bytes.copy_from_slice(
                &pixel(index as u32 % width, index as u32 / width, phase).to_ne_bytes(),
            );
        }
        let surface = self.window.wl_surface();
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.frame(qh, FrameCallbackData(surface.clone()));
        buffer.attach_to(surface).expect("fixture attach");
        self.window.commit();
        self.frame += 1;
        self.last_draw = Instant::now();
        println!(
            "REDRAW commit={} unix_ms={} elapsed_ms={} size={}x{} animation_argb={:08X} focused={}",
            self.frame,
            unix_ms(),
            self.started.elapsed().as_millis(),
            width,
            height,
            if phase { 0xffff00ffu32 } else { 0xff00ffffu32 },
            self.focused
        );
    }
}
impl CompositorHandler for Fixture {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        factor: i32,
    ) {
        self.log(&format!("output_scale={factor} buffer_scale_remains=1"));
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        time: u32,
    ) {
        println!(
            "FRAME_CALLBACK unix_ms={} compositor_ms={time} latest_commit={}",
            unix_ms(),
            self.frame
        );
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
impl OutputHandler for Fixture {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
impl WindowHandler for Fixture {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.log("close_requested");
        self.exit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        self.width = configure.new_size.0.map(|n| n.get()).unwrap_or(WIDTH);
        self.height = configure.new_size.1.map(|n| n.get()).unwrap_or(HEIGHT);
        self.buffer = None;
        self.configured = true;
        self.log(&format!(
            "configure size={}x{} state={:?}",
            self.width, self.height, configure.state
        ));
        self.draw(qh);
    }
}
impl SeatHandler for Fixture {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = Some(
                self.seats
                    .get_keyboard(qh, &seat, None)
                    .expect("fixture keyboard"),
            );
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = Some(self.seats.get_pointer(qh, &seat).expect("fixture pointer"));
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(keyboard) = self.keyboard.take() {
                keyboard.release();
            }
            self.focused = false;
            self.log("keyboard_removed");
        }
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}
impl KeyboardHandler for Fixture {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        if surface == self.window.wl_surface() {
            self.focused = true;
            self.keyboard_enters += 1;
            self.log("keyboard_enter");
        }
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if surface == self.window.wl_surface() {
            self.focused = false;
            self.keyboard_leaves += 1;
            self.log("keyboard_leave");
        }
    }
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.log(&format!("key_press {event:?}"));
        if self.focused && event.keysym == Keysym::Escape {
            self.exit = true;
        }
    }
    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.log(&format!("key_repeat {event:?}"));
    }
    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.log(&format!("key_release {event:?}"));
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
    }
}
impl PointerHandler for Fixture {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.window.wl_surface() {
                continue;
            }
            let kind = match event.kind {
                PointerEventKind::Enter { .. } => "pointer_enter".to_owned(),
                PointerEventKind::Leave { .. } => "pointer_leave".to_owned(),
                PointerEventKind::Motion { .. } => {
                    self.motions += 1;
                    "motion".to_owned()
                }
                PointerEventKind::Press { button, .. } => {
                    self.presses += 1;
                    format!("button_press code={button}")
                }
                PointerEventKind::Release { button, .. } => {
                    self.releases += 1;
                    format!("button_release code={button}")
                }
                PointerEventKind::Axis { .. } => "axis".to_owned(),
            };
            self.log(&format!(
                "{kind} local_x={:.3} local_y={:.3}",
                event.position.0, event.position.1
            ));
        }
    }
}
impl ShmHandler for Fixture {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
delegate_registry!(Fixture);
impl ProvidesRegistryState for Fixture {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState,];
}
smithay_client_toolkit::delegate_dispatch2!(Fixture);
