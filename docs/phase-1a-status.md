# Phase 1A implementation status

**Status:** In progress; hard release gate not passed.

## Implemented

- Added a Rust workspace and `colorboard-picker-core` library.
- Added core types for output geometry, transforms, physical pixels, tightly packed RGBA frames, and nearest-neighbor magnifier rasterization.
- The coordinate mapper accepts negative logical output origins and fractional scale factors. It does not yet establish that the portal's stream geometry and cursor metadata use the assumed mapping on KWin.
- Added a Rust picker binary with an ashpd XDG ScreenCast portal session. It requests monitor streams, multiple outputs, cursor metadata, and non-persistent authorization, and retains the portal session while capture is active.
- Added a PipeWire adapter using the official `pipewire` Rust bindings. It negotiates common 32-bit RGB formats, normalizes row stride and channel order, rejects corrupted or malformed buffers, and reads `SPA_META_Cursor` and `SPA_META_Header` when present.
- Added a zbus proxy for KWin's `org.kde.kwin.ColorPicker.pick()` method and QRgb decoding. The picker currently does not invoke it because the overlay is not implemented.

The picker binary currently waits for one frame and exits. It is an integration prototype, not a usable picker.

## Environment limits

The workspace has a Rust toolchain under `/workspace/.rustup` and `/workspace/.cargo`. `cargo check --offline -p colorboard-picker` passes with warnings for the not-yet-integrated UI and selection paths. `cargo test` and `cargo clippy` have passed for the picker core. Plasma/KWin runtime binaries are absent, `WAYLAND_DISPLAY` and `XDG_SESSION_TYPE` are unset, and the network currently cannot reach crates.io. The Wayland client crate needed for the overlay is not cached, so that implementation cannot be compiled in this environment.

## Remaining Phase 1A work

1. Implement a pointer-transparent, non-keyboard-interactive Wayland layer-shell overlay that follows live cursor metadata and renders the source pixels.
2. Integrate asynchronous KWin `org.kde.kwin.ColorPicker.pick()` invocation only after the overlay is visible, then coordinate selection/cancellation and clean shutdown.
3. Validate monitor-to-stream mapping and transforms on KWin; the current core mapper is not yet connected to portal geometry and the color-picker D-Bus result.
4. Verify pixel accuracy, live updates, feedback exclusion, focus, click routing, cancellation, portal permissions, and resource cleanup on an actual Plasma 6 Wayland session.

Until those steps pass, the picker must not be described or presented as functional, and no later phase should start.
