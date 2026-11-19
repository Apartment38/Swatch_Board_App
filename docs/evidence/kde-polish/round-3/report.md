# KDE polish: discoverable Theme project controls, round 3

Builder: gpt-6.1-terra. Source-blind rendered critic: gpt-6.1-sol.

A visible Project action opens a native dialog containing plugin selection, New theme, saved theme selection, Open theme, Reload, and the theme name. These controls are directly available at compact and normal sizes instead of being buried below the role list. The editor retains its compact role, palette link, Show preview, and Save controls.

Verification: build, Breeze native Qt smoke, and whitespace check passed. Smoke covers Project with no project, new theme, name change, closing the dialog, saving and reopening a saved theme, preview toggling, and Tab traversal to Export. The offscreen harness reactivates its main window after closing the top-level dialog before sending keys.

The lead independently tested the actual X11 desktop window: opened Project, closed it, pressed Tab without manually activating the main window, observed focus return to Save, and continued through role controls until Export received focus and scrolled fully into view. The attached desktop screenshot records focused Export. The fixture then closed normally. No normal user data was used, and no export was performed during the focus check.

Critic visual verdict: **accept scoped Theme editor layout**. The direct Project route and compact native dialog resolve the last discoverability concern, while preserving the editable area gained in rounds 1–2. Blank history space and excess whitespace around the normal-height preview are separate remaining workspace polish opportunities. This verdict does not claim physical Plasma/Wayland or whole-application acceptance.
