# Toggle knob alignment

The shared AuroraToggle now centers its thumb through flex layout inside the
bordered track. Fixed absolute offsets had put the knob too low. Track dimensions,
thumb size, hit targets, colors, accessibility and controlled state are preserved.

[Before](before.png) and [after](after.png) show Sync's master switch and five
participating-instance switches at 125% scaling. Pixel measurements of all six
switches change from 5px above / 3px below to 4px above / 4px below. Both on and
off states are covered. Native UI Automation also exercised a participant toggle
and Revert draft against disposable showcase data; no Sync Apply was performed.

Native debug build, all 385 tests, formatting and whitespace checks passed.
Earlier frozen review captures were not overwritten.
