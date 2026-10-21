# Phase 1A implementation status

**Status:** In progress; hard release gate not passed.

## Implemented

- Added a Rust workspace and `colorboard-picker-core` library with output geometry, transforms, RGBA frame types, and nearest-neighbor magnifier rasterization.
- Added an ashpd XDG ScreenCast session requesting monitor streams, multiple outputs, cursor metadata, and application-scoped authorization persistence through the portal.
- Added a PipeWire adapter that negotiates 32-bit formats across a frame-size range, requests cursor and header metadata buffers after format negotiation, handles row stride and channel order, rejects malformed buffers, and reads cursor and timing metadata.
- Added a Smithay Client Toolkit Wayland layer-shell surface with `KeyboardInteractivity::None` and an explicitly empty pointer input region. It renders a 15×15 nearest-neighbor source-pixel lens at 10× and a live hex readout.
- Connected portal stream origins to Wayland outputs, derives stream-to-logical scale from the captured frame dimensions, and positions the lens beside the cursor so its visible pixels do not cover the sampled point.
- Wired startup to wait for a frame and a visible cursor-driven lens before calling KWin's `org.kde.kwin.ColorPicker.pick()`. Startup now times out and cleans up when cursor metadata is unavailable. The proxy explicitly uses KWin's lowercase D-Bus member name `pick` and decodes its `(u)` QRgb response.
- Added stop channels for orderly PipeWire and overlay shutdown after a selection or startup failure.

## Live environment observations

- `cargo fmt --all` and `cargo check --offline -p colorboard-picker` pass. `cargo test --offline -p colorboard-picker-core` passes (7 tests), covering magnifier nearest-neighbor sampling, checkerboard/gradient samples, negative origins, fractional scaling, all 8 output transforms, and required 100/125/150/200% scales. The app reports the last displayed center sample, frame age, presentation timestamp, and whether its RGB matches KWin's result. Timestamp skew correction is not implemented.
- On KDE Wayland with KWin 6.8.0 and PipeWire 1.6.9, the portal authorized capture and PipeWire negotiated `BGRA 2880x1800`; the stream entered `Streaming` and delivered frames.
- A live run on stream 120 delivered cursor position `(1836, 1496)`, reached the cursor-driven overlay ready state, and completed KWin selection with `#3A2F2D`. A later run over the `#0000FF` fixture returned `#272777`; the app's displayed center was `#282898`, with RGB mismatch and a 201 ms old frame. The user also observed the shade darkening toward black. This is consistent with PipeWire capturing the lens over the cursor and feeding those displayed pixels into later samples.
- The lens is positioned to the side of the cursor, leaving a gap between the sampled point and all visible lens/readout pixels. The later controlled blue run stayed at `#0000FF` and matched KWin, with no observed progressive darkening.
- After the user disabled KWin's Dim Inactive effect, a live run over the controlled `#0000FF` fixture returned `#0000FF` from KWin and from the displayed center sample (frame age 130 ms, `RGB match=true`). This confirms exact sample agreement on the current single 1.75× display and that the offset lens avoids the earlier progressive darkening in this run.
- A subsequent run canceled with Escape and KWin returned `org.kde.kwin.ColorPicker.Error.Cancelled`. This confirms Escape cancellation is reported through the normal cancellation path.
- The ScreenCast portal now requests `PersistMode::Application`. KDE controls whether it can remember the grant and may still ask for approval; this has not been exercised since the change.
- A single 1.75× display was available (`1646x1029` logical, `2880x1800` captured). The latest user runs verify controlled blue agreement and Escape cancellation. Multi-output, negative-origin, rotated-output, other scales, live cursor following as observed by the user, focus preservation, click routing/no click-through, right-click cancellation, animation/stale-frame behavior, and resource cleanup still need validation.

## Remaining Phase 1A work

1. Verify successful left-click confirmation and right-click cancellation, focus preservation, and no click-through while the picker is active.
2. Check live cursor following and sampling against additional controlled targets, including contrasting stripes/checkerboard and changing desktop content, to evaluate cursor/video timestamp skew.
3. Validate required scaling and output geometries, including multiple outputs, negative origins, and rotation, on suitable live configurations.
4. Verify successful and canceled selection both promptly release capture, overlay, and portal resources; also exercise portal denial/restoration and failure cleanup.

For controlled solid-color samples, use `phase-1a-color-fixture.html` from the `docs` directory. The picker now prints the last displayed center color, frame age, PTS, and whether that color matched KWin's result.

Do not mark Phase 1A complete or start Phase 1B until every hard exit criterion in `colorboard-agent-implementation-plan.md` passes on a real Plasma Wayland session.
