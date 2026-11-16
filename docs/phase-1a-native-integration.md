# Native application integration — resumed acceptance

> **Historical:** This acceptance concerns the former custom picker, not the
> current KWin picker. See the [current handoff](current-handoff.md).

**Phase 1A PASSED. The Rust-owned picker is the default and only backend.**

The resumed work fixed the backend's remaining known routing and display-transform defects, then exercised the fixes in both a private two-output KWin lab and the actual 2880×1800, 1.75× eDP-1 target. These are real compositor/portal/PipeWire/Wayland integration checks, not unit mocks.

## Resumed implementation

- Route portal streams to Wayland outputs by unique compositor origin. Portal `size` is retained as stream identity metadata; Wayland logical geometry is authoritative for pointer routing. A size difference is diagnosed rather than rejected because portal size is compositor-coordinate metadata and may differ from the stream or Wayland logical size.
- Preserve portal position/size identity checks after startup so a changed stream still fails closed.
- Support transformed outputs. Capture orientation is detected from actual frame/logical dimensions: compositor-oriented frames use normal capture mapping, while raw-oriented frames use the Wayland output transform.
- Render the native lens's live `#RRGGBB` readout and a two-tone center marker that remains visible over light and dark pixels.
- Extend the private input helper with bounded left/right button holds for failure-path acceptance.
- Throttle full-frame RGBA conversion to at most once every 75 ms on the 10 Hz presentation path.

## Live acceptance results

| Scenario | Observed result | Evidence |
|---|---|---|
| Mixed scale, two outputs | Passed blue-primary → red-secondary → white-primary routing; selected `#FFFFFF`; both transition barriers used destination video newer than their cutoffs; fixture mouse/focus counters did not change; sessions closed. KWin changed the virtual output mode once during capture startup, then it remained stable. | [result](evidence/native-resume/native-matrix-fractional-stable-result.json), [trace](evidence/native-resume/native-matrix-fractional-stable.log), [stdout](evidence/native-resume/native-matrix-fractional-stable.stdout) |
| Left-rotated secondary output | Passed selection on the transformed output with `#FF0000`; fixture counters did not change; sessions closed. KWin supplied an already compositor-oriented 768×1024 stream for the 768×1024 logical output. | [result](evidence/native-resume/native-matrix-rotated-left-result.json), [trace](evidence/native-resume/native-matrix-rotated-left.log), [stdout](evidence/native-resume/native-matrix-rotated-left.stdout) |
| Animated presented-sample authority | Passed: the genuinely presented center was `#00FFFF`, stdout was exactly `#00FFFF`, fixture counters did not change, and sessions closed. | [result](evidence/native-resume/native-animated-presented-result.json), [trace](evidence/native-resume/native-animated-presented.log), [stdout](evidence/native-resume/native-animated-presented.stdout) |
| Geometry failure while left button held | Passed this failure stage: output geometry changed while the synthetic button remained held; picker exited with failure; the later release reached neither fixture; both sessions closed. | [result](evidence/native-resume/native-held-failure-result.json), [picker trace](evidence/native-resume/native-held-failure.log), [input trace](evidence/native-resume/native-held-failure-input.log) |
| Native marker/readout | Visually observed the two-tone center marker and `#FFFFFF` readout on the live native surface over a white source. | [screenshot](evidence/native-resume/native-readout-visual.png), [trace](evidence/native-resume/native-readout-visual.log) |

KScreen rejected a requested negative output position with `Position of enabled output Virtual-0 is negative`; KWin normalized the usable virtual layout. Negative-origin and all eight transform mappings remain covered by deterministic geometry tests, but this target compositor could not provide a live negative-origin topology.

## Physical-target acceptance

The actual target ran Plasma Wayland at 2880×1800@120 Hz, logical 1646×1029,
1.75× scale. Recorded physical results:

| Scenario | Observed result |
|---|---|
| Controlled static target | Presented and printed exactly `#FF0000`; sessions closed. |
| Animated target after capture throttling | Last genuinely presented commit and stdout were exactly `#FF00FF`; fixture presses/releases remained 0/0; sessions closed. |
| Physical left rotation at 1.75× | KScreen reported logical 1029×1646, PipeWire negotiated BGRA 1800×2880, Wayland reported transform `_90`, and the picker presented/printed `#FF0000`; the display was restored to normal 1646×1029 afterward. |
| Right-click cancellation | Cancelled with both portal sessions closed. |
| Escape lifecycle | First Escape cancelled and closed both sessions; a second Escape reached and closed the restored fixture, proving the temporary global binding was gone. |
| Fullscreen/focus/click safety | Fullscreen animated fixture retained zero presses/releases during picker confirmation; normal focus returned after cleanup. |
| Lock/unlock | Lock suspended presentation for 8098.831 ms. After unlock, the first presented sample used a new video PTS and was 78 ms old; selection resumed, fixture presses/releases remained 0/0, and both sessions closed. |

See the [structured result](evidence/native-physical/physical-acceptance-result.json)
and raw traces in `docs/evidence/native-physical/`.

## Performance observations

The initial physical capture exposed a proven hotspot: full-frame RGBA conversion ran at the 120 Hz capture rate despite the native lens presenting at 10 Hz.

| Run | PSS (KiB) | USS (KiB) | Lifetime CPU |
|---|---:|---:|---:|
| Physical, before throttle | 118,574 | 87,200 | 73.2% |
| Physical, 75 ms throttle | 98,252 | 66,880 | 12.6% |
| Earlier private two-output lab | 28,767 | 18,444 | 3.4% |

Right-click cancellation removed the optimized physical process within two seconds and closed both sessions. Post-change animated and rotated exact-selection runs passed. These are recorded observations, not invented budgets or idle-tray measurements.

## Existing integrated acceptance retained

Earlier integrated native evidence already covers successful exact static selection and self-occlusion recovery, right-click cancellation, Escape lifecycle and shortcut release, focus/click preservation, explicit portal closure, and normal two-output transitions. See `docs/evidence/native-integrated/` and `docs/evidence/native-matrix/`.

## Controlled failure matrix

The private KWin lab now covers the remaining fault-injection paths:

- Escape cancelled a genuine portal chooser request before capture; retrying immediately authorized capture and right-click cleanup closed both sessions.
- Pausing private KWin for four seconds caused `stale capture image`; both shields and both sessions cleaned up after KWin resumed.
- Terminating the private PipeWire server produced the expected stream-disconnected failure, removed both shields, closed GlobalShortcuts, and recognized the portal-destroyed ScreenCast object as already closed.
- Disabling private `Virtual-1` during active selection failed closed on output/layer removal and closed both sessions.
- Authorization completion to the first genuinely presented lens commit was 63.865 ms in the private 1024×768 run. The permanent diagnostic records the authorization monotonic timestamp so later physical runs can measure the same interval without consent time.

See the [structured result](evidence/native-failure-matrix/failure-matrix-result.json)
and raw traces in `docs/evidence/native-failure-matrix/`.


## Phase 1A decision

The physical fractional, transformed, fullscreen, static, animated, confirmation,
cancellation, focus, click-safety, cleanup, and lock/unlock checks pass. The
private multi-output, hotplug, portal denial/restoration, slow-frame, and
backend-loss checks pass. Physical hotplug could not be attempted because eDP-1
was the only physical output; the real private KWin output-removal case passed.

The Rust-owned backend is now the default and only picker implementation.
`--native`, `--repeat`, and the obsolete KWin ColorPicker path were removed.
Phase 1A passes; no Phase 1B work has started.
