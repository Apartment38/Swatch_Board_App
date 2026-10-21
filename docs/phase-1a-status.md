# Phase 1A implementation status

**Status:** In progress; hard release gate not passed.

## Implemented

- Added a Rust workspace and `colorboard-picker-core` library with output geometry, transforms, RGBA frame types, and nearest-neighbor magnifier rasterization.
- Added an ashpd XDG ScreenCast session requesting monitor streams, multiple outputs, cursor metadata, and application-scoped authorization persistence through the portal.
- Added a PipeWire adapter that negotiates 32-bit formats across a frame-size range, requests cursor metadata buffers after format negotiation, handles row stride and channel order, rejects malformed buffers, and reads cursor and timing metadata.
- Added a Smithay Client Toolkit Wayland layer-shell surface with `KeyboardInteractivity::None` and an explicitly empty pointer input region. It renders a 15×15 nearest-neighbor source-pixel lens at 10× and a live hex readout.
- Connected portal stream origins to Wayland outputs, derives stream-to-logical scale from the captured frame dimensions, and positions the lens around cursor metadata.
- Wired startup to wait for a frame and a visible cursor-driven lens before calling KWin's `org.kde.kwin.ColorPicker.pick()`. Startup now times out and cleans up when cursor metadata is unavailable. KWin's QRgb response is decoded and reported.
- Added stop channels for orderly PipeWire and overlay shutdown after a selection or startup failure.

## Live environment observations

- `cargo fmt --all` and `cargo check --offline -p colorboard-picker` pass. The check reports unused PipeWire timestamp fields; timestamp synchronization is not implemented.
- On KDE Wayland with KWin 6.8.0 and PipeWire 1.6.9, the portal authorized capture and PipeWire negotiated `BGRA 2880x1800`; the stream entered `Streaming` and delivered a frame.
- Before the cursor metadata buffer request was added, the first captured frame had `pointer=None`; the cursor-driven lens did not become ready and KWin selection was not started. A follow-up run was started to validate the request, but remained at portal authorization until the 35-second run limit. The cursor metadata fix is therefore not yet confirmed at runtime.
- The ScreenCast portal now requests `PersistMode::Application`. KDE controls whether it can remember the grant and may still ask for approval; this has not been exercised since the change.
- A single 1.75× display was available (`1646x1029` logical, `2880x1800` captured). Multi-output, negative-origin, rotated-output, cursor movement, focus, pointer routing, color agreement, cancellation, feedback recursion, and normal completion cleanup have not been validated.

## Remaining Phase 1A work

1. Complete a live run with the cursor metadata request, diagnose any remaining absence of usable cursor positions, then demonstrate live lens movement over current desktop pixels.
2. Compare the highlighted center pixel with KWin's selected result and verify confirmation/cancellation, focus preservation, no click-through, and no capture feedback.
3. Validate required scaling and output geometries, including multiple outputs, negative origins, and rotation, on suitable live configurations.
4. Verify successful and canceled selection both promptly release capture, overlay, and portal resources; record observed results here.

Do not mark Phase 1A complete or start Phase 1B until every hard exit criterion in `colorboard-agent-implementation-plan.md` passes on a real Plasma Wayland session.
