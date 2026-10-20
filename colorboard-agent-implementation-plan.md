# Colorboard — Agent Implementation Plan

**Status:** Requirements confirmed, 2026-10-20; external plugin contract and KWin QML fallback clarified (revision 1.2).  
**Working name:** Colorboard (renameable without changing interfaces).  
**Initial platform:** KDE Plasma 6 **Wayland only**.  
**Languages:** Rust for the resident picker and its transient UI; Python/PySide6 for the main board; a **packaged KWin QML/JavaScript effect may be used without C++** where its supported APIs meet the mandatory picker requirements, with a **minimal C++ KWin component permitted only if necessary**. Theme plugins are external data packages by default, independent of the host language.  
**Architecture decision:** Keep Rust + Python for implementation. An all-Rust editor remains a future option, not a current work item; revisit editor-toolkit choice only at the Phase 1 → Phase 2 gate without delaying or rewriting the Rust picker.

## 1. Product and non-negotiable requirements

Build two independently runnable applications that share colors, palettes, configuration conventions, and an IPC contract:

1. **Color Picker:** a low-memory Rust system-tray application that picks pixels anywhere on the desktop through a **live, pixel-accurate magnifying glass**, maintains color history, provides a centered history popup, and exposes keyboard shortcuts.
2. **Colorboard:** an on-demand Python/PySide6 GUI with three tabs, built **one at a time**: Color Mixer, Palette Editor, Theme Generator.

### Hard constraints

- The live magnifier is **required for Phase 1 completion**. Do not replace it with KDE's single-pixel `org.kde.kwin.ColorPicker.pick` operation or a frozen screenshot.
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
- <https://develop.kde.org/docs/plasma/kwineffect/> (declarative KWin QML effects)
- <https://api.kde.org/org-kde-kwin-qmlmodule.html> (published KWin QML types)

The applet's `GrabWidget::pick()` calls KWin's D-Bus method `org.kde.kwin.ColorPicker.pick` at `/ColorPicker`. KWin performs the interactive selection and returns a `QColor` (serialized as a D-Bus structure containing a packed RGBA integer). **That public operation returns one color, not a live pixel neighborhood**. It is useful as a behavioral reference or validation oracle but **does not independently implement the required magnifier**.

KWin declarative QML/JavaScript effects can be packaged and installed independently of a C++ binary; KWin hosts them and they may be used alongside an application written in Rust. **However, the documented KWin QML effect API does not expose a general-purpose live framebuffer pixel-read operation.** Treat QML effects as a potential compositor-hosted overlay/input integration, **not** an assumed substitute for obtaining underlying pixel data. Any Rust + QML + portal composition must prove source-frame transport, positioning, non-recursive sampling, exact color return, and focus behavior on real KWin before adoption.

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
         |                      Wayland overlay + capture + magnifier
         |                      (optional packaged KWin QML effect;
         |                       tiny KWin C++ integration only if required)
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
- `ashpd`: XDG Desktop Portal calls for GlobalShortcuts and ScreenCast.
- `wayland-client`, `wayland-protocols-wlr`, or suitable Smithay client toolkit: Wayland surfaces and pointer handling.
- PipeWire bindings or `pipewire-rs` with an explicitly versioned capture/format pipeline.
- `rusqlite`: SQLite storage; WAL and migrations.
- `serde`, `toml`: typed configuration.

Check actual crate versions and API signatures when coding, rather than copying unverified older examples.

### Capture/overlay implementation strategy

**Primary path:** Request monitor capture via XDG ScreenCast portal, consume PipeWire streams, and render a non-keyboard-activating Wayland magnifier/selection surface using KDE-supported layer-shell behavior. Request **hidden cursor** in captured frames if advertised, track pointer through the overlay's supported input routing, map logical coordinates to physical pixels, and explicitly test whether the overlay appears recursively in screen capture.

The ScreenCast portal's capture permissions and monitor selection are controlled by KDE. If supported, use a user-approved restore token to avoid repeating source selection; never assume persistence or silently bypass consent. Portal prompts are allowed to receive focus. Capture resources must be released after each pick.

**Second path — packaged KWin QML/JavaScript effect (no C++ required if sufficient):** If the primary portal/overlay path does not meet all mandatory behavior, evaluate a separately installable Plasma 6 declarative `KWin/Effect` package (`metadata.json`, `contents/ui/main.qml`). It can run alongside a Rust application without compiling a native KWin plugin. Determine which parts it can provide: compositor-hosted overlay, pointer input and focus behavior, and a supported way for Rust to initiate/cancel sessions and receive results. Document and test the **actual** activation/IPC interface; do not invent an incoming QML D-Bus API. The published QML effects API does **not** document arbitrary pixel-read access, so any required screen data must still come from a verified source, potentially Rust's authorized PipeWire capture. Do not call the QML effect path a successful magnifier unless source pixels, input and coordinates are correct end-to-end.

**Last-resort native path — minimal C++ KWin integration:** If neither Rust/portal nor Rust plus a supported KWin declarative effect can satisfy live sampling, non-recursive magnification, click interception, and preserved keyboard focus, permit the **smallest practical KWin-native C++ component**, after documenting what exact capability is missing in the previous approaches. Portal permission prompts are not by themselves a reason to bypass user consent or introduce native code.

**Strict compositor-component boundary:** Either the QML effect or C++ component may perform **only compositor-dependent overlay, sampling/capture (if actually supported), pointer selection, and result/cancel delivery**. Expose a narrow, documented, versioned contract to Rust and keep the tray, shortcut management, clipboard, SQLite, configuration, main app and theme plugins out of KWin. The Rust picker is the orchestrator; all backends must satisfy the same end-to-end acceptance suite.

Packaged declarative effects avoid the native KWin C++ ABI problem but still require API compatibility and user installation/enabling. Native C++ KWin effects have no stable cross-release ABI guarantee. If C++ is truly needed, detect KWin version/ABI compatibility at build/install/startup and fail clearly on mismatch; document the rebuild/package strategy. Never mark success based only on a visual demonstration that fails exact pixel selection or focus preservation.

### Wayland-specific event rules

- Picker: request **no keyboard focus** for its visual surface. Test that pointer events arrive and that the original app retains keyboard focus. Do not promise Escape support if doing so would require keyboard capture; right-click cancellation is required.
- History popup: may request ordinary keyboard focus to support Escape and close-on-focus-loss. On Wayland, do not assume direct positioning of a regular XDG toplevel; use an appropriately centered layer surface or another compositor-supported solution, and verify click-away dismissal.
- Do not use `pynput`, `xdotool`, X11 screen-scraping, or unsafe system-wide event interception as a Wayland workaround.
- If a core behavior requires compositor integration, evaluate the packaged declarative KWin QML effect before the approved narrow C++ exception. Neither fallback may weaken magnification, input, color fidelity, or focus requirements.

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

Deliver a runnable Rust command that activates live picking under a real KDE Plasma 6 Wayland session and returns the chosen color. Implement **the real magnifier**, not a diagram, stub, or frozen screenshot.

Work required:

1. Inspect the verified KDE applet and KWin effect sources.
2. Establish the actual target compositor/portal capabilities and capture path as part of implementing the feature.
3. Implement screen capture, pointer sampling, correctly scaled per-monitor coordinates, and magnifier drawing.
4. Implement left-click confirmation/right-click cancellation while maintaining keyboard focus on the previously active application.
5. Resolve overlay recursion, cursor contamination, and mixed DPI/rotation handling.
6. If the portal/layer-shell route cannot meet ALL requirements, evaluate the packaged declarative KWin QML/JavaScript effect and its Rust integration; use the permitted minimal C++ component only when a missing compositor-native capability requires it. Do not substitute a weaker picker.
7. Shut down frames, effects, and overlay upon pick completion/cancellation.
8. Add unit tests for pixels/format conversion/coordinate mapping and real-session manual tests.

**Acceptance:** live 15×15 grid (configurable), crisp 10× sample zoom (configurable), center-pixel accuracy on checkerboard images, animated-content updates, correct multi-monitor/fractional-scale sampling, no unexpected focus changes, no click-through, clean cancellation, clean resource teardown. Test 100%, 125%, 150%, 200% scaling where hardware/config permits. **Do not move on until this works.**

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
- Package helper binaries, desktop metadata, icons, and any separately installable KWin QML effect; add version/ABI checks if the last-resort C++ KWin extension is required.
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
├── kwin/                     # optional packaged QML effect and/or last-resort native component
│   └── effect-qml/           # KWin/Effect package with metadata.json + contents/ui/main.qml
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
- For missing Wayland functionality, test a packaged KWin QML/JS effect with Rust orchestration before introducing native C++. The public scripted API does not promise direct pixel reads. If absolutely necessary, isolate a minimal version-checked C++ KWin effect behind the same Rust-facing contract.
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
| KWin component | Rust portal/Wayland first; packaged KWin QML/JS effect second; minimal C++ effect only as last resort | QML effects are compatible with a Rust application but do not document arbitrary framebuffer pixel reads. Require end-to-end demonstration before selecting a backend. |
| Python lifetime | Fully transient; exits on close | Keep resident memory and startup overhead isolated to Rust. |

Any change to these decisions requires recording the technical reason and updating the acceptance/integration tests. Decisions must **not** weaken the mandatory live magnifier or focus requirements.

## 10. Immediate copy/paste agent kickoff instruction

> Implement **Phase 1A** of `docs/implementation-plan.md` (or this plan if not yet committed). Target KDE Plasma 6 Wayland. The deliverable is a **working Rust live pixel magnifier and picker**, not an architecture-only proof-of-concept. Live capture, cursor-following magnification, exact center pixel selection, left-click confirmation, right-click cancellation, and preservation of the previously active application's keyboard focus are mandatory. Investigate and implement the XDG ScreenCast/PipeWire + Wayland overlay approach. If that approach cannot meet all constraints, evaluate an independently installable KWin QML/JavaScript effect working with Rust; do not assume the QML API reads framebuffer pixels. Only if neither approach meets the requirements, implement the expressly permitted minimal KWin-native C++ component without replacing the Rust program. Add coordinate/pixel tests and validate the actual desktop UX on a KDE Wayland session. Do not begin tray, Python, or theme-generator work. Report what was implemented, precise test results, any version-dependent interfaces, and remaining Phase 1A blockers. Do not mark Phase 1A complete until the live magnifier meets every acceptance criterion.
