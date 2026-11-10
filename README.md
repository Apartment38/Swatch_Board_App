# Swatch Board

Build beautiful palettes with swatches.

Swatch Board is a Rust-only KDE Plasma Wayland color utility. Phase 1A's live
magnifying picker and Phase 1B's tray orchestration have been exercised on the
target desktop. See [Phase 1A status](docs/phase-1a-status.md) and
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

To start the tray, build the executables and run them from the same directory:

```sh
cargo build --offline --locked -p swatch-board-picker -p swatch-board-tray -p swatch-board-history
./target/debug/swatch-board-tray
```

The picker also requires `share/applications/org.apartment38.SwatchBoard.desktop`
to be installed in an XDG applications directory. For an uninstalled development
build, copy that entry to `~/.local/share/applications/` and change its `Exec`
to the absolute path of the built `swatch-board-picker`; the portal rejects an
entry whose executable it cannot find. Remove the development entry when done.

Left-click the tray icon or select **Pick Color**. Successful captures are
saved to `${XDG_DATA_HOME:-$HOME/.local/share}/swatch-board/library.sqlite3`,
including repeated colors. The tray restores the last color on restart and owns
the copied text independently of the picker process. **Show Palette** opens the
history popup; selecting a swatch copies it without creating another capture,
closes the popup, and shows a notification. Repeat the action, press Escape, or
click outside to close without copying. Board and Configure remain disabled
until their later phases. See the [D-Bus API](docs/ipc.md).

When the pointer's output cannot be determined, the popup queries KDE's
`kscreen-doctor` for the configured primary output. That executable must be
installed for this fallback; an unavailable or ambiguous configuration produces
an error instead of silently choosing another display.

To run the picker directly from a KDE Plasma Wayland desktop session:

```sh
cargo run --offline --locked -p swatch-board-picker
```

The picker requests screen-capture authorization through KDE's portal. Left
release confirms the exact center sample from a genuinely presented magnifier
commit; right release or Escape cancels. Diagnostics go to stderr and the
selected `#RRGGBB` value to stdout. When run directly the picker does not update
history or clipboard; use the tray for those actions.

Inspect the CLI without opening a capture session:

```sh
./target/debug/swatch-board-picker --help
./target/debug/swatch-board-picker --version
```

The executable requires access to the desktop's session D-Bus and Wayland socket.
If launched outside the desktop environment, ensure `WAYLAND_DISPLAY` points to
the actual compositor socket; do not assume a particular socket name.

## History and clipboard settings

Optional `${XDG_CONFIG_HOME:-$HOME/.config}/swatch-board/config.toml`:

```toml
history_limit = 100
history_display_limit = 24
history_columns = 4
history_rows = 3
auto_copy = true
clipboard_format = "hex"
```

`history_limit` must be positive. `history_display_limit` is 1–10000; popup
`history_columns` and `history_rows` are each 1–12. The display limit is
independent of retained history. Formats are `hex` (`#RRGGBB`), `hex-rgba`,
`rgb`, `rgba`, `hsl`, `hsv`, and `cmyk` (approximate, not color-managed).
Repeated captures remain separate rows; cancel changes neither history nor
clipboard. Clipboard ownership requires a compositor exposing Wayland data
control; the tray must remain running unless a clipboard manager retains the
selection. Invalid settings or incompatible databases produce an error rather
than silently discarding data.

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
