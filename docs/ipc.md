# Session D-Bus API, version 1

Service `org.apartment38.SwatchBoard1`, object `/org/apartment38/SwatchBoard1`, interface `org.apartment38.SwatchBoard1`. The session bus controls access; any client permitted to connect to the user's session bus can invoke the methods and observe signals. This API does not authorize cross-user access.

- `Activate()`: signal an existing instance that another tray invocation occurred; does not start a pick or change focus.
- `PickColor() -> s`: `started` if a picker was started, `busy` if another pick is active. A launch failure produces `org.freedesktop.DBus.Error.Failed` and `ColorPickFailed`.
- `GetLastColor() -> s`: uppercase `#RRGGBB`, or the empty string before a successful pick. State is in memory only until Phase 1C.
- `Quit()`: request tray shutdown, terminating and reaping any active picker.
- `ColorPicked(s color)`, `ColorPickCancelled()`, `ColorPickFailed(s reason)`: completion signals for a started pick. Failure reasons distinguish launch, timeout, abnormal exit, and invalid output.

At most one picker is active; `busy` never queues another. The asynchronous picker has a five-minute deadline. Cancellation is a successful exit with empty stdout; selection is a successful exit with exactly one `#RRGGBB` line. All other outcomes are failures, preserving the prior color. Exit, timeout, and tray shutdown reap the child. Signals are transient; clients reconnecting should query `GetLastColor`. Future history, clipboard, configuration, and Board operations are not part of this version's Phase 1B implementation.
