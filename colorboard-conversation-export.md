# Colorboard: Conversation Export

**Date:** October 20, 2026  
**Topic:** Designing a KDE Plasma color picker, palette editor, and theme builder; architectural decisions and implementation plans.

> **Export fidelity:** This document was reconstructed from the visible conversation context, not obtained from ChatGPT's native data export. The **user's requests and decisions are transcribed**, and assistant responses are presented as **substantive summaries**, not guaranteed verbatim quotations. Two complete Markdown implementation plans available in the session are included as appendices. System/developer instructions, hidden reasoning, tool internals, and incidental status messages are excluded. For a byte-for-byte official chat history, use ChatGPT's account data export.

---

## Exchange 1 — Initial language selection

### User

I want to build a color palette app for Linux, specifically targeting KDE. It will have a color chooser that can select a color from anywhere on the screen that can be triggered via a hotkey, and should live in the system tray. What is the best language to use to build the app?

### Assistant — summary

Initially recommended **C++20 with Qt 6 and KDE Frameworks 6** as the most native approach for KDE Plasma, with **Python/PySide6** as the rapid-development alternative. The suggested native stack included `KStatusNotifierItem` for the system tray, `KGlobalAccel` for hotkeys, `QColor` and `KColorUtils` for color handling, `KConfig` for settings, and the XDG Desktop Portal `Screenshot.PickColor` interface for selecting colors under Wayland. Explained that Wayland prohibits unrestricted pixel reading and global key interception by ordinary applications; portals or compositor interfaces must mediate those operations. Noted that KDE's KColorChooser source was worth consulting, and distinguished the standard one-pixel eyedropper from a custom live magnifier.

---

## Exchange 2 — Python limitations and workarounds

### User

Tell me more about the python limitations & possible workarounds

### Assistant — summary

Reassessed Python favorably: **the requested basic picker could be built entirely in Python/PySide6 without C++**. Explained that Qt/PySide6 natively supports tray icons (`QSystemTrayIcon`), `QColor`, the clipboard, QML/Kirigami, and D-Bus. The biggest KDE-specific gap was the lack of a documented Python binding for `KGlobalAccel`. Proposed the **XDG GlobalShortcuts portal** accessed over QtDBus or another Python D-Bus package, or a KDE user-configured shortcut that triggers the running app. Warned that `pynput` and `keyboard` are not suitable substitutes for system-wide input interception on Wayland.

Described using `org.freedesktop.portal.Screenshot.PickColor`, converting normalized RGB values to `QColor`, and using `QSystemTrayIcon` with a menu. Distinguished this straightforward eyedropper from a live moving pixel-grid magnifier, which would require additional screen capture and rendering. Mentioned the `qasync`/`asyncio` event-loop issue when integrating async Python D-Bus libraries with Qt and advocated QtDBus for a minimal dependency footprint. Recommended distro-provided KDE/Qt bindings to avoid binary-version mismatches. For this relatively small project, Python/PySide6 could be an excellent choice.

---

## Exchange 3 — Theme-builder expansion and memory architecture

### User

What if I want to add support for a theme builder? It would display a mock UI for a particular app, say ghostty, zed, or KDE colorschemes, and allow the user to apply their palette & modify individual elemets wysiwyg style, then allow the user to export a config file. I want to keep the app as memory light as possible. Could that be done by splitting it into two integrated applets, the color picker and the scheme editor? Or does that change your recommendation entirely?

### Assistant — summary

Recommended **separate processes**, not merely separate modules: a persistent minimal tray picker and an on-demand visual theme editor that exits fully when closed. Suggested either all C++/Qt or a **C++ tray utility plus Python/PySide6 editor** to optimize both idle memory and editor-development speed. Recommended against making both components Plasma widgets: ordinary desktop processes with tray integration give simpler lifecycle control.

For the theme editor, proposed **application-specific import/preview/validation/export adapters** and a common palette model. Ghostty requires a relatively simple terminal preview and text config; Zed requires detailed editor UI and syntax color roles with JSON output; KDE color schemes need color-set/state mappings and `.colors` export. Preferred native Qt rendering (`QGraphicsView` or QML) instead of Chromium/WebEngine. Suggested separate palette objects, theme project mappings, and exporters; data sharing through JSON and/or D-Bus, with SQLite when history and saved projects justify it. Emphasized measuring PSS/USS rather than misleading RSS when assessing memory.

---

## Exchange 4 — Detailed feature specification and request for an implementation plan

### User

I want to use Rust for the color picker portion, and Python for the the main app. 

The color picker should behave similarly to the builtin KDE color picker widget (I think this is the source, please double-check: https://github.com/KDE/kdeplasma-addons/tree/master/applets/colorpicker), but with these differences:
- It should live in the system tray instead of being a widget.
- The tray icon should be a pipette with an indicator dot the color of the last color chosen.
- Left clicking on the tray icon, should start the color picker
- Right clicking on the tray icon should show a menu with the options and behaviours:
  - Pick Color
  - Configure
    - Starts the color picker
  - Brings up scollable pallette window similar to the KDE widget, but overlayed in the middle of the screen
    - Shows a number of last colors picked, scrollable
    - clicking on a color copies it to the clipboard
  - Open Board 
    - Opens the full app
  - Configure
    - Brings up a configure menu with that allows changing:
      - The number of colors displayed in the palette window
      - The number of rows and columns in the pallette window
      - Radius of pixes around picker to magnify
      - Format of color that gets copied to the clipboard (hex, rgb, cmyk, etc)
      - Number of colors to keep in history (Default 100)
      - Hotkey to trigger color picker
      - Hotkey to bring up palette window
- When picking a color from the screen, it should display a magnifying glass that allows pixel accuracy.
- When starting the color picker, it shouldn't steal focus from the active window

The main app should:
- Be written in Python
- Have a color mixer
- Share color history with the picker
- Have the ability to launch the color picker & have the option to hide itself when launching the color picker.
- Have a tabbed interface with the following tabs:
  - Color mixer that:
    - shows swatches of past colors
    - Displays color values in various formats
    - allows colors to be added to the active pallette in the palette editor
  - Pallette editor that:
    - Generate configurable smart palettes with complementary & contrasting colors, or totally random
    - Lets the user mark generated palettes to save
    - select a generated or saved palette to edit
    - Have a palette editor that lets the user edit the current selected palette one swatch at a time & save the palette. Should be similar to https://coolors.co/generate
    - has swatches of the past chosen colors that can be added to and locked in the palette editor.
    - choose the number of colors in a palette
    - have a history of past saved palettes
    - allows the pallette to be used in the theme generator
  - Theme generator that
    - Has an interactive preview of the app that the theme is for.
    - Has a list of the elements to choose colors for, with a color chooser for each. It should allow editing the value directly via a format configured in the options menu (default hex).
    - Allow dragging and dropping colors from the history onto an element
    - Allows clicking on elements in the preview to display other elements normally hidden in the main app view like sub menus or different views
    - uses plugins to change the app the theme is being generated for. These should be written in python, lua, or something else if you have a better suggestion.
    - uses jinja2 templates in the plugins to manage text based exported theme file formats
- display a scrollable row of swatches from the color history on all tabs

Please do the following:
- Let me know if these requirments change your recommended languages & why, then ask me if I would like to change.
- Ask me follow up questions for unclear elements
- Write a plan that I can give to an agent to start implementing. I want to start with the color picker, and once that's working move to the main app & Implement it one functional tab at a time.

### Assistant — summary

Verified that `KDE/kdeplasma-addons/applets/colorpicker` is the correct source. Its QML applet calls `GrabWidget`, whose C++ implementation invokes KWin's session D-Bus service `org.kde.KWin` at `/ColorPicker` with interface `org.kde.kwin.ColorPicker`, method `pick()`. This returns a `QColor`; it is not itself a configurable live frame magnifier. KWin's color picker implementation was inspected as well.

Recommended preserving **Rust for the picker** and **Python/PySide6 for the Board**, with an optional narrow C++ KWin component if indispensable. Proposed Rust `ksni`, `zbus`, `ashpd`, Wayland/layer-shell, PipeWire, SQLite, and TOML; Python/PySide6 with `QGraphicsView`, Jinja2, and theme plugins. The plan was phased so the picker must be completed before building the Board, followed by one functional tab at a time. Clarified menu ambiguity to **Pick Color / Show Palette / Open Board / Configure / Quit**. Asked six decisions about platforms, live/snapshot magnification, focus behavior, popup behavior, menu layout, and language contingency.

---

## Exchange 5 — Magnifier is mandatory

### User

The magnifying glass for the color picker shouldn't be a separate feasibility milestone, it is a core requirement.

### Assistant — summary

Agreed and explicitly made the live, pixel-accurate magnifier a **release-blocking functional requirement**, not an optional proof of concept. Required live pixels tracking the pointer, configurable radius, marked center pixel, scaling and multi-monitor support, focus preservation, confirmation and cancellation. Recommended preserving the Rust/Python split. The initial implementation plan (revision 1.0) included a mandatory first Phase 1A for live magnification and pixel selection, plus later tray, history, settings, shell, mixer, palette editor, and theme generator stages. It warned that portal permissions may temporarily focus KDE-owned dialogs and that unsupported compositor behavior must be tested on a real Plasma Wayland session. It included acceptance checks for fractional scaling, pixel accuracy, cancellation, no idle capture, and no persistent focus shift.

---

## Exchange 6 — Six architectural decisions finalized

### User

Here are my decisions for the color picker and palette app:
1. Target desktop environments: KDE Plasma 6 Wayland only (recommended initially)
2. Magnifier capture behavior: Live underlying desktop, updating with cursor movement (recommended)
3. Focus preservation: Keep keyboard focus on the previous application during picking; permission dialogs excepted
4. History palette window behavior: Centered, frameless popup that closes on click-away / Escape (recommended)
5. Confirm tray menu: Pick Color / Show Palette / Open Board / Configure / Quit
6. Confirm technology choices: Allow a small C++ KWin component if technically necessary

### Assistant — summary

Ratified all six as final requirements. Confirmed **Rust picker + Python/PySide6 Board**, with a **small C++ KWin component permitted only where necessary**. Reiterated the non-negotiable live magnifier, focus preservation during picking except permission dialogs, and the process split with only the Rust tray resident. Explained that native KWin C++ extensions have KWin version/ABI risks and should be tightly scoped. Produced a Markdown implementation plan at `colorboard-agent-implementation-plan.md`, including phases, tests, D-Bus and SQLite integration, and ready-to-copy Phase 1A instructions. The agent must not move to tray or Python development until the live picker satisfies the acceptance tests.

---

## Exchange 7 — All Rust versus Rust + Python

### User

Please look over the plan one more time, & answer: Would it be simpler and easier to implement as a full Rust application, or do you recommend sticking with Rust, Python, and a small C++ Kwin component if needed?

### Assistant — summary

Recommended **retaining Rust for the resident picker and Python/PySide6 for the on-demand editor**, while acknowledging a full-Rust application as viable. Explained that the hardest requirement—the KDE Wayland live magnifier—is common to both approaches; a pure Rust rewrite would not eliminate compositor restrictions. Python Qt's `QGraphicsScene`/`QGraphicsView`, bindings, and easy runtime plugin loading simplify a complex WYSIWYG editor. An all-Rust alternative would use **Slint** for the GUI, `palette` for color calculations, `rusqlite`, `minijinja`, and declarative plugins with optional Rhai scripting. Maintained that both architectures can achieve similar idle footprint if the editor completely exits when closed. Proposed four improvements: data-driven previews; Rust-owned database migrations; a strictly optional narrowly scoped KWin C++ component; and no resident Python interpreter or service.

---

## Exchange 8 — Apply recommended architectural changes

### User

Please implement your recommended changes to the existing plan.

### Assistant — summary

Updated the implementation plan to **revision 1.1** with the four agreed improvements:

1. A common data-driven theme preview renderer based on declarative scene/color-role/state definitions, with Python hooks only for complex logic.
2. Rust exclusively owns SQLite schema upgrades, including a one-shot migration utility for independent Board startup.
3. C++ KWin integration remains an optional backend, isolated by IPC and version compatibility constraints.
4. No persistent Python runtime: the main Board and plugin machinery launch on demand and fully exit.

The phased development order and mandatory magnifier/focus behavior remained unchanged. The updated Markdown file was linked as `colorboard-agent-implementation-plan.md`.

---

## Exchange 9 — C++ versus full Rust versus hybrid

### User

Please look over the plan one more time, & answer: Would it be simpler and easier to implement as a full C++ application, a full Rust application, or do you recommend sticking with the current architecture?

### Assistant — summary

Changed the recommendation to **full C++20 with Qt 6 and KDE Frameworks 6**, if starting from scratch. The main reasons were direct KDE APIs (`KGlobalAccel`, KDE color schemes), mature native Qt WYSIWYG infrastructure, a unified model/toolchain, and fewer language boundaries. Placed Rust + Python second and full Rust third for this KDE-specific GUI-intensive project. Retained **a low-memory resident tray process separated from transient picker/popup/editor processes**, even under a unified language. Suggested reusable declarative theme scenes, Jinja-compatible C++ template engines, optional scripting, and shared C++ data/storage code. Noted that a C++ KWin plugin still carries ABI/version risk, and that no language bypasses Wayland restrictions. Exact Python/Jinja2 plugin support remained an advantage of the hybrid architecture.

---

## Exchange 10 — External plugin requirement and QML KWin effects

### User

In section 4, you said:

> This would make new theme plugins primarily configuration files, rather than compiled C++ modules. You wouldn't need to rebuild Colorboard every time you added support for another application.

This is the functionality I want no matter the language. If that's not already the case, please update the plan accordingly.

In section 5 you said:

> KDE documents QML/JavaScript effects as the preferred distribution mechanism for third-party KWin effects,

Could this method be used with a full Rust implementation rather than needing C++?

### Assistant — summary

Strengthened the spec: **third-party theme plugins must be external, independently installable packages** containing versioned manifests, semantic role definitions, interactive scene descriptions, assets, and Jinja-style templates. New target applications must be available without rebuilding or reinstalling the host; plugins must support runtime discovery/reloading. Optional code hooks are not needed for standard plugins, and the declarative format should remain language-independent.

Explained that KWin's packaged **QML/JavaScript effects can be used by a Rust application** because KWin loads the effect independently of the invoking application's implementation language. Such effects can display compositor-hosted visuals and handle input, but the public QML API doesn't obviously expose arbitrary framebuffer pixel reads; live magnification still needs pixel data. Thus a QML effect might avoid a custom compiled C++ extension but could not yet be considered proven for the mandatory magnifier. Revision **1.2** of the plan added the external plugin guarantee and evaluated a QML effect before compiled native fallback.

---

## Exchange 11 — Further investigation of Rust plus QML KWin effects

### User

Please look further into your recommendation of a Rust application paired with a packaged QML for the KWin question.

### Assistant — summary

Investigated KDE/KWin source and concluded that while Rust can use a KWin QML effect, **a custom QML effect should not be the preferred magnifier backend**. KWin's `SceneEffect` is designed to control/reconstruct the rendered scene and does not provide a simple overlay plus public arbitrary pixel-read interface. Instead, recommended a **Rust-only picker using existing KWin services**:

- Get authorized live screen frames via the **XDG ScreenCast portal and PipeWire**.
- Request `cursor_mode=4` and use PipeWire `SPA_META_Cursor` metadata for pointer positions; KWin's screencast source implements this metadata and coordinate scaling.
- Display a non-keyboard-focusable, pointer-transparent Wayland **layer-shell** magnifier written in Rust.
- Call KWin's existing `org.kde.kwin.ColorPicker.pick()` D-Bus method for final left-click selection and cancellation.
- Decode the returned Qt color payload in Rust without Qt runtime linkage.

Flagged unverified combined behavior: overlay feedback loops, pointer/selection coordination, fractional-scaling pixel correspondence, and capture authorization prompts. Mentioned ScreenCast permission persistence, testing against installed KWin versions, and that a tiny native KWin extension remains only a last-resort fallback if supported mechanisms cannot satisfy a required behavior. Preserved a hard acceptance gate; no fake or frozen magnifier substitution.

---

## Exchange 12 — Apply Rust/KWin integration recommendation

### User

Please update the plan with your recommendation

### Assistant — summary

Updated the plan to **revision 1.3**: primary architecture is Rust ScreenCast/PipeWire frames, `SPA_META_Cursor`, pointer-transparent nonfocusable Wayland magnifier, and existing KWin `ColorPicker.pick` D-Bus selection. Removed custom QML from the preferred path. Narrow C++ KWin extension is the fallback only if needed. Added integration criteria for live updates, overlay recursion, cursor metadata, multi-monitor scaling, accurate selection, capture shutdown, and focus preservation. Kept Rust/Python main-app split and declarative external theme plugins. The available full **revision 1.3** plan is reproduced in Appendix B.

---

## Exchange 13 — Rewrite plan as Rust-only

### User

Please rewrite the plan as a rust-only implementation

### Assistant — summary

Rewrote the complete plan as **revision 2.0, Rust-only**. Replaced Python/PySide6 Board with an **on-demand Rust `egui`/`eframe` GUI**; moved shared models, settings, storage, and IPC into a Rust Cargo workspace. The resident `ksni`/`zbus` tray remains separate from transient picker, history, configuration, and editor processes. The theme editor uses a data-driven custom scene renderer (egui Painter), `palette` for color science, `rusqlite` for SQLite, external declarative plugin packages, **MiniJinja** exports, and optional restricted **Rhai** logic. No Python, C++, Qt bridge, or authored KWin QML/JavaScript effect is allowed under this version; existing KDE/KWin services are allowed. If an indispensable behavior can't be implemented with Rust-facing supported APIs, development must stop at that release gate rather than silently change languages or drop features.

Preserved the mandatory live magnifier and exact feature set. Restated sequence: finish the Rust picker, then tray/history/configuration, then Board shell, Color Mixer, Palette Editor, Theme Generator, and Ghostty/Zed/KDE plugins. Linked `colorboard-agent-implementation-plan.md` and the preserved `colorboard-agent-implementation-plan-v1.3.md`. The full revision 2.0 text is in Appendix A.

---

## Exchange 14 — Export request

### User

Please export this entire conversation as a markdown file

### Assistant

This Markdown file is the requested export artifact. The historical assistant turns above are **summaries**, not verbatim transcripts; the available implementation plans follow in full. This accuracy limitation is stated at the start of the file.

---



---

# Appendix A — Rust-only implementation plan (revision 2.0)

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



---

# Appendix B — Prior hybrid implementation plan (revision 1.3)

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

