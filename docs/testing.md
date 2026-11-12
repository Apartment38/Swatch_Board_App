# Release test matrix

Record the exact Swatch Board commit, distro/release, kernel, Rust version,
Plasma/KWin and portal versions, GPU driver, output model/scale/transform, and
whether the result came from an automated test, private session, or physical
user observation. A headless result never substitutes for a physical GUI result.

| Area | Automated/headless evidence | KDE Plasma Wayland evidence | Remaining gate |
| --- | --- | --- | --- |
| Build, data, and packaging | Locked offline workspace build/tests, strict Clippy, rustfmt, installer contract, staged installed-worker renders; fresh Arch container makepkg build/install, 178 tests and package inspection passed | Historical target-desktop acceptance recorded separately | Official devtools clean-chroot qualification; physical release-build qualification |
| Picker and display mapping | Geometry/transform/scaling, stale frames, stream/output loss, protocol and cleanup regressions | Picker acceptance on eDP-1, 1.75x, plus physical eDP/HDMI hotplug; separate private multi-output session | Additional physical GPUs, monitors, fractional scales, and other portal implementations |
| Portal prompts and revocation | Isolated bus scenarios for denial/restoration, binding and session cleanup | Manual permission prompts and portal recovery observed on KDE; prompts are always user-operated | Repeat on each supported Plasma/portal release and record exact versions |
| Compositor recovery | Private KWin output toggles and private compositor replacement/recovery | Normal manual logout/login passed | **Do not restart the live KWin service.** The prior attempt ended the session and required reboot. In-place physical restart recovery remains unverified. |
| Clipboard and history | Tray supervision, persistence, ownership error paths, and database tests | Color selection, clipboard paste, history popup, restart persistence passed | Clipboard-manager-specific lifetime variation |
| Keyboard shortcuts and focus | Settings validation, shortcut conflict/status tests, focus-preserving picker model coverage | User confirmed shortcuts, reassignment, conflict preservation, picker focus/click safety, and settings controls | Broader screen-reader and assistive-technology evaluation; no full accessibility claim |
| Theme packages | Schema/path/symlink checks, fixture/bundled golden renders, worker limits, saved-link propagation, export refusal/overwrite behavior | Runtime-authored external plugin reload, hidden-scene navigation, drag/edit, save/reopen and export passed | Physical validation of every bundled target and third-party target syntax |
| Board accessibility | egui interaction and focus behavior covered by application tests where present | Keyboard focus was observed in the accepted workflow | Systematic keyboard-only traversal, screen reader, contrast and AT review |
| Other Linux distributions | None | None | Fedora and openSUSE build, dependency, portal, and desktop acceptance |

The currently recorded physical KDE tests are those in
[`current-handoff.md`](current-handoff.md), with durable Phase 5 evidence under
`evidence/phase5-physical/`. Existing private KWin acceptance is explicitly
separate from actual target desktop evidence. Do not fabricate or extrapolate
results for an untested cell.
