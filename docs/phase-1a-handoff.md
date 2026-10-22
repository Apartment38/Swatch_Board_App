# Phase 1A handoff

> **Work stopped by user request.** The authoritative current checkpoint is
> [the stopped-work resume handoff](phase-1a-resume-handoff.md). It includes native
> integration, the successful two-output test, and the unresolved mixed-scale lab
> failure. The older checkpoints below are historical. Do not resume automatically.

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

## Earlier live observations

Phase 1A is **in progress; its hard exit gate has not passed**. PipeWire capture is live and delivers 2880×1800 BGRA frames on the current scaled KDE Wayland display. Requesting cursor metadata yields usable positions. The lens is positioned beside the cursor to avoid capture feedback. With KWin's Dim Inactive effect disabled, a live run over the `#0000FF` fixture returned exactly `#0000FF` from both KWin and the displayed center sample (frame age 130 ms, `RGB match=true`). A subsequent Escape run returned KWin's explicit `org.kde.kwin.ColorPicker.Error.Cancelled` result. These verify exact color agreement for that controlled target and Escape cancellation on the current display. Multi-output, pointer routing/focus, other input paths, dynamic content/timing, and cleanup checks remain.

The repo contains the `swatch-board-picker-core` geometry and magnifier library, plus a Rust `swatch-board-picker` using ashpd ScreenCast, PipeWire, KWin's ColorPicker D-Bus method, and a pointer-transparent/non-focusable SCTK layer surface. Stream origins are matched to Wayland output positions, and scale is derived from logical output and actual PipeWire frame dimensions. After format negotiation the app explicitly requests cursor and header metadata buffers, following PipeWire's [video stream example](https://docs.pipewire.org/video-play_8c-example.html).

## Commits

- `3b4c08c` — Add live Wayland magnifier overlay prototype

Follow-up commits `79ccff1`, `121eda5`, `90688f5`, and `64a6d23` add capture/overlay mapping, request portal permission persistence and cursor metadata buffers, and correct the KWin D-Bus method name. Later commits add sample-to-KWin diagnostics, the controlled-color fixture, and the cursor-offset feedback fix. The latest verification evidence is recorded above.

## Build and runtime

- `cargo fmt --all` passes.
- `cargo check --offline -p swatch-board-picker` passes. `cargo test --offline -p swatch-board-picker-core` passes (7 tests), covering nearest-neighbor rendering, checkerboard/gradient samples, negative origins, 100/125/150/200% scales, and all 8 output transforms. The picker logs its last displayed center sample, frame age, presentation timestamp, and RGB equality with KWin's returned color. Timestamp skew evaluation on changing live content remains outstanding.
- A real portal session was authorized; PipeWire negotiated `VideoFormat::BGRA 2880x1800` and delivered a frame.
- A subsequent live run received cursor position `(1836, 1496)` and KWin returned `#3A2F2D`.
- An initial run over the known `#0000FF` fixture reported KWin `#272777` versus displayed center `#282898`, with frame age 201 ms and `RGB match=false`; the user observed progressive darkening. The lens was moved to one side of the cursor to prevent recursive capture.
- A follow-up run with Dim Inactive disabled reported `#0000FF` from both KWin and the displayed center sample, with frame age 130 ms and `RGB match=true`. The original color discrepancy is resolved for the controlled blue fixture under this configuration.
- Escape cancellation was exercised. KWin returned `org.kde.kwin.ColorPicker.Error.Cancelled: Color picking got cancelled` and the picker process returned to the shell.
- The portal request now uses `PersistMode::Application` so KDE can offer to remember permission. Whether this stops repeat prompts depends on the portal and the user's choice; it has not been exercised since the change.

## Environment and remaining work

KWin reports 6.8.0, PipeWire 1.6.9, Rust 1.99.1, and SCTK 0.21.1. One 1.75× output is available at 1646×1029 logical / 2880×1800 captured. Multi-monitor, negative-origin, rotated-output, left-click confirmation, right-click cancellation, focus, click routing/no click-through, changing content/timestamp behavior, and completed-session cleanup remain unverified.

Next verify left-click confirmation, right-click cancellation, focus preservation, click routing/no click-through, live cursor following on changing targets, live additional scale/output geometries, and prompt resource cleanup on success/cancel/failure. Continue the Phase 1A criteria in `swatch-board-agent-implementation-plan.md`; do not begin Phase 1B until the entire hard gate passes. See `phase-1a-status.md` for the current evidence.

Use `phase-1a-color-fixture.html` in this directory for large flat known-color targets. The picker logs the last displayed center RGB and whether it matches KWin's final sample.
