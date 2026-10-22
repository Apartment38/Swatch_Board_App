# Swatch Board

Build beautiful palettes with swatches.

Swatch Board is a Rust-only KDE Plasma Wayland color utility. The current
implementation is the **Phase 1A live magnifying picker prototype**, not the
finished tray service or Board editor. The Phase 1A live-desktop acceptance gate
has **not** passed; later phases must not start until it does. The current blocker
is a demonstrated animated-content mismatch between the displayed magnifier frame
and KWin's later sample, plus the lack of remote cancellation for an interrupted
KWin selection. See [live validation and decisions](docs/phase-1a-live-validation.md).
The user-approved next investigation is
[Rust-owned displayed-sample selection](docs/decisions/native-selection-authority.md);
its safety and live acceptance checks remain open.

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

The picker requests screen-capture authorization through KDE's portal, starts a
live pointer-transparent magnifier, and delegates selection to KWin. Left click
selects; right click or Escape cancels through KWin. These are the intended
controls; see the handoff for which live acceptance checks remain outstanding.
The prototype logs results to stderr; clipboard, history, tray, and Board UI are
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
