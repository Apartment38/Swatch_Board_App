# Swatch Board

Build beautiful palettes with swatches.

Swatch Board is a Rust-only KDE Plasma Wayland color utility. Phase 1A's live
magnifying picker passed on the target desktop. Phase 1B adds a lightweight tray
and asynchronous picker orchestration; its real-desktop acceptance gate remains
open until focus, repeated invocation, and cleanup are verified. See
[Phase 1A status](docs/phase-1a-status.md) and
[native integration acceptance](docs/phase-1a-native-integration.md).

## Build and test

```sh
cargo build --offline --workspace --locked
cargo test --offline --workspace --locked
cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
```

Offline commands require the pinned dependencies to be cached already. A first
build on a fresh machine needs network access and the native PipeWire/SPA and
Wayland development packages, plus the libclang tooling used by their bindings.

## Run the tray

To start the tray, build both executables and run them from the same directory:

```sh
cargo build --offline --locked -p swatch-board-picker -p swatch-board-tray
./target/debug/swatch-board-tray
```

Left-click the tray icon or select **Pick Color**. The current color is kept
in memory until the tray exits. Palette, Board, and Configure menu entries are
disabled until their later phases. See the [D-Bus API](docs/ipc.md).

To run the picker directly from a KDE Plasma Wayland desktop session:

```sh
cargo run --offline --locked -p swatch-board-picker
```

The picker requests screen-capture authorization through KDE's portal. Left
release confirms the exact center sample from a genuinely presented magnifier
commit; right release or Escape cancels. Diagnostics go to stderr and the
selected `#RRGGBB` value to stdout. Clipboard, history, and Board UI are not
implemented yet.

Inspect the CLI without opening a capture session:

```sh
./target/debug/swatch-board-picker --help
./target/debug/swatch-board-picker --version
```

The executable requires access to the desktop's session D-Bus and Wayland socket.
If launched outside the desktop environment, ensure `WAYLAND_DISPLAY` points to
the actual compositor socket; do not assume a particular socket name.

## Project references

- [Implementation plan](swatch-board-agent-implementation-plan.md)
- [Phase 1A status](docs/phase-1a-status.md)
- [Live validation and blockers](docs/phase-1a-live-validation.md)
- [Current developer handoff](docs/current-handoff.md)
- [Known-color fixture](docs/phase-1a-color-fixture.html)
- [Historical conversation export](colorboard-conversation-export.md) — preserved
  verbatim under its original filename; its former product names and superseded
  designs are archival, not current implementation instructions.

The workspace also contains the lightweight `swatch-board-tray` and
`swatch-board-ipc` packages. The former binary/package names are no longer
supported.
