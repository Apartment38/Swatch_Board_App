# Phase 1A implementation status

**Status:** In progress; hard release gate not passed.

## Implemented

- Added a Rust workspace and `colorboard-picker-core` library with output geometry, transforms, RGBA frame types, and nearest-neighbor magnifier rasterization.
- Added an ashpd XDG ScreenCast session requesting monitor streams, multiple outputs, cursor metadata, and application-scoped authorization persistence through the portal.
- Added a PipeWire adapter that negotiates 32-bit formats across a frame-size range, requests cursor and header metadata buffers after format negotiation, handles row stride and channel order, rejects malformed buffers, and reads cursor and timing metadata.
- Added a Smithay Client Toolkit Wayland layer-shell surface with `KeyboardInteractivity::None` and an explicitly empty pointer input region. It renders a 15×15 nearest-neighbor source-pixel lens at 10× and a live hex readout.
- Connected portal stream origins to Wayland outputs, derives stream-to-logical scale from the captured frame dimensions, and positions the lens around cursor metadata.
- Wired startup to wait for a frame and a visible cursor-driven lens before calling KWin's `org.kde.kwin.ColorPicker.pick()`. Startup now times out and cleans up when cursor metadata is unavailable. The proxy explicitly uses KWin's lowercase D-Bus member name `pick` and decodes its `(u)` QRgb response.
- Added stop channels for orderly PipeWire and overlay shutdown after a selection or startup failure.

## Live environment observations

- `cargo fmt --all`, `cargo check --offline -p colorboard-picker`, and `cargo test --offline --workspace` pass (4 core tests). The app now reports the last displayed center sample, frame age, presentation timestamp, and whether its RGB matches KWin's result. Timestamp skew correction is not implemented.
- On KDE Wayland with KWin 6.8.0 and PipeWire 1.6.9, the portal authorized capture and PipeWire negotiated `BGRA 2880x1800`; the stream entered `Streaming` and delivered frames.
- A live run on stream 120 delivered cursor position `(1836, 1496)`, reached the cursor-driven overlay ready state, and completed KWin selection with `#3A2F2D`. This verifies cursor metadata and the corrected KWin method on the current 1.75× display. The user has not yet confirmed the visual lens/readout or compared the displayed center sample with the returned value; a diagnostic comparison is now printed after each pick.
- The ScreenCast portal now requests `PersistMode::Application`. KDE controls whether it can remember the grant and may still ask for approval; this has not been exercised since the change.
- A single 1.75× display was available (`1646x1029` logical, `2880x1800` captured). Multi-output, negative-origin, rotated-output, cursor movement, focus, pointer routing, color agreement, cancellation, feedback recursion, and normal completion cleanup have not been validated.

## Remaining Phase 1A work

1. Run the new diagnostic and verify the lens visibly follows cursor movement over current desktop pixels; compare the reported last displayed center color with KWin's selected result on controlled colors.
2. Verify confirmation/cancellation, focus preservation, no click-through, and no capture feedback.
3. Validate required scaling and output geometries, including multiple outputs, negative origins, and rotation, on suitable live configurations.
4. Verify successful and canceled selection both promptly release capture, overlay, and portal resources; record observed results here.

For controlled solid-color samples, use `phase-1a-color-fixture.html` from the `docs` directory. The picker now prints the last displayed center color, frame age, PTS, and whether that color matched KWin's result.

Do not mark Phase 1A complete or start Phase 1B until every hard exit criterion in `colorboard-agent-implementation-plan.md` passes on a real Plasma Wayland session.
