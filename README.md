# Swatch Board

Build beautiful palettes with swatches.

Swatch Board is a Rust-only KDE Plasma Wayland color utility. The picker,
tray, history, settings, Board shell, and Color Mixer have passed target-desktop
acceptance. Palette and theme editors are future phases.
See the [current handoff](docs/current-handoff.md).

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
cargo build --locked -p swatch-board-picker -p swatch-board-tray -p swatch-board-history -p swatch-board-config -p swatch-board
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
click outside to close without copying. **Configure** opens a separate settings
window; closing it exits that process. **Open Board** launches the separate Board
window. See the [D-Bus API](docs/ipc.md).

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
selected `#RRGGBB` value to stdout. When moving between outputs, the picker waits
for fresh target video and for the reported presentation time before using a
sample; timeouts and output loss fail closed. When run directly the picker does
not update history or clipboard; use the tray for those actions.

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
magnifier_radius = 7
magnifier_zoom = 10
pick_shortcut = "Meta+Shift+C"
history_shortcut = "Meta+Shift+H"
hide_board_while_picking = false
autostart = false
version = 1
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

Configure validates settings and applies changes to the running tray. Magnifier
radius is 0–31 and zoom is 4–32. Global shortcuts require desktop portal
authorization. On KDE, Save requests assignment of the saved shortcuts to the
existing portal actions; both actions must already be assigned. Unassigned or
revoked shortcuts must first be enabled in KDE System Settings. Conflicting
shortcuts are rejected; use a free intermediate binding to swap two shortcuts.
The assignment is asynchronous and not atomic: inspect shortcut status for
failures or partial changes. Concurrent desktop shortcut edits can race the
conflict check. KDE's system shortcut settings also provide an explicit fallback
when the portal is unavailable. Autostart uses one per-user XDG desktop entry.
The Board applies the hide preference to picks started from its own window.

For a staged installation, build release binaries and use
`PREFIX=/usr DESTDIR=/tmp/swatch-board-stage bash packaging/install.sh`.

## Run the Board

```sh
cargo run --offline --locked -p swatch-board
```

The Board opens independently of the tray. Its tabs are **Color Mixer**,
**Palette Editor**, and **Theme Generator**, with a collapsible history sidebar in
every tab. Toggle it with **Color history**. History comes from the shared SQLite library and refreshes on tray
signals and reconnection. Settings reload while the Board is open.

Click a history swatch to select it in the mixer, or drag it onto the current
color or a mix source. Edit HEX, RGB, RGBA, HSL, or HSV values and apply them;
editing a format without alpha preserves the current alpha. CMYK is approximate
and unprofiled. Copy uses the configured clipboard format.

Click or drag the spectrum to set hue and saturation; the adjacent vertical bar
sets HSV brightness. The full spectrum stays above the scrolling details.
Edit individual HSV (degrees/percent), RGB, and alpha (0–255) values by dragging
their numbers or double-clicking to type. These controls update the current
color immediately and participate in undo/redo.

Mix two or more sources with normalized weights in encoded sRGB or OKLCH.
OKLCH averages hue along the shortest arc relative to the first chromatic source,
ignores achromatic hues, and clips out-of-gamut linear RGB channels. Alpha is
weighted independently. Undo and redo cover mixer state and draft changes.
The active unsaved draft is pinned to the bottom of every tab, independently of
mixer scrolling. **Add current color** appends to that draft, not capture history.
It remains in memory across tabs and window hide/restore but is discarded when
Board exits. Saving palettes belongs to Phase 4.

**Pick Color** requires the tray; **Settings** also works without it. When
`hide_board_while_picking` is enabled, the Board closes its native window before
requesting a pick and recreates it after selection, cancellation, or failure.
It does not activate a window during the pick. Reopening preserves the selected
tab and saved window geometry. Normal close exits the Board process; tray Quit
does not close the Board. A second launch activates the existing Board instance.

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
