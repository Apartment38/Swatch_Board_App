# Swatch Board current handoff

## Repository state

- Project: `/home/erick/Projects/Swatch_Board_App`
- Branch: `work`
- Phase 1A completion commit: `4ba80c2` (`Complete Phase 1A native picker acceptance`)
- Repository commit guidance: `ad0eb7c` (`Add logical checkpoint commit guidance`)
- No push was requested or performed.

Read the root `AGENTS.md` before changing the repository. Commit coherent, verified work at logical checkpoints; never push without an explicit user request.

## Current milestone

**Phase 1A and Phase 1B acceptance passed.** The Rust-owned presented-sample picker is the default and only backend. No tray history, clipboard, settings, or Board implementation has started.

The picker:

- requests monitor capture through the XDG ScreenCast portal;
- uses PipeWire video and cursor metadata;
- owns pointer shields and confirms on left release without click-through;
- cancels on right release or the temporary application-scoped Escape shortcut;
- selects only the center sample attached to a genuinely presented lens commit;
- handles multiple outputs, fractional scale, transformed outputs, self-occlusion, stale capture, stream loss, output loss, and lock/unlock cleanup;
- prints the selected `#RRGGBB` value to stdout and diagnostics to stderr.

The obsolete KWin ColorPicker implementation, `--native`, and `--repeat` were removed. Do not restore compatibility aliases or a second backend.

## Acceptance evidence

Primary summaries:

- [`phase-1a-status.md`](phase-1a-status.md)
- [`phase-1a-native-integration.md`](phase-1a-native-integration.md)
- [`evidence/native-physical/physical-acceptance-result.json`](evidence/native-physical/physical-acceptance-result.json)
- [`evidence/native-failure-matrix/failure-matrix-result.json`](evidence/native-failure-matrix/failure-matrix-result.json)

Important observations:

- Actual target: KDE Plasma Wayland, eDP-1, 2880×1800@120 Hz, 1646×1029 logical, 1.75× scale.
- Static, animated, rotated, right-cancel, Escape, focus, click-safety, and lock/unlock cases passed.
- Lock suspended presentation for 8098.831 ms; the first post-unlock frame had a new video PTS and 78 ms age.
- Fixture press/release counters remained 0/0 during physical confirmation and post-unlock selection.
- Private multi-output routing, output removal, portal denial/restoration, four-second compositor stall, and PipeWire loss passed their expected recovery or fail-closed behavior.
- Active-capture lifetime CPU fell from 73.2% to 12.6% after the verified frame-conversion throttle.
- Physical hotplug was unsafe because eDP-1 was the only physical output; real output removal passed in the private KWin lab.
- KScreen rejected live negative output coordinates; negative origins and all eight transforms retain deterministic geometry coverage.

## Last verification

Executed after the default-backend cutover:

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo test --offline --workspace --all-targets --locked
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
cargo build --release --offline --workspace --locked
./target/release/swatch-board-picker --help
./target/release/swatch-board-picker --version
```

Observed result: 100 tests passed across 9 suites; formatting, Clippy, release build, help, and version checks passed. Both structured acceptance JSON files parsed successfully with `jq`.

## Host and runtime cleanup

- The temporary per-user desktop entry used for physical acceptance was removed from `~/.local/share/applications/`.
- The packaged project entry remains at `share/applications/org.apartment38.SwatchBoard.desktop`.
- Physical display rotation was restored to normal.
- Picker, fixture, private KWin, PipeWire, and portal test processes were stopped.
- Desktop-control access used for lock/unlock acceptance was released.
- Human-session permission dialogs must remain manual.

## Phase 1B acceptance

The lightweight `swatch-board-tray` and `swatch-board-ipc` packages provide
single-instance D-Bus control, asynchronous picker supervision, selected-color
state, and a StatusNotifierItem. The versioned API is documented in
[`ipc.md`](ipc.md). Palette, Board, and Configure menu entries are disabled until
their later phases; no history or clipboard is initialized.

The tray has been exercised against the real session bus using a temporary
picker process: a second tray invocation exited, concurrent picks returned
`busy`, a completed pick updated the last color, and another pick could start
before quit. Automated supervision tests cover cancellation, invalid output,
crashes, timeout, and child reaping.

On the actual KDE Plasma Wayland desktop, the user confirmed the specified menu
and its phase-appropriate disabled entries, repeated selection, cancellation,
focus preservation, indicator updates, left-click activation, and Quit. Picker
diagnostics recorded three selections and a right-click cancellation, each
closing the ScreenCast and GlobalShortcuts sessions. After cancellation,
`GetLastColor` returned `#171A26`, matching the last selection, and no picker
process remained. The user confirmed a subsequent left-click pick/cancel and
tray Quit.

An uninstalled development desktop entry initially made the portal reject the
picker. Using an absolute picker path in a temporary user entry fixed registration.
The user then confirmed a visible KDE failure notification with the entry removed.
The temporary entry and acceptance tray were removed. Human-session permission
dialogs remained manual.

## Next work

Proceed with Phase 1C — history, formatting, and clipboard — from the
implementation plan. Do not start the Board before Phases 1A–1E jointly pass.
