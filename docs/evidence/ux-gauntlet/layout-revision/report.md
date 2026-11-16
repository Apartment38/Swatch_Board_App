# swatch-layout-revision-1 — bounded builder handoff

**Builder slice complete; not native acceptance and not gauntlet acceptance.** Stop after this revision and existing native re-review `t_26e83fb5`; no subsequent revision cycle or new tasks. No commits, stash, push, host packages, profile changes, or host GUI access.

## Reproduction result (do not overstate)

The original native 540×360 **output** screenshot remains an observation of clipping, not proof of an exact 540×360 **client** defect. This builder did not reproduce persistent pinned-detail clipping in the settled integrated headless 540×360 client, with typography styles set to 14, eight colors, history open, and a real pointer Lock → Delete refusal. Before the repair, horizontal wheel navigation reached all eight names and HEX values. Initial first-swatch name bounds were y=339.9–355.9 inside its text clip ending y=357.0. This is **not** proof that the critic's capture was harness-only; its actual native client geometry remains unknown.

Two integrated problems **were** reproduced before production fixes (`regression-red.log`, exit 101):

1. `layout_tests::integrated_palette_preserves_expanded_height`: seed the same persisted PanelState used by the native resize handle with height 220; production content shrinks it to 140.2 (rect y=219.8–360), defeating expansion. After the repair it stays 220 (rect y=140–360). The early pointer-resize probe did not conclusively establish successful native dragging; final regression explicitly tests persisted layout sizing, not native drag delivery.
2. `layout_tests::integrated_minimum_client_font14_mixer_apply_reachable_above_expanded_palette`: request panel height 220, retain history, render real Mixer inside the production Board layout, and wheel over the remaining central region. Before the repair, Apply is never reachable: the unscrollable channel editor consumes the region before the details scroll area begins. Before sizing was repaired the requested 220 had actually shrunk to 140.2; Apply was unreachable even there. After both repairs the actual 220 panel is retained and Apply is fully visible during the scroll sequence.

The new tests call the same `Board::show_ui` used by `eframe::App::update`; this is not an isolated palette-only harness. RawInput specifies exact logical 540×360 and 960×640 rectangles at default pixel scale; it is not a native client-geometry measurement.

## Small repair, not redesign

- `rust/board/src/main.rs`: extract unchanged UI drawing into `Board::show_ui` for integrated tests; respect the currently available/user-selected panel height with `ui.set_min_height(ui.available_height())`. Existing 130/145/220 bounds are unchanged. No fixed-height inflation was added.
- `rust/board/src/mixer.rs`: move the channel editor inside the existing mixer details scroll area, so expanding the palette does not strand the remaining mixer controls. Mixer heading, Undo/Redo and error text stay outside that scroll area.
- `rust/board/src/palette.rs`: two added lines beyond the inherited candidate: comment and `AlwaysVisible` on the existing **horizontal** scrollbar. The selected-swatch action bar and full locked-refusal status stay outside it. No lock/model/storage/picker behavior changed.
- `rust/board/src/layout_tests.rs`: three isolated integrated checks. Subprocess-local XDG paths avoid changing process-global environment or touching ordinary user repositories. Fixture data is project-local under target and cleaned after the subprocess.

A trial two-axis palette scroll introduced two theme-harness failures because those existing callers use a content-sized panel and assume names are initially painted. That trial was removed; no tests were weakened and no `theme.rs` edits were made. The final horizontal implementation passes every theme test. This bounded iteration does not add a speculative vertical palette redesign when the original persistent clipping has not been reproduced at exact settled client geometry.

## Coverage and exact logs

Final `regression-green.log` passes all three tests:

- `layout_tests::integrated_palette_preserves_expanded_height` — RED → GREEN.
- `layout_tests::integrated_minimum_client_font14_mixer_apply_reachable_above_expanded_palette` — RED → GREEN.
- `layout_tests::integrated_minimum_client_font14_palette_details_reachable_with_locked_refusal` — characterization/guard, not a claimed RED → GREEN for the original screenshot. It checks all eight names and HEX values with full clip/window containment at both client sizes and requested panel heights 130, 145, 220. At the smaller requests the inherited content-sized panel can settle larger than requested (e.g. 156.2 with refusal); the final 220 case is actually 220. All six combinations retain complete refusal and Add/Unlock/Edit/Delete/Earlier/Later labels during horizontal navigation.

`red.log` is the **initial passing characterization**, despite its early filename. `reachability-probe.log` contains an invalid exploratory assertion that tried to recover Swatch 1 vertically after scrolling it off to the right; it is **not a product regression**. `regression-red.log` is the valid two-failure baseline. Intermediate `board-first/second/third.log` and `first-fix.log` preserve trials, environmental failures, and a test-pointer correction; they are not final results. The final test derives wheel Y from the painted HEX region instead of sending it into blank space below the strip at expanded height.

### Final executed checks

Docker was verified with `/.dockerenv`; effective sandbox user was `pn`. The initial PATH lacked cargo; the existing `/opt/kestrel-rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin` toolchain was used (cargo/rustc 1.100.0). No installation or host fallback. Reused existing sandbox Cargo cache and build target at `/workspace/kestrel-swatch/{cargo,target}`. `verify.py` records the exact environment and commands; invoke from the repository root with `python3 docs/evidence/ux-gauntlet/layout-revision/verify.py`.

Workspace built **before any tests** (`build-before.log`, exit 0), and again before final tests:

| Command | Actual result | Log |
|---|---|---|
| `cargo build --offline --locked --workspace --all-targets` | exit 0 | `build-final.log` |
| `cargo test --offline --locked -p swatch-board --bin swatch-board layout_tests:: -- --nocapture --test-threads=1` | 3 passed, exit 0 | `regression-green.log` |
| `cargo test --offline --locked --workspace --all-targets --no-fail-fast -- --test-threads=1` | **201 passed, 0 failed, 1 ignored**, exit 0 | `workspace-tests.log` |
| `cargo clippy --offline --locked --workspace --all-targets -- -D warnings` | exit 0 | `clippy.log` |
| `cargo fmt --all` then `cargo fmt --all -- --check` | both exit 0 | `fmt-apply.log`, `fmt-check.log` |
| `git diff --check` | exit 0 | `diff-check.log` |

The ignored test is the existing `isolation::tests::pipe_fixture`. Totals were parsed and summed from the workspace log by `manifest.py`, not inferred from prior handoffs.

### Three documented baseline theme failures

The prior report listed:

- `theme::tests::export_preview_remains_reachable_above_pinned_palette_in_short_window`
- `theme::tests::external_plugin_hidden_scene_text_role_accepts_selection_and_drop`
- `theme::tests::named_palette_link_propagates_and_survives_theme_undo_redo_and_reopen`

These are **all passing in the final run**; do not count them as remaining failures or claim this revision fixed their theme behavior. This Cargo places test executables in `debug/build/<crate>/<hash>/out`, where the existing helper lookup cannot find `swatch-board-theme-worker` in `debug/`. As documented by the prior builder, the already-built helper was copied beside the Board/theme-core test executables (build artifacts only). `helper-staging.json` records destinations and helper hash. Early Board runs without staging reproduced missing-helper-related failures. No shipped helper lookup, theme source, or baseline evidence was modified.

## Provenance

HEAD: `195cb9f41128cf25c44b802cd20d4858a3a3c0ca`.

`candidate-source.diff` is the exact three tracked-source diff against HEAD followed by the no-index diff adding `layout_tests.rs`. It deliberately includes the inherited uncommitted swatch-actions candidate, not just this revision.

- Source diff SHA-256: `5930d0ff3949c9754f80f8dad70f6c8ca42eccef2dd87b65967cdda5d770e94d`
- Built Board: `/workspace/kestrel-swatch/target/debug/swatch-board`
- Board SHA-256: `b305123d60c4b8b98893747cc00acd4252002d352bbd99ba83939aecab97b9af`
- Machine-readable evidence: `manifest.json`, `results.json`.

Reviewed final diffs: only main/mixer/palette and the supporting layout test changed; inherited action/storage protection and picker/worker paths are untouched. No changes to `theme.rs`, Cargo manifests/lock, other docs/progress, or unrelated source. Existing untracked evidence/progress files were preserved.

## Remaining gaps / native reviewer handoff

- Original native bottom clipping is **not disproved or established as harness-only**. Measure actual client rect independently of virtual-output dimensions and inspect settled frames.
- Native pointer scrollbar/resize usability, keyboard scroll/focus reachability at actual minimum size, larger fonts/platform scaling, AT-SPI, and physical desktop are unverified here. `AlwaysVisible` config and synthetic wheel tests do not certify native scrollbar target usability or assistive technology.
- The mixer regression establishes full painted Apply reachability during wheel navigation, not a new full native Apply/Undo/Redo workflow. Existing mixer and palette pointer/keyboard/model tests all pass, but native delivery still requires the dependent review.
- Final palette navigation remains horizontal. Do not describe this as a new vertical palette scroller. Long/custom names, taller-than-tested typography and additional multi-line errors are not newly certified.
- No native processes were started by this Docker builder; no compositor/session cleanup is claimed. The dependent reviewer owns its own native runtime cleanup.
- Only existing native re-review `t_26e83fb5` follows. Report any remaining FAIL/UNTESTED findings and stop; user explicitly prohibited another loop or new cards.
