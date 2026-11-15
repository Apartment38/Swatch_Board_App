# Swatch Board — resumed-work handoff

> **Historical:** The custom picker described here has since been replaced by
> KWin ColorPicker. See the [current handoff](current-handoff.md).

> **Completed:** Work resumed after checkpoint `f2d436e`. Mixed-scale routing,
> transformed capture mapping, animated presented authority, held-button failure,
> readout/marker, physical 1.75×/rotated/lock acceptance, and the private failure
> matrix now pass. Observed active-capture CPU fell from 73.2% to 12.6% after
> frame-conversion throttling. The Rust-owned picker is the default and only
> backend; the obsolete KWin ColorPicker path was removed. Phase 1A passed. See
> [native integration acceptance](phase-1a-native-integration.md).

The stopped-work history below explains the checkpoint; its pause and unfinished
state are no longer current instructions.

## User direction and boundary

**Work is paused at the user's request.** The user asked to stop, write this
handoff, then commit the changes. Do not restart development, agents, the private
lab or desktop tests until the user requests a resume. The requested commit is a
checkpoint, **not** a Phase 1A completion/release assertion. No push was requested.

Project: `/home/erick/Projects/Swatch_Board_App`, branch `work`.
The application and native examples remain Rust. Python files are test orchestration.
All task-created files/configuration remain inside the project. The one previously
authorized temporary host desktop entry was removed earlier; no new host entry was
installed. Never automate the human desktop's permission dialogs.

## Current release status

**Phase 1A PASSED.** The Rust-owned presented-sample implementation is the
default and only backend. `--native`, `--repeat`, and the obsolete KWin
ColorPicker implementation were removed. No Phase 1B work has started.

## Implemented

- Active packages, CLI and documentation renamed from colorboard to Swatch Board;
  historical conversation export retained unchanged. The implementation plan is
  `swatch-board-agent-implementation-plan.md`.
- Latest-only authorized capture feed, cursor-only timing preservation, geometry
  checks, viewporter rendering, bounded dispatch, explicit portal/session cleanup,
  and actual presentation feedback.
- Native normal-layer magnifier and separate full-pointer-input, no-keyboard
  shields. Selection is tied to an exact genuinely presented sample, not a queued
  buffer or frame callback. Output is `#RRGGBB` on stdout; diagnostics use stderr.
- Bare Escape through GlobalShortcuts, subscription before binding, trigger checks,
  explicit session closure, paired mouse-release handling, typed cancellation and
  actual errors for no-sample left selection.
- Self-occlusion guard tracks all distinct submitted lens footprints. It invalidates
  selection, presents a transparent non-null buffer while mapped, and requires fresh
  VIDEO PTS strictly later than local CLOCK_MONOTONIC receipt of that hide event.
  Cursor-only metadata and old stream/generation feedback cannot clear the barrier.
- Multiple authorized normal-transform monitor streams are routed by actual pointer
  shield/output origin, not stream ordering. A monitor transition hides the old lens,
  waits for its genuine presentation plus fresh destination-stream video, then
  recreates the lens. Negative origins and fractional geometry have unit coverage.
- Native fixture supports `--fullscreen-output EXACT_OUTPUT_NAME` through ordinary
  xdg-toplevel fullscreen requests. Its parser went RED→GREEN and live placement
  on Virtual-1 was verified.

## Verified live observations

### Legacy backend

Seven static selections and both cancellation paths passed, but the animated
cyan/magenta target produced a real KWin-vs-presented-sample mismatch. KWin has no
request-owned remote cancellation for its independent grab. These remain reasons
not to claim the default path passes Phase 1A.

### Probes

Cursor-role rendering was visible and excluded from raw video, but produced zero
Presented feedback even with larger buffers and explicit frame callbacks. Do not
resurrect it as strict authority. The normal-layer alternative produced real lens
and transparent-hide Presented events and passed selection, right cancellation and
idle-timeout self-occlusion scenarios. See `spikes/003-clean-overlay/README.md`.

### Integrated native application

- Escape cancelled, destroyed lens/shields and closed both portal sessions. The
  fixture remained alive; another Escape then reached and exited it. Active-pick
  mouse and keyboard-focus counters stayed unchanged from post-consent baselines.
- Moving from blue `(512,227)` into the old blue lens at `(606,227)` recovered the
  underlying white target after the hide/fresh-video barrier. Independent cursor-free
  screenshots distinguished blue lens pixels from white fixture pixels. Left release
  printed exactly `#FFFFFF`; counters were preserved and sessions closed.
- A **two-authorized-monitor, 1×/1×** run passed: primary blue → secondary red →
  primary white. Both monitor transitions had hide receipt cutoffs followed by
  strictly newer video from distinct destination streams. Final stdout `#FFFFFF`,
  exit 0, both fixtures' mouse/focus counts unchanged, both sessions closed.

Integrated evidence already preserved in `docs/evidence/native-integrated/`.
Probe evidence is in `docs/evidence/clean-overlay/` and `docs/evidence/native-shield/`.
The newest normal two-monitor and failed fractional-matrix traces are also being
preserved with this stopped-work checkpoint under `docs/evidence/native-matrix/`.

## Exact unfinished issue at stop

The mixed 1×/1.75× virtual-output experiment **did not pass**. Initially the private
KScreen report showed Virtual-1 physical mode 1024×768, logical geometry 586×439,
scale 1.75. During subsequent capture/screenshot attempts, its reported mode changed
progressively: captured sizes included 585×439 and then 334×251. The final private
KScreen report showed mode 334×251, logical 191×144; the fullscreen fixture received
191×143. The picker failed closed with `portal/Wayland output geometry mismatch`.

The cause of that private virtual-output resizing has **not** been established.
Do not blame the picker, KScreen, screenshots or a specific KWin bug without tracing
it. Do not label this a successful fractional/physical-pixel acceptance test.

A preliminary, still-present change in `native.rs::route_for_output` permits up to
one pixel of startup portal/Wayland logical-size rounding, with positive dimensions
and larger mismatches rejected. Its new regression test was observed RED→GREEN.
**That change did not fix the mixed-scale live failure.** It was based on the first
rounding hypothesis, before progressive mode changes became apparent; reassess or
revert it on resume rather than building more guesses on top. Frame identity and
geometry-change rejection remain in place. No rollback was performed after the
user's stop request.

CLI help/startup text was updated to describe authorized normal-transform monitors,
not the earlier single-monitor restriction. The release binary was rebuilt after
the small routing-tolerance change. No further application changes should be made
merely to prepare this checkpoint commit.

## Other remaining gates

- Stable fractional-scale and physical-pixel acceptance, negative origins, required
  rotated/transformed configurations (currently explicitly rejected).
- Animated-content native selection checked against the exact presented decision.
- Failure cleanup while buttons remain held: bounded cleanup is explicitly **not**
  proof of click safety. Do not equate the unit rejection with live safety.
- Broader combined failure/cancellation/hotplug/geometry-change resource matrix.
- Native readout/marker/performance requirements and the eventual default-path
  decision against the implementation plan. No automatic promotion is authorized.
- Full integrated final checks after all future modifications. Earlier totals are
  snapshots and examples reuse capture/portal modules, not unique behavior counts.

## Shutdown / resume environment

All delegate tasks had finished when stop was requested; no live subagents remained.
The private lab received `--stop`, and its tracked supervisor reported
`NESTED_KWIN_STOPPED` and exited. It must not be assumed alive on resume.

Lab state is under `target/isolated-native-lab-1/`; runtime `session-env.json` is
private runtime data, not an artifact to commit. Target scratch files may be pruned.
The stopped lab had a degraded second-output mode/scale. Start a clean, explicitly
validated private configuration on resume instead of trusting that geometry.

Important scripts:
- `target/run-isolated-native-lab.py` (preserved under spike 002 testing): private
  D-Bus, KWin, PipeWire, policy-only WirePlumber, portals; frontend uses `--replace`
  because the KDE backend can autoactivate it. The holder's normal expiry can race
  the supervisor deadline and be mislabelled failure; address separately if needed.
- `target/private-lab-command.py`: checks private bus/runtime; never inject through
  the real `wayland-1` or human session.
- `target/run-native-matrix.py`: activate primary fixture before launch, select both
  private monitors, then test blue→red→white transitions. First failed attempts were
  harness placement mistakes (chooser appeared on secondary; red target was wrongly
  addressed at blue local x=450). The successful normal case used secondary global
  `(1024+122,150)`. Do not report those harness mistakes as application bugs.
- `target/native-matrix-normal-result.json`, `native-matrix-normal.log`, stdout and
  PNGs: successful normal geometry matrix.
- `target/native-matrix-fractional-175.log`: current failed mixed-scale trace.

Private packaging: copy the project desktop entry into the lab's data/applications
and make Exec point to the actual project binary. The initial relative uninstalled
Exec did not register. This is private staging, not permission to install a host
entry. A saved private shortcut assignment can remove a later approval dialog;
observe the stage before clicking any former OK position.

## Stop-checkpoint verification

Fresh commit-preparation checks all passed against the stopped working tree:
- `cargo test --offline --workspace --all-targets --locked`: **107 passing test
  executions**, including repeated capture/portal tests in examples.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --offline --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo build --offline --workspace --locked`: passed.
- Tracked `git diff --check`: passed. After staging new evidence, the staged
  whitespace check reports one final blank line in the raw Cargo test log and an
  intentional Markdown hard-break in the renamed plan. These documentation/trace
  whitespace warnings are preserved, not treated as an application test failure.

Raw check output and structured results are preserved in
`docs/evidence/checkpoint/`. These do not change the failed mixed-scale live gate.
An independent read-only checkpoint review is a commit-preparation step, not
permission to resume implementation or auto-fix further features.

## Verification commands for a future resume

Run only when the user requests resumption or as explicitly scoped commit checks:

```sh
cargo test --offline --workspace --all-targets --locked
cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
cargo build --offline --workspace --locked
cargo build --release --offline --locked -p swatch-board-picker
```

The commit preparation records fresh check outputs separately in the handoff's
verification note. A checkpoint commit must not be tagged `[verified]` or described
as the milestone passing merely because compilation/unit tests pass.
