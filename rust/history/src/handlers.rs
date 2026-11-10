use super::*;
impl Dispatch<wl_pointer::WlPointer, ()> for App {
    fn event(
        app: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                if let Some(i) = app
                    .shields
                    .iter()
                    .position(|s| *s.layer.wl_surface() == surface)
                {
                    app.show_cursor(pointer, serial);
                    app.active = Some(i);
                    if !app.started {
                        app.selected_output = Some(i);
                    }
                    POSITION.with(|p| p.set((surface_x, surface_y)));
                }
            }
            wl_pointer::Event::Leave { .. } => app.active = None,
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => POSITION.with(|p| p.set((surface_x, surface_y))),
            wl_pointer::Event::Button {
                button: 0x110,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } if app.started => {
                app.pressed = Some(
                    app.active
                        .and_then(|i| POSITION.with(|p| app.hit(i, p.get()))),
                );
            }
            wl_pointer::Event::Button {
                button: 0x110,
                state: WEnum::Value(wl_pointer::ButtonState::Released),
                ..
            } if app.started => {
                if let Some(start) = app.pressed.take() {
                    let end = app
                        .active
                        .and_then(|i| POSITION.with(|p| app.hit(i, p.get())));
                    match (start, end) {
                        (None, _) => app.exit = true,
                        (Some(a), Some(b)) if a == b && a != usize::MAX => app.copy(a),
                        _ => {}
                    }
                }
            }
            wl_pointer::Event::Axis {
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value,
                ..
            } if app.started => app.scroll(value),
            _ => {}
        }
    }
}
thread_local! { static POSITION: std::cell::Cell<(f64,f64)> = const { std::cell::Cell::new((0.0,0.0)) }; }
impl Dispatch<wl_keyboard::WlKeyboard, ()> for App {
    fn event(
        app: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } if app.started => app.key(key),
            wl_keyboard::Event::Leave { .. } if app.started => app.exit = true,
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
        match cap {
            Capability::Pointer => {
                let p = seat.get_pointer(qh, ());
                if let Some(manager) = &self.cursor_shapes {
                    self.shape_devices
                        .push((p.clone(), manager.get_shape_device(&p, qh)));
                }
                self.pointers.push((seat, p));
            }
            Capability::Keyboard => {
                let k = seat.get_keyboard(qh, ());
                self.keyboards.push((seat, k));
            }
            _ => {}
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        match cap {
            Capability::Pointer => {
                if let Some(i) = self.pointers.iter().position(|(s, _)| *s == seat) {
                    if let Some(index) = self
                        .shape_devices
                        .iter()
                        .position(|(p, _)| *p == self.pointers[i].1)
                    {
                        self.shape_devices.swap_remove(index).1.destroy();
                    }
                    self.pointers.swap_remove(i).1.release();
                }
            }
            Capability::Keyboard => {
                if let Some(i) = self.keyboards.iter().position(|(s, _)| *s == seat) {
                    self.keyboards.swap_remove(i).1.release();
                }
            }
            _ => {}
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {
        if self.started {
            self.exit = true;
        }
    }
}
impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.exit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        c: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(i) = self.shields.iter().position(|s| s.layer == *layer) {
            let s = &mut self.shields[i];
            s.width = c.new_size.0;
            s.height = c.new_size.1;
            if s.width == 0 || s.height == 0 {
                self.exit = true;
                return;
            }
            s.configured = true;
            self.draw(i);
        }
    }
}
impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if self.started {
            self.add_output(qh, output);
        }
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self.shields.iter().any(|s| s.output == output) {
            self.exit = true;
        }
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
