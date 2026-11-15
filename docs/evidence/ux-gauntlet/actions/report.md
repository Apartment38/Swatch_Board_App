# Swatch Board — persistent, keyboard-operable pinned swatch actions

**Scope:** builder slice `swatch-actions-1` (Kanban `t_6039c5ea`).
**Source base:** `195cb9f` ("Rebuild Arch package with physically accepted Mixer repair").
**Changed file:** `rust/board/src/palette.rs` only (1017 insertions, 61 deletions). No other source file, packaging pin, or baseline/planning evidence was touched. No commit was made.

**Verdict:** The pinned working palette now exposes lock/unlock, edit, delete and reorder as always-painted, labelled controls that are reachable with a pointer *and* a keyboard, with the lock state stated in words and visible focus/selection feedback. 19 headless regression tests in `rust/board/src/palette.rs` cover the pointer and keyboard paths, painted bounds and clipping at the minimum window size and font size, lock protection through generation/reorder/replacement/delete, and undo/redo/save/reopen. Strict Clippy and rustfmt are clean. **This is headless egui evidence only — no native GUI, screenshot, real-input or assistive-technology evidence was produced.** The native critic must still exercise the candidate.

## The change

### Persistent action bar (replaces hover-only controls)

The previous implementation painted per-swatch `Lock`/`Unlock` and `Delete` buttons only while the pointer hovered the swatch tile, so the safety-relevant actions were undiscoverable at rest and had no keyboard route at all. `PaletteEditor::pinned_ui` now paints a labelled action bar above the swatch strip:

- A positional label naming the target swatch and stating its lock state in words — e.g. `1/3 Rose (Unlocked)` — so lock is never communicated by colour alone.
- Always-painted buttons: `Lock`/`Unlock`, `Edit`, `Delete`, `Earlier`, `Later`.
- A status line carrying the outcome of the last action or the reason an action was refused.
- The bar is `horizontal_wrapped`, so it degrades to multiple lines instead of clipping when the window is narrow.

The swatch tile itself keeps a `Locked` badge (white text on a translucent dark plate) painted for every locked swatch, plus a selection outline for the swatch the action bar targets, and a focus outline when the tile holds keyboard focus.

### Keyboard operability

- Every action is a normal egui widget, so Tab reaches it and Space/Enter activates it. No global Space handling was introduced; `rust/board/src/main.rs` contains no `Event::Key`/`key_pressed` interception at all, and the test `typing_in_a_swatch_name_field_is_not_stolen_by_a_button` asserts a typed space lands in the focused text field and does not trigger `Lock`.
- `Edit` hands keyboard control to that swatch's HEX field (`pending_hex_focus` → `Response::request_focus`) so the value can be replaced and confirmed with Enter without touching the pointer.
- `Earlier`/`Later` provide the required **non-drag** reorder alternative. The pointer drag path is preserved and still works.
- Focus is drawn explicitly: `paint_focus_ring` strokes a 2 px ring around whichever action button holds focus, so the focused control is visible rather than implied.

### Lock invariants

A locked swatch refuses destructive operations and says why, in words, in the status line:

| Operation on a locked swatch | Result |
| --- | --- |
| Delete | Refused — `Swatch N is locked. Unlock it before deleting.` |
| HEX edit | Refused — `Swatch N is locked. Unlock it before editing.`; the HEX field is also disabled for locked swatches |
| History drop (replace color) | Refused — `Swatch N is locked. Unlock it before replacing its color.`; the color is kept |

Lock, color and name travel together through reorder (`move_swatch` moves all three in one step) and through regeneration (`locked_swatch_keeps_name_and_color_through_generation`), and survive save/reopen (`locked_state_and_order_survive_save_and_reopen`).

### Preserved behaviour

Existing HEX editing, the pinned palette across tabs, the collapsible history strip, saved data, the Rust-only/eframe stack and the lightweight tray split are unchanged. The hover drag-to-reorder path and the pinned-palette payload used by the theme tab are unchanged. No automatic theme generation was added and theme-linking semantics were not touched.

## Test-first evidence

Tests were written before or alongside the behaviour they pin, and each was observed failing for the expected reason before the implementation was completed. The following is the resulting coverage in `rust/board/src/palette.rs` (19 tests, all passing):

| Test | What it pins |
| --- | --- |
| `swatch_actions_are_always_painted_at_minimum_size` | All five actions are painted without hover, inside the 960×640 window, at font size 14 |
| `swatch_actions_remain_visible_in_a_540x360_window_with_more_swatches` | Actions and the first swatch stay inside the 540×360 window with 5 swatches at size 14 |
| `swatch_actions_are_not_clipped_by_the_pinned_panel_at_minimum_size` | Actions are fully inside their clip rect — not merely painted — at pinned-panel heights 130 / 145 / 220 px |
| `a_keyboard_focused_action_button_has_a_visible_focus_ring` | The focused action button carries a ≥2 px focus stroke |
| `the_selected_swatch_has_a_visible_selection_outline` | The swatch the bar targets is visibly outlined |
| `pointer_lock_button_locks_and_unlocks_the_selected_swatch` | Pointer lock/unlock, and `Locked` stated in words |
| `keyboard_space_activates_the_lock_button` | Space activates the focused `Lock` button |
| `keyboard_can_reorder_a_swatch_without_dragging` | Tab+Space on `Later` reorders — the non-drag alternative |
| `keyboard_can_delete_an_unlocked_swatch` | Tab+Space on `Delete` deletes an unlocked swatch |
| `the_edit_action_focuses_the_hex_field_for_keyboard_editing` | `Edit` transfers keyboard control to the HEX field; typed value applies on Enter |
| `typing_in_a_swatch_name_field_is_not_stolen_by_a_button` | A typed space reaches the text field and does not trigger `Lock` |
| `hex_editing_still_applies_a_changed_color_and_supports_undo` | A changed (non-default) HEX value applies and is undoable |
| `locked_swatch_refuses_delete_and_explains_why` | Delete refused on a locked swatch, with a worded reason |
| `locked_swatch_refuses_history_replacement_and_explains_why` | History drop refused on a locked swatch; color kept |
| `later_button_reorders_and_keeps_lock_with_its_swatch` | Reorder keeps name, color and lock together |
| `locked_swatch_keeps_name_and_color_through_generation` | A locked swatch keeps its name and color through regeneration |
| `undo_and_redo_restore_the_lock_state` | Undo/Redo restore the lock state |
| `locked_state_and_order_survive_save_and_reopen` | Lock state and new order survive save and reopen |
| `locked_generation_save_reopen_and_mixer_share_active_colors` | Pre-existing invariant: generation, save, reopen and mixer share the same colors |

Two failures observed during development were **test-harness defects, not product defects**, and were corrected in the harness rather than by weakening an assertion:

- `keyboard_can_reorder_a_swatch_without_dragging` and `keyboard_can_delete_an_unlocked_swatch` initially failed with `keyboard could not reach "Later"/"Delete"`. A temporary diagnostic showed the focus *does* reach both (Delete at Tab step 3, Later at step 4); the helper had cached the target rectangle from an earlier frame while the panel re-laid out by 8 px between frames. The helper now re-reads the target each step, matching the proven pattern in `rust/board/src/mixer.rs`.
- `the_edit_action_focuses_the_hex_field_for_keyboard_editing` initially appended to the existing HEX value, producing an invalid color. The test now selects all before typing, as a real edit would.

## Exact executed results

All commands ran in the repaired Docker sandbox (`kestrel-rust-swatch:local`, Rust 1.100.0, cargo 1.100.0) with `CARGO_TARGET_DIR=/workspace/targets/swatch-board`, `CARGO_HOME=/workspace/.cargo`. No host packages were installed, no host settings were changed, no Wayland/session bus was mounted, and no live KWin was touched.

```
cargo build --workspace --locked --all-targets        → exit 0
cargo fmt --all && cargo fmt --check --all            → exit 0 (no diff)
cargo clippy --workspace --locked --all-targets -- -D warnings → exit 0
cargo test -p swatch-board --locked --bin swatch-board palette::
    → test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out
cargo test --workspace --locked --all-targets --no-fail-fast
    → 195 passed; 3 failed  (see below)
```

### Pre-existing failures, not regressions

Three `theme::tests` failures remain in the board crate. They were confirmed to fail **identically on untouched HEAD `195cb9f`** by stashing the change and re-running each test in isolation, so they are not caused by this slice:

- `theme::tests::export_preview_remains_reachable_above_pinned_palette_in_short_window`
- `theme::tests::external_plugin_hidden_scene_text_role_accepts_selection_and_drop`
- `theme::tests::named_palette_link_propagates_and_survives_theme_undo_redo_and_reopen`

`theme.rs` is outside this slice's file ownership and these were not introduced by `palette.rs`.

### Environment note on `swatch-board-theme-core`

Three `swatch-board-theme-core` tests (`resolves_and_renders_strictly`, `intermediate_allocation_is_contained_and_next_render_succeeds`, `bundled_package_golden_outputs`) initially failed at HEAD with `missing swatch-board-theme-worker beside executable`. This is a sandbox layout issue: the helper lives in `target/debug/`, but in this cargo version the test binary is placed under `target/debug/build/<pkg>/<hash>/out/`, so the `deps`-parent fallback in `rust/theme-core/src/isolation.rs` does not apply. Copying the already-built `swatch-board-theme-worker` beside the test binary (a build artifact only — no source change) makes all three pass, including the packaging golden-output check. The same failure occurs on untouched HEAD.

## Remaining gaps and limitations

- **No native GUI evidence.** Every check here is headless egui. No screenshot, no real Wayland/X11 input injection, no KWin session, and no verification against the active Plasma theme or font. Per the task constraint, headless checks are not native screenshots or input.
- **No assistive-technology evidence.** No AT-SPI tree was inspected and no screen-reader operation was exercised. Whether the action bar's labels reach an AT client as usable names is unverified; `rust/board/src/palette.rs` does not set explicit accessibility metadata, so this must be checked by the native critic and reported honestly.
- **540×360 pinned-panel height is now covered headlessly, not visually.** The earlier builder caveat about the action bar adding a row of height is addressed by `swatch_actions_are_not_clipped_by_the_pinned_panel_at_minimum_size`, which asserts full clip-rect containment at the panel's smallest allowed height (130 px). That is painted-output evidence in egui; it is not a native screenshot, and the lead's note that this lies outside this slice's ownership of `main.rs`/`mixer.rs` still applies to any change to the panel's configured geometry. The panel height constants were left untouched.
- **Focus order is asserted, not designed.** Tests confirm Tab reaches each action; they do not establish that this is the *ideal* traversal order for a screen reader or for a user who never uses a mouse.
- **The `Locked` badge is drawn over the swatch tile** and may overlap a very light color at the tile's top-left. It carries a dark translucent backing plate for contrast, but native visual confirmation is outstanding.
- **Changed swatch names are still validated for uniqueness and non-emptiness**, and an invalid name resets the field with a visible error — that behaviour is inherited and unchanged, and is not covered by a new test in this slice.
- Only `rust/board/src/palette.rs` changed. `main.rs` was not modified; the `Edit` button's focus handover is implemented entirely within `palette.rs`.
