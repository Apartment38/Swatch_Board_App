# 002: Native pointer shield and cursor-image exclusion

## Question

Can a Rust-only Wayland picker intercept mouse confirmation/cancellation without
keyboard focus, render its lens as an actual `wl_pointer` cursor excluded from
ScreenCast video, and obtain genuine presentation feedback for that image?

## Verdict: PARTIAL

The pointer shield and cursor-image exclusion were demonstrated in an isolated
KWin session. **Cursor presentation authority was not.** This is disposable
investigation code, not a production magnifier or a Phase 1A pass.

### What worked

- Two 1024×768 virtual outputs; transparent overlay shields with full pointer input
  and `KeyboardInteractivity::None`. No keyboard objects are bound.
- Left release selects; right release cancels. Press/release pairs are consumed.
  In the tested cases, the underlying native fixture retained keyboard focus and
  its mouse counters stayed at the calibration baseline of one press/one release.
- A magenta TEST cursor was visible in pointer-inclusive compositor screenshots.
  At the same position, metadata-mode ScreenCast video remained blue: 8/8 samples
  for confirmation, 8/8 for right cancellation, and 59/59 for idle timeout.
- Selection, cancellation and the 12-second idle timeout closed the capture session
  and removed the shield. The fixture received pointer focus back after teardown.
- A requested cursor `wl_surface.frame` callback arrived once per final scenario.

### What did not work

- `wp_presentation` was advertised and feedback was requested before cursor commit,
  but **zero Presented events** arrived. Both 80×80 and 512×512 buffers were tried.
- Explicitly requesting a frame callback did not produce presentation feedback.
  A frame callback is not proof of actual presentation and must not be substituted.
- The larger buffer experiment does not prove hardware-cursor-plane selection or
  software fallback; no such backend-plane observation was collected.

### Evidence and limits

See [native probe validation](../../docs/phase-1a-native-probes.md) and the raw
[evidence](../../docs/evidence/native-shield/). These runs cover the first output
at scale 1 with both outputs shielded. They do not validate cross-output motion,
fractional scaling, held-button failure cleanup, or a combined Escape+shield picker.
The sentinel is deliberately not a complete magnifier.

### Reproduce on this workstation

Build `cargo build --release --offline -p swatch-board-picker --examples`.
The preserved Python **test orchestration** sources are in `testing/`; stage them
into the project's `target/` directory before running them. They are deliberately
bound to `/home/erick/Projects/Swatch_Board_App`, not a portable installation API.

1. Run `python target/run-isolated-native-lab.py` in a terminal; wait for
   `NESTED_KWIN_READY`.
2. In another terminal, run `python target/approve-private-test-chooser.py private-shield-live-5`
   (left confirmation), then `... private-shield-live-6` (right cancellation), then
   `... private-shield-live-7` (12-second idle timeout).
3. Inspect each `target/private-shield-live-N-result.json`, log, and cursor PNG.
4. Stop with `python target/private-lab-command.py --stop`.

The lab uses a private D-Bus, runtime, HOME, configuration, data, cache, state and
PipeWire instance beneath `target/isolated-native-lab-1/`. WirePlumber runs its
**policy-only** profile; it does not load audio/camera hardware monitors. A held
fake-input device keeps pointer seat capability present on the virtual desktop.
The chooser coordinator asserts the private session identity before injecting
input. Its fixed coordinates apply only to this disposable test desktop. It must
never be adapted to approve the human desktop's consent dialogs.

The Rust probe itself can be inspected without capture using `--inspect`.
`--run` opens ScreenCast authorization and shields all outputs for at most 12
seconds after the preparation interval. `--run --large-cursor` uses a 512-square
buffer with the same 80-square visible sentinel, for the controlled comparison.
Do not run it against the human session unattended.

### Recommendation for the real build

Keep actual-presentation authority strict. Investigate a normal layer-shell lens
with presentation feedback and a separately verified self-occlusion guard, rather
than promoting this cursor prototype on the strength of submission or callbacks.
No production backend has been switched by this spike.
