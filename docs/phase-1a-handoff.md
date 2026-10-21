# Phase 1A handoff

## Current state

Phase 1A is **in progress; its hard exit gate has not passed**. PipeWire capture is live and delivers 2880×1800 BGRA frames on the current scaled KDE Wayland display. Requesting cursor metadata yields usable positions. A run over the `#0000FF` fixture instead selected `#272777`; the displayed center was `#282898`, 201 ms old, and the user observed progressive darkening. This indicates a likely capture feedback loop because the visible lens covered the sampled cursor position. The lens placement has been changed to keep its rendered pixels beside the cursor and out of the sampled area; a live rerun is needed to validate the fix. Multi-output and other hard-gate checks remain.

The repo contains the `colorboard-picker-core` geometry and magnifier library, plus a Rust `colorboard-picker` using ashpd ScreenCast, PipeWire, KWin's ColorPicker D-Bus method, and a pointer-transparent/non-focusable SCTK layer surface. Stream origins are matched to Wayland output positions, and scale is derived from logical output and actual PipeWire frame dimensions. After format negotiation the app explicitly requests cursor and header metadata buffers, following PipeWire's [video stream example](https://docs.pipewire.org/video-play_8c-example.html).

## Commits

- `3b4c08c` — Add live Wayland magnifier overlay prototype

Follow-up commits `79ccff1`, `121eda5`, `90688f5`, and `64a6d23` add capture/overlay mapping, request portal permission persistence and cursor metadata buffers, and correct the KWin D-Bus method name. The latest sample-to-KWin comparison diagnostic and this handoff update are the current uncommitted change.

## Build and runtime

- `cargo fmt --all` passes.
- `cargo check --offline -p colorboard-picker` and `cargo test --offline --workspace` pass (4 core tests). The picker now logs its last displayed center sample, frame age, presentation timestamp, and RGB equality with KWin's returned color. Timestamp skew correction is still outstanding.
- A real portal session was authorized; PipeWire negotiated `VideoFormat::BGRA 2880x1800` and delivered a frame.
- A subsequent live run received cursor position `(1836, 1496)` and KWin returned `#3A2F2D`.
- The latest run over the known `#0000FF` fixture reported KWin `#272777` versus displayed center `#282898`, with frame age 201 ms and `RGB match=false`. The user observed the lens darkening progressively toward black. To prevent recursive capture, the lens is now placed to one side of the cursor with a gap beyond its visible pixel grid. Re-run against the same fixture and verify a stable `#0000FF` readout and KWin selection.
- The portal request now uses `PersistMode::Application` so KDE can offer to remember permission. Whether this stops repeat prompts depends on the portal and the user's choice; it has not been exercised since the change.

## Environment and remaining work

KWin reports 6.8.0, PipeWire 1.6.9, Rust 1.99.1, and SCTK 0.21.1. One 1.75× output is available at 1646×1029 logical / 2880×1800 captured. Multi-monitor, negative-origin, rotated-output, post-fix live cursor following and center-color agreement, focus, click routing, confirmation/cancellation, absence of feedback after the placement change, and completed-session cleanup remain unverified.

Next re-run the controlled `#0000FF` sample to validate the side-placement fix, then verify cancellation, focus, click routing, and cleanup. Continue the Phase 1A criteria in `colorboard-agent-implementation-plan.md`; do not begin Phase 1B until the entire hard gate passes. See `phase-1a-status.md` for the current evidence.

Use `phase-1a-color-fixture.html` in this directory for large flat known-color targets. The picker logs the last displayed center RGB and whether it matches KWin's final sample.
