# Discover search button inset

The search frame is 40 logical pixels tall and uses 4px trailing padding around
the 30px Search button. With the 1px border, the interior gap is 4px above,
below and to the right. The leading search icon retains 12px padding.
Previously the 34px frame left the button cramped vertically while applying
12px padding to its right.

[Before](before.png), [after](after.png) and [minimum-size](after-minimum.png)
native captures verify spacing in the wide and wrapped toolbar layouts. The
Search Modrinth button was activated through native UI Automation and returned
results. Input state, submit handlers, focus styling and search behavior are
preserved. Native debug build, all 385 tests, formatting and whitespace checks
passed. Earlier review captures were not overwritten.
