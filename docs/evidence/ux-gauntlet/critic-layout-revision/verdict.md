# Native swatch-layout re-review — bounded critic verdict

**Not accepted; review is incomplete.** A real, independently built candidate was exercised under private KWin. With one locked swatch at the requested 540×360 output, the delete-refusal message is visible, but the swatch's bottom HEX/details are cut by the output edge in the settled screenshot. This does **not** establish that details cannot be reached by scrolling or resizing, nor does it prove a defect at a fully visible 540×360 client: the Wayland client declares 540×360 while the compositor's initial available configure bounds are 540×324 and the output includes a titlebar. Keyboard, expanded-palette, font-14 and AT-SPI acceptance are not demonstrated. Do not treat this as a whole-gauntlet verdict or a blind review: the dependency handoff was visible.

## Provenance and method

- Built current workspace with `cargo build --offline --locked --workspace --target-dir target/ux-gauntlet-critic-layout-revision/build`, exit 0. Board SHA-256 `70afce3e3c467c62a847c9c803483e27a1102b04d922d59c2dc36141df29bd7b`; HEAD `195cb9f41128cf25c44b802cd20d4858a3a3c0ca`. Tracked board main/mixer/palette binary diff SHA-256 `06f8162ba798cf8cdceee3ee95bd787f9741e0f56dc050154a046106f110dd86`; untracked `rust/board/src/layout_tests.rs` SHA-256 `474442213844b69512abafee979532490f1bfdca2db9e48f68793357b4c9acaf`. This diff hash omits the untracked test; do not compare it to a hash that includes that file. No source was edited in this review.
- Harness `target/ux-gauntlet-critic-layout-revision/run.py` builds isolated HOME/XDG/runtime and private D-Bus/virtual KWin environments; every pointer move, click and screenshot first verifies exact runtime, bus, display, socket existence and the KWin virtual backend. No keyboard injection was attempted. The harmless Add and Palette Editor interactions visibly changed the interface; Board Wayland logs independently contain pointer button delivery and keyboard enter. Injection roundtrip alone is not treated as delivery. Input traces are in `*-trace.json`; filtered real Wayland events in `*-wayland-events.txt`.
- Output requested 540×360 / 960×640, scale 1. Client `xdg_surface.set_window_geometry` declared 540×360 / 960×640 and corresponding toplevel configure reported those dimensions; initial configure bounds are 540×324 / 960×604 respectively. These are protocol-level measurements, **not** a verified fully onscreen client rectangle. Output PNG dimensions are separately 540×360 and 960×640. The titlebar consumes visible output space. No native 14-point font setting was applied or verified. Logs are under `target/ux-gauntlet-critic-layout-revision/<size>/logs/board.log`.
- Initial harness startup failed because the Wayland socket path exceeded 108 bytes; shortening the private socket name fixed it. Failed startup cleaned up; no human session fallback. Earlier exploratory clicks landed below Add; final evidence uses corrected coordinates and visible state transitions. Screenshots are private native renders, not a user study.
- KDE HIG references consulted: [layout/navigation](https://develop.kde.org/hig/layout_and_nav/), [accessibility](https://develop.kde.org/hig/accessibility/), [status changes](https://develop.kde.org/hig/status_changes/). These are design reference points, not a complete WCAG audit.

## Task verdicts

PASS applies only to the precisely described narrow interaction; unmet portions are explicitly UNTESTED.

| Task | Verdict | Evidence and limits |
|---|---|---|
| Pointer Add → Lock → locked Delete, 540 and 960 outputs | PASS (narrow) | `*-after-add.png`, `*-after-lock.png`, `*-after-refusal.png` show a new swatch, locked state and retained swatch with actionable delete refusal. `*-wayland-events.txt` captures delivered button events. Default font, one swatch only. |
| 540×360 visible output with locked refusal | FAIL (painted visibility) | `540x360-after-refusal.png`: full refusal text is visible, but lower swatch details/HEX are cut by the bottom output edge. Exact fully visible client geometry and reachability after scroll/resize are unproven; do not claim scroll is broken. |
| Minimum actual client, expanded several-swatches palette, font 14; pointer and keyboard reachability, before/after scroll | UNTESTED | One default-font swatch; no expansion, horizontal/vertical scroll or keyboard path was exercised. Output size and declared client size do not prove the required fully visible client. |
| 960×640 comparison | PASS (narrow) | `960x640-after-refusal.png` shows locked swatch and refusal in the output. Not a regression assessment for the required font-14/multiple-swatches/expanded and keyboard scenarios. |
| Visible focus and Enter/Space for lock/edit/delete/earlier/later; keyboard reorder and Space typing | UNTESTED | Focus and delivery for those actions were not established; do not turn this absence into a product nonactivation finding. |
| Changed HEX Apply → Undo → Redo → invalid input → recovery | UNTESTED | Not exercised. |
| Locked edit and history replacement refusal | UNTESTED | Locked delete only. |
| Cross-tab pinned persistence | PASS (one transition only) | After locking/refusal, `*-after-tab.png` shows Palette Editor with the locked single swatch retained. Other tab transitions and persistence across reopen remain untested. |
| AT-SPI names/roles | UNTESTED | No isolated accessibility tree probe completed; neither absence nor unsupported status has been established. |
| Native platform themes, text scaling, physical desktop | UNTESTED | No light theme, settings integration or physical desktop interaction. |

## Findings and dimensions

**High — lower details are cut off in the small-output locked-refusal state (observed; reachability unresolved).** Reproduce by launching this build in a private 540×360 scale-1 KWin output, adding the current color, locking it and pressing Delete; inspect `540x360-after-refusal.png`. The whole actionable refusal remains visible, but the lower swatch details are beyond the displayed output. The client's requested size exceeds the compositor's initial available bounds once the titlebar is considered, so the underlying cause and behavior in a fully visible exact client remain open. Smallest effective fix depends on diagnosis: ensure the actual supported minimum visible window can expose both the complete message and swatch details, through a discoverable, keyboard-operable scroll/reflow or correct minimum/window geometry. Verify with measured client and output bounds, font 14, multiple swatches, expanded palette, before/after scroll screenshots and real pointer/keyboard traces. Do not dismiss the screenshot as a confirmed harness error.

**High — keyboard safety workflow remains a release-risk hypothesis, not a demonstrated failure.** Verify focus indicator and independently delivered Enter and Space on each action, two-swatch reorder and typing Space in an editable field; check state and order after every step. Missing coverage is especially important for lock/delete and the requested keyboard-only path. Do not infer failure from this review.

**Strengths worth preserving:** persistent labeled actions, visible textual locked state and specific instruction to unlock before deletion; single-transition pinned state survived switching to Palette Editor.

Five dimensions, with no aggregation: native consistency **unscored** (theme and platform settings untested); discoverability **4 provisionally** for visible worded action controls only; editing/feedback **3 provisionally** (locked-delete explanation demonstrated, editing path untested); accessibility/layout **2 provisionally** (small-output clipping, keyboard/AT coverage missing); workflow preservation **unscored** (one tab transition only). Scores are bounded observations, not proof the other requirements pass.

## Next actions for the lead

1. Measure on-screen client bounds separately from output and reproduce the expanded several-swatch font-14 case, including pointer and keyboard scroll and complete lock refusal. Resolve the small-output cut-off without discarding the visible safety controls.
2. Run the verified-focus keyboard matrix with event traces and observable state assertions, especially reorder and text-entry Space.
3. Complete HEX/error recovery, locked edit/history, cross-tab persistence and an actual isolated AT-SPI names/roles probe; report any genuine unsupported platform capability separately from untested work.

## Cleanup and boundaries

Start and end `/proc/*/environ` scans filtered for runtime directories below this task's target directory reported `[]`. Direct child process exit codes are in the per-size trace JSON; private services were signalled and reaped, which is not a test of normal application close. An unrelated pre-existing private compositor and the human compositor were not touched. No human-session input, portal permissions, packages, settings, theme application, commits, pushes, source edits or new tasks. No further revision loop is authorized.
