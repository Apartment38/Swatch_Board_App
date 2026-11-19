# KDE polish: compact Theme editing, round 2

Builder: gpt-6.1-terra. Independent rendered critic: gpt-6.1-sol, without source or implementation-report access.

Theme role editing now precedes project setup. A visible Show preview toggle preserves access to the scene while the preview initially collapses in short windows. The compact Theme view limits the palette shelf's height to leave an editable role, its palette link, and Save visible. The normal view keeps the preview visible until the user chooses otherwise.

Verification: workspace build and isolated Breeze Qt smoke passed. New checks cover a complete role and Save inside the compact viewport, preview expansion/collapse, and bounded Tab-key traversal to Export. The bottom screenshot shows the export controls. The lead also launched the actual X11 desktop window, clicked Show preview, observed the scene collapse and controls expand, saved the attached desktop capture, and closed the disposable fixture normally through its unsaved-change prompt. These checks do not qualify Plasma/Wayland behavior.

Critic: **scoped editing-layout win, conditional on project-controls discoverability**. The complete compact role and normal view's three roles improve substantially over the original. Remaining blocker: Plugin/Saved/New/Open/name controls now sit below the role list without a clear route. The next round must expose those project controls directly without shrinking the compact editor again. Blank history space and inefficient normal-width role spacing remain separate polish opportunities.

Baseline provenance correction: the initial `target/kde-smoke` capture raced a candidate build and is not an original baseline. An exact archive of `7fc121e059df67598c71f278fb8cf92f12ec59b4` was subsequently built in `/workspace/swatch-baseline-target`, with source verified against Git. Its actual Breeze captures are `/workspace/swatch-baseline-capture/images`; its smoke fails only the existing 540×360 size assertion. Candidate evidence in this directory is from the verified round-2 build.
