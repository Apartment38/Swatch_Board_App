# 003: Presented normal overlay with a capture self-occlusion barrier

## Verdict: PARTIAL — live feasibility demonstrated, not Phase 1A complete

The cursor-role approach in spike 002 could not establish genuine presentation
acknowledgments. This alternative uses a normal, small layer-shell magnifier,
plus separate transparent full-pointer-input shields with no keyboard interaction.
The magnifier has an empty input region and never binds a keyboard.

## Actual live result

Two isolated KWin runs passed the targeted self-occlusion scenario:

1. The pointer was over a controlled blue pixel at global `(512,227)`.
2. A blue magnifier received a genuine per-commit `wp_presentation` Presented event.
3. The harness moved to `(606,227)`, inside that magnifier's old rectangle. An
   independent screenshot there initially contained the old blue lens pixels;
   the actual fixture underneath was white.
4. The guard invalidated the selectable sample, submitted a **transparent buffer
   while keeping the lens mapped**, and received Presented for that hide commit.
5. A subsequently decoded VIDEO frame passed the clean barrier. Only then was the
   contamination history reset and a new, off-source white magnifier submitted.
6. That white commit received Presented. An independent cursor-free compositor
   screenshot of the uncovered source pixel was RGB `(255,255,255)`.
7. Left release returned `#FFFFFF`, not the old lens's blue. The underlying fixture
   retained keyboard focus and its calibration mouse counts stayed at 1/1.
   Shields/lens were removed and the capture session explicitly closed.

The second run strengthened the barrier to require video PTS **strictly later
than local CLOCK_MONOTONIC receipt time of the hide's presentation event**, rather
than merely later than its presentation timestamp. Its preserved trace contains:

- actual hide presentation: `131746478063474` ns;
- local receipt cutoff: `131746479887227` ns;
- qualifying video PTS: `131746561396804` ns.

These are recorded observations, not reusable timing constants.

Two additional live runs exercised right-click cancellation and the 12-second
idle timeout after the same old-footprint recovery. Both closed the capture
session, restored pointer entry, kept keyboard focus and calibration counts 1/1,
and returned no selected color. Evidence: [right cancellation](../../docs/evidence/clean-overlay/right-cancel/)
and [idle timeout](../../docs/evidence/clean-overlay/idle-timeout/). The separate
harnesses are `run_private_clean_overlay_live_2.py` and `_3.py` in `testing/`.
These tests do not establish cleanup while a button remains held.

## Guard contract

- Every potentially captured submitted lens rectangle remains contaminating,
  including old positions and commits that were not acknowledged as presented.
- A source neighborhood intersecting any such rectangle cannot be sampled.
- The hide invalidates the selectable sample immediately; late old-generation
  feedback cannot restore it. Only genuine presentation events authorize samples.
- The transparent hide must itself be presented. Missing/incomparable clock/PTS,
  discarded hide, bounded-history/buffer exhaustion and timeouts fail closed.
- Linux CLOCK_MONOTONIC is checked against the Wayland presentation clock. Native
  `clock_gettime` supplies the receipt cutoff. Cursor-only metadata updates retain
  their previous video PTS in CaptureFeed and cannot satisfy a later cutoff.
- The PTS ordering argument is **KWin 6.8 implementation-specific**, not a generic
  PipeWire contract. See the source references below.

## Build and test

```sh
cargo test --offline -p swatch-board-picker --example phase1a_clean_overlay_probe
cargo clippy --offline -p swatch-board-picker --example phase1a_clean_overlay_probe -- -D warnings
cargo build --release --offline -p swatch-board-picker --example phase1a_clean_overlay_probe
```

The example passed 21 test executions: reused capture/portal tests plus the guard
and native monotonic-clock tests. The new receipt-cutoff test was observed failing
when video between presentation and receipt was incorrectly accepted; it then
passed after the correction. The native clock test also went red with a zero stub
and green with a real CLOCK_MONOTONIC read.

Preserved evidence: [baseline](../../docs/evidence/clean-overlay/baseline/) and
[receipt-cutoff run](../../docs/evidence/clean-overlay/receipt-cutoff/).
The `testing/` Python files are orchestration, not application implementation.
They assert private D-Bus/runtime/Wayland identity, select **only Virtual-0**, and
verify screenshots, presentation events, source colors and fixture counters.
Stage them into project `target/` before executing the harness, alongside the lab
scripts from spike 002. These scripts are bound to this workstation's project
path and two-output 1024×768 test geometry; they are not a portable public API.

```sh
# In one terminal (private lab only):
python target/run-isolated-native-lab.py
# After NESTED_KWIN_READY, another terminal:
python target/test_clean_overlay_driver.py
python target/run_private_clean_overlay_live_1.py
# When finished:
python target/private-lab-command.py --stop
```

Never automate a human-session sharing dialog with these helpers.

## Limits

- Single selected capture stream/output only; both virtual outputs are shielded.
  Cross-output motion, rotated/negative-origin/fractional-scale operation are not
  validated by this spike, and unsupported transitions are rejected.
- No integrated GlobalShortcuts Escape cancellation. Separate Escape feasibility
  evidence exists, but it does not establish this combined picker lifecycle.
- Fixed radius/zoom/size and throttled experimental rendering; no production UI or
  performance acceptance claim. The selected sample is the last acknowledged
  magnifier sample, not a promise that an animated source is unchanged at click.
- Held-button failure/timeout cleanup is bounded but not proven click-safe. A
  no-sample click is logged as failure; the standalone prototype's exit status is
  not a sufficient accuracy verdict. The harness asserts the exact selected color.
- This has **not** replaced the production backend. Production integration is
  separate work, with its own safety tests and live acceptance gate.

## Source basis

- [KWin presentation clock](https://github.com/KDE/kwin/blob/Plasma/6.8/src/wayland/presentationtime.cpp#L21-L24): CLOCK_MONOTONIC.
- [VsyncSource](https://github.com/KDE/kwin/blob/Plasma/6.8/src/utils/vsyncsource.cpp#L29-L51): schedules the synthetic capture timestamp.
- [PreciseTimer](https://github.com/KDE/kwin/blob/Plasma/6.8/src/utils/precisetimer.cpp#L16-L55): CLOCK_MONOTONIC timerfd with an absolute deadline.
- [ScreenCastStream](https://github.com/KDE/kwin/blob/Plasma/6.8/src/plugins/screencast/screencaststream.cpp): records current scene video after the timer and writes the timestamp into SPA header PTS.

A screenshot or frame callback alone is not being used as actual presentation
proof. The normal lens and its transparent hide both produced genuine Presented
events in the live traces.
