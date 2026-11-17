# KDE polish: Theme layout, round 1

Starting revision: `7fc121e059df67598c71f278fb8cf92f12ec59b4`, freshly pulled from `origin/work`.

Builder: gpt-6.1-terra, fresh context. Independent critic: gpt-6.1-sol, fresh context, viewed rendered screenshots and official KDE reference screenshots without inspecting source or the builder's implementation report.

The scene selector now sits beside the preview. A font-relative preview height cap and 1:3 preview/editor stretch allocation give the theme controls more vertical space. No model or persistence behavior changed.

Verification: workspace build, isolated native Qt control smoke with KDE platform integration and Breeze style/icons, and `git diff --check` passed. The attached PNGs are actual QWidget captures from the application, not mockups. They do not establish physical Plasma/Wayland acceptance.

Critic verdict: **needs another round**. At 540×360 only Plugin and Saved fit in the editor viewport; editable roles, theme name, and Save remain below the fold. At 900×700 only the first roles are visible while whitespace surrounds the preview. The critic recommends a collapsible preview or side-by-side allocation, with primary editing controls given priority. A compact screenshot with a complete role and scroll/keyboard evidence is required before acceptance.

References inspected: official screenshots linked from the user's KColorChooser, Dolphin, Kate, Spectacle, Plasma System Monitor, and ghostwriter KDE application pages.
