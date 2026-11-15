# Session D-Bus API, version 1

Service `org.apartment38.SwatchBoard1`, object `/org/apartment38/SwatchBoard1`, interface `org.apartment38.SwatchBoard1`. Any client permitted to connect to the user's session bus can invoke methods and observe signals; this API does not authorize cross-user access.

- `Activate()`: signal an existing instance; does not start a pick or change focus.
- `PickColor() -> s`: `started` or `busy`. Launch failures return `org.freedesktop.DBus.Error.Failed` and emit `ColorPickFailed`.
- `ShowHistory() -> s`: `started` opens the transient history popup; `closed` dismisses an existing popup; `busy` means the picker is active. Launch failures return a D-Bus error.
- `CopyHistory(s id) -> s`: copy an existing capture by UUID in the configured clipboard format, request a desktop notification, and return the copied text. Missing captures or clipboard failures return a D-Bus error. Copying does not insert, reorder, or signal a history change; the tray retains clipboard ownership.
- `Configure() -> s`: `started` opens the transient settings window, or `already-open` if one is running. Launch errors return a D-Bus error.
- `ReloadConfig() -> s`: validate and apply saved settings to the tray, returning `applied`. Invalid or unreadable settings leave the active settings unchanged. Queues shortcut reassignment asynchronously; a worker failure is reported even if other settings were already applied.
- `ShortcutStatus() -> s`: report the global shortcut portal status, assigned shortcuts, and any reassignment failure.
- `RebindShortcuts() -> s`: request registration and KDE reassignment of the saved shortcut preferences. Both portal actions must already be assigned; conflicts or unassigned actions require manual desktop configuration. The result is asynchronous and not atomic; inspect `ShortcutStatus` for failures or partial changes.
- `GetLastColor() -> s`: uppercase `#RRGGBB`, or empty when history is empty. Restored from the database at startup.
- `GetHistory(u limit) -> a(ssss)`: newest captures first, each `(UUID, #RRGGBBAA, UTC timestamp, source)`; source is `picker` or `board`. Zero returns no rows. Storage errors return a D-Bus error.
- `Quit()`: shut down and reap any active picker and history popup.
- `ColorPicked(s id, s rgba, s timestamp)`, `HistoryChanged()`, `ColorPickCancelled()`, `ColorPickFailed(s reason)`: transient notifications. A committed capture emits `ColorPicked` and then `HistoryChanged`. Clipboard failures are reported separately as tray notifications and do not undo committed captures. Cancellation does not alter history or clipboard.

At most one picker is active; `busy` never queues another. The asynchronous picker has a five-minute deadline. Cancellation is a successful exit with empty stdout; selection is a successful exit with exactly one `#RRGGBB` line. Invalid output, launch failure, timeout, abnormal exit, and storage failure preserve the previous color. Exit, timeout, and shutdown reap the child. Signals are transient; clients should reload authoritative history on startup or reconnect.

## Board instance

The on-demand Board owns `org.apartment38.SwatchBoard.Board1`, object
`/org/apartment38/SwatchBoard/Board1`, interface
`org.apartment38.SwatchBoard.Board1`. `Activate()` requests window activation;
it is ignored while the Board is awaiting a picker outcome to avoid stealing
focus during capture. Launching a second Board invokes this method and exits.
This service exists independently of the tray and disappears when Board closes.
