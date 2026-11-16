# Release test matrix

Record the exact Swatch Board commit, distro/release, kernel, Rust version,
Plasma/KWin and portal versions, GPU driver, output model/scale/transform, and
whether the result came from an automated test, private session, or physical
user observation. A headless result never substitutes for a physical GUI result.

## Automated container checks

From the repository root, run:

```sh
testing/container-tests.sh
```

Docker builds a Debian/Rust test image from the current working tree and runs
locked workspace builds and tests, strict Clippy, rustfmt, packaging tests, and
Board and Settings Qt offscreen control smokes. The runner uses separate private
D-Bus sessions and temporary XDG directories; it does not interact with the
running desktop or its data. A temporary Git bundle supplies commit objects for the packaging source-pin
test; it is deleted after the run. The first image build needs network access for
system packages and locked crates; the test run itself uses Cargo offline mode.

To retain the smoke JSON and client-area screenshots, set an output directory:

```sh
SWATCH_TEST_ARTIFACTS="$PWD/target/test-artifacts" testing/container-tests.sh
```

The runner exits nonzero on a failed command, timeout, or failed smoke result.
The offscreen controls and snapshots do not test real Wayland input, the tray,
compositor behavior, screen readers, or physical desktop acceptance. The
existing ignored test and the Arch-only `makepkg` check remain skipped in this
Debian image. System packages and the image tag can change upstream, so this is
a repeatable test setup rather than a bit-for-bit reproducible release build.

## Qt frontend cutover

The Board, Settings, and history interfaces now use Qt 6 Widgets. All physical
frontend observations in the table below describe the replaced implementation;
they are not acceptance of the Qt rewrite. Re-run keyboard-only traversal,
screen-reader inspection, live KDE palette/font changes, fractional/mixed scaling,
history popup focus/outside dismissal, Board activation and hide/pick restoration
on the target desktop. Do not restart live KWin for verification.

The Rust model tests exercise the same parsing, mixing, palette locks and
storage, theme links/drafts and worker-backed export used by the Qt controls.
Build all normal binaries before model tests that launch the sibling theme
worker. The Qt C++ sources are built as part of `cargo build --workspace`.

To exercise real Qt controls and collect client-area images, use a fresh,
isolated directory (the Board scenario writes a palette and theme draft):

```sh
root=$(mktemp -d)
mkdir -p "$root"/{config,data,state,images}
dbus-run-session -- env \
  XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
  XDG_STATE_HOME="$root/state" SWATCH_BOARD_QT_SMOKE_DIR="$root/images" \
  ./target/debug/swatch-board
```

Repeat with a fresh root and `swatch-board-config` for invalid-save protection.
The opt-in hook clicks native controls, asserts resulting model values, writes
`*-result.json` and `QWidget::grab` PNGs, then exits nonzero on failure. These
client snapshots do not verify compositor decorations, focus policy, popup
grabs, screen-reader exposure, or physical mouse/keyboard operation.

## Historical acceptance matrix

| Area | Automated/headless evidence | KDE Plasma Wayland evidence | Remaining gate |
| --- | --- | --- | --- |
| Build, data, and packaging | Repaired source: 180 tests (one ignored), strict Clippy, rustfmt, eight packaging checks; fresh Arch container makepkg 0.1.0-2 build/install, full data inventory and four installed-helper golden renders passed | Host-built release from same repaired source passed assigned Board/Settings round; container package bytes not GUI-tested | Official devtools clean-chroot and reproducibility qualification; redistribution notice audit |
| Picker | 17 targeted Rust tests; isolated native GL/input scenarios verified live HEX/swatch, exact cursor pixel square, 1×/4×/8×/32×, two outputs, 1.5× scaling, selection/cancellation, caller scoping, client death, output loss and no click-through | User-approved installation/load and live 4× overlay/cancellation smoke passed without restarting KWin | Broader physical selection/multioutput, screen lock, HDR and output-transform acceptance |
| Shortcut portal prompts and revocation | Isolated bus scenarios for binding and session cleanup | Manual shortcut permission prompts and portal recovery observed on KDE; prompts are always user-operated | Repeat on each supported Plasma/portal release and record exact versions |
| Compositor recovery | Private KWin output toggles and private compositor replacement/recovery | Normal manual logout/login passed | **Do not restart the live KWin service.** The prior attempt ended the session and required reboot. In-place physical restart recovery remains unverified. |
| Clipboard and history | Tray supervision, persistence, ownership error paths, and database tests | Color selection, clipboard paste, history popup, restart persistence passed | Clipboard-manager-specific lifetime variation |
| Keyboard shortcuts and focus | Settings validation and shortcut conflict/status tests | User confirmed tray shortcuts, reassignment, conflict preservation and settings controls; old picker focus evidence is historical | Native KWin picker interaction and broader accessibility evaluation |
| Theme packages | Schema/path/symlink checks, fixture/bundled golden renders, worker limits, saved-link propagation, export refusal/overwrite behavior | Runtime-authored external plugin reload, hidden-scene navigation, drag/edit, save/reopen and export passed | Physical validation of every bundled target and third-party target syntax |
| Board accessibility | Input-driven error-visibility regression at default/minimum window sizes and both palette heights; mouse and keyboard Apply/Undo/Redo | User passed visible focus, three-tab activation, history toggle, corrected Mixer edits/Undo/Redo, Settings invalid-save rejection and valid-save/reopen | Broader Palette/Theme keyboard-only traversal, screen reader, contrast and AT review |
| Process resources and cleanup | Normal tray dependency graph excludes heavy GUI/template/capture crates | Separate Board and Settings idle samples recorded; exact process identities and all staged executables absent after normal close; SQLite integrity ok | Fresh tray/picker/history resource coverage remains separate; samples are not a regression baseline or wakeup measurement |
| Other Linux distributions | None | None | Fedora and openSUSE build, dependency, portal, and desktop acceptance |

The currently recorded physical KDE tests are those in
[`current-handoff.md`](current-handoff.md), with durable Phase 5 evidence under
`evidence/phase5-physical/`. Existing private KWin acceptance is explicitly
separate from actual target desktop evidence. Do not fabricate or extrapolate
results for an untested cell.
