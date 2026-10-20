# Colorboard — Rust-Only Agent Implementation Plan

**Revision:** 2.0 — 2026-10-20  
**Status:** Requirements ratified; implementation not yet validated on a live Plasma session  
**Project name:** Colorboard (working name; identifiers are placeholders until finalized)  
**Initial target:** KDE Plasma 6 on **Wayland only**  
**Application implementation language:** **Rust exclusively**  
**Primary GUI:** `egui` / `eframe`, started on demand  
**Theme plugins:** Externally installable declarative packages with **MiniJinja** templates; optional tightly limited Rhai scripts only when justified  
**Required development sequence:** Complete the Rust magnifying picker and tray utility before starting the Board; finish each Board tab in order.

> **Agent instruction:** This document supersedes the Rust + Python + possible C++ revision 1.3. Do not create Python, PySide6, C++, Qt bridge, KWin C++ plugins, or custom QML/JavaScript application code. Using existing KDE/KWin/portal services implemented by the operating system is allowed. No feature, especially the live magnifier, may be omitted to preserve the single-language constraint. If a mandatory behavior cannot be implemented with supported Rust-accessible APIs, document the blocker and stop at its release gate; do not silently reduce the requirement or change languages.

---

## 1. Product scope and immovable requirements

Build **one Rust Cargo workspace with separate, independently runnable Rust processes**:

1. **Colorboard Picker:** A lightweight, always-on system-tray service orchestrating an on-demand **live, pixel-accurate magnifier**, color picking, copied color formats, history, centered history popup, global shortcuts, and configuration.
2. **Colorboard Board:** An on-demand Rust desktop application with **Color Mixer**, **Palette Editor**, and **Theme Generator** tabs, plus a shared color-history strip. Its full GUI must not remain resident when the Board is closed.

The processes share Rust crates for models, color science, configuration, storage, and IPC. A stable D-Bus interface is retained for independent launching and future automation, although the applications share a language.

### Non-negotiable acceptance conditions

- **Live magnification is mandatory from the first development milestone.** Magnified source pixels update with pointer motion **and** underlying desktop changes. A frozen screenshot, ordinary KDE single-pixel picker without magnification, or fake preview is not acceptable.
- The selection crosshair marks **the exact physical pixel** that will be selected. The final selected color must agree with KWin's returned color on known test imagery.
- Invoking the picker must not focus/raise a Colorboard window or steal keyboard focus from the previously focused application. A KDE-controlled screen-capture permission dialog is the **only** accepted exception during picking.
- KWin must handle left-click selection and right-click/Escape cancellation, or another supported Rust-accessible selection path must demonstrate equivalent behavior with no click-through to underlying apps. A picker overlay must not intercept KWin's selection pointer input.
- KDE Plasma 6 Wayland is the target; X11 and other compositors are out of scope for the first release.
- No screen capture, plugin engine, or main GUI runtime is left running when only the tray is idle.
- **Theme plugin packages must install, reload, preview, edit, and export without recompiling, modifying, or reinstalling Colorboard.** A standard plugin needs no executable plugin code.
- Preserve a low resident memory footprint. Measure actual USS/PSS and idle CPU; do not declare invented memory budgets as achieved benchmarks.
- Any unmet mandatory criterion blocks the relevant phase and subsequent phases, rather than being relabeled an optional enhancement.

## 2. Confirmed UX behavior

### 2.1 System tray

- Icon: pipette with a small indicator dot colored to match the **most recently picked color**; use a neutral dot with empty history.
- **Left click:** immediately initiate screen picking.
- **Right click:** show the menu in this order:
  1. Pick Color
  2. Show Palette
  3. Open Board
  4. Configure
  5. Quit
- Dynamically update icon/tooltip after a successful pick; ensure light/dark KDE tray contrast and high-DPI sizes.
- Quit explicitly terminates the persistent tray service; closing other windows does not.

### 2.2 Live screen picker

- Invocable via left click, context menu, system-wide shortcut, or Board action.
- Show a magnifying lens following the pointer, rendering a configurable nearest-neighbor pixel grid, contrasting center-pixel outline, and live color value.
- Use actual sampled desktop pixels; support multiple monitors, negative coordinates, fractional scaling, output transform/rotation, and moving content.
- Left click confirms; right click cancels; Escape cancels through KWin where available without the picker taking keyboard focus.
- On success, update history, tray dot, clipboard, and D-Bus subscribers. On cancel/error, preserve history and clipboard and remove transient resources.
- No normal window raises over the current application. KDE portal prompts are allowed to temporarily receive focus.

### 2.3 Centered color-history popup

- Frameless, centered **on the output containing the pointer at invocation** (fallback: primary output if pointer cannot be determined).
- Scrollable swatch grid with configurable display limit, visible rows, and columns, separately from retained-history count.
- Clicking a swatch copies its value in the configured format and briefly acknowledges success; does **not** create a new capture or reorder original capture timestamps.
- Keyboard navigation and swatch labels/tooltips where feasible.
- Closes on Escape, click-away, or repeat activation. Unlike the picker, the history popup **may temporarily receive focus**.
- Wayland cannot freely position ordinary top-level windows. Use a compositor-supported layer-shell/xdg-popup strategy that **actually verifies** centered placement and dismissal on KWin; don't claim unsupported window positioning works.

### 2.4 Default settings

| Setting | Initial default |
|---|---|
| Retained history | 100 captures |
| History-popup display limit | 24 swatches |
| Visible history grid | 4 columns × 3 rows |
| Magnifier sample radius | 7 source pixels (15 × 15 sample) |
| Magnifier zoom | 10× nearest-neighbor |
| Clipboard format | Uppercase `#RRGGBB` |
| Auto-copy after picking | Enabled |
| Pick shortcut | Suggested `Meta+Shift+C`, configurable and subject to desktop approval |
| History shortcut | Suggested `Meta+Shift+H`, configurable and subject to desktop approval |
| Main Board hide while picking | Configurable; default off until UX verified |

Configure must allow editing the history limit, display count, visible rows and columns, magnifier radius, zoom level, clipboard format, and both shortcuts. Reject conflicting/invalid settings with clear feedback and apply changes without restarting the tray where possible.

---

## 3. Technology choices and boundaries

### 3.1 Single-language approach

Use **Rust for all application source**: persistent service, screen picker, history popup, configuration utility, core libraries, Board GUI, theme scene renderer, plugin loader, and export engine.

Do not use:

- Python runtime/PySide6/Jinja2 Python package;
- authored C++ components, C++/Qt bindings, or native KWin effects;
- authored QML/JavaScript KWin effects;
- Electron, Chromium, WebEngine, or a web server to present desktop UI;
- arbitrary native Rust dynamic libraries as the externally installable theme-plugin contract (no stable Rust ABI).

Using system libraries and existing desktop services (PipeWire, SQLite, KWin, xdg-desktop-portal) through Rust bindings is **not** a violation of this decision. Theme files may be JSON/TOML/INI, templates, styles, and assets. Optional sandboxed/limited plugin *scripts* can be interpreted by a Rust runtime without adding Python/C++ application components.

### 3.2 Technology stack

| Area | Preferred implementation | Constraint |
|---|---|---|
| Persistent tray | Rust + `ksni` | StatusNotifierItem without loading Board GUI |
| Session IPC, KWin selection | Rust `zbus` | Async calls, structured errors/signals |
| Portal integration | `ashpd` | ScreenCast and GlobalShortcuts, verify installed versions |
| Capture | PipeWire Rust bindings and SPA | Video buffers + `SPA_META_Cursor` |
| Picker overlay / popup | `wayland-client`, layer-shell protocol, Smithay client toolkit where suitable | Explicit Wayland input/focus semantics |
| Lightweight transient drawing | `tiny-skia` / a small Rust rasterization path | No full Board renderer in the tray |
| Board GUI | **`egui` + `eframe`** | Initialize only on Board or Configure launch |
| Declarative preview renderer | `egui::Painter` + Rust hit-testing/scene models | No per-plugin application code needed |
| Shared color calculations | `palette` and explicit formatting code | OKLCH, HSL, HSV, contrast |
| Storage | `rusqlite` with bundled/system SQLite decision documented | WAL, migrations, short transactions |
| Configuration / plugin manifests | `serde`, `toml`, `serde_json`, JSON Schema validation | Versioned and typed |
| Theme file exports | **`minijinja`** | Jinja-style syntax, documented supported subset |
| Optional advanced plugin hooks | **`rhai`** (not required for normal plugins) | Disabled unless explicitly trusted; limited API and resource bounds |
| Build and tests | Cargo workspace, rustfmt, Clippy, `cargo test` | Single `Cargo.lock` |

**Why `egui` for the Board:** The application needs a dynamically constructed, selectable WYSIWYG scene rather than a fixed set of screens. `egui` provides custom painting, click/hover regions, drag-and-drop interaction, sliders, swatches, tabs, and undo/redo-friendly state management in Rust. The preview is a faithful *mock* of each application, not live native widgets. `egui` will not automatically inherit KDE's native widget appearance; use a carefully designed KDE-friendly theme. If visual fidelity demands a switch to Slint, record an explicit architectural decision **before** writing the Board, not a mixture of GUI engines in the resident service.

**Why MiniJinja:** It runs in Rust, accepts Serde data, and supports much of Jinja/Jinja2 syntax. It is not guaranteed to implement every Python Jinja2 extension/filter. Define the supported template dialect as part of the plugin API; add custom filters only centrally and cover syntax through golden tests. Standard theme bundles must not depend on Python Jinja2.

Crate names are **candidates, not promised API compatibility**. During each phase, pin working crate versions and confirm their current interfaces, KDE environment support, and licenses.

### 3.3 Processes and memory architecture

```text
Plasma tray
   |
   v
colorboard-tray [RUST, ALWAYS RESIDENT]
  ksni + zbus + configuration cache + history service + clipboard ownership
   |
   +--- on demand ---> colorboard-picker [RUST]
   |                       ScreenCast + PipeWire + SPA cursor metadata
   |                       Rust Wayland pixel-grid overlay
   |                       existing KWin ColorPicker.pick() D-Bus
   |
   +--- on demand ---> colorboard-history [RUST]
   |                       centered, scrollable transient swatch UI
   |
   +--- on demand ---> colorboard-config [RUST]
   |                       eframe/egui settings; exits on close
   |
   +--- on demand ---> colorboard [RUST]
                           eframe/egui Board; exits on close
                           Mixer | Palette Editor | Theme Generator
                           external plugin loader initialized here only

All processes reuse Rust crates for: color models, storage, settings, IPC,
color science, schema validation, and theme-project data.
```

- No `eframe`, theme plugin loading, scene rendering, or PipeWire capture initialization in the idle tray process.
- Each helper must exit and reclaim its resources after use. Closing Board means **exit**, not hide in background.
- Only one tray instance, one active picker, and one Board instance. Second invocations signal the existing instance; a Board launched directly remains functional if the tray is stopped except for picker actions.
- The tray supervises transient helpers: start, cancellation, timeout/error, and crash recovery without spawning duplicates.
- Use asynchronous events and D-Bus signals, not short-interval polling.
- Measure tray idle USS/PSS/CPU separately from per-pick and Board-open footprints.

---

## 4. KDE/Wayland source references and mandatory integration design

Upstream reference implementations/documentation:

- KDE applet: <https://github.com/KDE/kdeplasma-addons/tree/master/applets/colorpicker>
- KDE applet picker D-Bus call: <https://github.com/KDE/kdeplasma-addons/blob/master/applets/colorpicker/grabwidget.cpp>
- KWin selection: <https://github.com/KDE/kwin/blob/master/src/plugins/colorpicker/colorpicker.cpp>
- KWin ScreenCast cursor metadata: <https://github.com/KDE/kwin/blob/master/src/plugins/screencast/screencaststream.cpp>
- KDE portal backend: <https://github.com/KDE/xdg-desktop-portal-kde/blob/master/src/screencast.h>
- XDG ScreenCast portal: <https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html>
- KWin effect documentation (reference only): <https://develop.kde.org/docs/plasma/kwineffect/>
- `egui`/`eframe`: <https://docs.rs/egui/> and <https://docs.rs/eframe/>
- MiniJinja: <https://docs.rs/minijinja/>

The KDE applet's `GrabWidget::pick()` calls:

- D-Bus service: `org.kde.KWin`
- Object: `/ColorPicker`
- Interface: `org.kde.kwin.ColorPicker`
- Method: `pick()`

KWin initiates compositor-controlled pointer selection and returns a Qt `QColor` serialized over D-Bus as a structure containing a packed 32-bit `QRgb` (`0xAARRGGBB` in conventional Qt representation). The existing service handles left-click confirmation and cancellation/busy/error cases. It **does not** expose neighboring desktop pixels for a live magnifier.

KWin ScreenCast supports PipeWire cursor metadata. Under the ScreenCast portal, request cursor metadata mode (`cursor_mode = 4`) where the installed portal advertises support. `SPA_META_Cursor` can carry cursor position and hotspot in stream coordinates; KWin's code also distinguishes cursor video from cursor metadata. This is a promising pointer source but **must be verified alongside the KWin picker**, including selection timing and output transforms.

### 4.1 Preferred picker backend — Rust with existing KWin services

**No custom compositor code is required by the initial architecture.**

1. Start an authorized XDG ScreenCast portal session using `ashpd`, request appropriate monitor sources, request cursor metadata mode, and open the PipeWire remote.
2. Decode negotiated PipeWire frames and SPA metadata, mapping each output's stream pixels, logical screen geometry, scale, rotation, and pointer hotspot accurately.
3. Render a Rust-owned, nearest-neighbor magnifier on a **non-keyboard-interactive, pointer-transparent Wayland layer surface**. Place it near but not over the sampling point.
4. When capture and overlay are ready, asynchronously invoke KWin's existing `ColorPicker.pick()` method. Let **KWin own selection/cancellation** and suppress click-through. Avoid conflicting pointer grabs or input interception.
5. Compare the display's highlighted sampled pixel with the final color returned by KWin. Treat the KWin reply as authoritative, but do not claim accurate magnification unless discrepancies have been resolved.
6. Immediately release capture, overlay, buffers, and portal session after success, cancel, or failure. Restore ordinary desktop appearance.

### 4.2 Known technical constraints that cannot be hand-waved

- **Simultaneous capture and KWin selection is not proven.** Both work independently in upstream code; validate the combined workflow on the actual Plasma Wayland installation.
- **Recursive overlay capture:** The compositor may include the magnifier layer in frames. Merely moving the lens away from the sampled point is not a proof of exclusion. Ensure actual source samples, capture timestamps, lens placement, and renderer feedback do not cause visual feedback or wrong colors. If exclusion is impossible through supported protocols, record a blocker.
- **Pointer coordinates:** SPA cursor positions are in stream coordinates, not universally in global screen coordinates. Validate output origin, scaling, transforms, hotspot, non-integral coordinates, and movement across outputs.
- **Cursor/video synchronization:** Cursor metadata and video frames may not describe an identical instant. Avoid presenting a knowingly stale or wrong center pixel as exact. Match known color fixtures to KWin's definitive selection and explain unavoidable capture-frame latency.
- **Click ownership:** The overlay must have an empty pointer input region; KWin still receives selection/cancel clicks. No click-through to the underlying application, as verified by testing, not assumption.
- **Keyboard focus:** An overlay may not take keyboard focus; desktop portal permission dialogs are the one accepted exception. KWin may receive Escape as part of compositor selection. Don't create or activate a normal top-level picker window.
- **Monitor scope/permissions:** Capturing all outputs may require user authorization, and restore tokens or source selection may be unavailable/denied. Never bypass KDE's permission model. Tokens may expire/rotate and are not guaranteed to remove prompts.
- **Layer-shell availability:** Verify the specific layer-shell protocol and input/output behavior supported by the installed KWin version. Ordinary xdg-toplevel APIs do not guarantee arbitrary overlay placement.
- **Backend loss:** Handle portal rejection, stream disconnection, lock-screen transitions, output hotplug, KWin restart, and repeated/canceled picks without leaving stale surfaces or capture resources.

### 4.3 Rust-only failure policy

A custom KWin QML/JavaScript effect is **not** a prescribed fallback. A native C++ KWin effect is **prohibited by the Rust-only implementation decision**. Existing KWin services remain external dependencies, not code shipped by Colorboard.

If a gate fails:

1. Capture a reproducible failure, exact environment/KWin versions, screenshots/logs (without sensitive desktop contents), and expected versus observed behavior.
2. Try an alternative **supported Rust-accessible Wayland/portal/KWin approach** that retains the required semantics.
3. If no supported Rust-only solution meets the gate, **stop** and present a precise architectural decision to the user. Do not silently use native C++, ship a scripted compositor effect, fake live magnification, or relax focus/pixel-accuracy requirements.

The hard gate is deliberate; it prevents building later phases on a picker that fails the core requirement.

---

## 5. Shared core, configuration, database, and IPC

### 5.1 Shared Rust libraries

Provide separate reusable crates for:

- `colorboard-color`: canonical sRGB/RGBA types, parsing, formatters, perceptual conversion, blending, palette harmony and contrast logic.
- `colorboard-model`: history entry, palette, palette swatch/link/lock, theme project, role assignment, preview state, plugin manifest types.
- `colorboard-storage`: SQLite schema, migrations, transactions, typed repositories and data integrity.
- `colorboard-config-core`: XDG paths, versioned `config.toml`, validation, schema migration for settings, shortcut preferences.
- `colorboard-ipc`: versioned session D-Bus interface, events, typed errors, async Rust clients.
- `colorboard-theme-core`: plugin loader/schema validation, scene graph, semantic roles, MiniJinja export pipeline.

These are Rust library crates, not always-running processes. Link the smallest subset into each binary; minimize tray dependencies.

### 5.2 XDG paths

- Config: `${XDG_CONFIG_HOME:-$HOME/.config}/colorboard/config.toml`
- Data: `${XDG_DATA_HOME:-$HOME/.local/share}/colorboard/library.sqlite3`
- Saved project/export metadata: `${XDG_DATA_HOME:-$HOME/.local/share}/colorboard/themes/`
- User plugin installation: `${XDG_DATA_HOME:-$HOME/.local/share}/colorboard/plugins/`
- System plugin installation: each `${XDG_DATA_DIRS}` entry followed by `/colorboard/plugins/`

Resolve XDG directories properly. Do not literally expand `${HOME}`/`~` inside a stored path string. Respect user-specific directory permissions.

### 5.3 SQLite ownership and migrations

- **All schema creation and migrations live in `colorboard-storage` (Rust)**. Every executable invokes the same migration/check library at startup as needed. A one-shot `colorboard-db migrate/check` CLI is optional but recommended for troubleshooting; it must use the exact same Rust migration code.
- The database can be opened by tray and Board independently; neither depends on the other being running.
- Use WAL mode, `busy_timeout`, foreign-key enforcement, short transactions, versioned migrations, and clear `SQLITE_BUSY`/incompatible-schema errors. Make destructive migrations reversible through backups where feasible.
- Store capture history, saved palettes, draft/generation history, and theme projects as separate logical entities. Design appropriate indexes before large data growth.
- Initially create `schema_migrations`, `color_history`, `settings_meta`; add `palettes`, `palette_swatches`, `palette_generations`, `theme_projects`, and `theme_assignments` during their corresponding phases.
- Canonical captured sample: unambiguous **8-bit sRGB RGBA**, normally opaque. Include capture UUID, UTC timestamp, source (`picker` or `board`), and explicit alpha. Preserve original capture independently from its display string.
- Successful repeated captures may appear as repeated history rows; re-copying a history swatch does not create another pick or reorder history by default.
- Database data changes emit D-Bus notifications; consumers refresh from authoritative storage on startup/reconnect. Signals are not the only persistence mechanism.

### 5.4 Color formats and clipboard

Support `#RRGGBB`, `#RRGGBBAA`, `rgb()`, `rgba()`, HSL, HSV, and CMYK-like text. CMYK is an approximation unless ICC/profile-aware conversion is implemented; label it accordingly. Provide parser validation, deterministic formatting, locale-independent numeric output where formats require it, and golden tests.

**Clipboard lifetime:** The tray service should own or broker clipboard data as required by Wayland so copying remains valid when the transient picker/popup exits. No clipboard clearing on cancel.

### 5.5 D-Bus API contract

Define stable versioned service names and signatures in `docs/ipc.md` before implementation. Initial conceptual operations:

- `PickColor() -> started | busy | error`
- `ShowHistory() -> started | already-open | error`
- `OpenBoard() -> started | already-running | error`
- `GetHistory(limit) -> list`
- `GetLastColor() -> optional color`
- `ReloadConfiguration() -> success/error`

Signals:

- `ColorPicked(id, rgba, timestamp)`
- `ColorPickCancelled()`
- `ColorPickFailed(error)`
- `HistoryChanged()`
- `ConfigurationChanged()`

Specify method signatures, source authentication requirements as applicable, busy-state behavior, timeouts, and lifecycle/cancellation cleanup. The Board can request a pick and optionally hide until a completion/cancel/failure signal arrives; ensure it restores appropriately even if the picker crashes.

---

## 6. Implementation phases — strict sequence and release gates

### Phase 1A — Live Rust magnifier and picking engine (FIRST, HARD GATE)

**Deliverable:** A Rust command that selects a pixel anywhere on KDE Plasma Wayland while visibly rendering an accurate, live magnification grid, without disrupting the prior application's focus.

Tasks:

1. Record actual KDE Plasma, KWin, portal, PipeWire, Wayland protocol, and Rust crate versions on the target workstation. Read the upstream KWin picker and ScreenCast sources listed above.
2. Implement `ashpd` ScreenCast session management with video streams and cursor metadata mode (`cursor_mode=4`, when advertised). Support selected/all authorized outputs and restore-token persistence only when KDE grants it.
3. Implement PipeWire negotiation and frame decoding with correct buffer strides, formats, SPA cursor position/hotspot, logical-to-physical coordinate transformations, and timestamps. Add a debug diagnostic mode outputting output ID, frame dimensions, source pixel coordinates, and center color.
4. Add deterministic fixture tests: 1-pixel checkerboard, contrasting stripes, gradient, negative monitor origin, 100/125/150/200% scale, rotated display, output crossings, and timestamp skew.
5. Implement a Rust-owned Wayland layer-shell overlay with zero keyboard interactivity and an empty pointer input region. Paint a lens offset from the pointer with nearest-neighbor squares, center outline, source coordinates, and live color value. Adjustable source radius and magnification, without initializing `eframe`.
6. Integrate KWin's `org.kde.kwin.ColorPicker.pick()` **only after live capture/overlay is ready**. Decode successful Qt `QRgb` D-Bus reply, normal cancellation, busy, and errors. Let KWin handle mouse confirmation/cancel without a competing pointer grab.
7. Demonstrate that the exact highlighted pixel and authoritative KWin result agree on controlled target images. Validate both pointer movement and underlying desktop animation/content changes.
8. Explicitly test capture-overlay feedback, stale cursor-video pairing, slow frame delivery, cross-output cursor moves, hotplug, full-screen windows, fullscreen scaling, lock/unlock, portal denial/restoration, and cancellation/cleanup.
9. Validate no normal-window focus change, click-through, stuck selection state, or orphaned screen-capture process. Confirm the full process exits promptly on success/cancel/failure.
10. Document one complete real KDE Wayland test run with actual observations; unit mocks alone do not satisfy acceptance.

**Phase 1A hard exit gate:** Live frame updates and pixel-accurate center highlight; correct KWin-selected final value; left-click confirm/right-click cancel/Escape as supported; no click-through; original keyboard focus remains except permission dialogs; works at required monitor/scaling configurations; clean shutdown with no persistent capture or overlay. If any criterion fails, stop and report the Rust-only blocker as described in §4.3.

**Do not implement tray or Board before this gate passes.**

### Phase 1B — Minimal Rust tray and orchestration

- Implement single-instance `colorboard-tray` using `ksni` and `zbus`.
- Pipette plus last-color indicator rendered as an appropriate tray pixmap; account for multiple sizes/themes.
- Left-click pick; right-click exact specified menu.
- Start transient picker asynchronously, enforce one pick at a time, surface errors without blocking tray events.
- Expose versioned D-Bus control methods/events and maintain selected color state.
- Do not initialize GUI/editor or capture stacks at idle.

**Gate:** Tray behaves correctly while an unrelated app is focused; picker can be invoked repeatedly; all processes and error paths are stable.

### Phase 1C — History, formatting, and clipboard

- Build common Rust SQLite schema/migrations, typed history repository, XDG storage, history retention (default 100), and crash-safe commits.
- On success write the capture, emit signals, update tray icon and clipboard. Cancellation writes nothing.
- Add color formatters, validation, clipboard persistence, duplicate-handling policy, and unit tests.
- Verify new tray process starts with the prior last-picked color.

**Gate:** Repeated captures persist correctly; copy/paste remains usable after transient picker exit; malformed config/database states return controlled errors.

### Phase 1D — Rust scrollable history popup

- Implement on-demand frameless centered history surface with variable rows/columns/display limit and mouse wheel/touchpad scrolling.
- Use verified Wayland layer/xdg-popup placement; treat click-away and Escape dismissal as acceptance tests, including focus behavior and multiple monitors.
- Clicking a swatch copies without creating a new capture; show label/tooltip and confirmation.
- Dismissal exits transient process without stopping tray; second activation toggles close.

**Gate:** Popup genuinely centers, scrolls, copies, closes on all specified triggers, and releases resources on KWin Wayland.

### Phase 1E — Rust settings, global shortcuts, startup, packaging

- Implement `colorboard-config` as a **transient Rust egui** settings utility. No dependence on the unfinished main Board.
- Settings cover all choices in §2.4 and validate min/max/geometry/format values. Persist through typed TOML settings and notify tray.
- Implement XDG GlobalShortcuts through `ashpd` where supported, with explicit KDE system shortcut fallback only if needed; never rely on X11 global key hooks. Handle permission/conflict/revocation and user reassignment.
- Add session autostart via a single selected desktop-file/systemd-user path to avoid duplicates.
- Package the Rust binaries and icons; **do not require custom KWin effects or non-Rust application modules**.
- Test suspend/resume, portal restart, KWin restart, monitor hotplug, and single-instance startup.
- Measure tray idle memory (USS/PSS), CPU wakeups, and per-pick startup/memory. Record results and regressions.

**Phase 1 complete only when:** Phase 1A–1E jointly pass on actual KDE Plasma Wayland. The Board implementation must not begin sooner.

### Phase 2 — On-demand Rust Board application shell

- Create a standalone `colorboard` binary using `eframe/egui`; it must start with or without the tray present.
- Create three tabs in this order: **Color Mixer**, **Palette Editor**, **Theme Generator**. Only the shell and shared layout are implemented in this phase.
- Add a scrollable horizontal color-history strip visible in **every tab**.
- Read history and settings through shared Rust crates; subscribe to D-Bus changes while the Board is open, with database refresh on reconnect.
- Add **Pick Color** command calling the tray D-Bus service. Optional setting hides Board during picking and restores it on selection, cancellation, or error; do not inadvertently steal focus during the active pick.
- Implement single-Board-instance behavior, settings link, clear save/error states, and main-window state persistence.
- Closing Board exits its process fully; never keep it running invisibly for future launches.

**Gate:** History synchronizes with new Rust picker captures; picker invocation works; restoration is correct; closing Board removes its GUI runtime.

### Phase 3 — Color Mixer (first functional tab)

- Editable HEX, RGB, RGBA, HSL, and HSV controls and values; display CMYK approximate conversion and other configured formats.
- A large current-color swatch and a scrollable list of recent picked colors; select or drag a history swatch into the mixer.
- Mix two or more source colors with a ratio/slider; offer explicit sRGB versus perceptual OKLCH interpolation choices.
- Copy current color in the configured format.
- Add current/mixed color to the **active palette**, creating an unsaved draft palette if none exists.
- Undo/redo mixing state where practical, and reject invalid input without corrupting the current selection.

**Gate:** User can select past colors, adjust or mix them, copy results, and add them to the active palette without using another tab.

### Phase 4 — Palette Editor (second functional tab)

**Generator features:**

- Configurable number of colors and optional base color.
- Complementary, split-complementary, analogous, triadic, tetradic, monochromatic, perceptual contrast, and fully random generation.
- Adjustable hue/lightness/chroma constraints and optional minimum contrast target; use OKLCH and clip/map back to sRGB deliberately.
- Reproducible random seed option.

**Editor features (Coolors-style workflow):**

- Large interactive editable swatches; add/remove/reorder, lock/unlock, edit each color directly, regenerate unlocked colors only.
- Apply prior captured colors by click or drag-and-drop; adding a history swatch may lock it with an explicit control.
- Choose palette size, mark a generated palette to save, rename/duplicate/delete/save, and use undo/redo.
- Saved palette history, search/list, distinct draft generation history, and re-openable saved palettes.
- Select active palette for Color Mixer and Theme Generator. Palette updates propagate to any linked theme roles according to a documented override policy.

**Storage:** Add appropriate palette/generation migrations to `colorboard-storage`, used automatically by every executable. The Board is still the only process initializing the palette editor GUI.

**Gate:** Create, generate, lock, regenerate, edit, save, reopen, and select a palette; both mixer and theme-tab context can reference that palette.

### Phase 5 — Theme Generator (third functional tab)

**This entire subsystem is implemented in Rust. Plugins are still installable as data.**

#### Phase 5A — Generic interactive theme editor and scene renderer

- Select an application/theme plugin and active source palette.
- Draw a mock app preview and an element/role inspector side by side.
- Each role displays a color swatch and an editable value; default format is HEX, configurable in app settings.
- Drag a history/palette color onto any eligible preview element or role in the inspector; provide visible hover/drop feedback.
- Click preview elements to select their corresponding roles. Click actionable elements (menu, tab, dialog, sidebar) to transition to **other preview scenes**, including initially hidden views.
- Link a role to a named palette swatch or override with a direct color. Make override/link semantics explicit, stable, and undoable.
- Save/reopen theme projects, including target plugin/version, chosen palette ID, role assignments, overrides, active preview scene, and unsaved changes.
- Export validated files without applying live themes to the system. A future explicit Apply feature is separate and **must not** be implemented implicitly.

**Shared renderer:** Build one data-driven Rust scene renderer on top of `egui::Painter`. It interprets versioned scene-node records (rectangles, backgrounds, borders, text runs, syntax tokens, images/icons, tabs, lists, windows, dialogs), handles hit-testing, hover/selection, drag/drop, coordinate transforms, view transitions, state changes, and semantic role binding. It must **not** require a hand-coded egui scene for each plugin.

**Preview fidelity:** Mock application UI and configuration role behavior; do not promise pixel-identical screenshots of installed Ghostty, Zed, or KDE. Provide meaningful editable views and transitions. Test the mock against target formats/role semantics, not an assumption of exact rendering engine equivalence.

#### Phase 5B — External plugin contract (hard requirement)

A user can create a new plugin directory or install a compatible third-party plugin by copying its files to an XDG plugin directory; clicking **Reload Plugins** makes it available **without recompiling, patching, or reinstalling the host**.

Example **data-only** plugin package:

```text
my-terminal/
├── manifest.json                # ID, API version, name, supported targets/versions
├── roles.json                   # semantic color roles, defaults, validation
├── scenes.json                  # preview nodes, role bindings, clickable transitions
├── templates/
│   └── theme.j2                 # MiniJinja template, output format declarations
├── assets/                      # optional icons/fonts/images, bounded & validated
│   └── terminal.svg
└── tests/
    ├── sample-palette.json
    └── expected-theme.txt
```

Plugin schema requirements:

- Versioned manifest and capability flags; stable plugin ID, compatible schema/API versions, target application metadata, supported app/config versions, and expected output filename/extensions.
- `roles.json`: roles with stable IDs, title/help text, type, default, format/conversion policy, validation, optional grouping, and permissible palette link mappings.
- `scenes.json`: scene IDs, nodes, coordinates, stacking, text/icon assets, semantic color-role bindings, selectable/hittable elements, click actions and state transition targets. A plugin can model multiple views, menus, dialogs, and disabled/inactive states with data only.
- `templates/*.j2`: **MiniJinja**-compatible text templates, strict undefined variables, explicit filters and role-value context. Template rendering is separate from role validation and color conversion. Do **not** claim support for the full Python Jinja2 extension ecosystem.
- Optional import mappings/parsers only where generic declarative parsing supports them. Preserve unrecognized keys for round trips where feasible; do not promise lossless imports across incompatible formats.
- JSON Schemas are published with examples. Loader validates all references and versions, rejects invalid plugin assets or conflicting IDs with an actionable error, and never assumes an arbitrary filename/path is safe.
- Discover user plugins from XDG data directories, installed system plugins from XDG data directories, and bundled plugins from the installed resources. Resolve duplicate IDs deterministically (prefer reject with diagnostics rather than silent shadowing).
- Expose **Reload Plugins**; on reload validate manifest, update plugin chooser, preserve current project safely, and report removed/incompatible selected plugins without data loss.
- Plugin discovery reads only small manifest metadata; load heavy preview assets/rendering data only when the plugin is selected. The tray must never initialize plugins.

**Security:** Treat user-installed declarative packages as untrusted data. Constrain file sizes, image decode sizes, template execution budgets as available, output count, export paths, directory traversal/symlinks, recursion/expansion, and referenced asset paths. Never execute shell commands or load native `.so` libraries from plugin directories. Exports go only to paths the user chooses or application-owned staging locations, with preview/diff and explicit confirmation for overwrites.

**Optional advanced behavior:** Data-only plugins are the v1 baseline, capable of all ordinary role editing, selection, preview transitions, and export. If a documented target cannot be described declaratively, evaluate an opt-in `rhai` runtime for bounded parsing, validation, and transformation hooks. Such code runs only for explicitly enabled trusted plugins, has no filesystem/network/process APIs by default, and enforces operation/time/memory or collection limits as supported. Rhai is **not** required for Ghostty/Zed/KDE plugins unless a proven feature demands it. Do not embed Python or Lua interpreters, and do not require third-party compiled Rust plugins.

#### Phase 5C — Theme export pipeline

1. Load and validate selected plugin manifest, role schema, scene definitions, and compatible target version.
2. Resolve active palette links, locked colors, theme-specific overrides, and defaults to a canonical role-value map.
3. Validate required roles, alpha policies, output structure, and supported target format.
4. Convert colors to the target application's native string representations through registered **host-owned** formatters.
5. Render MiniJinja templates with strict undefined variables and bounded, deterministic context.
6. Validate emitted files where practical (JSON parser for Zed, INI-style KDE checks, Ghostty grammar/fixtures).
7. Preview output filenames/content and export atomically to a user-selected destination; protect against accidental overwrite.
8. Save/update the theme project and preserve error diagnostics without losing edits.

Golden-file tests must cover all exported files and include representative locked/overridden palettes. Running real target apps is optional for test validation and is not a plugin-execution mechanism.

#### Phase 5D — Initial external theme plugins

Implement each as an installable **data-only plugin package** (where possible) rather than special logic baked into the Board:

1. **Ghostty** — terminal background/foreground, ANSI 0–15, cursor, selection, other supported theme roles; terminal screen preview; generate native Ghostty theme text. This is the **first end-to-end external plugin example**.
2. **Zed** — editor, tabs, sidebar, panes, status bar, borders, text/syntax roles, inactive views, menus/dialogs; export versioned JSON theme files.
3. **KDE `.colors`** — window/view/button/selection/tooltip, enabled/inactive/disabled sets and representative controls; export a valid KDE color-scheme INI-style file. Test target KDE semantics against documented configuration format; don't pretend an egui mock is literally a running KDE widget.

**Phase 5 hard gate:** A **new fixture plugin authored after Colorboard is built** can be placed into the user plugins directory, discovered by Reload Plugins, rendered with the shared scene renderer, interacted with (including opening a hidden submenu), assigned colors by drag/drop, edited, saved, and exported with its MiniJinja template **without modifying or rebuilding any Rust source**. Ghostty, Zed, and KDE plugins should satisfy the same data-package contract.

### Phase 6 — Packaging, documentation, and hardening

- Package for Arch initially, with Fedora/openSUSE compatibility targets as appropriate; use installed libraries and desktop components as documented, not an embedded Python/C++ plugin runtime.
- Add `.desktop` files, icons, XDG autostart or user service, MIME/config install paths, and AppStream metadata where appropriate. Avoid simultaneous duplicate autostart strategies.
- Add a full test matrix for actual KDE Wayland, multi-display/scaling, portal prompts/revocation, compositor restarts, clipboard lifetime, notifications, and keyboard shortcut conflicts.
- Keyboard accessibility, predictable focus handling, error surfaces, safe plugin loading and import/export, helpful documentation, and release reproducibility.
- Provide a plugin-authoring guide, JSON schemas, MiniJinja supported-feature table, fixture plugin template, schema/IPC version policy, and troubleshooting guide.
- Track idle tray, picking, history popup, settings, and Board memory separately; confirm the Board and plugin interpreter are unloaded when closed.

---

## 7. Cargo workspace and repository layout

```text
colorboard/
├── Cargo.toml                       # one workspace / one Cargo.lock
├── Cargo.lock
├── crates/
│   ├── colorboard-color/             # typed colors, conversions, harmony, formatters
│   ├── colorboard-model/             # history/palette/theme/plugin data structures
│   ├── colorboard-storage/           # sqlite, schema migrations, repositories
│   ├── colorboard-config-core/       # XDG settings, defaults, validation
│   ├── colorboard-ipc/               # D-Bus service/proxy contract
│   ├── colorboard-picker-core/       # portal, PipeWire, cursor, magnifier core
│   ├── colorboard-wayland-ui/        # small transient overlay/popup drawing
│   ├── colorboard-theme-core/        # scene graph, plugin load, MiniJinja export
│   └── colorboard-ui-common/         # reusable egui controls, Board/config only
├── apps/
│   ├── tray/                         # colorboard-tray, ALWAYS RESIDENT
│   ├── picker/                       # colorboard-picker, transient
│   ├── history/                      # colorboard-history, transient
│   ├── config/                       # colorboard-config, transient egui
│   ├── board/                        # colorboard, transient egui
│   │   ├── src/mixer/
│   │   ├── src/palettes/
│   │   └── src/themes/
│   └── db-cli/                       # optional one-shot migration/check tool
├── plugins/
│   ├── ghostty/
│   ├── zed/
│   ├── kde/
│   └── example-terminal/            # independent external fixture bundle
├── schemas/
│   ├── plugin-manifest.schema.json
│   ├── theme-roles.schema.json
│   ├── preview-scenes.schema.json
│   ├── palette.schema.json
│   └── theme-project.schema.json
├── assets/
├── packaging/
├── tests/
│   ├── fixtures/
│   ├── integrations/
│   └── golden/
└── docs/
    ├── implementation-plan.md
    ├── architecture.md
    ├── requirements.md
    ├── decisions.md
    ├── ipc.md
    ├── storage.md
    ├── plugin-api.md
    ├── template-compatibility.md
    ├── testing.md
    └── roadmap.md
```

Crates may be consolidated when separation creates boilerplate without clear reuse. Do not split them into separately running services merely because they are crates. Shared library code should remain cheap to link into the tray without dragging in the Board's rendering stack.

## 8. Engineering rules for the coding agent

1. **Rust only:** Rust is the implementation language for all shipped application executables/libraries. Plugins are declarative files and MiniJinja templates; bounded optional Rhai hooks may be added by an explicit phase decision. Do not add Python, C++, custom QML/JS KWin components, or generated Qt-bridge build chains.
2. **Strict phase gates:** Complete and verify Phase 1A on a real KDE Wayland session before the tray; complete Phase 1A–1E before starting Board; complete Board tabs sequentially.
3. **No downgraded magnifier:** Do not substitute KWin's single-pixel picker, a static screenshot, fake grid, or focus-stealing normal window for a fully working live magnifier.
4. **No presumed compositor APIs:** Read current KWin/portal/Wayland/Rust binding signatures. Treat independent API support as distinct from demonstrated joint behavior.
5. **No resident heavyweight GUI:** `eframe`, preview rendering, MiniJinja/Rhai, and screen capture must stay outside the idle tray lifetime.
6. **One canonical data model:** Reuse typed Rust data structures for D-Bus serialization and SQLite mapping; don't create divergent application-specific color representations.
7. **Storage migrations:** Only `colorboard-storage` performs SQLite migrations. Test fresh installs, independent Board startup, rollback/error handling, and concurrent access.
8. **External plugins:** A fixture plugin newly installed after compilation must pass discover → reload → preview → interact → edit → save → export with no rebuild. Reject untrusted files/syntax safely.
9. **Tests:** Use rustfmt, Clippy, cargo unit/integration/property tests where useful, snapshot/rendering tests for scenes, and golden-file tests for exports. Real KDE Wayland manual acceptance is required in addition to headless tests.
10. **No invented performance claims:** Benchmark idle PSS/USS, CPU, startup, per-pick capture memory, and transient process cleanup. Optimize proven hotspots.
11. **Version/license discipline:** Keep Cargo.lock, record supported KWin/Plasma and portal versions, audit dependency licenses, and respect upstream KDE source licenses if copying code (prefer independent implementations).
12. **Handoff discipline:** At each phase report files changed, tests actually executed (with results), real-desktop tests still required, blockers, and the next actionable phase. Never claim live Wayland acceptance from mocks alone.

## 9. Architectural decision record

| Decision | Selected implementation | Reason |
|---|---|---|
| Language | **100% Rust application code** | Unified Cargo toolchain, typed shared models, no Python/C++ authored components |
| Process design | Tiny resident tray + transient picker/history/config/Board | Minimize idle memory; release GUI/capture assets when done |
| Main GUI | **egui/eframe** on demand | Convenient custom scene painting, hit testing, swatches, drag/drop, and runtime data-driven views |
| Preview engine | Shared Rust declarative scene renderer | New applications install as plugin data, not compiled UI extensions |
| Theme export | **MiniJinja** + host-owned validators/formatters | Jinja-style authoring without Python; defined compatibility subset |
| Plugin extensibility | **External declarative packages**, optional restricted Rhai hooks | New theme targets require no host rebuild; avoid unstable native Rust plugin ABI |
| Storage | Rust `rusqlite` and shared migration library | One source of truth; Board can launch independently |
| Picker pixel feed | Rust XDG ScreenCast/PipeWire + `SPA_META_Cursor` | Live authorized desktop pixels and cursor metadata |
| Selection/cancellation | Existing KWin `ColorPicker.pick()` D-Bus | Reuse compositor-owned desktop selection; no authored compositor extension |
| Magnifier visual | Rust pointer-transparent, non-keyboard-focusable Wayland layer surface | Preserve focus and click ownership; needs live KWin integration test |
| Non-Rust compositor fallback | **Not allowed without new user decision** | Rust-only choice takes precedence over an implicit C++/QML fallback |

The initial language/runtime decision is settled. Changes require documented user approval and revised acceptance criteria; failures must not be hidden by cutting requirements.

---

## 10. Immediate copy/paste kickoff task for an implementation agent

> Implement **Phase 1A ONLY** of `docs/implementation-plan.md`, Revision 2.0. The application must be written entirely in **Rust**, targeting **KDE Plasma 6 Wayland**. Build a **real live pixel-accurate magnifier** using the XDG ScreenCast portal, PipeWire frames, and `SPA_META_Cursor` metadata (request `cursor_mode=4` if supported), with a Rust-rendered Wayland layer-shell surface that has **no keyboard interactivity and an empty pointer input region**. Track source pixels correctly across fractional scaling, multiple monitors, negative geometry, and transforms. Once the magnifier is displaying live source pixels, call the existing KWin `org.kde.kwin.ColorPicker.pick()` D-Bus method asynchronously for final selection and cancel. Verify that the center highlighted pixel matches KWin's selected pixel, the overlay never steals keyboard focus, clicks do not reach underlying windows, the magnifier does not recursively capture itself, moving content is live, permissions behave correctly, and all capture/overlay resources shut down cleanly. Tests must include actual observations from a **real Plasma Wayland session**; passing mocks alone is not sufficient. Do **not** implement tray, history, Board, or theme plugins yet. Do **not** introduce Python, C++, a custom QML effect, or a frozen/single-pixel fallback. If a mandatory criterion is impossible under available Rust-facing interfaces, stop with a reproducible blocker report and an explicit decision request. Record versions, build/test results, outstanding limitations, and the next task.
