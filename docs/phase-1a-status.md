# Phase 1A implementation status

**Status:** In progress; hard release gate not passed.

## Implemented

- Added a Rust workspace and `colorboard-picker-core` library with output geometry, transforms, RGBA frame types, and nearest-neighbor magnifier rasterization.
- Added an ashpd XDG ScreenCast session requesting monitor streams, multiple outputs, cursor metadata, and non-persistent authorization.
- Added a PipeWire adapter that negotiates 32-bit formats across a frame-size range, handles row stride and channel order, rejects malformed buffers, and reads cursor and timing metadata.
- Added a Smithay Client Toolkit Wayland layer-shell surface with `KeyboardInteractivity::None` and an explicitly empty pointer input region. It renders a 15×15 nearest-neighbor source-pixel lens at 10× and a live hex readout.
- Connected portal stream origins to Wayland outputs, derives stream-to-logical scale from the captured frame dimensions, and positions the lens around cursor metadata.
- Wired startup to wait for a frame and a visible cursor-driven lens before calling KWin's `org.kde.kwin.ColorPicker.pick()`. Startup now times out and cleans up when cursor metadata is unavailable. KWin's QRgb response is decoded and reported.
- Added stop channels for orderly PipeWire and overlay shutdown after a selection or startup failure.

## Live environment observations

- `cargo fmt --all` and `cargo check --offline -p colorboard-picker` pass. The check reports unused PipeWire timestamp fields; timestamp synchronization is not implemented.
- On KDE Wayland with KWin 6.8.0 and PipeWire 1.6.9, the portal authorized capture and PipeWire negotiated `BGRA 2880x1800`; the stream entered `Streaming` and delivered a frame.
- The first captured frame had `pointer=None`. The cursor-driven lens therefore did not become ready, and the KWin selection call was not started in that run. The new 10-second readiness timeout reports this condition and shuts down both workers.
- The ScreenCast portal is configured with `PersistMode::DoNot`, so it asks for capture approval on every launch. This is the current explicit privacy behavior.
- A single 1.75× display was available (`1646x1029` logical, `2880x1800` captured). Multi-output, negative-origin, rotated-output, cursor movement, focus, pointer routing, color agreement, cancellation, feedback recursion, and normal completion cleanup have not been validated.

## Remaining Phase 1A work

1. Diagnose why the advertised cursor metadata mode produced no usable cursor position, then demonstrate live lens movement over current desktop pixels.
2. Compare the highlighted center pixel with KWin's selected result and verify confirmation/cancellation, focus preservation, no click-through, and no capture feedback.
3. Validate required scaling and output geometries, including multiple outputs, negative origins, and rotation, on suitable live configurations.
4. Verify successful and canceled selection both promptly release capture, overlay, and portal resources; record observed results here.

Do not mark Phase 1A complete or start Phase 1B until every hard exit criterion in `colorboard-agent-implementation-plan.md` passes on a real Plasma Wayland session.
