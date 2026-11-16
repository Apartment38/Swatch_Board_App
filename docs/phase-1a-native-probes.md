# Phase 1A — native investigation checkpoint

> **Historical:** The custom picker described here has since been replaced by
> KWin ColorPicker. See the [current handoff](current-handoff.md).

**Historical probe checkpoint. The completed native path now passes Phase 1A and is the only backend.**

## Autonomous test environment

After the request to test without human intervention, a private KWin virtual
session was created entirely beneath the project `target/` directory. It has its
own D-Bus, Wayland socket, two 1024×768 outputs, PipeWire, policy-only WirePlumber,
portal services, home/config/data/cache/state directories and virtual input device.
Only this disposable desktop's sharing chooser was automated. Human-session
permission dialogs were not automated. No additional host desktop-entry file was
created; the previously authorized temporary entry had already been removed.

The missing session manager initially left capture Paused. Adding WirePlumber's
policy-only profile established the private stream links without opening audio or
camera devices. Holding a fake-input device preserves pointer capability in a
compositor that has no physical seat devices. These are lab prerequisites, not
changes to the user's real compositor or media session.

## Native shield/cursor results

| Scenario | Underlying mouse counts | Keyboard focus | Captured blue samples under visible magenta cursor | Cursor frame callbacks | Cursor Presented events |
|---|---|---|---|---|---|
| Left confirmation (run 5) | Baseline 1/1 unchanged | Retained | 8/8 | 1 | **0** |
| Right cancellation (run 6) | Baseline 1/1 unchanged | Retained | 8/8 | 1 | **0** |
| 12-second idle timeout (run 7) | Baseline 1/1 unchanged | Retained | 59/59 | 1 | **0** |

For all three runs, the pointer-inclusive screenshot pixel at fixture origin +
(490,190) was programmatically verified as RGB (255,0,255). Raw video samples at
that location were RGB (0,0,255): **75/75** across these scenarios. Thus the visible
TEST cursor was excluded from the sampled video; an invisible-cursor submission
was not mistaken for capture exclusion.

All three probes exited 0, reported cursor/shield cleanup, explicitly closed the
ScreenCast session and restored pointer entry to the fixture. Their final fixture
counters remained one calibration press and one release, with keyboard enter=1,
leave=0. This is evidence for those scenarios only; it does not prove cleanup
while buttons remain held, failures at every stage, or full production integration.

The normal 80-square buffer (earlier run 3) and a 512-square buffer (runs 4–7) both
returned zero cursor presentation feedback. Adding an explicit `wl_surface.frame`
request produced a callback but still no `wp_presentation` Presented event.
No hardware/software cursor-plane inference is made from the larger buffer alone.

Raw JSON, probe/fixture logs, and private cursor screenshots are preserved in
[evidence/native-shield/](evidence/native-shield/). The Rust spike and guarded lab
orchestration are preserved in [spike 002](../spikes/002-native-shield/README.md).
They are not a replacement production backend.

## Separate Escape evidence

The earlier identified GlobalShortcuts probe's actual result was recovered and
checked: activation was observed, its session closed, the fixture stayed alive
while Escape was globally bound, and another Escape reached and exited the
fixture after closure. See
[native-escape-live-3-result.json](evidence/native-shield/native-escape-live-3-result.json)
and its probe/driver traces. This was a separate human-authorized real-session
probe; it was not a combined Escape+shield acceptance test. Its temporary host
application entry was removed after testing.

## Automated checks

The full offline workspace/all-targets run passed **66 test executions**:
31 picker, 4 branding, 1 input helper, 18 shield example (including reused capture
and portal module tests), 9 core and 3 placement. Two other examples contained
zero tests. This total intentionally includes reused module tests; it is not a
claim of 66 unique independent behaviours.

Strict all-targets Clippy passed. Formatting was corrected only in the native
probe/input-helper sources identified by `cargo fmt --all -- --check`; the format
check then passed. The release shield example built successfully.

## Remaining blocker

Actual display authority is still not established for cursor-role rendering.
Neither a successful cursor request, a screenshot, nor a frame callback is being
relabelled as a genuine presentation acknowledgment. The candidate must not be
promoted as a completed picker. The source-grounded review recommended an alternative
normal layer-shell lens with conservative capture self-occlusion protection.
[Spike 003](../spikes/003-clean-overlay/README.md) subsequently passed two targeted
live tests: genuine lens/hide Presented events, clean post-barrier video, and white
selection after moving into an old blue lens footprint. The second test requires
VIDEO PTS after local monotonic receipt of the hide acknowledgment. Screenshots
independently distinguished old blue lens pixels from the uncovered white fixture;
focus and baseline mouse counters were preserved. Its 21 example tests and strict
example Clippy pass; the harness's two parser tests also pass.

This establishes feasibility, not the full release gate. The opt-in `--native`
application path is now implemented and independently passed integrated Escape
release and left-selection/self-occlusion scenarios; see
[the integrated acceptance snapshot](phase-1a-native-integration.md).
The default backend remains unchanged. Multi-output routing is being extended
separately; fractional scale/output switching, transformed outputs, held-button
failure safety and final all-target verification after those edits remain outstanding.
No Phase 1B, tray or Board work has started.
