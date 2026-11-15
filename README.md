# Swatch Board

Build beautiful palettes with swatches.

Swatch Board is a KDE Plasma Wayland color utility with a Rust frontend and a
GPL-2.0-or-later C++ KWin effect. The tray, history, settings, Board shell,
Color Mixer, and Palette Editor have passed target-desktop acceptance. The
replacement picker still needs desktop acceptance.
The Theme Generator's runtime-authored external-plugin workflow has passed native
KDE Wayland acceptance; this does not establish physical testing of every bundled
target plugin. See the [current handoff](docs/current-handoff.md).

## Build and test

```sh
cargo build --offline --workspace --locked
cargo test --offline --workspace --locked
cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
cmake -S kwin -B target/kwin -DCMAKE_BUILD_TYPE=Release
cmake --build target/kwin
```

Offline commands require cached Rust dependencies. A fresh build needs network
access, a C++ compiler, CMake, extra-cmake-modules, Qt 6 and matching KWin
development headers/libraries, Vulkan headers, and Wayland development packages. KWin's effect
ABI depends on the installed KWin version: rebuild the plugin after KWin changes.

## Run the tray

To start the tray, build the executables and run them from the same directory:

```sh
cargo build --locked --workspace
./target/debug/swatch-board-tray
```


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

The required KWin effect displays a crosshair with a square around the current
pixel, a live color swatch and `#RRGGBB`. Configure integer `picker_zoom` in
Settings (1–32, default 4), or use `--zoom` for a direct pick. There is no
ScreenCast prompt, screen-capture portal, or fallback picker. The selected
`#RRGGBB` goes to stdout; direct picking does not update history or clipboard.
The tray's GlobalShortcuts integration still uses the desktop portal.

Inspect the CLI without opening a capture session:

```sh
./target/debug/swatch-board-picker --help
./target/debug/swatch-board-picker --version
```

The picker requires the installed matching KWin plugin and KDE session D-Bus.
The other desktop components also require the Wayland session.

## History and clipboard settings

Optional `${XDG_CONFIG_HOME:-$HOME/.config}/swatch-board/config.toml`:

```toml
history_limit = 100
history_display_limit = 24
history_columns = 4
history_rows = 3
auto_copy = true
clipboard_format = "hex"
pick_shortcut = "Meta+Shift+C"
history_shortcut = "Meta+Shift+H"
picker_zoom = 4
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

Configure validates settings and applies changes to the running tray. Global
shortcuts require desktop portal authorization. On KDE, Save requests assignment of the saved shortcuts to the
existing portal actions; both actions must already be assigned. Unassigned or
revoked shortcuts must first be enabled in KDE System Settings. Conflicting
shortcuts are rejected; use a free intermediate binding to swap two shortcuts.
The assignment is asynchronous and not atomic: inspect shortcut status for
failures or partial changes. Concurrent desktop shortcut edits can race the
conflict check. KDE's system shortcut settings also provide an explicit fallback
when the portal is unavailable. Autostart uses one per-user XDG desktop entry.
The Board applies the hide preference to picks started from its own window.

Build the effect as above and first verify it in an isolated KWin session. System
plugin installation and loading require user confirmation; never restart live
KWin. For staging use `PREFIX=/usr DESTDIR=/tmp/swatch-board-stage
bash packaging/install.sh`. The installer requires the built effect and installs
it under Qt's plugin root; `QT_PLUGIN_DIR` overrides discovery and `EFFECT_LIB`
overrides its build path. Set `RUST_LICENSE_DIR` to the build toolchain's
`share/doc/rust` if it is no longer active. See `docs/release.md`.

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
The active palette is pinned to the bottom of every tab, independently of mixer
scrolling. **Add current color** appends to it, not capture history. Unsaved
changes survive window recreation but are discarded when Board exits.

### Palette Editor

Generate 1–64 colors with a seeded complementary, split-complementary, analogous,
triadic, tetradic, monochromatic, perceptual-contrast, or random generator. An
optional base color, non-wrapping hue interval, OKLCH lightness and chroma bounds,
and minimum contrast against the base (or white) constrain generation. OKLCH
candidate colors are converted by clipping linear sRGB channels; gamut clipping
and 8-bit quantization can change the resulting OKLCH coordinates. Contrast is
measured after conversion, ignoring alpha. Bounded search reports an error when
constraints cannot be met, including exceptionally narrow feasible regions.

The active swatches appear only in the pinned bottom panel. Drag a swatch onto
another to reorder it, hover it for lock and delete controls, or edit its HEX
value and press Enter. A locked swatch cannot be edited, replaced from history,
or deleted until unlocked; it is also preserved on regeneration. In the
Palette Editor, clicking history adds a swatch; the checkbox controls whether
additions are locked. History can also be dragged onto a pinned swatch. Save,
rename, duplicate, search, reopen, or delete saved palettes. Generation history
is separate from saved palettes and retains the latest 100 snapshots. Reopening
a snapshot creates an unsaved draft. The active palette is shared with the Mixer
and Theme Generator.

### Theme Generator

Choose a plugin to start a project. Select preview elements to inspect their
roles; clickable preview controls open other scenes. Drop a history or palette
color onto a preview element or role swatch to override it. Role values can
also be edited using the configured clipboard format. Save a palette before
linking its named swatches to roles. Linked roles follow changes to the named
swatch in the active saved palette; overrides remain independent. Renaming or
deleting a linked swatch leaves an unresolved link until corrected. Undo and
redo apply to project changes. Save a named project or recover the latest draft
when Board restarts.

Reload Plugins discovers data-only packages in
`${XDG_DATA_HOME:-$HOME/.local/share}/swatch-board/plugins/` and each
`$XDG_DATA_DIRS/swatch-board/plugins/`. Duplicate IDs are rejected. Bundled
Ghostty, Zed, and KDE packages use the same contract; schemas are installed under
`share/swatch-board/schemas/`. Each package contains `manifest.json`,
`roles.json`, `scenes.json`, and MiniJinja templates under `templates/`. Templates
receive `roles` and host filters `hex`, `hex_no_hash`, and `rgb_csv`; undefined
variables are errors. API 1 accepts bounded PNG assets and validates references,
relative paths, output sizes, and templates. Rhai and executable plugins are not
supported. See the [plugin-authoring guide](docs/plugin-authoring.md) for the
package contract, minimal multi-scene example, template compatibility, resource
limits, and acceptance checklist. Rendering uses the required sibling
`swatch-board-theme-worker` executable with memory, CPU, wall-clock, and message
limits. The helper provides resource containment, not a filesystem/network
sandbox; templates receive no filesystem/network/process hooks.

Preview rendered files before exporting to an existing directory. Existing files
are replaced only when the overwrite confirmation is checked. Files are staged
and atomically renamed individually; an error in a multi-file export can leave
earlier files written. Export never applies a theme to the desktop.

**Pick Color** requires the tray; **Settings** also works without it. When
`hide_board_while_picking` is enabled, the Board closes its native window before
requesting a pick and recreates it after selection, cancellation, or failure.
It does not activate a window during the pick. Reopening preserves the selected
tab and saved window geometry. Normal close exits the Board process; tray Quit
does not close the Board. A second launch activates the existing Board instance.

## Project references

- [Implementation plan](swatch-board-agent-implementation-plan.md)
- [Phase 1A status (historical)](docs/phase-1a-status.md)
- [Live validation and blockers (historical)](docs/phase-1a-live-validation.md)
- [Current developer handoff](docs/current-handoff.md)
- [Known-color fixture](docs/phase-1a-color-fixture.html)
- [Historical conversation export](colorboard-conversation-export.md) — preserved
  verbatim under its original filename; its former product names and superseded
  designs are archival, not current implementation instructions.

The workspace also contains the lightweight `swatch-board-tray` and
`swatch-board-ipc` packages. The former binary/package names are no longer
supported.

## Packaging and release checks

The Arch-first PKGBUILD, locked source-build notes, staged installation and
installed theme-worker smoke check are in [release documentation](docs/release.md).
The local release smoke is `packaging/verify-release.sh`; it builds all six
release executables and the effect, stages them, and verifies installed payload
and helper rendering. It does not start desktop applications. See the
[test matrix](docs/testing.md) for coverage and outstanding gates, and the
[troubleshooting guide](docs/troubleshooting.md) for recovery steps.
