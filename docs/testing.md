# Release test matrix

Record the exact Swatch Board commit, distro/release, kernel, Rust version,
Plasma/KWin and portal versions, GPU driver, output model/scale/transform, and
whether the result came from an automated test, private session, or physical
user observation. A headless result never substitutes for a physical GUI result.

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
