# Phase 1A implementation status

**Status:** Blocked by demonstrated accuracy and lifecycle gaps; hard release gate not passed.

## Native investigation checkpoint

[Autonomous lab results and retained blockers](phase-1a-native-probes.md):
left/right shielding and idle-timeout cleanup preserved fixture focus and mouse
counters; a visible TEST cursor was excluded from capture. Cursor-role rendering
still produced zero genuine presentation acknowledgments. Separate Escape
activation/session-close pass-through evidence was recovered. Production has not
been switched, and Phase 1A remains blocked.

## Current production gate — blocked, not complete

The latest optimized live batch passed seven controlled static-color selections
and both cancellation paths, with no mouse click-through and completed-session
resource cleanup. The animated target **failed exact agreement**: KWin returned
cyan while the compositor-presented center sample was magenta. Current code now
fails closed on mismatched or missing displayed samples instead of reporting success.

The latest all-targets run passed 66 test executions, including module tests reused
in the shield example; see the native checkpoint for the breakdown. Latest-only frame delivery,
per-output cursor routing, physical-pixel viewporter rendering, presentation-feedback
tracking, bounded dispatch/shutdown, explicit portal closure, and adjustable
radius/zoom are implemented. Required multi-output/scaling configurations remain
live-unverified on the single physical 1.75× display. The separate two-output
virtual lab does not replace physical scaling or output-switching acceptance.

KWin has no remote cancellation operation for its independent selection grab;
mid-pick backend failure still needs Escape/right-click. The hard gate therefore
required an architectural decision. The user has now approved investigating
Rust-owned selection with the presented sample authoritative; see
[the decision](decisions/native-selection-authority.md). That path must still prove
Escape, focus, click safety and autonomous cleanup. It is not a Phase 1A pass.

See [the full live validation and blockers](phase-1a-live-validation.md) and its
raw controlled-fixture evidence. No tray/Board work has started. The old notes
below are retained as historical development context, not the current gate verdict.

## Historical baseline — rename and headless verification

- Renamed the live packages to `swatch-board-picker` and `swatch-board-picker-core`,
  the Rust library to `swatch_board_picker_core`, and runtime labels to Swatch Board.
  The current plan is `swatch-board-agent-implementation-plan.md`; the conversation
  export is preserved verbatim as a historical archive under its original name.
- Added `--help`/`-h` and `--version`/`-V` without portal/capture startup. Unknown or
  extra arguments fail before connecting to the portal.
- Reproduced and fixed invalid geometry: NaN desktop points and non-finite output
  origins/sizes previously mapped to pixel `(0, 0)` instead of returning `None`.
- The workspace now passes **18 automated tests**: 9 core tests, 5 capture decoder
  tests, and 4 CLI/branding integration tests. Decoder characterization covers all
  four negotiated channel orders, alpha, padded stride, malformed/truncated buffers,
  unsupported formats, and consecutive frames. The new geometry and CLI behavior
  tests were observed failing before their fixes.
- `cargo build --offline --workspace --locked`, `cargo test --offline --workspace --locked`,
  `cargo fmt --all -- --check`, and
  `cargo clippy --offline --workspace --all-targets --locked -- -D warnings` pass.
- Read-only live discovery confirms KWin 6.8.0 in Wayland mode, one 1646×1029 output
  at 1.75× scale, and portal cursor modes bitmask `7` (metadata available).
  This shell has session D-Bus but no `WAYLAND_DISPLAY`; `/run/user/1000/wayland-1`
  exists. This is discovery, **not** a new capture/selection acceptance run.
- KWin currently reports Dim Inactive loaded and active. Earlier exact-blue results
  were recorded with it disabled; do not assume that earlier controlled configuration
  still applies. No compositor settings were changed during this pass.

**Phase 1A remains incomplete.** Existing live results below are historical evidence,
not results rerun after that baseline pass. At that baseline, the overlay ignored
other streams. Current output switching is implemented; live cross-output validation
still remains outstanding, as documented above.
No tray or Board implementation has been started.

## Implemented

- Added a Rust workspace and `swatch-board-picker-core` library with output geometry, transforms, RGBA frame types, and nearest-neighbor magnifier rasterization.
- Added an ashpd XDG ScreenCast session requesting monitor streams, multiple outputs, cursor metadata, and application-scoped authorization persistence through the portal.
- Added a PipeWire adapter that negotiates 32-bit formats across a frame-size range, requests cursor and header metadata buffers after format negotiation, handles row stride and channel order, rejects malformed buffers, and reads cursor and timing metadata.
- Added a Smithay Client Toolkit Wayland layer-shell surface with `KeyboardInteractivity::None` and an explicitly empty pointer input region. It renders a 15×15 nearest-neighbor source-pixel lens at 10× and a live hex readout.
- Connected portal stream origins to Wayland outputs, derives stream-to-logical scale from the captured frame dimensions, and positions the lens beside the cursor so its visible pixels do not cover the sampled point.
- Wired startup to wait for a frame and a visible cursor-driven lens before calling KWin's `org.kde.kwin.ColorPicker.pick()`. Startup now times out and cleans up when cursor metadata is unavailable. The proxy explicitly uses KWin's lowercase D-Bus member name `pick` and decodes its `(u)` QRgb response.
- Added stop channels for orderly PipeWire and overlay shutdown after a selection or startup failure.

## Live environment observations

- `cargo fmt --all` and `cargo check --offline -p swatch-board-picker` pass. `cargo test --offline -p swatch-board-picker-core` passes (7 tests), covering magnifier nearest-neighbor sampling, checkerboard/gradient samples, negative origins, fractional scaling, all 8 output transforms, and required 100/125/150/200% scales. The app reports the last displayed center sample, frame age, presentation timestamp, and whether its RGB matches KWin's result. Timestamp skew correction is not implemented.
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

Do not mark Phase 1A complete or start Phase 1B until every hard exit criterion in `swatch-board-agent-implementation-plan.md` passes on a real Plasma Wayland session.
