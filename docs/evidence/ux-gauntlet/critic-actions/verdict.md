# Pinned swatch action bar — native critic verdict

**Verdict: not accepted yet.** The persistent, worded action bar and explicit locked-delete explanation are useful improvements over the baseline. At 540×360, however, the swatch details are visibly cut off at the bottom of the window. Keyboard and several workflow requirements remain unverified; do not interpret this review as a pass for them.

## Scope, provenance and method

The intended users are people building a working palette, including keyboard users; primary tasks are protecting, editing, deleting and reordering colors. This is an independent execution against the candidate, **not a blind review**: the board automatically exposed the builder's handoff in the task context. I did not use the builder's report as evidence.

The prior attempt built and operated the candidate in project-local virtual KWin sessions; this continuation inspected the saved screenshots and harness, checked artifact hashes and cleaned up surviving private processes. The first attempt recorded `cargo build --offline --locked --workspace --target-dir target/ux-gauntlet-critic-actions/build` as passing (exit 0); this continuation did not repeat it. Candidate path `target/ux-gauntlet-critic-actions/build/debug/swatch-board`, SHA-256 **21a4f563ec8254d65d0ff04d9aab63e6001c56fb7c482e120619da3943af73fd**. Baseline binary SHA-256 **61d86d1851a08a7154e95dfdbf29540cf9079dce5de0e507fce8da069b884639**; HEAD `195cb9f41128cf25c44b802cd20d4858a3a3c0ca`. `sha256sum ...` and `git rev-parse HEAD` exited 0 in this continuation. Different binary hashes alone do not prove source provenance; the saved candidate harness specifies that exact candidate path and a surviving Board process was observed at that path. A reproducible build-to-source attestation was not retained.

Private virtual outputs requested at 960×640 and 540×360, scale 1. Do not assume the output dimensions establish exact client dimensions. Saved harness `target/ux-gauntlet-critic-actions/harness.py` sets private runtime, bus and Wayland names, but unlike the baseline harness its per-action `click`, `key`, and `snap` entry points do not themselves call the identity check. This is a methodological limitation: saved screenshots are native rendered evidence, but per-action private-session identity checks and input delivery were not fully preserved as a durable trace. No human-session interaction is claimed. Baseline comparison: `../baseline/960x640-after-add.png` shows the swatch but not the new persistent bar. KDE HIG reference: [simple by default](https://develop.kde.org/hig/simple_by_default/), [layout and navigation](https://develop.kde.org/hig/layout_and_nav/), [communicating status changes](https://develop.kde.org/hig/status_changes/), [accessibility](https://develop.kde.org/hig/accessibility/). This is not a WCAG audit.

## Task coverage

Paths below are relative to this directory. PASS means only the narrow evidence described; missing portions remain UNTESTED.

1. **PARTIAL** — `960x640/after-add.png` shows changed swatch `#6162C3FF` and labelled Lock, Edit, Delete, Earlier, Later controls without requiring a hover to see the bar. `960x640/after-lock.png`, `960x640/after-delete-locked.png` and `960x640/after-unlock.png` show the locked state, retained swatch with “Swatch 1 is locked. Unlock it before deleting.”, and restored unlocked state. The screenshot sequence supports the pointer flow; reproducible per-click event trace was not retained. Locked edit and history replacement are **UNTESTED**.
2. **FAIL on complete layout / PARTIAL on action-bar rendering** — `540x360/after-add.png` shows all five action labels and selected-swatch status painted, but the lower swatch HEX/details are clipped by the output's lower edge. A long refusal/status message, scrolling, full action reachability, expanded palette and exact panel height were not verified. Screenshot alone cannot prove a lack of scrolling or a control's hit target.
3. **UNTESTED** — Saved focus and key images include `960x640/kbd-focus.png`, `960x640/kbd-space-lock.png` and `960x640/kbd-return.png`. The prior attempt reported non-activation, but the available screenshot does not unambiguously establish that Lock had keyboard focus before the injected key, and input delivery is not independently evidenced. Treat this as a high-priority hypothesis, not a confirmed defect. Full Tab order, visible focus on each action, Enter/Space activation, text-field Space and keyboard reorder remain unverified.
4. **UNTESTED** — `960x640/t4-hex-focused.png` and `960x640/t4-hex-typed.png` capture an attempted entry, not a verified Apply → Undo → Redo → invalid input → recovery sequence. Do not infer a typing defect from the incomplete entry.
5. **UNTESTED** — persistence across the three tabs was not captured in this candidate sequence.
6. **UNTESTED** — no AT-SPI action-bar tree or names/roles dump was retained. A private AT-SPI launcher process existed during cleanup; that does not establish accessibility-tree reachability or names.
7. **UNTESTED** — no native size-14 or light-theme comparison captured. The screenshots show a dark interface; no claim is made about platform theme integration.

## Findings and priorities

### High — minimum-size palette content is cut off

**Observed:** In `540x360/after-add.png`, after adding one changed color the lower swatch HEX/details extend beyond the bottom edge. The visible action bar and selected-state text do not establish that the entire working palette is usable at minimum size. This conflicts with the requested minimum-window experience and is at odds with the layout and accessibility aims of the KDE HIG.

**Reproduce:** Launch the candidate in a private KWin virtual output at 540×360, add a changed current color, and inspect the pinned palette at its initial height. **Expected:** the full swatch details and action/status area are visible or reachable by an obvious, usable scroll/resize mechanism without losing access to the rest of the task. **Smallest fix:** give the palette's content a usable scrollable region or adapt its minimum-size layout; retain the persistent action bar and do not hide the locked-state explanation. **Acceptance test:** in native 540×360 and size-14 runs, add several colors, lock one and trigger a refusal; capture the full status and every swatch detail after scrolling, and verify pointer and keyboard access. Verify actual client geometry and preserve input traces. The current evidence does not establish that scrolling fails; test it rather than assuming it.

### High — keyboard safety actions need decisive verification

**Research hypothesis, not confirmed defect:** prior key attempts did not produce an established activation, but the saved focus/delivery evidence is insufficient to attribute this to the product. Verify focused action buttons with both Enter and Space, including actual lock-state transitions and two-swatch reordering; verify a typed space in an editable name does not activate any button. Provide obvious focus and normal button semantics if any test fails.

### Low — Locked badge covers some color area

**Observed:** `960x640/after-delete-locked.png` and `960x640/zoom-locked-tile.png` confirm a dark “Locked” badge overlaps the swatch tile. It is redundant with the textual selected-state label, which is good for non-color-only communication; the overlap is a visual trade-off, not by itself a demonstrated usability failure. Consider placing the badge adjacent to the tile if it materially obscures color comparison; validate with real users.

## Scores (1–5, no aggregation)

These are provisional, bounded to observed evidence, not accessibility certification: native consistency **unscored** (theme integration untested); discoverability **4** (visible, labelled controls in the captured state); editing/feedback **3** (lock/refusal visible, editing workflow untested); accessibility/layout **2** (minimum-size clipping; keyboard and AT unverified); workflow preservation **unscored** (cross-tab flow untested). A score of 4 is not acceptance of untested functionality.

## Top next actions

1. Fix minimum-size layout and capture scroll/reachability plus long refusal at 540×360 and size 14.
2. Run a traced keyboard-only matrix with verified focus and delivered Enter/Space events for all actions and text entry.
3. Complete HEX apply/undo/redo/error recovery, locked edit/history refusal, cross-tab persistence, and AT-SPI naming checks.

## Cleanup

On continuation, a process scan found surviving private 960×640 Board, D-Bus, fake-input and portal/AT-SPI processes despite the earlier attempt's cleanup claim. A Python `/proc/*/environ` scan selected only processes whose `XDG_RUNTIME_DIR` began with the critic-owned `target/ux-gauntlet-critic-actions/s/` path; those were sent SIGTERM, then any survivors SIGKILL. The command exited 0 and repeated scan returned `private processes after: []`. This is forced cleanup, not verification of normal close. No human desktop, host package, theme or shipped source was modified by this continuation. Exact commands for the previous attempt's individual input injections and their exit statuses were not preserved as a single durable trace; do not treat them as independently reproduced here.
