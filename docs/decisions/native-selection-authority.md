# Decision: investigate Rust-owned selection with displayed-sample authority

## Status

**User-approved investigation; not a Phase 1A pass.**

After reviewing the real animated-content mismatch and the lack of request-owned
KWin cancellation, the user selected:

> Investigate Rust-owned selection, making the displayed sample authoritative.

This explicitly changes the authority model for a prospective native backend.
The application's selected value must be the sample that was actually presented
in its magnifier, rather than requiring equality with KWin's independent later
scene render. KWin may remain a diagnostic reference for static fixtures; it is
not the final authority for the investigated backend.

## Requirements retained

- All application source remains Rust.
- A real live magnifier is mandatory: no frozen screenshot or fabricated preview.
- The selected sample must come from actual captured desktop pixels and the
  compositor-presented magnifier state, not an unpresented queued candidate.
- No normal picker window activation or keyboard focus stealing.
- Left-click confirmation, right-click/Escape cancellation, no mouse click-through.
- Autonomous cleanup: no orphaned input shield, cursor, capture or portal session
  on success, cancellation or failure.
- Required multi-output, fractional-scale/rotation and dynamic-content live tests.
- No tray or Board work before the updated Phase 1A gate passes.

## Risks to prove before integration

| Probe | Given / when / then | Status |
|---|---|---|
| Escape without keyboard focus | Given an explicitly authorized global cancel shortcut, when Escape is pressed while another app keeps focus, then native selection cancels and the shortcut session is released | Unvalidated |
| Pointer ownership | Given a transparent non-keyboard-interactive input shield, when left/right buttons are released, then the shield handles the action and the underlying fixture receives no mouse press/release | Unvalidated |
| Cursor-surface exclusion | Given cursor-metadata capture, when a test cursor contains visible sentinel pixels, then captured video below those pixels remains the underlying fixture rather than the cursor image | Unvalidated |
| Presented cursor sample | Given a cursor surface with updated pixels, when the compositor reports presentation, then only that sample may become selectable | Unvalidated |

The cursor-surface approach is a supported-protocol hypothesis: a magnifier drawn
as the actual pointer image may avoid normal-overlay capture feedback because
cursor-metadata capture excludes the cursor image from video. It is not yet a
claim about KWin's actual implementation. The sentinel image is a test fixture,
not a fake application magnifier.

The GlobalShortcuts authorization/setup UI, if required, must be separate from an
active pick. No new focus-stealing exception has been granted. Permission dialogs
must never be automated, and denial/conflicts must fail safely without entering
native selection. Synthetic global Escape is not production cancellation.

## Implementation boundary

Experiments live under `spikes/` and compile as explicit Cargo examples. They do
not replace the existing picker until feasibility and fresh native acceptance
checks pass. The existing KWin-backed implementation remains fail-closed and its
previous gate failure remains documented in `../phase-1a-live-validation.md`.
