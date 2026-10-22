# 001: Escape cancellation without overlay keyboard focus

## Question / acceptance

**Given** a Rust-owned pointer selection and a keyboard-noninteractive overlay,
**when** the user presses bare Escape during an active pick,
**then** a compositor-delivered `GlobalShortcuts.Activated` event cancels the pick,
without focusing the overlay, and closing the session releases its active binding.

This standalone spike probes only the cancellation transport. It creates no overlay,
selects no color, injects no keys, and does not change the production selection backend.
It is not a Phase 1A pass, a nofocus/clickthrough pass, or live Escape evidence.

## Findings from the actual APIs and KDE Plasma/6.8 source

### Is bare Escape permitted?

The [XDG shortcut grammar](https://specifications.freedesktop.org/shortcuts-spec/latest/)
allows a key identifier without modifiers (its example is `a`). Identifiers are
xkbcommon keysyms without `XKB_KEY_`; the requested spelling is `Escape`, not Qt's
human-readable `Esc`. The specification does not prohibit bare Escape.

[KDE's Plasma/6.8 XdgShortcut::parse](https://invent.kde.org/plasma/xdg-desktop-portal-kde/-/raw/Plasma/6.8/src/xdgshortcut.cpp)
starts with no modifiers, resolves the final identifier using
`xkb_keysym_from_name(..., XKB_KEYSYM_CASE_INSENSITIVE)`, then converts to a Qt key.
There is no Escape-specific rejection in that parser.

[KDE BindShortcuts and ShortcutsModel](https://invent.kde.org/plasma/xdg-desktop-portal-kde/-/raw/Plasma/6.8/src/globalshortcuts.cpp)
parse the preferred trigger, check global/standard/internal conflicts, and pass accepted
assignments to KGlobalAccel. The [dialog](https://invent.kde.org/plasma/xdg-desktop-portal-kde/-/raw/Plasma/6.8/src/GlobalShortcutsDialog.qml)
disables OK for global/internal conflicts; standard shortcut conflicts are warnings.
This is source-level feasibility, **not proof that this compositor will bind or deliver
bare Escape**. The user may edit the assignment, a conflicting binding may block it,
and compositor/input-grab interactions still need live testing.

The portal returns only `trigger_description` (localized display text), not a machine-readable
effective trigger. This CLI prints that exact string and fails closed unless it is `Esc`
or `Escape`. A localized equivalent therefore needs investigation, not a claim that Escape
was forbidden. No modified-key fallback is requested or accepted.

### Does Session.Close stop activations?

The [portal contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)
binds shortcuts to the session. In KDE [Session::close](https://invent.kde.org/plasma/xdg-desktop-portal-kde/-/raw/Plasma/6.8/src/session.cpp),
the backend emits Closed, unregisters the session object and schedules deletion.
`GlobalShortcutsSession` owns its QActions in `unique_ptr`s and has a default destructor.
[KGlobalAccel's destruction callback](https://invent.kde.org/frameworks/kglobalaccel/-/raw/master/src/kglobalaccel.cpp)
marks ordinary actions inactive; only `_k_session:` actions are fully unregistered.
[KGlobalAccelD GlobalShortcut::setInactive](https://invent.kde.org/plasma/kglobalacceld/-/raw/Plasma/6.8/src/globalshortcut.cpp)
unregisters active keys.

Consequently closing should end this session's callbacks and active key grabs, **but does
not mean deleting remembered shortcut configuration**. Deletion and daemon calls are
asynchronous: a successful Close reply alone is not a measured no-late-activation proof.
The Frameworks citation is master, not a pinned installed-framework revision. Live
post-close release and any multi-session/same-app interactions remain untested.

### Is a permission dialog required on every bind?

Not universally. XDG says BindShortcuts typically presents a dialog, and allows only one
bind attempt per session. KDE's implementation loads remembered actions at creation;
BindShortcuts compares requested IDs with remembered IDs:

- Any new IDs: shows the registration/configuration dialog.
- Returning IDs only: `setActions` and success without showing that dialog.
- Rejection of new IDs stores them with empty assignments; subsequent runs may therefore
  return an unassigned shortcut rather than asking again. Reconfiguration is explicit.
- Saved user assignments win through KGlobalAccel Autoloading; a preferred trigger is not
  a command to override the user's choice.

**Important CLI identity caveat:** KDE `componentName()` uses the portal-provided app ID,
or `token_<session-token>` if app ID is empty ([header](https://invent.kde.org/plasma/xdg-desktop-portal-kde/-/raw/Plasma/6.8/src/globalshortcuts.h)).
A raw unsandboxed example may have no stable app identity and thus prompt each new session.
This spike does not install desktop files or spoof identity. Production needs a legitimate,
stable application identity and observed rebind behavior before promising one-time setup.
There is no restore token in these ashpd GlobalShortcuts options.

### Can setup be separate from picking and keep nofocus?

Architecturally yes: explicit user-authorized setup can show the portal dialog **outside**
a pick. Global activation is independent of application window focus; the Rust overlay
need not acquire keyboard focus. This does not make the setup dialog itself nofocus.
If the acceptance gate requires absolutely no focus changes during picking, never show
that dialog inside the gated pick interval.

A retained live session means bare Escape stays globally registered even while idle;
ignoring callbacks while idle does not prove Escape passes through to other apps.
A close-after-pick lifecycle releases active registration but needs a new session next time,
with the identity/prompt caveat above. Test setup/rebind and release before selecting a
production lifecycle. Neither global binding nor a successful probe proves overlay
clickthrough, authoritative color sampling, pointer-layer interaction, or autonomous cleanup
of the full picker.

## Actual cached Rust API used

Read `ashpd-0.13.13/src/desktop/global_shortcuts.rs` and `desktop/session.rs` before implementation:

- `GlobalShortcuts::new().await?`, then synchronous `version()`.
- `create_session(Default::default()).await?`.
- `receive_activated().await?` **before** binding.
- `NewShortcut::new("cancel-native-pick", description).preferred_trigger(Some("Escape"))`.
- `bind_shortcuts(&session, &[shortcut], None, Default::default()).await?.response()?`.
- `Shortcut::{id, trigger_description}`, `Activated::{session_handle, shortcut_id}`.
- `Session::close().await` on success, timeout, denial, empty/changed assignment,
  subscription failure, and all other returned paths after successful session creation.

`Session::path()` is private; the probe obtains the path via its public `Serialize`/D-Bus
object-path representation using `zbus::zvariant::to_bytes` and deserialization. The session
Debug output also contains only that path. No restore secrets/options are printed.
No Drop cleanup is assumed. Process kill/panic cannot execute the explicit returned-path
cleanup; portal client-disconnection handling is a separate fallback, not tested here.

## Commands

Run from the repository root:

```sh
cargo check --offline -p swatch-board-picker --example phase1a_cancel_probe
cargo run --offline -p swatch-board-picker --example phase1a_cancel_probe -- --inspect
# No argument also means inspect, never authorization:
cargo run --offline -p swatch-board-picker --example phase1a_cancel_probe
```

**Only a human/parent with explicit permission should run this next command. It was NOT
run during this spike.** It can show a dialog and persist shortcut configuration:

```sh
cargo run --offline -p swatch-board-picker --example phase1a_cancel_probe -- --authorize --duration 30
```

Authorize the bare Escape assignment manually if appropriate, then press Escape manually
while another application remains focused. Record returned actual trigger, matching
`observed_shortcut_id=cancel-native-pick`, session handle, and `session_close=ok`.
Activation wait is 1–30 seconds, default 30, starting **after** the bind response; time
spent by the user in the permission dialog is not included. CLI argument parsing happens
before portal access. No keyboard injection or authorization automation is provided.

## Executed evidence

- `cargo check --offline -p swatch-board-picker --example phase1a_cancel_probe`: passed.
- `cargo run --offline -p swatch-board-picker --example phase1a_cancel_probe -- --inspect`: passed:

```text
global_shortcuts_version=2
mode=inspect; no session created; no authorization requested
```

- No-argument invocation also passed with the same inspect-only output.
- `target/debug/examples/phase1a_cancel_probe --duration 31` exited 1 with
  `probe_error=--duration must be 1..=30 seconds`, before any portal version output.

An initial compile revealed that `Value::new(&Session)` is not supported; replaced with
the supported serialization path above. No live session, bind, permission dialog, or
keyboard activation was requested by the investigator.

## Verdict: PARTIAL

**What worked:** cached ashpd API verified, Rust example compiles offline, read-only
inspection observes portal version 2, and Plasma/6.8 source accepts the requested syntax.

**What remains:** real bare-Escape binding/activation, overlay nofocus/clickthrough under
pointer selection, release/no late callbacks after Close, stable app identity, repeated
bind without prompting, and rejection/timeout cleanup under live conditions.

**Recommendation:** preserve this as an explicit opt-in investigation. Do not promote
native selection or claim Escape works until live evidence exists. If Escape is rejected,
returned unassigned, or changed, report that exact returned condition; do not substitute a
different shortcut or focus the overlay to hide the failure.
