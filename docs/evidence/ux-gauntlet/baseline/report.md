# Swatch Board — independent rendered UX baseline

**Verdict:** The Board renders and the two initial pointer actions below work in a private KWin virtual session. Its pinned palette makes the current working colors readily available across screens. The largest demonstrated first-slice concern is that per-swatch lock and delete are discovered through hover rather than being persistently exposed. Preserve the pinned palette and collapsible history; address the swatch action affordance before proposing a broader layout redesign. This is a partial baseline, not gauntlet acceptance or a blind before/after review.

## Artifact and environment

- Source commit `195cb9f`; no tracked source changes or commits made. `cargo build --offline --workspace --locked --target-dir target/ux-gauntlet-baseline/build` passed. Debug Board SHA-256 `61d86d1851a08a7154e95dfdbf29540cf9079dce5de0e507fce8da069b884639`.
- Each run used its own project-local HOME, XDG runtime/config/data/cache/state and D-Bus, a virtual KWin and dedicated socket. The harness checks runtime/bus/display identity and virtual backend before each interaction/capture. It did not connect to the human Wayland session. Outputs `Virtual-0` were 960×640 and 540×360, scale 1. Wayland protocol traces show the Board's `xdg_toplevel.configure(960,640,...)` and `configure(540,360,...)` respectively; KWin's reported available bounds were 960×604 and 540×324, so screenshots include a constrained output and must not be interpreted as testing every resize or maximization scenario. See `960x640-wayland-geometry.txt` and `540x360-wayland-geometry.txt`.
- Exact harness: `target/ux-gauntlet-baseline/run.py`. It terminates and waits for its own process groups; reported Board and input holder terminated, KWin exited and bus exited in each case. A process scan for the project-local test runtimes found no remaining matching process. This was forced test cleanup, **not** normal-close verification.
- KDE HIG references consulted: [simple by default](https://develop.kde.org/hig/simple_by_default/), [layout and navigation](https://develop.kde.org/hig/layout_and_nav/), [accessibility](https://develop.kde.org/hig/accessibility/). Related reference observations and scenario IDs are in `../planning/acceptance-matrix.md`; these guide this critique, not proof of conformance. Read-only host KDE configuration showed `ColorScheme=EmeraldDark` and WM `activeFont=Noto Sans,12`; these do not establish the Board's effective palette or general UI font. The isolated session did not import or apply the host theme.

## First-time pointer walkthrough — observed, not a user study

Assumption: a new user wants to put the displayed color into a working palette and find palette editing actions. Starting with isolated empty data, at each output:

1. The Mixer and the pinned `Active palette - 0 colors` with `Add current color` are visible (`*-initial.png`).
2. One left-click on `Add current color` (960: 190,557; 540: 190,277) produces `Active palette - 1 colors` and a `#247AC3FF` swatch (`*-after-add.png`). **Count: one click**, excluding pointer movement and startup.
3. One left-click on `Palette Editor` (120,70) opens the editor while retaining the swatch (`*-palette-editor.png`). **Count: two clicks cumulative**. Screenshots verify the state change; helper roundtrips by themselves are not delivery evidence.
4. Move pointer to swatch area (960: 40,590; 540: 40,310); see `*-swatch-hover.png`. The hover state exposes swatch actions. The implementation corroborates the hover condition at `rust/board/src/palette.rs:341–382`, and README lines 165–169 explicitly describe hover lock/delete. Screenshots alone do not establish a keyboard alternative. Pointer hover is not a click and no lock or delete action was performed.

Strengths: clear split between current working palette and capture history; empty history explains how to begin; generated palettes preserve locked colors according to the UI/documentation, though preservation was not exercised in this run. Both output sizes show the pinned swatch and relevant editor after the two-action path. This brief script is not a full capture-to-export journey, a task-completion rate, or a measured user study.

## Priority finding and first proposed slice

**High — per-swatch safety actions depend on hover for discovery (observed visual pattern; keyboard impact is a hypothesis).** After the two-step sequence above, compare `960x640-palette-editor.png` with `960x640-swatch-hover.png` (and corresponding 540 files). At rest the lock and delete actions are not persistently exposed; on hover the controls appear. A time-pressed new user may not discover how to protect or remove a color. Hover dependence also merits keyboard and assistive-technology investigation, but this run did not establish that keyboard access is impossible. Severity is high because lock protects swatches against destructive edits/regeneration and discovery affects a core palette task; it is not a claim of data loss observed in this run.

**Smallest builder slice:** Make lock/unlock and delete discoverable and operable without pointer hover, using persistent compact labeled actions or a clearly labeled always-visible swatch action menu. Provide a keyboard-operable route with a clear non-color-only locked state. Preserve lock invariants, existing hex editing, pinned palette, history and saved data. Do not silently introduce automatic theme generation or rearrange the application. A menu and exact wording are proposals, not a mandated architecture.

**Acceptance for this slice:** On the actual new binary at 960×640 and 540×360, add a changed color, locate the swatch actions without hover, lock/unlock and delete using both pointer and keyboard; visually verify focus and clear textual state. Generate with one locked and one unlocked swatch and verify the locked identity/color persists; verify edit/replacement/delete cannot alter a locked swatch, unlocking restores operations, and existing palette/save/reopen behavior survives. Confirm essential feedback remains visible and preserve reproducible pre/post screenshots and input traces. Add regression tests for lock invariants and keyboard/pointer action paths; do not count synthetic egui events as native input.

## Baseline scores and coverage

Scale: 1 blocked, 3 functional with material friction, 4 reference quality, 5 demonstrably better. No averaging; untested is not a pass.

| Dimension | Baseline | Evidence and limits |
|---|---|---|
| Native consistency | Untested | Private generic dark rendering is not proof of matching the active EmeraldDark scheme or system fonts. |
| Discoverability | 3, provisional | Two-step add/navigation works; hover-only swatch safety controls create material discovery friction. Broader journey untested. |
| Editing / feedback | Untested | Changed-value Apply/Undo/Redo and invalid recovery not exercised. |
| Accessibility / layout | Untested | Two static scale-1 sizes and pointer actions are insufficient to establish keyboard, focus, AT, contrast, zoom or complete reachability. |
| Workflow preservation | Untested | Pinned palette persists across the tested tab transition; lock, storage, theme, picker, export and normal-close preservation not tested. |

Input support: private KWin fake-input helper successfully delivered the two pointer clicks as corroborated by screenshots; the helper only advertises Escape and Return as its keyboard commands, not general keyboard traversal. No independent fixture event counters were collected. AT-SPI tree and screen-reader operation were not inspected. `cargo tree --offline --locked -p swatch-board -e features -i accesskit` returned `nothing to print`; this is resolved-dependency evidence, not a substitute for inspecting a live AT tree or proof about every build. No actual assistive support is claimed.

Not covered: capture, clipboard, settings, save/reopen, theme assignment/link/export, lock/regeneration, keyboard-only paths, normal close, non-default fonts, light/dark switching, 175%/200% scale, full layout/error/empty-state matrix, screen-reader use, or physical desktop. Full workspace tests/Clippy were not run during this baseline; the workspace build passed. No themes applied, host settings changed, permissions approved, packages installed, or source committed. Subsequent critics must independently test the candidate fix and the full gauntlet remains open.
