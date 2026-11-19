# KDE polish: Mixer channel clarity, round 4

Builder: gpt-6.1-terra. Fresh source-blind rendered critic: gpt-6.1-sol.

The HSV and RGBA spinboxes now have individual component labels. Hue shows degrees; saturation and value show percentages. HSV display uses one decimal place, while the original precision is retained for untouched channels. Leaving an unchanged field does not alter the color.

Verification: workspace build, workspace all-target tests, Breeze Board smoke, and whitespace check passed. New native control assertions cover unchanged HSV editing completion and a fractional hue edit preserving the exact untouched saturation/value channels. Normal and compact captures use a fresh isolated profile; the compact image is deliberately scrolled to the numeric controls. An earlier reused-profile capture had restored compact geometry into both views and was rejected by the critic, then replaced with the attached independently inspected evidence.

Critic verdict: **accept, clear scoped win**. Individual labels, concise precision, units, and legibility at both sizes meet the relevant native KDE reference bar. The critic noted the RGBA 0–255 scale is not explicit beside each integer field, a minor nonblocking opportunity. The visual review does not independently establish interaction behavior; that evidence comes from the control smoke.
