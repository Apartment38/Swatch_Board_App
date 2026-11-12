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
below.** Phase 2 Board shell also passed physical Board/picker acceptance.
Phase 3 Color Mixer passed user-reported physical desktop acceptance.
Phase 4 Palette Editor passed user-reported physical desktop acceptance.
**Phase 5's runtime-authored external-plugin hard gate passed on the actual native
KDE Wayland desktop with user-operated GUI interactions**, after fixing palette
drop consumption and clipped export layout. Database and filesystem readback
verified saved links/colors, exported bytes, overwrite behavior, and process exit.
Durable evidence is under `docs/evidence/phase5-physical/`. This is not automated
native-input coverage or physical testing of every bundled target plugin.
The template intermediate-allocation defect is addressed by an exec-based,
resource-limited rendering helper; this is not a filesystem/network sandbox.
Automated palette-link and PNG-rendering acceptance also passes. The current
Computer Use process lacks display environment; a diagnostic with the real KDE
session environment reaches AT-SPI and portal capture, but the native KWin input
backend refuses raw target-bound input. The user therefore performed physical
interactions; supplied screenshots are separately identified as such.

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

## Phase 6 packaging and documentation checkpoint

Arch-first packaging and release documentation are now present: `packaging/arch/PKGBUILD`,
`share/metainfo/org.apartment38.SwatchBoard.Board.metainfo.xml`, staged installer
integration, an installed-helper release verifier, the declarative
`plugins/example-terminal/` template, and release, troubleshooting and test
matrix guides. Installer contract tests cover the new AppStream and fixture
assets. The existing bundled-plugin golden test now also renders the starter
fixture through the real helper. Desktop-file category hints were corrected to
one main category, and all launchers validate cleanly.

Verification on 2026-11-11: initial `cargo build --workspace --locked
--offline` passed; `python3 packaging/test_install.py` passed (2); `cargo test
--workspace --all-targets --locked --offline` passed: 178 passed, zero failed,
one pipe fixture intentionally ignored across 23 suites; strict
workspace/all-targets Clippy and rustfmt check passed. `appstreamcli validate
--no-net` passed. The final
`packaging/verify-release.sh` run passed: release build, installer contract,
staged six-binary checks, staged picker help/version, actual installed-helper
render matching the fixture golden bytes, desktop-file validation, and
AppStream validation. The helper used only staged installed plugin data. No
GUI, user settings, grants, compositor restart, publication, or push was used.

On CachyOS (Arch-derived), the parent independently repeated the full offline
workspace build/tests, strict Clippy and formatting checks. Review found and
corrected missing Git/Clang build prerequisites and made the normal debug helper
build explicit before recipe tests. Four packaging tests pass, including RED-to-
GREEN regressions for the missing Git dependency and missing release validators.
The strengthened staged gate passes all four plugin golden renders, byte-for-byte
plugin/schema inventory and license checks, and mandatory desktop/AppStream
validation. System-PATH `namcap` now validates the recipe without warnings;
the earlier shim error was an interpreter PATH issue, not a missing installation.
`makechrootpkg` remains unavailable. Source-payload and pinned-recipe checkpoint
commits are separate so the recipe can name the exact immutable payload revision.
No Arch clean-chroot package build has been performed.

The package recipe pins a Git commit and the Cargo lockfile but has not had an
Arch clean-chroot build or package inspection. Fedora/openSUSE, release bitwise
reproducibility, dependency license audit, full assistive-technology review,
and broader physical acceptance remain open. The safe compositor substitute is
private-KWin restart/recovery plus normal manual logout/login. Never restart the
live KWin service; that previously ended the session and required reboot. The
test matrix in `testing.md` and release gates in `release.md` retain these limits.

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
gate above.

## Phase 2 Board shell

`swatch-board` is an independently launched eframe process. Open Board is enabled
in the tray and the installer includes its executable and desktop launcher. Tabs
are Color Mixer, Palette Editor, and Theme Generator, with shared scrollable
history. The Mixer is implemented in Phase 3; Palette Editor and Theme Generator
remain assigned to Phases 4 and 5.

The Board loads shared settings and SQLite history without a tray, subscribes to
tray changes, refreshes history on reconnect, and reloads settings while open.
Its independent D-Bus name handles second-launch activation. Settings launches
independently; closing Board exits its process and does not terminate the tray.
Selected tab and native window geometry persist through eframe storage.

The pinned winit Wayland backend ignores visibility and focus commands and
cannot unminimize a window. Board therefore closes/recreates its native window
for explicit activation and for the optional hide-during-pick preference. Before
a hidden pick, its exit callback commits a NULL buffer to the borrowed Wayland
surface and roundtrips; only after that unmap does it ask the tray to pick.
It retains the Board service and state while awaiting completion, and recreates
the window after selection, cancellation, failure, or tray loss. No private KDE
window-management protocol or KWin script is used.

An isolated D-Bus worker scenario passed standalone load, duplicate activation,
no-tray rejection, authoritative history refresh on signals/connect/reconnect,
settings reload, completion-before-method-reply, cancellation, picker error,
and tray disappearance while picking.

In a private KWin Wayland desktop, the actual Board and tray with a synthetic
picker verified hiding before selection, cancellation, and failure; restoration
in the same process; a saved `#247AC3` capture in the strip; history in all tabs;
second-launch activation; independent Board survival after tray Quit; and the
Settings link without a tray. Normal window close exited the Board process,
removed its D-Bus name, and left no Board window. These are not physical-picker
or physical-desktop acceptance results. Isolated screenshots/logs remain under
ignored `target/phase2-lab/`.

On the physical KDE Wayland desktop, the user approved the Phase 2 acceptance
run and confirmed picking from Board, hide/restore, the matching clipboard value
`#1B3242`, shared history in every tab, right-click cancellation without a new
capture, and normal close/reopen through the tray with history and tab preserved.
The original Board process exited with status 0. `GetLastColor` returned
`#1B3242`; `GetHistory(10)` contained two captures, `#644744FF` and `#1B3242FF`.
Picker logs recorded those two genuine presented selections, a right-click
cancellation, and a later cancelled screen-sharing request reported as failure.
Both successful picker runs and the right-click cancellation closed their
ScreenCast and GlobalShortcuts sessions. Physical visual observations are
user-reported because desktop capture remains unavailable to automation.

Formatting, strict Board/tray all-targets Clippy, five existing tray supervision
tests, debug Board/tray build, staged installation, and Board desktop-file
validation passed (the validator emitted only a multiple-main-category hint).
The throwaway integration and lab scripts were removed. Private services and
physical acceptance processes were stopped, temporary desktop registrations
were removed, and the user confirmed removing the temporary C/H shortcut
assignments. Test data and logs remain under ignored `target/phase2-physical/`;
normal user history was not used.

## Phase 3 Color Mixer

The Board provides editable HEX, RGB, RGBA, HSL and HSV values, approximate CMYK,
configured-format copying, selectable and draggable history, weighted sRGB and
OKLCH mixing, undo/redo, and an active in-memory draft palette. The draft survives
window recreation but is discarded on Board exit; palette saving is Phase 4.
OKLCH mixing uses the first chromatic source as the shortest-arc hue reference,
ignores achromatic hues, and clips out-of-gamut linear RGB channels.

Focused Board and color package tests and strict package Clippy passed. During
physical acceptance, the first launch used an older executable; rebuilding Board
made history selection and dragging available. The user then reported passing
editing, invalid-input handling, weighted sRGB/OKLCH mixing, copying, and undo/redo.
After clarifying that the active draft is separate from capture history, the user
confirmed that applying and adding red and then blue retained both draft swatches.
An input-driven egui regression test also verifies two appended draft colors and
undo/redo of the second addition. No palette replacement defect was reproduced by
that exact physical sequence.

The focused Board/color run passed 16 tests across four suites, and strict
all-targets package Clippy passed. These desktop results are user-reported:
automation's AT-SPI connection was refused, and Wayland capture is unavailable
without the tool's `wayland-pipewire` feature. Normal shared history was used;
the user's new captures were not removed.

Board and tray remain running for further interactive use. The development picker
entry at `~/.local/share/applications/org.apartment38.SwatchBoard.desktop` points
to `target/debug/swatch-board-picker`; it is required for portal registration.
Portal approvals remained manual.

### Spectrum and layout revision

The mixer now exposes a clickable/draggable hue–saturation spectrum, a vertical
HSV value bar, and individual HSV, RGB, and alpha numeric controls. HSV uses
degrees and percentages; RGB/alpha use 0–255. Hue is retained for achromatic
selections. These edits use the existing undo/redo state.

History is a resizable left sidebar, collapsed from its header and reopened with
the toolbar's **Color history** toggle. The active draft and its **Add current
color** action are pinned in a bottom panel across all tabs; its swatches scroll
horizontally without moving the panel.

The user reported a spectrum ceiling around 50% saturation. A Spectacle window
capture showed that the scroll area's retained offset clipped the spectrum's
upper half. Moving spectrum/channel controls above the scrolling details fixed
that path; spectrum sizing adapts to available height. Pointer endpoint margins
also make exact 0%/100% accessible. The user subsequently confirmed full
saturation, channel edits, pinned palette, and collapsible history passed.

Five focused Board tests, strict all-targets Board Clippy, and the debug Board
build passed. The spectrum pointer regression checks full saturation and
preserved brightness/alpha. Native automation capture remains unavailable, but
Spectacle successfully captured the actual Board window as a fallback; ignored
evidence is under `target/mixer-spectrum-window.png`,
`target/mixer-spectrum-fixed.png`, and `target/mixer-spectrum-final.png`.
The final capture confirms the taller spectrum remains fully visible.
Normal history and manual portal approvals remain unchanged.

## Phase 4 Palette Editor implementation

The Board now supports seeded OKLCH generation, editable and lockable swatches,
history application, palette undo/redo, saved palettes, and separate persistent
generation snapshots. Mixer and Theme-tab context use the active palette.
Storage migrates version 1 databases transactionally to version 2. The README
documents the gamut/contrast behavior and intended future theme-link policy.

The affected Board, color, and storage suites passed 33 tests across seven
suites; strict all-targets Clippy for those packages passed. The user reported
that the physical desktop scenarios passed: generation and locks, editing and
undo/redo, history click and drag, saved and generated palette operations, and
Mixer/Theme context.

The user subsequently requested and verified the pinned-panel revision:
swatches appear only there, can be reordered by dragging, expose hover lock/delete
buttons, and have editable HEX values. History drag still works. The user then
reported that locked swatches could still be changed manually; after the fix,
the user confirmed locked HEX editing, history replacement, and deletion are
prevented, unlocking restores those actions, and reordering preserves the lock.
These physical observations are user-reported because native automation's
AT-SPI connection and capture are unavailable. Board tests, strict all-targets
Board Clippy, formatting, and a debug Board build passed after the lock fix.

## Phase 5 automated implementation checkpoint

The Theme Generator now uses a shared data-driven egui scene renderer, runtime
plugin discovery/reload, role overrides and named saved-palette links, undo/redo,
recoverable drafts, named theme projects, export preview, and explicit overwrite
confirmation. Ghostty, Zed, and KDE are data-only packages with golden outputs.
Storage migration 3 adds theme projects, indexed assignments, drafts, and named
palette swatches. Packaging stages plugin data and published schemas. See
[plugin-authoring.md](plugin-authoring.md) for the API and limitations.

Input-driven headless egui tests create a new package after the test executable
is built, install it in isolated child-process XDG storage, reload/select it,
open a hidden drawer, select and drop onto a text role, edit values with keyboard
input, undo/redo, save/read back SQLite state, restore the draft, reopen a saved
project, and preview/export actual files with overwrite protection. Drag payloads
are injected at the egui boundary: this is not a native drag-source test or
physical KDE acceptance. Palette links and image assets still need dedicated
interactive acceptance. Tests use isolated data, not normal user history.

Reproduced defects fixed in this checkpoint:

- Background nodes could consume drops intended for foreground nodes.
- Text-only semantic role bindings were not interactive.
- New projects retained the previous selected role; corrected color text retained
  a stale validation error.
- Output names could collide with staging filenames, corrupting or deleting
  exported files; staging now avoids requested output basenames.
- NUL-containing declared paths were accepted by the loader.
- Invalid JSON could replace a saved theme and clear its role index; saves now
  reject invalid JSON before modifying persisted state.

Parent verification of the integrated tree:

- `cargo test --offline --workspace --all-targets --locked`: 161 passed.
- `cargo clippy --offline --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `cargo build --offline --workspace --locked`: passed (debug, not release).
- Staged install using debug binaries: all five executables, three plugin
  packages and four schemas present under isolated `target/phase5-stage.*`.
- All nine bundled manifest/role/scene documents passed their published schemas.

Local verification logs are ignored under `target/phase5-{workspace-tests,clippy,build}.log`.
No native app was launched, no live themes were applied, no normal desktop
settings/history were changed, and no commits were pushed.

### Remaining gates at the initial checkpoint (superseded below)

**Security first:** MiniJinja fuel and output size limits do not contain
intermediate allocations. A reviewed probe captured an 8 MiB string inside a
block while emitting a tiny valid file. The renderer runs in Board, so hostile
templates can exhaust Board memory. Implement a resource-limited rendering
worker or a deliberately restricted template language before declaring
untrusted third-party packages safe. No claim of complete sandboxing is made;
use only reviewed trusted templates meanwhile.

Then exercise palette-link propagation/renames/deletion, asset handling, and
physical external-plugin acceptance on KDE (including real history/palette drag
sources, layout, save/reopen, and export). The full Phase 5 hard gate has **not**
passed. Do not advance to declaring Phase 6 complete or repeat disruptive KWin
restart testing. Per-file export is atomic; a multi-file export is not a
transaction and can leave earlier files written on later failure.

## Phase 5 rendering containment and link/asset checkpoint

`Plugin::render` now launches a fresh sibling `swatch-board-theme-worker`
executable for each request, with no shell or in-process fallback. Before reading
request data the worker sets RLIMIT_AS to 64 MiB, RLIMIT_CPU to two seconds, and
RLIMIT_CORE to zero. The parent enforces a three-second wall deadline, exchanges
bounded data over nonblocking duplex pipes, and kills/waits on errors. Requests
and responses are capped at 16 MiB each; results are revalidated against declared
outputs and limited to 4 MiB per file and 8 MiB aggregate. The worker receives
validated template/role data, clears its environment, and uses `/` as its cwd.
This is resource containment, not filesystem/network sandboxing; ordinary OS
permissions remain. Large otherwise-valid packages can exceed these budgets.

The installer now requires and stages six executables, including the helper,
and does not create a desktop launcher for the helper. Build the whole workspace
before running tests; a standalone package test on a fresh target directory may
otherwise report that the sibling helper is missing. No unsafe fallback exists.

Five new headless tests cover named saved-palette links and propagation, saved
source identity/rebinding, undo/redo, rename/removal errors without assignment
loss, independent overrides, stale-preview rejection, PNG texture/image meshes,
and corrupt asset rejection/reload recovery. A reproduced bug where linked
editable text kept the old color after palette propagation is fixed; unfocused
linked text refreshes without overwriting active typing.

Parent verification of the integrated checkpoint:

- Full offline locked workspace debug build passed.
- Full offline locked workspace/all-targets tests: 175 passed, one ignored
  subprocess fixture. Supervision tests explicitly invoke that fixture.
- Strict workspace/all-targets Clippy, workspace formatting, and whitespace checks passed.
- Two Python installer regression tests passed (synthetic executable fixtures;
  these are contract tests, not runtime application acceptance).
- An actual isolated staged installation contained all six executables, and its
  installed helper rendered the installed Ghostty package successfully.
- Release helper build passed. A 64 MiB intermediate-allocation attack exited with
  SIGABRT under its limit, emitted no protocol output, and a subsequent normal
  release-helper render passed. Debug parent-side regression verifies the caller
  survives the failed render and can render again.
- Timeout, stalled input, oversized output, simultaneous duplex traffic, missing
  executable, protocol validation, resource-limit initialization, and child-reaping
  checks passed in the Rust tests.

Local logs: `target/phase5-isolation-{build,tests,clippy,release-worker}.log`.
No native apps launched, no live themes applied, no normal history/settings
changed, and no pushes performed.

### Next gate

The full Phase 5 hard gate still requires physical KDE acceptance: runtime install
and reload of a newly authored plugin, hidden-scene navigation, real history and
palette drag sources, role/link editing, save/reopen, export, and native layout.
Headless PNG meshes and injected egui drag payloads are not physical appearance or
native input evidence. Do not repeat the disruptive physical KWin restart test.

## Physical Phase 5 acceptance in progress

A native Wayland Board acceptance session uses isolated data under
`target/phase5-physical/`. The launcher removes DISPLAY and native Wayland
protocol traffic was verified. The external `physical-fixture` package was
created after building and launching Board. Synthetic history and a named saved
palette were seeded through the production storage API for real GUI drag tests;
these rows are not real picker captures. Normal user data was not modified.
The user performs physical interactions because cua-driver currently refuses
safe target-bound KWin raw input. The KWin investigation handoff is saved at
`/home/erick/Desktop/KWin-Computer-Use-Handoff.md`.

The user reported first-round plugin reload, hidden submenu navigation, history
drag, role text editing, undo/redo and saving passed. Database readback confirmed
project `Physical acceptance`, scene `submenu`, and after clarification/re-save
Accent override `#A344CCFF`.

Second-round palette-to-preview dragging failed physically: the cursor changed
to a drag indicator but dropping did nothing. Reproduced with a typed palette
payload and a full pointer-driven drag from the pinned palette. Root cause:
egui's `take_payload<T>` consumes the payload even when its type mismatches;
the history-color probe discarded `PaletteDrag` before fallback. The handler
now inspects payload type before consuming. Added two RED-to-GREEN regressions,
including real source initiation without payload injection. The harness models
the pinned panel's production height bounds to keep source coordinates stable.

Integrated verification: 177 workspace/all-targets tests passed (one subprocess
fixture remains intentionally ignored during normal discovery), strict workspace
Clippy, formatting, whitespace checks, and debug Board build passed. Logs:
`target/phase5-palette-drop-{tests,clippy}.log`.

The running acceptance window still uses the previous executable until closed
and relaunched. Next: user saves/closes that window, relaunch the same isolated
native session, reopen `Acceptance palette` and the theme, then recheck palette
drop before continuing link propagation, export, overwrite protection and clean
close/reopen. The full physical gate is not passed; do not count the automated
repair as physical verification.

### Physical follow-up: links passed; export layout repair awaiting retest

After relaunching the corrected build, the user confirmed native palette dragging
worked; draft readback confirmed Accent override `#81A1C1FF`. Initial link testing
was ambiguous (draft held an override), so the session was reopened and tested
one transition at a time. Readback then proved `Link("Accent")` to the saved
source; the user confirmed editing the pinned swatch to `#46B878` updated both
the preview and role text. SQLite confirmed the green saved palette and saved
linked theme. The supplied screenshot independently showed the green preview,
role text and Accent link dropdown, and Saved state.

Preview export appeared to do nothing and window scrolling did not reveal output.
A direct diagnostic worker render returned the expected green theme; that was
not counted as GUI acceptance. The screenshot showed the controls flush against
the pinned palette. The Theme Generator had inner role/output scroll areas but
no outer scroll container, leaving expanded export content below the viewport
unreachable. A short-window regression reproduced this using wheel input and
clipped painted-text bounds (not merely model preview state). Wrapping the theme
tab content in a dedicated vertical scroll area fixes reachability while leaving
the separately pinned palette outside that scrolling region.

After the repair: 178 workspace/all-targets tests passed, strict workspace Clippy,
formatting, whitespace checks and debug Board build passed. The regression went
RED before the production change and GREEN afterward. Logs are under
`target/phase5-export-layout-{tests,clippy}.log`. Physical export, overwrite and
final reopen acceptance still require a relaunch and user verification; the old
window must be saved/closed before starting the rebuilt executable.

### Physical external-plugin workflow accepted

The user confirmed the repaired export preview became accessible and exported
successfully. Direct file readback matched:

```text
surface=#202830
accent=#46B878
text=#FFFFFF
```

With overwrite unchecked, the user reported the expected refusal and the file's
inode, nanosecond modification time, and SHA-256 remained unchanged. With explicit
overwrite confirmation, replacement was verified by changed file identity/time
and unchanged expected bytes. After saving and closing, Board's exact process
was gone. A final native Wayland relaunch recovered the saved link and submenu;
the user reopened the saved source palette and project, confirmed the green
linked state, and closed normally. Final verification found the process absent,
SQLite integrity `ok`, the saved Accent link intact, the source swatch green,
and the export still correct.

This satisfies the physical runtime-authored fixture workflow: package creation
after host build/launch, reload without rebuilding, hidden view interaction,
history and palette drag sources, direct editing and undo/redo, named links and
propagation, save/reopen, export preview, overwrite refusal/confirmation and clean
shutdown. GUI observations are user-reported; the user-supplied screenshot proves
only its visible state, not the entire interaction sequence. Headless tests and
file/database assertions remain separately identified.

Durable evidence: `docs/evidence/phase5-physical/acceptance-result.json`, the exact
fixture data package, verified exported text, and user-supplied linked-theme
screenshot. Test runtime/logs remain under ignored `target/phase5-physical/`.
No acceptance Board remains running; no tray/picker or permission grants were
started for this workflow. Normal user data and settings were not changed.

Remaining development is Phase 6 packaging/documentation/hardening and any broader
coverage it requires. This acceptance does not claim native automated KWin input,
new picker accuracy tests, multiple physical displays/scales, or physical runtime
validation of all bundled Ghostty/Zed/KDE packages. Nothing was pushed.
