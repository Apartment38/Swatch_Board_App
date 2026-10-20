# Phase 1A handoff

## Current state

Phase 1A is **in progress; its hard exit gate has not passed**. PipeWire capture is live and delivers 2880×1800 BGRA frames on the current scaled KDE Wayland display. The first frame had no usable cursor position, so the cursor-following lens did not become ready and KWin's selection method was not invoked. Startup now exits with a diagnostic and releases its workers if cursor metadata does not arrive within ten seconds.

The repo contains the `colorboard-picker-core` geometry and magnifier library, plus a Rust `colorboard-picker` using ashpd ScreenCast, PipeWire, KWin's ColorPicker D-Bus method, and a pointer-transparent/non-focusable SCTK layer surface. Stream origins are matched to Wayland output positions, and scale is derived from logical output and actual PipeWire frame dimensions.

## Commits

- `3b4c08c` — Add live Wayland magnifier overlay prototype

There are follow-up capture, startup, output-positioning, and handoff updates in progress after that checkpoint. Commit those as the next logical change after reviewing the final diff.

## Build and runtime

- `cargo fmt --all` passes.
- `cargo check --offline -p colorboard-picker` passes with a warning that presentation/receive timestamps are not yet used.
- A real portal session was authorized; PipeWire negotiated `VideoFormat::BGRA 2880x1800` and delivered a frame.
- That frame had `pointer=None`; no visible cursor-driven lens or KWin color result has been observed yet.
- Portal capture approval repeats because the request explicitly uses `PersistMode::DoNot`.

## Environment and remaining work

KWin reports 6.8.0, PipeWire 1.6.9, Rust 1.99.1, and SCTK 0.21.1. One 1.75× output is available at 1646×1029 logical / 2880×1800 captured. Multi-monitor, negative-origin, rotated-output, live cursor following, center-color agreement, focus, click routing, confirmation/cancellation, feedback recursion, and completed-session cleanup remain unverified.

Next diagnose absent cursor metadata, then validate the lens and selection behavior on the live desktop. Continue the Phase 1A criteria in `colorboard-agent-implementation-plan.md`; do not begin Phase 1B until the entire hard gate passes. See `phase-1a-status.md` for the current evidence.
