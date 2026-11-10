# Swatch Board

Build beautiful palettes with swatches.

Swatch Board is a Rust-only KDE Plasma Wayland color utility. The current
implementation is the **Phase 1A live magnifying picker**, not the finished tray
service or Board editor. Phase 1A passed on the actual 2880×1800, 1.75× KDE
Plasma Wayland target and in a private multi-output fault-injection lab. The
Rust-owned picker is now the only backend; the obsolete KWin ColorPicker path
and its animated-content mismatch were removed. See
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

## Run the picker

From a KDE Plasma Wayland desktop session:

```sh
cargo run --offline --locked -p swatch-board-picker
```

The picker requests screen-capture authorization through KDE's portal. Left
release confirms the exact center sample from a genuinely presented magnifier
commit; right release or Escape cancels. Diagnostics go to stderr and the
selected `#RRGGBB` value to stdout. Clipboard, history, tray, and Board UI are
not implemented yet.

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
- [Developer handoff](docs/phase-1a-handoff.md)
- [Known-color fixture](docs/phase-1a-color-fixture.html)
- [Historical conversation export](colorboard-conversation-export.md) — preserved
  verbatim under its original filename; its former product names and superseded
  designs are archival, not current implementation instructions.

Current Cargo packages are `swatch-board-picker` and `swatch-board-picker-core`;
the library import is `swatch_board_picker_core`. The former binary/package names
are no longer supported. Old build artifacts can remain in `target/`; use the
new executable above.
