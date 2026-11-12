# Troubleshooting

## Picker and portals

- **Picker is rejected by ScreenCast portal:** confirm the installed
  `org.apartment38.SwatchBoard.desktop` entry has an absolute `Exec` path to
  `swatch-board-picker` and is visible in an XDG applications directory. The
  portal associates the requester with this desktop entry. Reinstall the
  package after moving the prefix and retry from the tray.
- **No capture or shortcut prompt appears:** check that the KDE XDG portal
  backend is running and that the request is being made from a real graphical
  user session with its session D-Bus and Wayland socket. Human authorization
  dialogs are manual. Do not try to work around denied permission by restarting
  KWin or changing desktop grants from a script.
- **Global shortcuts show unavailable or revoked:** open KDE System Settings'
  Shortcuts page, approve/assign the existing Swatch Board actions, then use
  Settings status and retry. Saved preferences are suggestions until KDE has
  assigned both actions. Conflicts are rejected; use an unused intermediate
  shortcut to swap two assignments. The assignment sequence is not atomic, so
  inspect status after an error.
- **Picker output selection is ambiguous:** it fails closed. If pointer output
  identity is absent, the documented fallback queries `kscreen-doctor`; install
  KDE's `kscreen` package or use the picker on a known active output.

## Tray, clipboard, and history

- **Clipboard is lost when picker exits:** the tray owns the Wayland clipboard
  and must remain running unless a clipboard manager takes ownership. The
  standalone picker prints a color but does not update the tray's history or
  clipboard.
- **Saved settings or database rejected:** Swatch Board reports incompatible
  data instead of silently discarding it. Back up the affected file before
  repair. Settings are in `${XDG_CONFIG_HOME:-~/.config}/swatch-board/` and
  history is in `${XDG_DATA_HOME:-~/.local/share}/swatch-board/library.sqlite3`.
- **Autostart duplicates:** Swatch Board writes one per-user XDG desktop entry
  only when enabled in Settings. Remove stale duplicate user entries from
  `~/.config/autostart/`; the package does not install a system service.

## Board plugins and exports

- **Plugin is missing or rejected:** inspect the Board's plugin error, verify
  `manifest.json`, `roles.json`, `scenes.json`, and declared template paths
  against `schemas/`, and check that no path component or package root is a
  symlink. Duplicate plugin IDs are rejected. Restore the exact version saved
  by the project and choose **Reload Plugins**; unavailable plugins do not
  discard project edits.
- **Theme worker missing:** install `swatch-board-theme-worker` beside
  `swatch-board`. Rebuild with `cargo build --workspace` before running tests;
  a partial build or partial package is unsupported. There is no in-process
  fallback.
- **Render fails or hits a limit:** inspect the plugin's template, declared
  outputs, and sizes. API 1 uses bounded files and a fresh helper with memory,
  CPU, and wall-time limits. These limits can reject large packages; they are
  resource containment and do not make plugins a filesystem/network sandbox.
- **Export is refused:** preview again after changing palette links, choose an
  existing directory, and explicitly enable overwrite only after inspecting
  the preview. Symlinks, nonregular targets, traversal, and missing output
  subdirectories are rejected. Multi-file export is atomic per file, not as a
  whole transaction; if a later rename fails, check earlier output files.
- **Preview/export controls appear below the window:** scroll the Theme
  Generator content area. The palette remains pinned separately at the bottom.

For support reports, include the Swatch Board version, distro/kernel, Plasma
and portal versions, display scale/transform, relevant error text, and whether
the issue reproduces with the shipped `example-terminal` fixture. Do not attach
database files unless they have been reviewed for personal color/history data.
