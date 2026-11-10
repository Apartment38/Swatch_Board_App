# Phase 1A implementation status

**Status: PASSED on the actual KDE Plasma Wayland target and the private multi-output fault-injection lab.**

## Decision

The Rust-owned presented-sample picker is the default and only backend. The obsolete KWin ColorPicker path, `--native`, and legacy `--repeat` acceptance mode were removed. The authoritative selected value is the exact center sample attached to a genuinely presented lens commit.

No Phase 1B, tray, or Board work has started.

## Implemented behavior

- Rust-owned Wayland shields handle left-release confirmation and right-release cancellation without forwarding clicks to the prior application.
- Application-scoped GlobalShortcuts supplies Escape cancellation and is closed during cleanup.
- Fresh destination-stream video and hide presentation barriers prevent cross-output and self-occlusion feedback.
- Multiple authorized outputs route by Wayland logical origin while portal geometry remains stream identity metadata.
- Fractional logical/physical mapping and compositor-oriented or raw-oriented transformed frames are supported.
- The lens paints nearest-neighbor source pixels, a contrasting center marker, and live `#RRGGBB` text.
- Success, cancellation, geometry failure, portal denial, stale capture, backend loss, and output loss clean up shields and portal sessions.

## Live evidence

- Private normal and mixed-scale two-output transitions passed.
- Private held-button geometry failure delivered no release to either fixture.
- Physical 1.75× exact static and fullscreen animated selections passed.
- Physical left-rotated 1.75× selection passed and the display was restored.
- Right-click cancellation and Escape lifecycle passed, including release of the global binding.
- Physical confirmation and post-unlock selection produced zero fixture presses/releases.
- Lock suspended presentation for 8098.831 ms; the first post-unlock presented frame had a new video PTS and 78 ms age.
- Portal denial/restoration, a four-second compositor stall, PipeWire loss, and private output hotplug cleaned up or recovered as intended.
- Active physical capture dropped from 73.2% to 12.6% observed lifetime CPU after native-only frame-conversion throttling; post-change animated and rotated runs passed.
- Authorization completion to first presented readiness measured 63.865 ms in the private lab, excluding consent time.

KScreen rejected negative output coordinates; negative origins and all eight transforms retain deterministic geometry coverage. Physical hotplug was unsafe with eDP-1 as the only physical output, so real private KWin output removal supplied that live fault case.

See [native integration acceptance](phase-1a-native-integration.md), [physical evidence](evidence/native-physical/physical-acceptance-result.json), and [failure-matrix evidence](evidence/native-failure-matrix/failure-matrix-result.json).
