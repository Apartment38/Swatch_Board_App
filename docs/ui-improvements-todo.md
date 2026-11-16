# UI improvements plan

Requested 2026-11-16. Work started from freshly fetched `origin/main`.

- [x] Place a synchronized mixer color wheel and saturation/value gradient side by side.
- [x] Pin the mixer current color; use adjacent clickable, reorderable hex swatches; make history a collapsible sidebar.
- [x] Add icon navigation buttons and a persistent label toggle; convert the main editing actions to icon buttons.
- [x] Replace theme color fields on drop, remove the duplicate palette, and keep an automatically updating preview visible.
- [x] Route the tray's Show Palette action to the Board Palette page, including an already running Board.
- [ ] Verify the reported Settings footer error on the affected KDE session. The supplied transcript describes a portal app-ID lookup failure but does not include the screenshot or literal error.
- [x] Add actionable guidance for host-app registration failures and document the missing desktop-entry step for uninstalled development builds, while preserving the portal error and denial behavior.
- [x] Add a live, lock-aware generator preview and draggable harmony markers; Generate explicitly applies the result.
- [ ] Continue responsive layout verification across all views, especially small windows, keyboard use, and accessibility on the target desktop. Mixer, Theme, and Settings controls have been compacted and offscreen checks now reach 540×360 and verify the preview, controls, and palette are visible.
- [ ] Verify the tray action, drag/drop, theme preview and toolbar behavior on a real Plasma Wayland desktop.

Plan: verify the Settings diagnostic against the original message and KDE session, then complete the remaining real desktop verification.
