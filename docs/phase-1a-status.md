# Phase 1A implementation status

**Status:** In progress; hard release gate not passed.

## Work started

- Added a Rust workspace and `colorboard-picker-core` library.
- Added core types for output geometry, transforms, physical pixels, tightly packed RGBA frames, and nearest-neighbor magnifier rasterization.
- The coordinate mapper accepts negative logical output origins and fractional scale factors. It does not yet establish that the portal's stream geometry and cursor metadata use the assumed mapping on KWin.

These are backend-independent primitives only. They do not capture the desktop, draw a Wayland surface, invoke KWin, or constitute a usable picker.

## Environment limits

The available workspace environment has no `rustc`, `cargo`, `rustup`, `kwin_wayland`, or `pw-cli` executable. `WAYLAND_DISPLAY` and `XDG_SESSION_TYPE` are unset. Consequently, no crate versions, Plasma/KWin/portal/PipeWire versions, build result, or real-desktop observations can be recorded here. The code has not been compiled.

## Remaining Phase 1A work

1. Install/use a Rust toolchain and pin dependencies in `Cargo.lock`.
2. Implement and compile the XDG ScreenCast portal + PipeWire frame/cursor adapter.
3. Implement and compile a pointer-transparent, non-keyboard-interactive Wayland layer-shell overlay.
4. Add asynchronous KWin `org.kde.kwin.ColorPicker.pick()` integration after capture and overlay startup.
5. Verify pixel mapping, transforms, cursor/frame timing, feedback exclusion, focus, click routing, cancellation, and resource cleanup on an actual Plasma 6 Wayland session.

Until those steps pass, the picker must not be described or presented as functional, and no later phase should start.
