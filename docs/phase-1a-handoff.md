# Phase 1A handoff

## Current state

Phase 1A is **in progress; its hard exit gate has not passed**. PipeWire capture is live and delivers 2880×1800 BGRA frames on the current scaled KDE Wayland display. Requesting cursor metadata yields usable positions, and the latest run selected `#3A2F2D` through KWin on stream 120. The user has not yet confirmed that the lens visibly followed cursor movement or compared its center readout against KWin's result. Multi-output and other hard-gate checks remain.

The repo contains the `colorboard-picker-core` geometry and magnifier library, plus a Rust `colorboard-picker` using ashpd ScreenCast, PipeWire, KWin's ColorPicker D-Bus method, and a pointer-transparent/non-focusable SCTK layer surface. Stream origins are matched to Wayland output positions, and scale is derived from logical output and actual PipeWire frame dimensions. After format negotiation the app explicitly requests cursor and header metadata buffers, following PipeWire's [video stream example](https://docs.pipewire.org/video-play_8c-example.html).

## Commits

- `3b4c08c` — Add live Wayland magnifier overlay prototype

Follow-up commits `79ccff1`, `121eda5`, `90688f5`, and `64a6d23` add capture/overlay mapping, request portal permission persistence and cursor metadata buffers, and correct the KWin D-Bus method name. The latest sample-to-KWin comparison diagnostic and this handoff update are the current uncommitted change.

## Build and runtime

- `cargo fmt --all` passes.
- `cargo check --offline -p colorboard-picker` and `cargo test --offline --workspace` pass (4 core tests). The picker now logs its last displayed center sample, frame age, presentation timestamp, and RGB equality with KWin's returned color. Timestamp skew correction is still outstanding.
- A real portal session was authorized; PipeWire negotiated `VideoFormat::BGRA 2880x1800` and delivered a frame.
- A subsequent live run received cursor position `(1836, 1496)` and KWin returned `#3A2F2D`. The user has not yet confirmed the lens's visual appearance, movement, or center-sample match. A new run will log the app's last displayed sample alongside KWin's result.
- The portal request now uses `PersistMode::Application` so KDE can offer to remember permission. Whether this stops repeat prompts depends on the portal and the user's choice; it has not been exercised since the change.

## Environment and remaining work

KWin reports 6.8.0, PipeWire 1.6.9, Rust 1.99.1, and SCTK 0.21.1. One 1.75× output is available at 1646×1029 logical / 2880×1800 captured. Multi-monitor, negative-origin, rotated-output, live cursor following, center-color agreement, focus, click routing, confirmation/cancellation, feedback recursion, and completed-session cleanup remain unverified.

Next validate visible cursor tracking and center-sample agreement on controlled colors, then cancellation, focus, click routing, and cleanup. Continue the Phase 1A criteria in `colorboard-agent-implementation-plan.md`; do not begin Phase 1B until the entire hard gate passes. See `phase-1a-status.md` for the current evidence.
