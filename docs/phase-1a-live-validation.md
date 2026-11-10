# Phase 1A live validation and blocking findings

**Current gate status: PASSED.** This document preserves the earlier KWin-backed
validation and blocker evidence. The Rust-owned presented-sample picker that
resolved those blockers is documented in
[native integration acceptance](phase-1a-native-integration.md).

The retained KWin findings below are historical. They remain useful evidence for
why KWin ColorPicker was removed as the authority and backend.

## What was built

- Latest-only, authorized-stream-bounded frame storage replaces the old queue that
  dropped new frames when the renderer fell behind. Cursor-only updates preserve
  the original image receipt time and PTS; invalid cursor IDs clear the pointer.
- The lens can switch to the captured output containing the valid cursor instead
  of ignoring every stream except the first. Output mapping rejects ambiguous
  origins; output geometry changes are reconfigured or reported as failures.
- Wayland dispatch is bounded independently of frame callbacks. A hidden or stalled
  surface cannot prevent a requested overlay shutdown.
- A viewporter maps physical-pixel buffers to logical surface dimensions. The
  center marker has white and black outlines, and coordinates/hex have backplates.
- Lens placement protects the whole sampled neighborhood, tries vertical placement
  on narrow outputs, and fails when no non-overlapping placement is possible.
- `wp_presentation` feedback gates startup and publishes the actual displayed
  sample. Queued/discarded commits are not treated as displayed samples.
- Successful/cancelled picks stop and join both helpers and explicitly close the
  portal session. Setup errors also explicitly close created sessions. Busy and
  other D-Bus failures are not misclassified as normal cancellation.
- A mismatch or missing displayed sample now returns an error instead of reporting
  a successful accurate pick. Two regression tests cover that fail-closed behavior.

## Reproducible commands

```sh
cargo test --offline --workspace --all-targets --locked
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo build --release --offline --workspace
cargo build --release --offline --workspace --examples

# Run from a real Plasma Wayland session; use its actual WAYLAND_DISPLAY.
cargo run --release --offline -p swatch-board-picker -- --diagnostics
cargo run --release --offline -p swatch-board-picker -- --radius 7 --zoom 10

# Real-session helpers for controlled acceptance.
cargo run --release --offline -p swatch-board-picker --example phase1a_fixture
cargo run --release --offline -p swatch-board-picker --example phase1a_input -- move X Y
# Input helper actions: left, right, escape, return. Never use it on human-session permission dialogs.
```

`--radius` is 0..31 and `--zoom` is 4..32 physical pixels per source pixel.
Human-session KDE consent is never automated. Only the disposable private lab
chooser described in the native checkpoint is automated, after asserting private
D-Bus/runtime/Wayland identity.

The native fixture logs pointer press/release counters, keyboard enter/leave,
xdg activation state, commits, and compositor frame callbacks. Fixture coordinates
are surface-local logical coordinates; determine its global origin from observed
pointer events rather than assuming window placement.

## Actual environment and observations

Read-only discovery: KWin **6.8.0**, Wayland, one `eDP-1` output,
**1646×1029 logical / 2880×1800 captured**, **1.75×** scale. Portal cursor modes
bitmask `7` includes metadata; layer-shell, viewporter and presentation-time globals
are advertised. No display/effect settings were changed. Dim Inactive was reported
loaded/active by KWin at discovery; the fixture remained the activated window during
the controlled batch.

The optimized batch completed **10 live rounds** after the human authorized KDE's
sharing chooser. See the [machine-readable results](evidence/phase-1a-live-acceptance.json),
[real picker trace](evidence/phase-1a-picker.log), and
[real fixture trace](evidence/phase-1a-fixture.log).

| Test | KWin result | Displayed-sample agreement | Result |
|---|---|---|---|
| Blue | `#0000FF` | Exact | Pass |
| Red | `#FF0000` | Exact | Pass |
| Green | `#00FF00` | Exact | Pass |
| White | `#FFFFFF` | Exact | Pass |
| Black | `#000000` | Exact | Pass |
| Checkerboard white-cell center | `#FFFFFF` | Exact | Pass |
| Checkerboard black-cell center | `#000000` | Exact | Pass |
| Right-click | Exact KWin cancellation error classified normally | Not applicable | Pass |
| Escape | Exact KWin cancellation error classified normally | Not applicable | Pass |
| Cyan/magenta animation, 250 ms phase | `#00FFFF` | Last presented center was `#FF00FF` | **FAIL** |

The checkerboard cells in this native fixture are 20 logical pixels wide, not
single physical pixels. Deterministic one-pixel patterns remain covered by core
unit tests; this batch is not a live one-physical-pixel checkerboard claim.

All ten rounds logged helper shutdown. After the batch, the session logged closure.
The fixture's counters remained **one baseline press and one baseline release**:
none of the selection/cancel mouse clicks reached the fixture. Escape's release
was delivered after KWin ended its grab; its press did not reach the fixture.
The fixture remained `WindowState(ACTIVATED)`; no deactivation configure occurred
within the batch. KWin temporarily removes/restores seat keyboard focus during its
compositor selection grab; this is not an overlay taking focus. A literal requirement
that even seat keyboard focus never changes would need clarification.

After test cleanup, `/proc` inspection found no project picker/fixture processes,
and `pw-dump` found no objects with Swatch Board properties. This verifies completed
success/cancel batch cleanup, not all backend-failure or lock-screen paths.

The trace's historical `video_age=0..21ms` values measure time since adapter receipt,
not capture-to-display latency; PTS clocks were not calibrated end-to-end. The final
animation sample's receipt age was 39 ms **after helper cleanup**. Current diagnostic
labels say `receipt_age` and `Lens candidate` to avoid implying otherwise.

The recorded batch ran **before** fail-closed mismatch handling was added, so its
exit code 0 is preserved as observed but is not accepted as a gate pass. Current
unit tests require a mismatch/missing sample to fail. A later successful run cannot
erase this demonstrated counterexample to an unconditional exact-match guarantee.

## Blocking finding 1 — animation is not an atomic sample

The magnifier displays an asynchronously captured PipeWire frame. KWin's `pick()`
reply comes from a separate later scene render. Presentation feedback proves what
was displayed; it does not make KWin select that same captured frame or timestamp.

The real transition failure persisted after latest-only delivery, separate cursor
and video receipt times, optimized release rendering, physical-pixel buffers, and
compositor-confirmed sample publication. The present implementation detects and
rejects the mismatch rather than changing the returned color or rewriting the
record of what was displayed. It does not meet the current unconditional dynamic
accuracy gate.

Relevant upstream source:
- [KWin Plasma/6.8 ColorPicker implementation](https://github.com/KDE/kwin/blob/Plasma/6.8/src/plugins/colorpicker/colorpicker.cpp)
- [KWin Plasma/6.8 ScreenCast implementation](https://github.com/KDE/kwin/blob/Plasma/6.8/src/plugins/screencast/screencaststream.cpp)
- [ScreenCast portal contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)

## Blocking finding 2 — no remote KWin picker cancellation method

Live `/ColorPicker` introspection exposes `pick()` only. Dropping the local zbus
future unregisters a reply waiter; it is not a server cancellation request. The
reviewed KWin implementation does not expose a cancellation operation or connect
this grab to caller-disappearance cleanup.

On mid-selection overlay/capture failure, the program stops capture, explains the
error, and waits for the user's Escape/right-click to release KWin. It deliberately
does not claim that exiting or closing the capture portal cancels the independent
KWin pointer grab. This remains short of autonomous failure cleanup. The test-only
fake-input helper is not silently used as a production global-key-injection fallback.

## Supported alternatives checked

An independent source review confirmed the synchronization/cancellation limitation.
The parent also read KDE's complete `ScreenshotPortal::PickColor` implementation:
[Plasma/6.8 screenshot.cpp](https://github.com/KDE/xdg-desktop-portal-kde/blob/Plasma/6.8/src/screenshot.cpp).
It calls the same `/ColorPicker` `pick()` method and returns its RGB result. Moving
this call behind the Screenshot portal does not introduce a frame identity or a
request-owned cancellation path. Generic portal `Request.Close` must not be assumed
to cancel this independent KWin operation.

The [presentation-time contract](https://wayland.app/protocols/presentation-time)
confirms an individual surface commit was displayed, but supplies no bridge into
KWin's separately rendered color reply. Closing ScreenCast only closes capture.
Authorized synthetic Escape is a global input action, not a cancellation handle;
it can race selection completion and reach the previously active application.
It has not been adopted as production cleanup.

A Rust-owned selection layer could instead select the actually displayed sample,
but that changes the authority/equality model and still requires proving Escape,
focus and no-click-through semantics. It is an explicit architectural option to
investigate, not an implemented substitute that passes the existing plan.

## Remaining live gates

- Multiple outputs, output crossings, negative origins, rotation, and the required
  100/125/150/200% scales are **not live-validated**. Only one physical output was
  available. Core geometry and frame-store tests do not substitute for hardware tests.
- Live one-physical-pixel imagery, fast pointer motion across prior lens footprints,
  fullscreen content, lock/unlock, hotplug, portal denial/restoration, and backend-loss
  cleanup need further validation. No claim of overlay capture exclusion is made;
  safer placement prevents direct neighborhood overlap, not every possible trail race.
- Adjustable radius/zoom have parser and rendering coverage; the controlled live
  batch used defaults. No performance/memory budget is declared achieved.

## Decision required by the existing plan

Per §4.3, retain the hard gate and do not proceed to tray/Board while these findings
stand. A decision must preserve the Rust-only constraint or explicitly revise it:

1. Keep strict atomic accuracy and autonomous cleanup, and require a compositor-side
   supported API that couples capture, selection and cancellation; this cannot be
   claimed implemented by the current independent portal + KWin pick APIs.
2. Explicitly accept a live asynchronous magnifier with disclosed latency, KWin's
   authoritative click-time result, fail-closed mismatch handling, and a defined
   backend-failure cancellation policy. That would be a requirement change, not a pass
   of the current plan.

No requirement has been relaxed and no Phase 1B work has started.
