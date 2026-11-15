# History palette round 3

Real history executable captured beside KColorChooser in isolated Plasma 6.3.5 / KWin Wayland, 1800×1000, scale 1, 96 DPI, using synthetic D-Bus history entries. The light capture uses the Breeze Light fallback with no kdeglobals. The dark capture resolves BreezeDark from an isolated XDG_CONFIG_HOME containing General/ColorScheme=BreezeDark. KColorChooser remains light in both captures, so the dark image demonstrates variant support rather than a matching dark-reference comparison.

History now cascades XDG system/user kdeglobals, resolves named schemes in XDG data locations, and applies Window, View and Selection foreground/background roles. A separate critic accepted the light and dark variants and preferred history for its recent-color task. The remaining visual gap is selection clarity, particularly when the swatch matches the selection accent.

Validation: history build and both unit tests pass; whitespace check passes. Right moves selection to #1CDC9A in the real dark popup and Enter copies fixture id 1 (zero-based) and exits. This fixture does not validate physical screen picking.
