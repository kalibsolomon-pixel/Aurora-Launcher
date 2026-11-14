# Instance Content navigation

A Content button follows General in the selected instance's section row. It
opens the existing Content page on Mods with the editor's selected instance.
The button emits an explicit instance ID; the launcher reuses its guarded page
navigation and Content's existing target-loading path. The action does not save
configuration, change the active launch instance, or renumber the editing sections.
It is disabled while the editor is busy or its selected instance is unavailable.

Native UI Automation verified the first Content construction for Creative Builds,
then retargeting the retained Content view to Skyblock Classic and Survival Plus.
Each destination's Instance combobox value was checked. Survival Plus remained
the active launch instance while the other editor instances were browsed. The
button was also activated at the 1084x752 outer window size; the section row wraps
and remains reachable. The compact General form has a pre-existing narrow-column
wrapping issue visible in the minimum-size capture; this navigation change does
not modify that form layout.

[Instance controls](instances-final.png), [Creative Builds content](creative-content.png),
[Skyblock Classic content](skyblock-classic-content.png),
[Survival Plus content](survival-plus-content.png), and
[compact controls](instances-minimum.png) retain the verification evidence.

Native debug build, Windows boot, all 385 tests, formatting and whitespace checks
passed. No production data, content changes or installation actions were used.
