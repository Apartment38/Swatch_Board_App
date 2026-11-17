# KDE polish: consolidated status bar, round 5

Builder: gpt-6.1-terra. Fresh source-blind rendered critic: gpt-6.1-sol.

Connection state now shares the bottom row with the existing wrapping board message. The toolbar sits directly above the workspace instead of reserving a full extra band for connection state. Existing connection/loading/picking text and picker enablement behavior are unchanged; both labels have accessible names.

Verification: workspace build, fresh-profile Breeze Board smoke (all prior checks), and whitespace check passed. The critic inspected actual normal and compact Mixer and Theme captures against the original and official Dolphin/Kate screenshots.

Critic verdict: **accept / win**. Connection state and the existing message remain fully legible at both widths, toolbar content is intact, and the reclaimed space improves the work area. No scoped correction is needed; a status-bar border was not considered necessary.
