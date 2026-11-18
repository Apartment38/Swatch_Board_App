# KDE polish: Settings error recovery, round 6

Builder: gpt-6.1-terra. Fresh source-blind rendered critic: gpt-6.1-sol.

Invalid stored-history values now identify the visible field and show an accurate correction immediately beneath it. A rejected Save focuses and scrolls to that field while retaining the draft. Zero and overflowing input have distinct guidance. Tray/application failures have concise main messages, with original diagnostics available through native tooltips and accessible descriptions. Message details are cleared when their associated operation changes.

Verification: build, formatting, whitespace check, Settings native smoke, and the expanded Settings Rust test passed. Backend checks cover zero, overflow, corrected disk persistence, and stale-detail clearing. The lead additionally verified the real X11 window at exactly 540×360: entered zero, scrolled down to Save, submitted, observed the field regain focus and scroll into view, corrected it to 102 using only the keyboard, and saved. Disk readback confirmed `history_limit = 102` in the isolated fixture. The corrected status distinguished saved settings from the unavailable tray; hovering shortcut status displayed the original connection diagnostic. The window closed normally.

Critic verdict: **accepted for scoped native recovery**. The field error, focus, compact visibility, positive retry, saved state, and separate accessible diagnostic close the observed recovery gaps. The form's existing native styling remains intact. This is isolated Qt/Breeze X11 evidence, not physical Plasma/Wayland or screen-reader qualification.
