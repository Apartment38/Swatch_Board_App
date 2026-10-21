# Phase 1A handoff

## Current state

Phase 1A is **in progress; its hard exit gate has not passed**. PipeWire capture is live and delivers 2880×1800 BGRA frames on the current scaled KDE Wayland display. Requesting cursor metadata yields usable positions. The lens is positioned beside the cursor to avoid capture feedback. With KWin's Dim Inactive effect disabled, a live run over the `#0000FF` fixture returned exactly `#0000FF` from both KWin and the displayed center sample (frame age 130 ms, `RGB match=true`). A subsequent Escape run returned KWin's explicit `org.kde.kwin.ColorPicker.Error.Cancelled` result. These verify exact color agreement for that controlled target and Escape cancellation on the current display. Multi-output, pointer routing/focus, other input paths, dynamic content/timing, and cleanup checks remain.

The repo contains the `colorboard-picker-core` geometry and magnifier library, plus a Rust `colorboard-picker` using ashpd ScreenCast, PipeWire, KWin's ColorPicker D-Bus method, and a pointer-transparent/non-focusable SCTK layer surface. Stream origins are matched to Wayland output positions, and scale is derived from logical output and actual PipeWire frame dimensions. After format negotiation the app explicitly requests cursor and header metadata buffers, following PipeWire's [video stream example](https://docs.pipewire.org/video-play_8c-example.html).

## Commits

- `3b4c08c` — Add live Wayland magnifier overlay prototype

Follow-up commits `79ccff1`, `121eda5`, `90688f5`, and `64a6d23` add capture/overlay mapping, request portal permission persistence and cursor metadata buffers, and correct the KWin D-Bus method name. Later commits add sample-to-KWin diagnostics, the controlled-color fixture, and the cursor-offset feedback fix. The latest verification evidence is recorded above.

## Build and runtime

- `cargo fmt --all` passes.
- `cargo check --offline -p colorboard-picker` passes. `cargo test --offline -p colorboard-picker-core` passes (7 tests), covering nearest-neighbor rendering, checkerboard/gradient samples, negative origins, 100/125/150/200% scales, and all 8 output transforms. The picker logs its last displayed center sample, frame age, presentation timestamp, and RGB equality with KWin's returned color. Timestamp skew evaluation on changing live content remains outstanding.
- A real portal session was authorized; PipeWire negotiated `VideoFormat::BGRA 2880x1800` and delivered a frame.
- A subsequent live run received cursor position `(1836, 1496)` and KWin returned `#3A2F2D`.
- An initial run over the known `#0000FF` fixture reported KWin `#272777` versus displayed center `#282898`, with frame age 201 ms and `RGB match=false`; the user observed progressive darkening. The lens was moved to one side of the cursor to prevent recursive capture.
- A follow-up run with Dim Inactive disabled reported `#0000FF` from both KWin and the displayed center sample, with frame age 130 ms and `RGB match=true`. The original color discrepancy is resolved for the controlled blue fixture under this configuration.
- Escape cancellation was exercised. KWin returned `org.kde.kwin.ColorPicker.Error.Cancelled: Color picking got cancelled` and the picker process returned to the shell.
- The portal request now uses `PersistMode::Application` so KDE can offer to remember permission. Whether this stops repeat prompts depends on the portal and the user's choice; it has not been exercised since the change.

## Environment and remaining work

KWin reports 6.8.0, PipeWire 1.6.9, Rust 1.99.1, and SCTK 0.21.1. One 1.75× output is available at 1646×1029 logical / 2880×1800 captured. Multi-monitor, negative-origin, rotated-output, left-click confirmation, right-click cancellation, focus, click routing/no click-through, changing content/timestamp behavior, and completed-session cleanup remain unverified.

Next verify left-click confirmation, right-click cancellation, focus preservation, click routing/no click-through, live cursor following on changing targets, live additional scale/output geometries, and prompt resource cleanup on success/cancel/failure. Continue the Phase 1A criteria in `colorboard-agent-implementation-plan.md`; do not begin Phase 1B until the entire hard gate passes. See `phase-1a-status.md` for the current evidence.

Use `phase-1a-color-fixture.html` in this directory for large flat known-color targets. The picker logs the last displayed center RGB and whether it matches KWin's final sample.
