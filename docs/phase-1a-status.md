# Phase 1A implementation status

**Status:** In progress; hard release gate not passed.

## Implemented

- Added a Rust workspace and `colorboard-picker-core` library with output geometry, transforms, RGBA frame types, and nearest-neighbor magnifier rasterization.
- Added an ashpd XDG ScreenCast session requesting monitor streams, multiple outputs, cursor metadata, and non-persistent authorization.
- Added a PipeWire adapter that negotiates common 32-bit formats, handles row stride and channel order, rejects malformed buffers, and reads `SPA_META_Cursor` and `SPA_META_Header`.
- Added a Smithay Client Toolkit Wayland layer-shell surface with `KeyboardInteractivity::None` and an explicitly empty pointer input region. It renders a live 15×15 nearest-neighbor source-pixel lens at 10×, marks its center sample, and shows a compact live hex readout.
- Wired startup to wait for a frame with usable cursor metadata before showing the overlay and calling KWin's `org.kde.kwin.ColorPicker.pick()`. The KWin QRgb response is decoded and reported.
- Added a stop channel to the PipeWire loop so its objects can be released after a normal selection result.

The layer is an integration prototype, not a usable or validated picker. It is currently fixed in the upper-right corner; cursor metadata changes the sampled center pixel, but the lens does not yet follow the pointer. Portal stream geometry is not connected to Wayland output identity, scale, or transform. Capture-overlay recursion, cursor/frame synchronization, focus, click routing, agreement with KWin, and end-to-end cleanup have not been observed on screen.

## Environment and build

The environment reports `XDG_CURRENT_DESKTOP=KDE` and `WAYLAND_DISPLAY=wayland-1`. The `kwin_wayland` binary reports 6.8.0; PipeWire reports 1.6.9; Rust is 1.99.1; and Smithay Client Toolkit is 0.21.1. The sandbox cannot connect to the desktop user's D-Bus session, so a running compositor/portal was not confirmed. `plasmashell --version` aborted, so the Plasma version is not recorded. Crates.io access and the formerly missing Wayland dependency are now available.

`cargo fmt --all` and `cargo check -p colorboard-picker` pass. The check emits one dead-code warning for captured frame timestamp fields that are not yet used for synchronization. Tests, Clippy, and the picker itself were not run during this continuation.

## Remaining Phase 1A work

1. Make the lens follow the cursor and associate portal streams with the correct Wayland outputs; apply geometry, scale, transform, and hotspot correctly.
2. Resolve cursor/frame timestamp pairing and capture-overlay recursion; compare the exact magnified center pixel against KWin's returned color.
3. Verify focus, pointer routing, confirmation/cancellation, portal behavior, and orderly resource cleanup on the live Plasma Wayland session.
4. Record Plasma and portal versions and observed results here. Do not mark Phase 1A complete or start Phase 1B unless every hard exit criterion in the implementation plan passes.
