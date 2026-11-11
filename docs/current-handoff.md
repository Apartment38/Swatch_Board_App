# Swatch Board current handoff

## Repository state

- Project: `/home/erick/Projects/Swatch_Board_App`
- Branch: `work`
- Phase 1A completion commit: `4ba80c2` (`Complete Phase 1A native picker acceptance`)
- Repository commit guidance: `ad0eb7c` (`Add logical checkpoint commit guidance`)
- No push was requested or performed.

Read the root `AGENTS.md` before changing the repository. Commit coherent, verified work at logical checkpoints; never push without an explicit user request.

## Current milestone

**Phase 1A–1E jointly passed on the actual KDE Plasma Wayland target, with the
user-approved replacement for the unsafe physical KWin restart test described
below.** Board has not started and is the next phase.

**Phase 1E is accepted under that revised gate.** The settings utility, typed
configuration, shortcut portal worker, KDE shortcut reassignment, autostart
and staged installer have been implemented. An isolated session-bus scenario
verified single-instance startup, settings reload, invalid-setting rejection,
and picker arguments using a synthetic picker and StatusNotifierWatcher.

On actual KDE Wayland, the user confirmed the settings window's controls,
scrolling, save/reload, and duplicate-shortcut validation. The portal assigned
Meta+Shift+C and Meta+Shift+H; the user confirmed both actions worked. A portal
restart initially broke recovery; using a fresh registered D-Bus connection per
portal session fixed it, and the user confirmed history shortcut recovery after
a second portal restart. Suspend/resume and the history shortcut also passed.

Restarting `plasma-kwin_wayland.service` ended the graphical session; the user
could not log back in and had to reboot. Do not repeat that disruptive test.
The previous-boot user journal recorded that the restarted compositor chose a
Wayland backend because `WAYLAND_DISPLAY` was set; on later shutdown its service
timed out and was killed. These logs do not establish the cause of the login
failure. The physical restart must not be repeated as routine verification.
The user approved replacing the unsafe physical KWin restart gate with the
already recorded private KWin restart/recovery plus a normal physical logout
and login performed manually. The user subsequently logged out and back in
and completed the post-login joint scenario below. The original physical KWin
restart and shortcut recovery after a private KWin restart were not verified.
Automation cannot capture this desktop; visual observations are user-reported.

The debug tray's isolated idle sample reported PSS 14,983 KiB and private
clean plus dirty memory 14,956 KiB. The main thread accumulated 2.35 ms CPU
and 10 scheduled runs over approximately 10 seconds; this is not a total
process wakeup measurement or a release-build baseline.

In a private two-output KWin virtual session, disabling and re-enabling one
output left the tray reachable. A separate private KWin restart replaced the
compositor process, restored both virtual outputs, and left the release tray's
D-Bus service reachable. Shortcut authorization in that session was not approved,
so this does not establish shortcut recovery after KWin restart or physical
hotplug.

Physical hotplug was exercised with eDP-1 and HDMI-A-1. An initial crossing
failed on a stale cached image; waiting for fresh target video under the
existing clean-frame deadline addressed that path. A second crossing failed
closed because a presentation timestamp was ahead of receipt. The picker now
defers such presented feedback until its reported time, retaining bounded
timeouts and requiring post-hide video after the delayed barrier. The user
confirmed crossing both directions and cancellation on the corrected build;
logs showed deferred visible and hide feedback and subsequent clean frames.
Unplugging HDMI-A-1 during an active pick closed it fail-closed; the logs showed
output loss, shield cleanup, and closure of both portal sessions. The user
confirmed the built-in desktop remained usable and a new pick worked. After
HDMI-A-1 was reconnected, both displays were enabled and the user confirmed
another pick worked on both and cancelled. Both displays were left enabled.
The tray and temporary entries were removed, and the user removed the test
shortcuts. This covers the observed hotplug scenarios, not every failure stage.

On the real desktop, the user confirmed the portal configuration dialog opened
and manually changed the assignments to Meta+Shift+J/K; History then worked.
Saving different preferences opened KDE's shortcut settings but did not
automatically apply them. The portal treats preferences as suggestions:
the [KDE portal implementation](https://github.com/KDE/xdg-desktop-portal-kde/blob/Plasma/6.8/src/globalshortcuts.cpp)
rebinds returning IDs without an approval dialog and `ConfigureShortcuts` opens
KDE System Settings.

The user approved a KDE-specific reassignment policy for explicit Save/retry:
after successful portal binding, both existing actions must already have assigned
keys. The tray uses KGlobalAccel to change those actions, checks conflicts, and
reads back both results. This API is not a portal permission check or an atomic
transaction; concurrent shortcut edits can race the checks, and partial failure
is reported. Unassigned or denied actions are never enabled by this path.

With isolated settings and history on the real desktop, the user approved
Meta+Shift+C/H in KDE's portal, saved Meta+Shift+J/K in Settings, and confirmed
that both reassigned shortcuts worked; the picker was cancelled. Portal status
and KGlobalAccel readback reported J/K. The user then confirmed that trying to
swap the two bindings reported a conflict and left both unchanged, and restored
J/K preferences. The temporary tray and desktop entries were removed; the user
removed the temporary KDE assignments, and J/K were verified available.

After a normal logout/login, the user approved J/K for an isolated release tray
and confirmed a real pick, matching clipboard paste, history popup, and saved
settings. The user reported the picked value `#493C7D`; `GetLastColor` returned
that value, and `GetHistory(5)` returned the corresponding `#493C7DFF` picker
capture. After a clean tray restart, both captures and the last color remained,
the portal reported J/K, and the user confirmed Meta+Shift+K opened history
with the saved color. The tray
and temporary entries were removed, the user removed the test assignments, and
KGlobalAccel reported J/K available. Isolated data remains under ignored
`target/phase1e-reassign-check/`.

Replacing existing IDs with new IDs would cause KDE to offer a prefilled
approval dialog, but KDE also removes omitted old actions and can accept only
part of a request. Keeping both old and new IDs avoids immediate removal but can
leave duplicate persistent actions after interruption; the portal exposes no
transactional rollback or machine-readable key sequence for restoring prior
user assignments. No such workaround was applied.

The release tray in the private session used 7,871 KiB PSS and 7,840 KiB private
clean plus dirty memory; over 10.002 seconds its stable threads accumulated
zero whole CPU ticks (100 ticks/second) and 43 scheduled runs. A release picker
awaiting portal consent used 5,001 KiB PSS. On the actual desktop with an
approved live magnifier, the release picker used 98,201 KiB PSS and 66,804 KiB
private memory; its stable threads accumulated 264 CPU ticks and 1,354 scheduled
runs over 10.001 seconds. Scheduled runs are not wakeups.

A separate release-picker startup on the physical KDE session took 6,560.123 ms
from harness launch to the first `LENS_PRESENTED` log. That includes the user's
screen-capture approval: authorization completed at 6,487.224 ms, and the first
presentation was logged 72.899 ms later. This is one observation, not a
distribution, and is not a tray-to-pick startup measurement. The user cancelled
the picker; it exited successfully and its temporary desktop entry was removed.
No external monitor was available for the earlier physical hotplug test.

On the actual KDE desktop, the release tray's stable nine threads used 8,421 KiB
PSS and 8,392 KiB private memory. During a 20.001-second idle interval they
accumulated 6.331 ms CPU time, 100 scheduled runs, 99 voluntary and one
involuntary context switch. These scheduler counts are not wakeup counts.
A tray-initiated picker reached its first presented frame 18,171.955 ms after
the D-Bus request, including manual authorization; the authorization log arrived
at 18,099.865 ms, 72.090 ms before the presented-frame log. This single
observation is not a latency distribution. The user cancelled the picker, the
tray exited successfully, and the user removed its test shortcuts. The
temporary picker and tray entries and measurement harness were removed.
For a subsequent physical idle sample, the user temporarily enabled kernel
scheduler statistics. The release tray's five stable sampled threads recorded
138 `nr_wakeups` over 30.236 seconds (4.56 wakeups/second), with 8,426 KiB PSS
and 8,396 KiB private memory. Thread membership was sampled every 250 ms and
start times matched, so very short-lived threads between samples cannot be
excluded. The user restored `kernel.sched_schedstats` to 0, verified afterward,
and removed the acceptance shortcut assignments and temporary desktop entry.
There is no comparable release-tray wakeup baseline from before Phase 1E, so
these measurements do not establish a wakeup regression.

The latest full offline workspace/all-targets run passed 129 tests across 19
suites; strict workspace Clippy, formatting, release build, staged installation,
desktop-file validation (category hints only), and staged picker version passed.
The isolated settings reload scenario also passed after the shortcut change.

The acceptance tray was stopped; its temporary picker and tray desktop entries
were removed. The user removed the two KDE shortcut assignments created by the
acceptance test. The isolated settings and history remain under ignored
`target/phase1e-physical/`. An isolated autostart scenario verified that saving
the preference creates the entry and disabling it removes the entry.

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

Phases 1A–1E jointly passed with the explicitly revised compositor-recovery
gate above. Board implementation may now begin.
