# Colorboard — Agent Implementation Plan

**Status:** Requirements confirmed, 2026-10-20; Rust + existing KWin color selection + ScreenCast magnifier architecture prioritized (revision 1.3).  
**Working name:** Colorboard (renameable without changing interfaces).  
**Initial platform:** KDE Plasma 6 **Wayland only**.  
**Languages:** Rust for the resident tray, transient picker/magnifier and history UI; Python/PySide6 for the on-demand main board. **Use existing KWin services rather than installing a new KWin effect by default.** A narrowly scoped C++ KWin extension is permissible only if the mandatory behavior proves impossible with supported Rust-facing desktop interfaces. Theme plugins remain external, data-driven packages, independent of the host language.  
**Architecture decision:** Keep Rust + Python for implementation. An all-Rust editor remains a future option, not a current work item; revisit editor-toolkit choice only at the Phase 1 → Phase 2 gate without delaying or rewriting the Rust picker.

## 1. Product and non-negotiable requirements

Build two independently runnable applications that share colors, palettes, configuration conventions, and an IPC contract:

1. **Color Picker:** a low-memory Rust system-tray application that picks pixels anywhere on the desktop through a **live, pixel-accurate magnifying glass**, maintains color history, provides a centered history popup, and exposes keyboard shortcuts.
2. **Colorboard:** an on-demand Python/PySide6 GUI with three tabs, built **one at a time**: Color Mixer, Palette Editor, Theme Generator.

### Hard constraints

- The live magnifier is **required for Phase 1 completion**. KWin's `org.kde.kwin.ColorPicker.pick` may be used **for selection, confirmation, and final color return only**; it must operate alongside the live Rust magnifier, never replace it. A frozen screenshot is not an acceptable substitute.
- The underlying desktop view in the magnifier must update while the pointer moves and while desktop contents change.
- Starting the picker must not raise the Colorboard GUI or steal keyboard focus from the previously active application. KDE-managed permission dialogs are the sole permitted exception.
- The picker may intercept pointer input to confirm or cancel selection, but must not inadvertently send the selection click through to the application beneath it.
- No permanent screen-capture session or Python interpreter may run in the idle tray process.
- Failure to meet the magnifier or focus requirements **blocks release and all later phases**, rather than downgrading them into optional features.
- Target KDE Plasma 6 Wayland, not X11 or other compositors, for the first release.
- Favor low **idle proportional/private memory** and near-zero idle CPU; measure PSS/USS rather than using RSS alone.
- **Externally installable theme plugins are a core requirement:** installing a compatible plugin must never require modifying, rebuilding, or reinstalling Colorboard, the Rust picker, or the Python editor. Common plugins must be authorable entirely as declarative data, assets, and Jinja2 templates; code hooks are optional.

## 2. Confirmed interaction contract

### Tray

- Pipette tray icon with a small indicator dot matching the latest chosen color; neutral indicator before the first selection.
- **Left click:** begin picking immediately.
- **Right click:** show menu, in this order:
  1. Pick Color
  2. Show Palette
  3. Open Board
  4. Configure
  5. Quit
- Dynamic icon must work against dark/light Plasma panels, at typical tray icon sizes and fractional scaling.
- The tray process remains active when the popup, picker, or Python application closes.

### Picking

- Triggerable from tray, global shortcut, and Python application through IPC.
- Show a **live** magnifier following the pointer, with configurable sample radius and zoom.
- Clearly outline the **exact physical source pixel** that a confirmation would select. Prefer nearest-neighbor enlargement and a grid at sufficient zoom.
- Left click confirms, right click cancels; add keyboard cancel only where it does not violate keyboard focus preservation.
- On success, append history, update tray dot, copy according to the configured format, and publish a notification through IPC.
- On cancel, save nothing and restore the ordinary desktop state.
- Preserve keyboard focus on the previously active app during selection. A KDE portal permission prompt may temporarily receive focus.
- Cover all monitors and handle different scales, offsets, and rotation without sampling the cursor or magnifier overlay itself.

### Centered history popup

- Frameless popup centered on the **currently active/pointer monitor** (define and test one unambiguous selection policy).
- Scrollable grid of recent colors; click copies to clipboard.
- Configurable display limit, visible rows, and columns, independently of history retention.
- Close on Escape, click-away, or a second activation, without stopping the tray service.
- The popup **may take keyboard focus**, unlike the active screen picker.
- Display a copied-state acknowledgment and a readable label/tooltip for each swatch.

### Proposed defaults (configuration, not hardcoded logic)

| Setting | Default |
|---|---|
| Retained color history | 100 captures |
| History-popup display limit | 24 |
| Grid | 4 columns × 3 visible rows |
| Magnifier radius | 7 source pixels (15×15 sample grid) |
| Magnification | 10× nearest-neighbor |
| Clipboard format | Uppercase `#RRGGBB` |
| Automatically copy on pick | Enabled |
| Pick shortcut | Suggested `Meta+Shift+C`, subject to conflict/portal approval |
| Popup shortcut | Suggested `Meta+Shift+H`, subject to conflict/portal approval |

Settings UI must permit changing all settings named in the user's requirements, including both global hotkeys.

## 3. Source references and crucial finding

The linked KDE source directory is correct:

- <https://github.com/KDE/kdeplasma-addons/tree/master/applets/colorpicker>
- <https://github.com/KDE/kdeplasma-addons/blob/master/applets/colorpicker/grabwidget.cpp>
- <https://github.com/KDE/kdeplasma-addons/blob/master/applets/colorpicker/qml/main.qml>
- <https://github.com/KDE/kwin/blob/master/src/plugins/colorpicker/colorpicker.cpp>
- <https://github.com/KDE/kwin/blob/master/src/plugins/screencast/screencaststream.cpp> (cursor metadata generation and position mapping)
- <https://github.com/KDE/xdg-desktop-portal-kde/blob/master/src/screencast.h> (portal cursor modes)
- <https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html> (ScreenCast portal, source selection, cursor mode, restore tokens)
- <https://develop.kde.org/docs/plasma/kwineffect/> (declarative KWin QML effects)
- <https://api.kde.org/org-kde-kwin-qmlmodule.html> (published KWin QML types)

The applet's `GrabWidget::pick()` asynchronously calls KWin's D-Bus service `org.kde.KWin`, object `/ColorPicker`, interface `org.kde.kwin.ColorPicker`, method `pick()`. KWin performs interactive pointer selection and returns a Qt `QColor` (a D-Bus structure containing a `uint32` packed Qt `QRgb` value, conventionally `0xAARRGGBB`). KWin implements click selection/cancellation and rejects concurrent picks. **The method supplies the final color only; it does not provide live neighboring pixel data.** Use it with—not instead of—the Rust magnifier.

KWin's ScreenCast implementation supports cursor metadata (`SPA_META_Cursor`) when the stream requests the metadata cursor mode. It computes cursor position in stream coordinates, accounting for the source's device-pixel ratio. KDE's XDG ScreenCast portal advertises a cursor metadata mode (`cursor_mode = 4`) on supported versions. This provides a promising **cursor tracking source without a global pointer-querying API**, but the returned frame geometry, transforms, buffer formats, selected monitors, timing, and interaction with KWin's picker must all be validated on the target session.

**Selected integration strategy:** Rust obtains authorized live desktop frames and cursor position metadata through XDG ScreenCast + PipeWire, renders a transparent/non-keyboard-focusable Wayland magnifier overlay, and invokes **KWin's existing ColorPicker D-Bus selection** for the definitive picked color. This is an engineering hypothesis based on individual documented/code-backed facilities, **not a proven end-to-end combination**; Phase 1A must validate it in a real Plasma Wayland session.

KWin QML/JavaScript effects can be packaged separately and hosted by KWin, including alongside Rust applications. However, the published public QML effect API does not document arbitrary framebuffer pixel reads; `SceneEffect` also has scene-composition semantics that complicate a transparent desktop magnifier. **Do not create a custom QML effect in the preferred implementation or treat it as a mandatory intermediate fallback.** Evaluate one only if a specific, documented blocker can demonstrably be solved by a supported effect API. Native C++ KWin integration remains a narrowly scoped fallback after diagnosing the Rust approach.

The referenced KDE applet contains GPL-licensed code. Do not copy or adapt that implementation without verifying and complying with its license requirements; reference its behavior or independently implement the features when appropriate.

## 4. Architecture and process boundaries

Recommended layout:

```text
[ KDE Plasma tray ]
         |
[ colorboard-tray : Rust ]  <--- always resident
  ksni + zbus + config + SQLite + clipboard ownership
         |
         +---- launch/IPC ---> [ colorboard-picker : Rust ]
         |                      (1) XDG ScreenCast + PipeWire frames
         |                      (2) SPA_META_Cursor position
         |                      (3) non-focusable Wayland magnifier
         |                      (4) existing KWin ColorPicker D-Bus pick()
         |                      [minimal native KWin fallback only if needed]
         |
         +---- launch/IPC ---> [ colorboard-history : Rust ]
         |                      centered, transient history popup
         |
         +---- launch ------> [ Colorboard : Python / PySide6 ]
                                Mixer | Palettes | Theme Generator
```

### Process isolation and idle-memory policy

- **Rust is the only always-running application runtime.** `colorboard-tray` owns the tray, event-driven IPC, shortcuts, history coordination, and clipboard; no Python interpreter, PySide6 initialization, plugin loader, or hidden Theme Board process runs at login.
- Picker, history popup, and settings UI are Rust transient processes/components started only when requested and released when dismissed. Screen capture and rendering resources do not persist after a pick.
- `colorboard` (Python/PySide6) launches only via **Open Board** or direct invocation; closing its last main window exits the Python process rather than hiding it to the tray. The tray remains independent and functional even if the board is absent or crashes.
- Allow **one Theme Board instance**: `Open Board` activates an existing instance or starts one, instead of accumulating Python processes. Never require the Python process for color picking.
- Prefer event-driven D-Bus signals to polling for live history and status updates. Measure idle memory with USS/PSS and the Theme Board completely stopped; measure active-editor memory separately.

The precise Rust GUI toolkit for transient windows should be selected based on verified Plasma Wayland behavior and measured memory usage. Do not instantiate a large general GUI runtime in `colorboard-tray`. `ksni` implements StatusNotifierItem without requiring a resident GTK/Qt UI process.

### Initial Rust dependencies to evaluate

- `ksni`: StatusNotifierItem and dynamic icon pixmaps.
- `zbus`: session D-Bus API and event signals.
- `ashpd`: XDG Desktop Portal calls for GlobalShortcuts and ScreenCast, including `SelectSources` cursor metadata mode and `Start`/`OpenPipeWireRemote`.
- `zbus`: also invokes the **existing KWin ColorPicker D-Bus method asynchronously**, with Qt `QColor` response decoding.
- `wayland-client`, `wayland-protocols-wlr` (layer-shell), or suitable Smithay client toolkit: non-focusable overlay surfaces; verify the chosen protocol is actually available on supported KWin versions.
- PipeWire Rust bindings (`pipewire`/`pipewire-rs`) plus SPA cursor metadata decoding, with versioned buffer/format and coordinate handling.
- `rusqlite`: SQLite storage; WAL and migrations.
- `serde`, `toml`: typed configuration.

Check actual crate versions and API signatures when coding, rather than copying unverified older examples.

### Capture, magnifier, and KWin selection strategy (preferred)

**Primary backend: existing KWin picker + Rust capture and magnifier (no custom KWin effect).** Keep these responsibilities separate:

1. **Live desktop pixels and cursor coordinates — Rust:** Create an XDG ScreenCast portal session, select monitor sources (allow multiple outputs), request **cursor metadata mode (`cursor_mode = 4`)** where available, start the portal session, obtain the PipeWire remote, and consume the resulting video frames and `SPA_META_Cursor`. In metadata mode, KWin's frame excludes a rendered cursor while the cursor coordinates/hotspot are supplied in metadata; confirm this for the selected KDE version. Process metadata and frames in their negotiated stream coordinate space, not as guessed global screen coordinates.
2. **Visual magnifier — Rust:** Create a transient transparent Wayland layer-shell overlay with **no keyboard interactivity**, and a **pointer-empty input region** so it does not intercept KWin's selection events. Track the cursor using the ScreenCast stream metadata; present a nearest-neighbor zoomed neighborhood around the physical pixel, a center-pixel outline, and the live color value. Place/relocate the overlay so the source sample region is visible, and avoid recursive self-capture. Check that layer-shell, output positioning, and composition are supported by the target KWin release; normal Wayland toplevel positioning is not sufficient.
3. **Selection and cancellation — existing KWin:** Once capture, cursor metadata, and overlay are ready, asynchronously call `org.kde.KWin` / `/ColorPicker` / `org.kde.kwin.ColorPicker` / `pick()` via `zbus`. KWin owns left-click confirmation and right-click/Escape cancellation (as supported by the running KWin), suppressing click-through to the underlying application. Decode the successful packed `QRgb` reply (Qt's `0xAARRGGBB` convention), treat it as the **authoritative final picked color**, and propagate it to the tray/history service. Map the cancelled D-Bus error to an ordinary cancellation without a history entry. Handle KWin's busy/error paths explicitly.
4. **Coordinate and timing reconciliation:** KWin selection and PipeWire cursor/video metadata are **asynchronous**. Track each output's geometry, scale, rotation, buffer dimensions, cursor hotspot, timestamps, and source identity. The magnifier's indicated pixel should match KWin's eventual result; confirm this on known pixel patterns. Do not claim pixel accuracy based solely on metadata arrival or a floating-point coordinate conversion: test source-buffer addressing, frame freshness, and KWin's 1-pixel render/sampling semantics under multi-monitor/fractional-scale configurations.
5. **Lifecycle:** Portal approval may display a focus-taking KDE permission dialog; this is the agreed exception. Do not invoke `pick()` until the capture session and overlay are ready. After success, cancellation, error, portal revocation, compositor reset, or monitor removal, release PipeWire streams, portal sessions, buffers, overlays, and KWin selection state to the extent exposed by the API. No screen capture exists while idle.

**Why this is the preferred design:** The shipped KWin color picker already integrates compositor-controlled selection and cancellation. KWin ScreenCast already emits cursor metadata, avoiding the need for a separate privileged pointer-reading mechanism. Rust only needs live pixels for rendering its custom magnifier and the D-Bus result for definitive color selection.

**Do not assume this composition is proven:** KWin might include the overlay in the captured stream, selection might behave differently with a layer surface present, metadata updates might lag, or capture/selection might have permission and lifecycle interactions. Verify the *combined behavior* before selecting this as the finished backend. A temporary KWin-managed permission dialog can take focus; an ordinary Colorboard/picker window cannot.

**Capture permissions:** Portal permissions and monitor/source selection are controlled by KDE. Use the restore-token/persistence mechanism where supported and user-authorized, handling rejected/expired/rotated restore tokens. Persistence is never guaranteed, and screen capture must not bypass KDE consent.

**Fallback if the primary integration fails a mandatory criterion:** First identify and reproduce the precise blocker (recursive capture, stale frame/coordinate mismatch, lost keyboard focus, click-through, permissions, unsupported compositor protocol). Check whether an alternative **supported Rust-side capture/overlay strategy** fixes that blocker while preserving the requirements. A separately packaged KWin QML/JavaScript effect may be considered for a **verified compositor rendering/input capability**, but it is *not* the required second step; documented QML APIs do not promise direct pixel reads, and `SceneEffect` changes scene-composition responsibility. Do not add a QML subsystem merely as an architectural experiment.

**Last resort — minimal native C++ KWin integration:** If supported Rust-facing interfaces cannot satisfy the live magnifier, non-recursive capture, click ownership, and preserved keyboard focus together, permit the **smallest KWin-native C++ effect/helper** providing only the compositor-dependent capability that is missing. Keep Rust as orchestrator and retain a documented, versioned result/cancel contract. Native KWin C++ effects are version/ABI-coupled; package/build/detect against the installed KWin version. A portal permission prompt alone is not justification for bypassing the portal or adding a native effect.

**Strict compositor integration boundary:** Any optional compositor extension handles only compositor-dependent pixels/rendering/input and result delivery. It does **not** own history, SQLite, config, shortcuts, clipboard, theme plugins, or the Python UI. Reuse one end-to-end acceptance suite across capture/selection backends.

### Wayland-specific event rules

- **Active picker:** retain keyboard focus on the previously active application. The magnifier must not activate a normal toplevel, consume keyboard input, or intercept KWin's selecting pointer click. KWin's own interactive picker owns selection/cancellation; do not independently synthesize or compete for pointer grabs.
- **Cursor tracking:** prefer ScreenCast `cursor_mode = 4` and `SPA_META_Cursor`; do **not** rely on overlay-local pointer motion for desktop-wide coordinates, since a pointer-transparent overlay should not receive those events. Validate that KWin supplies timely metadata for every captured output; handle off-output cursor status.
- **History popup:** may request ordinary keyboard focus to support Escape and close-on-focus-loss. On Wayland, do not assume direct positioning of a regular XDG toplevel; use an appropriately centered layer surface or another compositor-supported solution, and verify click-away dismissal.
- Never use `pynput`, `xdotool`, X11 screen scraping, or unsafe global event interception as a workaround.
- If exact accuracy, absence of recursive feedback, click interception, or focus preservation cannot be proved, **Phase 1A remains blocked**. No fake magnifier, frozen desktop, or KWin-only single-color fallback counts as completion.

## 5. Shared data and integration contracts

### Storage

Use standard XDG paths:

- `${XDG_CONFIG_HOME:-~/.config}/colorboard/config.toml`
- `${XDG_DATA_HOME:-~/.local/share}/colorboard/library.sqlite3`
- `${XDG_DATA_HOME:-~/.local/share}/colorboard/themes/`

Do not literally embed `~` as a path; resolve the correct home/XDG location. SQLite is the authoritative datastore shared by both applications. Use WAL mode, `busy_timeout`, foreign keys, short transactions, and transactional changes wherever supported. Preserve backups before destructive migrations.

**Single migration owner:** Rust is the **only** implementation permitted to create or migrate the shared SQLite schema. Put schema definitions, ordered migrations, and compatibility checks in `rust/common/storage/` and expose a small **one-shot `colorboard-db migrate` / `colorboard-db check` CLI** using that same Rust code. The CLI does not run in the background.

- The tray runs the Rust migration/check path at startup before accessing history. The standalone Python Board invokes the packaged `colorboard-db migrate` CLI on startup (e.g., through a bounded `QProcess`) before it opens the database. This supports launching the Board without a running tray service.
- After migration, **both** Rust and Python may read and write their documented tables in SQLite; *neither* may perform schema changes from Python. Document table ownership, columns, constraints, and schema version compatibility in `schemas/` and `docs/storage.md`.
- When a future Palette Editor or Theme Generator needs new tables, implement the migration in Rust **as part of that Python feature's phase**, then update Python queries/models and add cross-language integration tests. Do not create tables via SQLAlchemy or Python-side ad hoc DDL.
- On a newer, incompatible schema, unsupported downgrade, unavailable migration utility, or migration failure, show a clear error and leave data unmodified; never let the Python application silently reinitialize or overwrite the database.
- Acquire write locks only for short commits, handle `SQLITE_BUSY` explicitly, and publish D-Bus change signals after successful commits. Both applications can receive signals but must also refresh from the database when started or reconnected.

Canonical captured color: RGBA **8-bit sRGB** values, including alpha (normally opaque for screen samples). Store typed components or an unambiguous `#RRGGBBAA`; keep display/copy format independent of storage. Timestamps in UTC and unique IDs. Do not confuse color conversion with changing the original captured sample.

Recommended initial tables: `color_history`, `settings_meta`, `schema_migrations`; add `palettes`, `palette_swatches`, and `theme_projects` via **new Rust migrations during their respective later phases**. Every genuine successful pick may be stored, even if identical to a prior capture, unless a later explicit user choice changes duplicate policy. Copying an existing swatch must **not** add a new pick or reorder history by default.

### Clipboard

Implement `#RRGGBB`, `#RRGGBBAA`, `rgb()`, `rgba()`, HSL, HSV, and approximate CMYK formatters. Mark CMYK as an approximate device-independent transform unless profiles are supported. Additional formatter plugins are optional. Ensure the copied text remains usable after the transient picker process quits; the persistent tray service may own the clipboard selection.

### Session D-Bus (versioned contract)

Reserve a stable application identifier and document it. Initial conceptual operations:

- `PickColor() -> started/error`
- `ShowHistory() -> started/error`
- `OpenBoard() -> started/error`
- `GetHistory(limit) -> entries`
- `GetLastColor() -> color/none`
- `ReloadConfiguration()`

Signals:

- `ColorPicked(id, rgba, timestamp)`
- `ColorPickCancelled()`
- `HistoryChanged()`
- `ConfigurationChanged()`

Define the exact D-Bus names, signatures, ownership, errors, busy-state behavior, and lifecycle in `docs/ipc.md` **before** adding Python consumers. Emit completed/canceled events so the Python app can restore itself after a picker invocation. Only one picker session may run at a time. Concurrent requests must return a documented busy result or reuse the current session without spawning duplicates.

## 6. Development stages and blocking gates

### Phase 1A — Core Rust live picker (FIRST TASK, BLOCKING)

Deliver a runnable Rust command that starts KDE Wayland picking, displays a **real live, moving pixel-grid magnifier**, and returns the final selected color. **KWin's native color-selection method performs selection only**; Rust must render live magnified pixels concurrently. A build that merely invokes KWin's method does not meet this gate.

**Implementation order (all steps are required within Phase 1A):**

1. Confirm installed KDE Plasma/KWin, portal/backend, Wayland layer-shell, PipeWire, source output/scale handling, and current API signatures. Inspect KDE applet `grabwidget.cpp`, KWin `colorpicker.cpp`, KWin ScreenCast cursor-metadata code, and ScreenCast portal API.
2. Implement the Rust XDG ScreenCast session: capture all selected monitors; request `cursor_mode = 4`; open PipeWire remote; decode negotiated buffer formats and frame/cursor metadata. Provide a diagnostic readout of current output ID, frame size, pointer position, and pixel under pointer during development.
3. Implement robust coordinate mapping and pixel extraction for mixed monitor origin, fractional scale, buffer transforms/rotation, cursor hotspot, off-output pointer position, and asynchronous cursor/video updates. Unit-test with deterministic checkerboard/single-pixel fixtures.
4. Implement the Rust non-keyboard-focusable, pointer-transparent Wayland layer-shell magnifier. Show the current pixel neighborhood as a nearest-neighbor grid, highlight the exact center pixel, display live color value, and honor configurable sample radius and zoom. Reposition it near the pointer without obscuring the sampled point.
5. Integrate the existing KWin `org.kde.kwin.ColorPicker.pick` D-Bus method **only after capture and overlay are ready**. Have KWin own left-click confirmation and right-click cancellation; decode its color reply and cancellation/error responses correctly. Compare the authoritative KWin result against Rust's previously presented center sample for known pixel patterns and investigate any mismatches.
6. Validate overlay/capture coexistence: no magnifier feedback or recursive frames, no rendered cursor contaminating the sampled neighborhood, no click-through to the underlying application, no disruption of original keyboard focus (excluding KDE portal prompts), and no stuck KWin picking state or orphaned capture after cancellation.
7. Validate live updates from **both** pointer movement and underlying desktop content changes; test multi-monitor crossing, 100/125/150/200% scaling, output rotation, monitor disconnect, portal authorization/rejection/restoration, repeated picks, rapid cancellation, and compositor/portal restart behavior as applicable to the target system.
8. If any mandatory criterion fails, record an actionable reproducible blocker, try a supported Rust-side solution first, and permit a **narrow native KWin extension** only if compositor-native access is demonstrably required. A packaged QML effect is an optional targeted investigation, **not** the default or mandatory fallback. Do not mark this phase complete with reduced functionality.
9. Terminate all per-pick resources and return success/cancel/error through a stable structured result to the future tray process. Add automated coordinate/pixel tests plus an explicit real-Plasma manual acceptance checklist with recorded results.

**Phase 1A exit gate:** Demonstrably correct live magnification, exact source-pixel selection matching KWin, left-click confirm, right-click cancel, consistent no-click-through behavior, preservation of previous keyboard focus except permission dialogs, working multi-monitor/scaled output handling, and complete cleanup on success/cancel/error. Do **not** begin Phase 1B while any of these requirements are unverified or failing.

### Phase 1B — Rust tray service

- Persistent single-instance `ksni` tray item; pipette icon plus dynamic last-color dot, including high-DPI sizes.
- Left-click pick; context menu exactly as confirmed.
- D-Bus control endpoint, busy-state control, transient picker invocation, and robust error handling.
- No Python or screen capture resources loaded while idle.

**Acceptance:** tray is responsive and icon/menu behavior is correct while other applications have focus.

### Phase 1C — History and clipboard

- Typed config and the **Rust-owned** SQLite schema/migrations/WAL; include and test the standalone `colorboard-db migrate/check` command for subsequent use by the Python Board.
- Capture on successful selection, default retain 100.
- Output formatter registry and automatic clipboard copy in selected format.
- New-color notification to UI and tray icon update.
- Safe handling of stale or corrupt configuration/history.

**Acceptance:** history persists restarts; copied values match selected pixels; copied data remains available after transient picker exits.

### Phase 1D — Scrollable history popup

- Transient Rust window, centered on selected output, frameless, focusable.
- Scrollable grid with independent display/retention limits and configurable rows/columns.
- Clicking a swatch copies it, without recording a fresh pick.
- Escape, click-away, and repeated shortcut close the popup.
- Resource teardown when closed.

**Acceptance:** actual KDE Wayland positioning, dismissal and clipboard behavior work with multi-monitor layouts.

### Phase 1E — Configure, shortcuts, deployment, performance

- Transient settings UI for popup count/rows/columns, magnifier radius/zoom, output format, history cap, both hotkeys, auto-copy.
- GlobalShortcuts portal shortcut session and KDE-supported configuration UI. Verify permissions, collisions, updates, startup restoration, and revocation.
- Autostart or systemd user unit (one chosen startup mechanism; no duplicate launches).
- Package the Rust tray/picker/history helpers, desktop metadata, and icons. **Do not package or require a custom KWin effect in the preferred backend.** Only distribute an optional QML/C++ compositor component if a documented, tested blocker justified it; add version/ABI checks if the last-resort native C++ component is required.
- Benchmark idle USS/PSS/CPU and active picker resource usage; document test platform.

**Phase 1 release gate:** All picking, magnification, focus, history, tray, popup, shortcut, config, startup, and clipboard requirements function together. No Python implementation may begin before this gate.

### Phase 2 — Python/PySide6 app shell

- Separate Python process, launched by Open Board or independently.
- Three tabs: **Color Mixer, Palette Editor, Theme Generator**.
- Scrollable history swatch strip visible across all tabs.
- Invoke `colorboard-db migrate/check` before opening the shared SQLite database, including when the tray is not running; never perform Python-side DDL.
- SQLite-backed shared history and D-Bus event subscription, with startup/reconnection refresh and short write transactions.
- `Pick Color` action from the board; configurable auto-hide during a pick, restore on success OR cancellation, and handle picker failures without leaving the board hidden.
- Shared active palette state and navigation; tabs may be empty placeholders in this phase.
- Main process must exit fully on close; Rust tray remains.

### Phase 3 — Color Mixer tab (first functional tab)

- Interactive color editor with HEX, RGB(A), HSV/HSL, and displayed CMYK formats.
- Current-color swatch and scrollable historical swatches.
- Mix multiple colors with adjustable weights and defined interpolation space (sRGB / perceptual OKLCH as appropriate).
- Copy in selected format; send selected colors to the active palette; undo/redo.
- Test color round trips, clipping, alpha behavior, and clipboard outputs.

**Gate:** independently functional mixer and active-palette handoff.

### Phase 4 — Palette Editor tab (second functional tab)

- First add any required palette/project tables through **Rust storage migrations**; verify Python can create, read, and update rows while the Rust tray holds its ordinary connection.
- Generate configurable harmonious, contrasting, or random palettes: complementary, split-complementary, analogous, triadic, tetradic, monochromatic, contrast-targeted, fully random.
- User-selected swatch count, base colors, OKLCH lightness/chroma ranges, optional contrast target, optional random seed.
- Coolors-inspired wide swatch row; per-swatch color editing; locking, adding, deleting, reordering, regenerating unlocked swatches, drag/drop from history.
- Save/bookmark generated palettes; separate generated history from saved palette library; rename, duplicate, reopen, delete; undo/redo.
- Preserve locked swatches exactly on regenerate; explicitly handle sRGB gamut clipping.
- Make selected saved/generated palette available to Theme Generator.

**Gate:** save, reopen, regenerate, and handoff work with no data loss.

### Phase 5 — Theme Generator tab (third functional tab)

- Before writing UI or Python models, add any theme-project tables via **Rust-owned migrations** and validate the cross-language schema version.
- Select a theme target plugin and active source palette.
- Render a **mock UI**, not the target application's real process, with selectable semantic elements; preview menus/dialogs/alternate states when clicked.
- Display a role/element list with swatches, chooser, direct editable value (default HEX).
- Apply colors from active palette; drag history colors onto roles or preview elements; support explicit per-role overrides, role locks, undo/redo.
- Clearly distinguish linked palette roles from detached overrides; changes should propagate predictably.
- Save/reopen theme projects and validate exports.

#### Primary design: shared, declarative WYSIWYG renderer

**Do not have every plugin create its own Qt window, scene, event handlers, or drag/drop implementation.** Build **one** `QGraphicsView` / `QGraphicsScene` preview renderer and reusable scene-item library in the Board. Plugins principally supply *data* describing an application's interface and theme mappings. The generic renderer handles:

- Draw ordering, geometry, clipping, text/shape/icon primitives, scaling, hit testing, selection highlights, and common widgets (tabs, panels, rows, menus, dialogs, editor text, terminal text).
- Mapping visible and nested elements to **stable semantic role IDs** (e.g., `editor.background`, `terminal.foreground`), separate from which colors are assigned to those roles.
- Clicking/selecting a preview element and synchronizing the inspector's role selection; editing through the inspector immediately updates all bound elements.
- **Declarative scene states and transitions** (e.g., `editor` → `open_settings_menu`; sidebar expanded/collapsed; tab switched) so users can reveal normally hidden elements without launching the real application.
- Drag/drop from history or palette onto compatible preview targets, keyboard/accessibility fallbacks, undo/redo, and consistent per-role overrides.
- Using swatches from the source palette through explicit role-to-palette references; locked/overridden roles are represented in **saved theme-project data**, not hard-coded into a preview implementation.

Define and version declarative JSON schemas in `schemas/`: `theme-plugin-manifest`, `theme-roles`, `preview-scenes`, and `theme-project`. Suggested shape: manifest (plugin ID, format/schema versions, compatible app versions, declared assets/templates) + roles (IDs, labels, color types, defaults, validation) + scenes (scene IDs, nodes, bounds, role bindings, click actions, transition targets). Include deterministic rendering order and validation of references/IDs.

**Python plugin code is an escape hatch**, used only for application-specific import/parsing, sophisticated validation, unusual preview geometry/interactions, or export helpers that cannot be described declaratively. Define a narrow, documented adapter interface for these behaviors; plugins must not own the application's event loop or arbitrary cross-tab UI. New theme targets should be possible using only manifest + role/scene data + templates when their requirements fit the generic renderer.

#### External theme-plugin installation, discovery, and export contract (MANDATORY)

**A new compatible theme target must be installable by copying a plugin package into a documented plugin directory, without recompiling, rebuilding, modifying, or reinstalling any Colorboard executable or source tree.** The Board must discover and load it at runtime. The core renderer must not contain hardcoded application identities, theme roles, UI layouts, or per-plugin source imports for Ghostty/Zed/KDE or later targets. Bundled plugins must use **the exact same public plugin format and loader** as third-party plugins.

**Install paths and discovery** (implement with XDG directory conventions):

- User plugins: `${XDG_DATA_HOME:-~/.local/share}/colorboard/plugins/<plugin-id>/`.
- System plugins: `${XDG_DATA_DIRS}` entries, e.g. `/usr/local/share/colorboard/plugins/<plugin-id>/` or `/usr/share/colorboard/plugins/<plugin-id>/`.
- Built-in plugins: shipped as ordinary packages in an installed application data directory, discovered using the same loader (development repo may include fixture/source copies).
- The plugin manager shows discovered/enabled/disabled/incompatible plugins, validation errors, API-version compatibility, and installation source. Provide a **Reload Plugins** action that scans plugin directories without recompilation or reinstall. Newly installed declarative plugins should appear immediately after reloading; ensure a safe transition when a selected plugin is replaced or removed.

**Language-neutral plugin bundle format (versioned)** — directory example:

```text
plugins/example-terminal/
├── manifest.json          # unique ID, plugin API version, target/version metadata
├── roles.json             # semantic color roles, defaults, constraints
├── scenes.json            # declarative preview elements and UI transitions
├── templates/
│   └── theme.j2           # Jinja2 output template(s)
├── assets/                # optional icons, fonts, sample text or mock images
├── fixtures/              # expected generated output for golden tests
└── hooks.py               # OPTIONAL trusted Python behavior, never required for ordinary themes
```

- A **data-only plugin** must support role editing, clickable/interactive preview states, palette assignment, drag-and-drop, theme validation through declarative rules, and output-file export using templates; it must **not** require compiled code or a Python module.
- Define a versioned plugin ABI/API at the **data-contract** level. Validate the manifest and referenced scene/role/template files against published JSON Schemas, reject malformed or incompatible plugins gracefully, and provide clear user-facing diagnostics. Use deterministic plugin-ID conflict resolution (or reject duplicate IDs with an actionable error); never silently shadow user data.
- Optional Python hooks are loaded only for explicitly trusted and enabled plugins, through a constrained documented adapter interface, and **execute with the user's permissions**. Never auto-install dependencies or auto-execute downloaded code. A declarative plugin should not run executable code beyond normal template rendering.
- The declarative manifest, roles, scenes, and templates must be **portable across future Python, Rust, or C++ host implementations**; keep language-specific hooks isolated so changing the implementation language does not invalidate data-only plugins. Jinja2 is the authoritative v1 template dialect; alternative hosts must implement a tested compatible subset or retain a separate exporter bridge. Do not claim byte-for-byte full Jinja2 parity in another engine without tests.
- Plugin installation is independent of the Rust picker; opening the picker must not initialize the plugin subsystem. The Theme Board only initializes the chosen plugin's preview/assets, and should list unused plugins using manifest metadata without importing code or instantiating scenes.

Use **trusted Python hooks** initially only when required. The supported v1 default authoring contract is **declarative data + Jinja2 templates, with optional Python hooks**. Do not embed Lua in v1; reconsider additional scripting engines only for validated requirements.
- Export pipeline: load plugin metadata → resolve palette links and overrides → normalize and validate semantic roles → format target-native color values → render Jinja2 template(s) with strict undefined-variable handling → validate the rendered output where practical → write requested export file(s). Keep validation and escaping separate from templates.
- Use a per-plugin **role adapter**, not direct Qt widget color values, as the input to exports. A plugin must declare supported application/config schema versions, output paths/extensions, template assets, and test fixtures. Do not infer arbitrary filesystem destinations from untrusted template values.
- Use reproducible golden-file export tests and data-driven scene tests (select/click/transition/drop), including an **external fixture plugin** installed after the Board is built, discovered via Reload Plugins, previewed, edited, and exported without source edits or recompilation. Add screenshot-based regression tests for representative mock states where feasible. Importers are optional if the target cannot be reliably round-tripped; do not claim lossless import/export otherwise.
- Load only the selected plugin's optional Python modules/assets and preview state on demand. Do not preinitialize all plugins or load them in Rust/tray. Plugins are treated as **trusted executable code**; never automatically execute third-party downloaded plugins.

Implement plugins **in order**:

1. **Ghostty**: terminal background/foreground, ANSI 0–15, cursor, selection; native theme/config text export. Use this first plugin to test that **the generic renderer and declarative state transitions work without custom Qt scene code**.
2. **Zed**: editor/tabs/sidebar/panels, syntax roles, dialogs, terminal colors; versioned JSON theme export. Exercise multiple scenes and nested menus.
3. **KDE `.colors`**: window/view/button/selection/tooltip/disabled/inactive color sets; representative native-looking widget mockup; INI-compatible export. Add application-specific adapters only where required by KDE semantics.

Use Jinja2 for text templates, but separate role validation and escaping/format conversion from template presentation. Golden-file tests must cover each export target; retain unknown imported keys when round-tripping where feasible. Never apply generated themes to the live desktop automatically.

**Phase 5 gate:** A newly created **external test plugin** can be installed into the user plugin directory, discovered with Reload Plugins, shown in the target selector, previewed, edited, and exported **without modifying/rebuilding/reinstalling Colorboard**. Ghostty's preview is rendered through the common data-driven system; at least one hidden state opens through a declarative click transition; inspector, drag/drop, override/link, saved project, and exported text all agree. Zed and KDE may follow as independent deliverables without modifying the generic editor for ordinary new roles or scenes.

### Phase 6 — Release hardening

- Accessibility and keyboard navigation, exceptions documented for non-focusable picker.
- Real KDE Wayland integration tests, monitor disconnects, portal revocation, sleep/resume, shortcut conflicts, KWin updates.
- Packaging for Arch/Fedora/openSUSE as applicable; document C++ KWin extension version coupling if used.
- Performance regression checks for idle resident process and transient memory cleanup.
- User guide, **external plugin installation/author guide and versioned plugin schema**, D-Bus API guide, installation and troubleshooting.

## 7. Repository layout

```text
colorboard/
├── Cargo.toml
├── rust/
│   ├── tray/
│   ├── picker/
│   ├── history-popup/
│   ├── db-cli/              # one-shot Rust migration/check command
│   └── common/
│       ├── colors/
│       ├── config/
│       ├── ipc/
│       └── storage/
├── kwin/                     # ONLY if mandatory tests justify a custom KWin component
│   ├── effect-qml/           # optional investigated QML effect; not a default dependency
│   └── native-effect/        # last-resort version-coupled C++ compositor integration
├── board/
│   ├── pyproject.toml
│   └── colorboard/
│       ├── app.py
│       ├── ipc/
│       ├── storage/
│       ├── widgets/
│       ├── mixer/
│       ├── palettes/
│       └── themes/
│           ├── api.py
│           ├── renderer/     # shared QGraphicsScene + reusable nodes
│           ├── schemas/      # validated data objects / loaders
│           └── loader.py       # external manifest validation, discovery, reload
├── plugins/                 # distributable example/bundled plugin source packages
│   ├── ghostty/
│   ├── zed/
│   ├── kde/
│   └── example-terminal/
├── schemas/
│   ├── theme-plugin-manifest.schema.json
│   ├── theme-roles.schema.json
│   ├── preview-scenes.schema.json
│   └── theme-project.schema.json
├── assets/
├── packaging/
├── tests/
└── docs/
    ├── architecture.md
    ├── requirements.md
    ├── ipc.md
    ├── storage.md
    ├── theme-plugin-api.md
    ├── decisions.md
    ├── roadmap.md
    └── testing.md
```

## 8. Engineering and agent operating rules

- Work **only on the current phase**. Do not start the Python UI while the Rust picker gate is incomplete.
- A successful build or unit test does not prove KDE/Wayland integration. Mark untested behavior explicitly; do not claim desktop validation when no graphical KDE session was used.
- Before adding a new framework, document why the standard Rust/Qt stack cannot meet the requirement.
- Keep the exact user behavior above in acceptance tests, not merely in prose.
- Use Rustfmt/Clippy and cargo tests, Ruff/pytest/type checking for Python, and automated export golden tests.
- Define one canonical color representation, a versioned IPC/schema contract across Rust and Python, and stable semantic role IDs for theme targets.
- Keep Rust as the **single schema migration owner**; test the one-shot migration CLI from both Rust and standalone Python start paths.
- Make **external, installable, data-only theme plugins** first-class. Test install/validate/discover/reload/preview/export of a new fixture plugin without modifying or rebuilding the Board. Avoid custom per-plugin Qt interfaces; keep optional Python hooks narrowly scoped.
- **Selected Phase 1A backend:** live XDG ScreenCast/PipeWire monitor frames + `SPA_META_Cursor` metadata + Rust layer-shell magnifier + asynchronous existing KWin `org.kde.kwin.ColorPicker.pick` selection/cancellation. Test all components concurrently. Do not implement custom KWin QML effects by default.
- If the preferred backend misses a mandatory criterion, document the specific limitation and attempt a supported Rust-side fix. Consider a KWin QML effect only if an actually documented API addresses the failure; introduce a minimal version-checked native C++ KWin integration only for capabilities unavailable through the supported interfaces. Never silently change to a screenshot-only or single-pixel-only picker.
- Keep the Python editor strictly on demand, with no resident Python process or interpreter loaded in the Rust tray service.
- Avoid constant polling or repeated frame capture while idle.
- Keep dependencies pinned by lockfiles; record verified Plasma/KWin/Qt/PipeWire versions during testing.
- Do not invent platform APIs. Consult the current API docs and annotate any private interfaces and their compatibility limits.
- Report changes, validation, blockers, and the next task at each phase handoff.

## 9. Ratified architecture decisions (for agent review)

| Decision | Selected approach | Rationale / revisit rule |
|---|---|---|
| Main language split | Rust picker/tray + Python/PySide6 Board | Qt's mature editing primitives speed up WYSIWYG construction; the editor is not resident, so Python does not contribute to idle tray memory. Reassess all-Rust only at the Phase 1 → 2 gate if warranted. |
| Theme preview authoring | **Externally installable, versioned declarative plugins** with a shared Qt renderer; optional trusted Python hooks | A new theme target must work by placing a plugin folder in an XDG plugin path and clicking Reload Plugins, without rebuilding/reinstalling Colorboard. Data-only bundles remain portable to Rust/C++ implementations. |
| Schema evolution | Rust migrations only; one-shot CLI usable by standalone Board | Prevent two languages from competing to mutate the same SQLite schema. |
| Picker/selection backend | **Rust ScreenCast (PipeWire video + `SPA_META_Cursor`) and Rust layer-shell magnifier, coordinated with existing KWin `ColorPicker.pick` via D-Bus** | KWin already owns desktop-wide selection/cancellation; Rust supplies the missing live zoomed pixels. Individual APIs are verified; simultaneous operation must pass Phase 1A acceptance tests. |
| Optional KWin extension | **None by default**; targeted QML effect only when demonstrably useful; minimal native C++ KWin extension only as last resort | The public scripted KWin API does not offer documented arbitrary framebuffer reads; native KWin C++ effects are ABI/version-coupled. Do not increase compositor coupling speculatively. |
| Python lifetime | Fully transient; exits on close | Keep resident memory and startup overhead isolated to Rust. |

Any change to these decisions requires recording the technical reason and updating the acceptance/integration tests. Decisions must **not** weaken the mandatory live magnifier or focus requirements.

## 10. Immediate copy/paste agent kickoff instruction

> Implement **Phase 1A** of `docs/implementation-plan.md` (or this plan if not yet committed), targeting KDE Plasma 6 Wayland. Build a **real, live Rust pixel magnifier** using the XDG ScreenCast portal, PipeWire monitor video, and `SPA_META_Cursor` (`cursor_mode = 4`) for screen-wide pointer position. Render the nearest-neighbor center-pixel grid in a non-keyboard-focusable, pointer-transparent Rust Wayland layer-shell overlay. **After capture/overlay are ready, asynchronously call the existing KWin `org.kde.kwin.ColorPicker.pick` D-Bus method** to handle left-click selection, cancellation, and the authoritative picked color. The single-color KWin method never substitutes for live magnification. Check KWin click capture, focus preservation, absence of capture-overlay recursion, pixel accuracy with asynchronous cursor/video frames, mixed-DPI and multi-monitor behavior, portal permissions, and cleanup. A custom KWin QML/JS effect is **not** a default dependency or a required intermediate step. If supported Rust interfaces cannot meet a hard requirement, document the blocker and only then consider the smallest necessary version-coupled native KWin extension. Do not start the tray, Python UI, or theme editor until a **real Plasma session** demonstrates all Phase 1A acceptance criteria. Document versions, tests, limitations, and outstanding blockers; never report Phase 1A complete based solely on mocks or a simple color picker without its magnifier.
