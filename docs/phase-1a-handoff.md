# Phase 1A handoff

## Current state

Phase 1A is **in progress; its hard exit gate has not passed**. A first Rust layer-shell magnifier prototype is connected to PipeWire frames and KWin's picker, but it has not been launched or validated in the live desktop. Do not proceed to Phase 1B until all Phase 1A criteria pass on KDE Plasma Wayland.

The repository contains:

- `colorboard-picker-core`: RGBA frame and pixel types, logical-to-stream geometry mapping, and nearest-neighbor magnifier rasterization.
- `colorboard-picker`: an ashpd ScreenCast session, PipeWire stream adapter, KWin D-Bus picker proxy, and a Smithay Client Toolkit layer-shell overlay.
- `docs/phase-1a-status.md`: concise implementation and blocker status.

The overlay has no keyboard interactivity and an explicitly empty pointer input region. It renders a 15×15 lens at 10× with a live hex readout from the frame matching cursor metadata, then starts KWin's authoritative color-picking call. It is fixed in the upper-right corner; it does not follow the cursor. Stream-to-output geometry, scale, and transforms are not integrated. Capture recursion, frame/cursor timing, focus, pointer behavior, and selection accuracy remain unverified.

## Commits on the work branch

- `3d3808d` — Start Phase 1A picker core
- `4d90e9a` — Test picker geometry and magnifier rasterization
- `9a21491` — Add portal and PipeWire picker capture prototype

The current continuation has uncommitted changes. Keep the handoff/status updates with the implementation when committing.

## Build and checks

In the current workspace, `cargo fmt --all` and `cargo check -p colorboard-picker` pass. The build has one dead-code warning for PipeWire timestamps not yet used for synchronization. Tests, Clippy, and the picker runtime were not run during this continuation. Crates.io access worked with approved Cargo access; Wayland dependencies are now cached.

## Environment and blockers

- The environment reports `XDG_CURRENT_DESKTOP=KDE` and `WAYLAND_DISPLAY=wayland-1`. The `kwin_wayland` binary reports 6.8.0; PipeWire reports 1.6.9; Rust is 1.99.1; and Smithay Client Toolkit is 0.21.1. The sandbox cannot connect to the desktop user's D-Bus session, so a running compositor/portal was not confirmed. `plasmashell --version` aborted, so the Plasma version is unknown.
- No actual portal/overlay session has been exercised. No monitor geometry, transform, cursor/frame synchronization, feedback exclusion, focus, click routing, cancellation, or cleanup observations have been recorded.
- The existing geometry mapper has not been connected to portal stream geometry or the KWin result. Treat the picker proxy and core mapper as unverified integration pieces.

## Next work

1. Make the lens follow the cursor and connect portal streams to Wayland output geometry, scale, and transforms.
2. Resolve timestamp pairing and overlay capture recursion; verify center-pixel agreement with KWin.
3. Verify focus, click routing, left-click confirmation, cancellation, portal behavior, and cleanup on the live session.
4. Record Plasma/portal versions and observed results in `docs/phase-1a-status.md`. Do not mark Phase 1A complete or start Phase 1B unless every required exit criterion passes.

## Acceptance reference

The authoritative phase definition and hard exit gate are in `colorboard-agent-implementation-plan.md` under “Phase 1A — Live Rust magnifier and picking engine,” and in `colorboard-conversation-export.md` under the Phase 1A requirements. Mock-only checks are insufficient for completion.
