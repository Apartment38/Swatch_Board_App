# Phase 1A handoff

## Current state

Phase 1A is **in progress; its hard exit gate has not passed**. PipeWire capture is live and delivers 2880×1800 BGRA frames on the current scaled KDE Wayland display. Requesting cursor metadata now yields usable positions; the latest run reported `(1801, 1473)` and reached KWin's selection call. It failed because zbus sent the default PascalCase `Pick`, while KWin exports lowercase `pick`. The proxy now overrides the D-Bus member name; a live selection run is still needed.

The repo contains the `colorboard-picker-core` geometry and magnifier library, plus a Rust `colorboard-picker` using ashpd ScreenCast, PipeWire, KWin's ColorPicker D-Bus method, and a pointer-transparent/non-focusable SCTK layer surface. Stream origins are matched to Wayland output positions, and scale is derived from logical output and actual PipeWire frame dimensions. After format negotiation the app now explicitly requests `SPA_META_Cursor` buffers, as PipeWire's video stream example does; this change has not yet been confirmed in a live run.

## Commits

- `3b4c08c` — Add live Wayland magnifier overlay prototype

Follow-up commits `79ccff1`, `121eda5`, and `90688f5` add capture/overlay mapping, request portal permission persistence, and request cursor metadata buffers. The lowercase KWin method mapping and this handoff update are the current uncommitted change.

## Build and runtime

- `cargo fmt --all` passes.
- `cargo check --offline -p colorboard-picker` passes with a warning that presentation/receive timestamps are not yet used.
- A real portal session was authorized; PipeWire negotiated `VideoFormat::BGRA 2880x1800` and delivered a frame.
- With explicit cursor metadata allocation, the latest run received a nonempty cursor position and reached the KWin call. KWin reported `UnknownMethod` because the generated proxy requested `Pick`; local D-Bus introspection confirmed the method is lowercase `pick` with return signature `(u)`. The proxy now specifies that exact member name. The user has not yet confirmed the lens's visual appearance or exercised selection.
- The portal request now uses `PersistMode::Application` so KDE can offer to remember permission. Whether this stops repeat prompts depends on the portal and the user's choice; it has not been exercised since the change.

## Environment and remaining work

KWin reports 6.8.0, PipeWire 1.6.9, Rust 1.99.1, and SCTK 0.21.1. One 1.75× output is available at 1646×1029 logical / 2880×1800 captured. Multi-monitor, negative-origin, rotated-output, live cursor following, center-color agreement, focus, click routing, confirmation/cancellation, feedback recursion, and completed-session cleanup remain unverified.

Next run the corrected KWin call and validate the lens and selection behavior on the live desktop. Continue the Phase 1A criteria in `colorboard-agent-implementation-plan.md`; do not begin Phase 1B until the entire hard gate passes. See `phase-1a-status.md` for the current evidence.
