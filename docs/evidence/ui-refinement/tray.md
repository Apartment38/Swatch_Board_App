# Native tray menu refinement

The baseline, candidate, and final captures show the production `rust/tray/src/tray.rs` Item rendered by the real Plasma 6.3.5 StatusNotifierHost, KWin Wayland, and Breeze menu style at 1800×1000, scale 1, 96 DPI. KColorChooser is visible as the native reference. A temporary Rust harness imports that source file unchanged and calls its `start` function, consuming action requests without opening windows; it avoids colliding with the history fixture service. The menu is not recreated by the harness. These captures verify presentation, not end-to-end action dispatch.

The baseline has five plain rows. The refinement adds theme-resolved Breeze icons, groups the three primary actions above a native separator, and adds the dialog ellipsis to Configure. Existing callbacks remain unchanged.

A fresh separate critic accepted the candidate over the baseline for scanability and native appearance. It identified the long “Configure Swatch Board…” label as the largest remaining gap and noted that the native red Quit icon is a minor nonblocking visual emphasis. The final version addresses the width feedback with “Configure…”. The final screenshot was visually inspected after rebuilding; the independent critique was of the longer-label candidate.

Validation: production tray package builds; all five existing tray tests pass; rustfmt and whitespace checks pass. Screenshots retain the full desktop: the baseline also has Plasma's expanded status panel behind the menu, while the candidate and final do not.
