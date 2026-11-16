# UI improvements plan

Requested 2026-11-16. Work started from freshly fetched `origin/main`.

- [x] Pin the mixer current color; use adjacent clickable, reorderable hex swatches; make history a collapsible sidebar.
- [x] Add icon navigation buttons and a persistent label toggle; convert the main editing actions to icon buttons.
- [x] Replace theme color fields on drop, remove the duplicate palette, and keep an automatically updating preview visible.
- [x] Route the tray's Show Palette action to the Board Palette page, including an already running Board.
- [ ] Identify and fix the reported settings footer error. The footer can display real shortcut-service errors; the specific message has not been established.
- [ ] Extend the generator's new interactive base-color wheel toward Adobe-style harmony handles and live palette generation.
- [ ] Complete responsive layout and input sizing across all views, especially settings; verify small windows, keyboard use, and accessibility on the target desktop.
- [ ] Verify the tray action, drag/drop, theme preview and toolbar behavior on a real Plasma Wayland desktop.

Plan: integrate and verify the current mixer, theme, generator, toolbar, and tray slices; record the remaining gaps in the handoff; commit coherent changes and push. Continue the open items with concrete desktop observations.
