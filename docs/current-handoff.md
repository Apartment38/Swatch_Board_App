# Swatch Board current handoff

## Repository state

- Project: `/home/erick/Projects/Swatch_Board_App`
- Branch: `work`
- Phase 1A completion commit: `4ba80c2` (`Complete Phase 1A native picker acceptance`)
- Repository commit guidance: `ad0eb7c` (`Add logical checkpoint commit guidance`)
- No push was requested or performed.

Read the root `AGENTS.md` before changing the repository. Commit coherent, verified work at logical checkpoints; never push without an explicit user request.

## Current milestone

**Phase 1A, Phase 1B, Phase 1C, and Phase 1D acceptance passed.** Settings UI and Board have not started.

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
[`ipc.md`](ipc.md). At the Phase 1B checkpoint, Palette, Board, and Configure
were disabled and history and clipboard were not yet initialized.

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

## Phase 1C implementation and verification

The tray stores distinct captures in the shared SQLite history, restores the
last color, emits history signals, and owns the Wayland clipboard. Settings
and format names are documented in the [README](../README.md); the history API
is in [ipc.md](ipc.md). A private KWin/Plasma session with a temporary picker
verified two identical captures, clipboard paste after picker exit, unchanged
history and clipboard after cancellation, and last-color restoration after tray
restart. The test used a synthetic picker, not a real capture.

### Physical KDE acceptance

On the real KDE Wayland desktop, with separate empty acceptance data, the user
picked the same color twice, cancelled a third pick with right-click, and
pasted `#171925`. Both real picker runs recorded presented samples of
`#171925` and closed their ScreenCast and GlobalShortcuts sessions; the
cancelled run closed both sessions as well. `GetHistory(10)` returned two
distinct picker captures, both `#171925FF`; cancellation added no third row.
The desktop clipboard also read `#171925`. After quitting and restarting the
tray, `GetLastColor` returned `#171925` and both history rows remained.

Malformed TOML and a non-SQLite database were each tested separately against
the real tray startup: both exited with status 1 and a descriptive error.
The temporary application entry and acceptance tray were removed. Test data
remains under the ignored `target/phase1c-physical` and `target/phase1c-errors`
directories; normal user history was not used.

## Phase 1D acceptance

The tray launches a transient layer-shell history popup, brokers UUID-based
copying, and retains clipboard ownership. The popup supports configurable
display limit, grid size, scrolling, keyboard navigation, repeat activation,
Escape, click-away, and dismissal with a desktop notification after copying.
The compositor supplies the cursor shape where available.

On the real KDE Wayland desktop with isolated synthetic history, the user
confirmed centering, scrolling, copying/pasting, dismissal, notification, and
the KWin-matched pointer. Twenty rows remained after copying. Only one physical
output was available; screen capture was unavailable to the automation backend,
so those physical visual observations are user-reported.

In a separate private KWin session, two enabled 1024×768 virtual outputs were
arranged side by side. A screenshot confirmed that a popup invoked with the
pointer on the second output was centered there; a click on the first output
dismissed it, and repeat activation toggled the next popup closed. In another
private KWin session with keyboard but no pointer capability, `Virtual-1` was
the configured primary output and a screenshot confirmed the popup centered on
it. The helper queries `kscreen-doctor -j` only when pointer routing is
unavailable and fails explicitly if the primary output cannot be determined.
Both private trays and compositors were stopped. Screenshots remain in the
ignored `target/phase1d-private-multiscreen.png` and
`target/phase1d-private-primary.png`; they are not physical multi-monitor tests.

Offline workspace tests passed 123 tests across 17 suites; formatting and strict
workspace Clippy passed.

Do not start the Board before Phases 1A–1E jointly pass.
