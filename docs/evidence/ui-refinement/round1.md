# History typography round 1

Captured real Swatch Board history and KColorChooser side by side in isolated Plasma 6.3.5 / KWin Wayland, Breeze, 1800×1000, scale 1, 96 DPI. History uses synthetic D-Bus entries; capture is the real executable, not a mock UI. Titles are visible, so critiques are independent but not fully blind.

Baseline critic preferred KColorChooser: tiny bitmap typography, oversized selection, unclear mouse action.

Round 1 replaces bitmap text with antialiased installed font resolved by fontconfig (bitmap fallback if no supported font is found), retains readable selected text, and explains click-to-copy. Fresh critic preferred history for recent-color selection but KColorChooser for native fit. Largest remaining gap: popup frame and shadow.

Validation: history build, existing history unit test and diff whitespace check pass. Physical screen picking is not covered by this fixture.
