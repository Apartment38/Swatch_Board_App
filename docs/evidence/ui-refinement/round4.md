# History selection round 4

Real history captures use the same isolated Plasma 6.3.5 / KWin Wayland session, synthetic fixture, 1800×1000, scale 1 and 96 DPI as round 3. Selected tiles add a one-pixel line in the palette's contrasting Selection foreground inside their accent edge. The fixture was navigated with the keyboard to capture cyan in light mode, and white and near-black in dark mode. Build and whitespace verification pass.

A fresh critic accepted the selection change: the full tile outline remains visible for cyan, white and near-black, and the recent-color task remains clear. The dark captures retain the light KColorChooser reference and demonstrate dark variant behavior rather than dark-reference parity.

The user's additional benchmark is KDE's built-in Color Picker:
https://github.com/KDE/kdeplasma-addons/tree/master/applets/colorpicker

Upstream contents/ui/main.qml specifies three columns, square cells, a Theme.textColor swatch border, centered labels over a theme-background strip, and PlasmaExtras.Highlight selection. Current history still has four default columns, rectangular swatches and left-aligned labels. The critic identified square tile composition and a centered, comfortably padded code strip as the next gap against this benchmark; these captures do not claim parity with it. The existing upstream reference was read to document its actual behavior.
