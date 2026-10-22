# Native application integration — partial acceptance

**Phase 1A remains NOT PASSED. `--native` is opt-in; default KWin path unchanged.**

## Independently reproduced integration checks

After the integration handoff, the parent ran the real release application in the
private KWin lab, not merely the prototype example:

```sh
target/release/swatch-board-picker --native --radius 5 --zoom 12 --diagnostics
```

Before subsequent output-routing development, full offline locked all-target
workspace tests passed: **99 test executions**, including reused capture/portal
tests in examples. This is a verification snapshot, not a claim that later
in-progress edits have already passed the same full gate.

### Integrated Escape lifecycle — passed in this scenario

- The actual native application registered its own identity, captured Virtual-0,
  bound bare Escape through GlobalShortcuts, and displayed a blue magnifier with
  genuine per-commit Presented events.
- First Escape cancelled native selection, removed both shields and the lens,
  and explicitly closed GlobalShortcuts and ScreenCast sessions.
- The fixture remained alive. A second Escape reached it and produced its normal
  Escape shutdown, demonstrating release of the global binding in this run.
- During active picking, mouse counts remained 1 press / 1 release; keyboard
  enter/leave counts remained 3/2 from the post-authorization baseline. Consent
  dialogs occur before this baseline and are not being misreported as zero focus
  changes throughout authorization.

See [Escape result](evidence/native-integrated/native-integrated-escape-result.json),
[application trace](evidence/native-integrated/native-integrated-2.log), and fixture trace.

### Integrated selection plus self-occlusion recovery — passed in this scenario

- Blue source `(512,227)` was magnified and genuinely presented.
- Moving to `(606,227)` entered the old lens footprint. A cursor-free independent
  screenshot there initially showed blue lens pixels, while the uncovered fixture
  target was white.
- The application submitted a transparent mapped hide, received genuine hide
  presentation, and required new VIDEO PTS beyond the local receipt cutoff.
- A new off-source white lens received Presented. An independent screenshot of
  the uncovered source was RGB `(255,255,255)`.
- Left release printed exactly **`#FFFFFF`** on stdout. Diagnostics remained on
  stderr. GlobalShortcuts and ScreenCast sessions closed explicitly.
- Mouse counts stayed 1/1 and keyboard enter/leave stayed 2/1 during picking.

See [selection result](evidence/native-integrated/native-integrated-select-result.json),
[application trace](evidence/native-integrated/native-select.log), exact
[stdout](evidence/native-integrated/native-select.stdout), and before/after PNGs.
The fixture was later reused during separate output-matrix preparation; acceptance
JSON records counters at the end of the relevant native test, before that later
fixture focus change.

## Packaging and isolation

The application desktop entry is a project packaging artifact:
`share/applications/org.apartment38.SwatchBoard.desktop`. No host entry was installed.
The private lab copied that entry into its own XDG data directory and used an
absolute Exec pointing to the real project release binary. Its original uninstalled
relative executable was not resolvable on the lab PATH, and app registration
correctly failed before capture; pointing the private entry to the actual binary
allowed registration. Normal packaging must install the binary and desktop entry
consistently. Do not infer that the uninstalled host CLI needs no setup.

Only private consent dialogs were automated. The initial private bare-Escape
approval was visually inspected before activation; a later session reused the
private application's stored assignment. No real desktop shortcut settings,
permissions, or display settings were altered by these integrated tests.

## Outstanding release gates

- Multi-authorized-output routing and transition barriers; a dedicated worker is
  implementing this and the parent will run the matrix after handoff.
- Fractional-scale, negative-origin and transformed-output live coverage.
- Animated-content selection versus the genuinely presented lens commit.
- Failure cleanup with buttons held: bounded exit is not proof of click safety.
- Full combined cancellation/failure/resource-cleanup matrix after final edits.
- Final native UI/readout/performance and default-backend decision. No automatic
  promotion to default or Phase 1B has occurred.

A fullscreen-output option has been added to the native fixture to prepare the
matrix. Its parser was observed failing then passing; its example test, strict
Clippy and release build passed. An actual `--fullscreen-output Virtual-1` run
configured 1024×768 fullscreen on the second virtual output and observed pointer
local `(450,150)` after global `(1474,150)`. This validates fixture placement,
**not** native multi-output sampling yet.
