# UI improvements plan

Requested 2026-11-16. Work started from freshly fetched `origin/main`.

- [x] Pin the mixer current color; use adjacent clickable, reorderable hex swatches; make history a collapsible sidebar.
- [x] Add icon navigation buttons and a persistent label toggle; convert the main editing actions to icon buttons.
- [x] Replace theme color fields on drop, remove the duplicate palette, and keep an automatically updating preview visible.
- [x] Route the tray's Show Palette action to the Board Palette page, including an already running Board.
- [ ] Identify and fix the reported settings footer error. The footer can display real shortcut-service errors; the specific message has not been established.
- [x] Add a live, lock-aware generator preview and draggable harmony markers; Generate explicitly applies the result.
- [ ] Continue responsive layout verification across all views, especially small windows, keyboard use, and accessibility on the target desktop. Mixer, Theme, and Settings controls have been compacted and offscreen checks now reach 540×360 and verify the preview, controls, and palette are visible.
- [ ] Verify the tray action, drag/drop, theme preview and toolbar behavior on a real Plasma Wayland desktop.

Plan: complete private Wayland and real desktop verification, diagnose the specific Settings message, and record verification in the handoff.
