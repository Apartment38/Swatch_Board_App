# Session D-Bus API, version 1

Service `org.apartment38.SwatchBoard1`, object `/org/apartment38/SwatchBoard1`, interface `org.apartment38.SwatchBoard1`. Any client permitted to connect to the user's session bus can invoke methods and observe signals; this API does not authorize cross-user access.

- `Activate()`: signal an existing instance; does not start a pick or change focus.
- `PickColor() -> s`: `started` or `busy`. Launch failures return `org.freedesktop.DBus.Error.Failed` and emit `ColorPickFailed`.
- `GetLastColor() -> s`: uppercase `#RRGGBB`, or empty when history is empty. Restored from the database at startup.
- `GetHistory(u limit) -> a(ssss)`: newest captures first, each `(UUID, #RRGGBBAA, UTC timestamp, source)`; source is `picker` or `board`. Zero returns no rows. Storage errors return a D-Bus error.
- `Quit()`: shut down and reap any active picker.
- `ColorPicked(s id, s rgba, s timestamp)`, `HistoryChanged()`, `ColorPickCancelled()`, `ColorPickFailed(s reason)`: transient notifications. A committed capture emits `ColorPicked` and then `HistoryChanged`. Clipboard failures are reported separately as tray notifications and do not undo committed captures. Cancellation does not alter history or clipboard.

At most one picker is active; `busy` never queues another. The asynchronous picker has a five-minute deadline. Cancellation is a successful exit with empty stdout; selection is a successful exit with exactly one `#RRGGBB` line. Invalid output, launch failure, timeout, abnormal exit, and storage failure preserve the previous color. Exit, timeout, and shutdown reap the child. Signals are transient; clients should reload authoritative history on startup or reconnect. History popup, configuration reload, and Board operations belong to later phases.
