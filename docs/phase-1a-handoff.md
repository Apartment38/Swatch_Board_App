# Phase 1A handoff

## Current state

Phase 1A is **in progress and blocked**. No usable picker or live magnifier has been delivered, and work must not proceed to Phase 1B until the Phase 1A exit gate in the implementation plan passes on KDE Plasma Wayland.

The repository contains a Rust workspace with:

- `colorboard-picker-core`: RGBA frame and pixel types, logical-to-stream geometry mapping (including fractional scale, negative origins, and transforms), and nearest-neighbor magnifier rasterization.
- `colorboard-picker`: an integration prototype requesting XDG ScreenCast monitor streams with cursor metadata, consuming PipeWire buffers, normalizing common 32-bit pixel formats and stride, reading cursor/header metadata, and a zbus proxy for KWin `ColorPicker.pick()`.
- `docs/phase-1a-status.md`: concise implementation and blocker status.

The binary currently waits for the first captured frame and exits. It does not display an overlay or invoke KWin's picker. The portal capture geometry is not yet connected to the core mapper, and capture/overlay shutdown is not integrated.

## Commits on the work branch

- `3d3808d` — Start Phase 1A picker core
- `4d90e9a` — Test picker geometry and magnifier rasterization
- `9a21491` — Add portal and PipeWire picker capture prototype

This handoff should be committed separately from those implementation milestones.

## Build and checks

Use the workspace toolchain and system development libraries where available. In the managed workspace used for this work, the successful check environment was:

```sh
export RUSTUP_HOME=/workspace/.rustup
export CARGO_HOME=/workspace/.cargo
export PKG_CONFIG_PATH=/workspace/pkgroot/usr/lib/x86_64-linux-gnu/pkgconfig
export BINDGEN_EXTRA_CLANG_ARGS='-I/usr/lib/gcc/x86_64-linux-gnu/14/include'
source /workspace/.cargo/env
cargo fmt --all -- --check
cargo test --offline -p colorboard-picker-core
cargo check --offline -p colorboard-picker
cargo clippy --offline -p colorboard-picker
```

Formatting, all four core tests, picker `cargo check`, and Clippy completed successfully. Clippy emits dead-code warnings for capture timestamps and the selection proxy because neither is wired into the UI yet. A strict `-D warnings` Clippy run therefore fails.

## Blockers and known limits

- The environment has no Plasma Wayland session (`WAYLAND_DISPLAY`/`XDG_SESSION_TYPE` are unset) and no KWin runtime to exercise the integration.
- At the last implementation attempt, crates.io access failed and the Wayland client dependency was not cached. Recheck network and dependency availability before implementing the layer-shell UI.
- No actual monitor geometry, transform, cursor/frame synchronization, feedback exclusion, focus, click routing, cancellation, or cleanup observations have been recorded.
- Captured cursor and video data are read from their respective PipeWire metadata, but their timing/coordinate relationship has not been demonstrated to match KWin's final color result.
- Treat the color picker proxy and core geometry tests as unverified integration pieces, not proof of an accurate picker.

## Next work

1. Implement and compile a Rust Wayland layer-shell magnifier with `KeyboardInteractivity::None` and an empty pointer input region. Render live portal/PipeWire source pixels around the metadata cursor; ensure the overlay is excluded from capture.
2. Retain and publish the latest frame per stream without blocking PipeWire callbacks. Associate each portal stream with the correct Wayland output using compositor/portal geometry, transform, and scale information.
3. Start `org.kde.kwin.ColorPicker.pick()` asynchronously only after the live overlay is visible. Keep KWin responsible for confirmation/cancellation and the authoritative final color; coordinate its result with the magnified center pixel.
4. Implement orderly success/cancel/error shutdown for the portal session, PipeWire main loop/streams, D-Bus request, and Wayland surfaces.
5. On a real Plasma 6 Wayland session, record Plasma/KWin, portal, PipeWire, and Rust dependency versions and test: live moving content; source-pixel/selected-color agreement; negative and rotated monitor geometry; fractional and mixed scaling; multiple monitors; no capture recursion; no underlying click-through; no keyboard focus theft; left-click confirmation; right-click/Escape cancellation as supported; portal permission behavior; and resource cleanup.
6. Update `docs/phase-1a-status.md` with observed results. Do not mark Phase 1A complete or start Phase 1B unless every required exit criterion passes.

## Acceptance reference

The authoritative phase definition and hard exit gate are in `colorboard-agent-implementation-plan.md` under “Phase 1A — Live Rust magnifier and picking engine,” and in `colorboard-conversation-export.md` under the Phase 1A requirements. Mock-only checks are insufficient for completion.
