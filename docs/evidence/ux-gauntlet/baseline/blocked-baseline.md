# UX baseline — blocked, not an acceptance report

Task `t_1d407be1`; intended baseline commit `195cb9f` (not independently verified). No application was built or launched in this run. No screenshots, task traces, interaction counts or scores were obtained. All five scoring dimensions are **untested**, not passed. Do not use this document as the requested BEFORE comparison or as a builder verdict.

## Concrete execution blocker

This worker execution context does not expose `terminal`, `computer_use`, or `browser_exec`; explicit tool description returned each as unavailable. File reading and web retrieval are available, but cannot start an isolated compositor, build the binary, inject verified private-session input, check process cleanup, or perform AT-SPI inspection. Do not route input to the human session as a workaround. A worker with a shell/process execution tool and private Wayland access is needed; no host settings or package changes are implied.

## Useful preliminary evidence (not fresh testing)

- `README.md:131–175` describes the shared history and pinned palette and says swatch lock/delete controls appear on hover and reordering is by drag. This suggests an accessibility/discoverability hypothesis worth checking in the actual app; documentation alone does not prove there are no keyboard alternatives.
- `docs/current-handoff.md:201–215` describes a historical user-reported keyboard round and a separate old clipped-feedback repair. These are not fresh baseline observations and do not establish current behavior at either required size.
- `~/.config/kdeglobals` read-only: `[General] ColorScheme=EmeraldDark`, accent RGB 233,58,154; `[Icons] Theme=breeze`; `[WM] activeFont=Noto Sans,12`. A general UI font was not specified in the inspected file. Configured values do not establish the current Board honors them.
- KDE HIG [Powerful when needed](https://develop.kde.org/hig/powerful_when_needed/) recommends visible primary controls and shortcuts/context menus as accelerators rather than exclusive access, and deferring to platform styling. This is a reference criterion, not proof of a defect.
- The inspected `spikes/002-native-shield/testing/run-isolated-native-lab.py` hard-codes `target/isolated-native-lab-1`, a 1024×768 two-output lab and a probe identity; it is not suitable to run blindly for this task's exclusive output paths or required sizes. Its companion `private-lab-command.py` also hard-codes that lab. Reassess current harness and adapt only within authorized owned paths before use.

## Required next step

Restore shell/process execution and an isolated private Wayland input/capture path to this task. Then verify the source revision and binary identity, inspect current harnesses, build the actual workspace, launch the real Board in isolated KWin at 960×640 and 540×360, save screenshots and action traces, test reachable task workflows and AT-SPI/input availability, and choose the first slice from observed results. Avoid any claim of having tested the physical desktop. No first builder slice is selected without the required rendered evidence.
