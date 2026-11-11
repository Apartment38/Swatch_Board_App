# Data-only theme plugins (API 1)

Plugins describe mock previews and exported configuration; they do not execute
code, apply themes, or reproduce a target application's rendering engine exactly.
Ghostty, Zed, and KDE under `plugins/` are complete examples of the same contract.

## Install and reload

Copy a package directory to
`${XDG_DATA_HOME:-$HOME/.local/share}/swatch-board/plugins/<directory>/`.
System roots are each absolute entry in `${XDG_DATA_DIRS:-/usr/local/share:/usr/share}`
with `/swatch-board/plugins` appended. Installed resources are also searched
relative to the executable prefix. Debug builds additionally search this checkout's
`plugins/` directory. Do not install a second copy of a bundled ID: duplicate IDs
are rejected, not silently overridden.

Open Theme Generator and click **Reload Plugins**. Discovery reads manifest
metadata; selecting a package loads its roles, scenes, templates, and assets.
An unavailable or version-mismatched selected plugin leaves the project retained
with an error rather than converting or discarding it. Restore the package and
reload to recover. Changing a plugin's version is not an automatic project migration.

Package layout:

```text
example-terminal/
  manifest.json
  roles.json
  scenes.json
  templates/theme.j2
  tests/sample-palette.json
  tests/expected-theme.txt
```

The `tests/` files are author-maintained golden fixtures, not executable hooks.
Published JSON Schemas are in `schemas/`; runtime validation additionally checks
cross-references, supported versions, resource budgets, and filesystem safety.

## Minimal package

`manifest.json`:

```json
{
  "id": "example-terminal",
  "name": "Example Terminal",
  "version": "1.0",
  "api_version": 1,
  "target": "Example Terminal",
  "target_versions": ["1"],
  "outputs": [
    {"path": "example.conf", "template": "templates/theme.j2", "format": "text"}
  ]
}
```

IDs start with a lowercase ASCII letter, contain only lowercase ASCII letters,
digits, `_` or `-`, and are at most 128 bytes. Manifest fields are required unless
shown as optional in the schema. API 1 rejects unknown record fields. Supported
output format names are `text`, `json`, `ghostty`, and `kde`; `target` is metadata,
not a command or parser selector. At least one nonempty target version is required.

`roles.json`:

```json
[
  {"id":"background", "title":"Background", "default":"#202020"},
  {"id":"accent", "title":"Accent", "default":"#81A1C1",
   "help":"Menu and tab highlight", "group":"Controls", "links":["Accent"]}
]
```

All API 1 roles are colors. Defaults and resolved assignments use `#RRGGBB` or
`#RRGGBBAA`. `alpha` defaults to false: nonopaque assignments are rejected for
opaque roles, and an explicit `FF` suffix is normalized away. `help`, `group`,
and `links` are optional. Empty `links` allows any named palette swatch; a
nonempty list restricts the inspector's available swatch names.

`scenes.json`:

```json
[
  {"id":"main", "title":"Main", "width":400, "height":200, "nodes":[
    {"id":"canvas", "kind":"background", "rect":[0,0,400,200], "role":"background"},
    {"id":"open", "kind":"tab", "rect":[20,20,140,36],
     "role":"accent", "text":"Open submenu", "target":"submenu"}
  ]},
  {"id":"submenu", "title":"Submenu", "width":400, "height":200, "nodes":[
    {"id":"canvas", "kind":"background", "rect":[0,0,400,200], "role":"background"},
    {"id":"back", "kind":"tab", "rect":[20,20,140,36],
     "role":"accent", "text":"Back", "target":"main"}
  ]}
]
```

The first scene is the initial view. Node array order is paint stacking order.
`rect` is `[x,y,width,height]` in logical scene coordinates; the renderer scales
the scene to the preview column. Node IDs are unique within a scene, scene IDs
and role IDs within their respective package collections. Role references and
transition targets must exist.

Supported kinds are `rect`, `window`, `dialog`, `list`, `tab`, `text`, `syntax`,
`icon`, `image`, `background`, and `border`. These are shared painter primitives,
not native target widgets. `role` binds the fill (or foreground for text/syntax),
`text_role` binds label text, and `border_role` binds the outline. `text`,
`font_size` (default 14), `target`, and `asset` are optional. `enabled:false`
disables node interaction. A `target` changes the active scene on click; this is
how menus and initially hidden dialogs are described without host code. A node
with an editable role can be selected or receive a dragged color.

Assets must be referenced PNG files under `assets/`. SVG, external URLs, custom
font execution/loading, scripts, native libraries, and shell commands are not
supported by this API.

`templates/theme.j2`:

```jinja
background = {{ roles.background | hex }}
accent = {{ roles.accent | hex_no_hash }}
```

## Template and output contract

| Feature | API 1 behavior |
| --- | --- |
| Context | `roles`: validated map from stable role ID to canonical HEX value |
| Undefined values | Strict error, including misspelled role IDs |
| `hex` | Uppercase HEX with `#`, preserving a permitted alpha suffix |
| `hex_no_hash` | HEX without `#` |
| `rgb_csv` | Decimal `r,g,b`; alpha is omitted |
| Engine | Rust MiniJinja; not the Python Jinja2 extension ecosystem |
| Package includes | Only templates declared in manifest outputs are registered |
| Execution | Fuel-limited; no filesystem/network/process hooks supplied |
| Trailing newline | Preserved |

`json` output must parse as JSON. `ghostty` and `kde` validation checks basic
key/value structure; KDE entries must follow a section header. These checks are
not full target-application schema validation. Maintain golden files and verify
against the supported application's documentation. `text` output must be nonempty.

Manifest files are limited to 64 KiB, role/scene/template files to 1 MiB each,
referenced PNG files to 4 MiB, and each rendered output to 4 MiB. PNG dimensions
are at most 2048 by 2048 with at most 16 MiB of decoded data. API 1 allows at most
32 outputs, 1024 roles, 256 scenes, and 4096 nodes per scene. Templates have a
100,000-unit fuel budget. These are implementation ceilings, not recommended
package sizes; keep previews small.

**Current security limitation:** fuel and the output writer do not bound
intermediate MiniJinja allocations (for example, string multiplication inside a
capture block). Rendering currently runs inside Board, so a hostile template can
exhaust its memory. Until resource-limited rendering or a sufficiently restricted
language is implemented, use only templates you have reviewed and trust. The
untrusted-third-party-package hardening gate is not passed.

Paths must be safe relative paths, with no empty, `.` or `..` components,
backslashes, or absolute paths. Template paths start with `templates/` and end
in `.j2`. Referenced file/directory components must not be symlinks. Package
root ancestry is checked too, so placing a plugin below a symlinked XDG path
is not supported.

## Projects, links, and safe export

Save the source palette before linking roles to named swatches. Links follow
the active saved palette only when its ID matches the project's source palette.
They resolve by name rather than position, so reordering is harmless; removing
or renaming a referenced swatch leaves an error to correct. Direct overrides
are independent of palette changes. Defaults return to package-defined values.
Project edits are undoable and the current draft is recoverable across restart;
named projects are stored separately in the shared SQLite database.

Choose an existing export directory, click **Preview export**, inspect filenames
and content, then **Export previewed files**. Existing files require explicit
overwrite confirmation. Exported subdirectories must already exist. Symlink and
nonregular-file targets are rejected. Files are staged and individually renamed
atomically, not committed as a multi-file transaction: an error after an earlier
rename may leave some outputs written. Linked palette changes invalidate stale
content at export time and require another preview. Export never applies a theme.

## Acceptance checklist for a new package

1. Build Board before authoring the new package, then install the package data.
2. Reload and select it without rebuilding or changing Rust source.
3. Open a hidden scene through a preview control, and return.
4. Select and edit a role; drop history and palette colors onto eligible nodes
   and inspector swatches. Check undo/redo and unresolved-link diagnostics.
5. Save/reopen the project and recover its draft after a clean restart.
6. Preview/export into isolated storage, inspect actual contents, and verify
   refusal to overwrite until confirmation is supplied.
7. Record headless egui checks separately from physical KDE desktop results;
   an input-driven unit test is not evidence of native window layout or drag UX.
