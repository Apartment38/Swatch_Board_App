# Board undo and redo shortcuts

The Board uses Qt’s standard Undo and Redo bindings in the active Mixer, Palette, or Theme editor. The shortcuts and toolbar controls reflect the editor’s available history. Editable text retains its native keyboard undo and redo.

In the Mixer smoke, focus on Apply allowed Ctrl+Z to restore `#01020342` from `#04050642`, and Ctrl+Shift+Z restored `#04050642`. Focus in the Color value field routed the same keys to the unsubmitted text. The smoke also exercised palette swatch changes, theme role changes, and shortcut hints.

The PNGs are QWidget captures from the Board running with Qt 6.8.2 and Breeze in a disposable offscreen session. The integrated Board and Settings control smokes completed without failures. The workspace build, formatting check, and all-target Rust test suite passed, with 77 tests passed and one existing ignored test.
