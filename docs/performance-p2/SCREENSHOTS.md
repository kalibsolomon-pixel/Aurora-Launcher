# Actual native screenshot inventory

All images below show the compiled frontend in the isolated Windows Tauri host,
not browser viewport fixtures. [First geometry/results](native/first-verification.json)
and [restart results](native/restart-verification.json) include client measurements,
DPR, executable SHA-256, exact retained configuration and test provenance.

| Capture | Purpose |
| --- | --- |
| [first-default.png](native/first-default.png) | Existing Configuration, 3217 MiB, default native client |
| [unsaved-efficient.png](native/unsaved-efficient.png) | Saved versus unsaved selection; no silent heap change |
| [custom-mode.png](native/custom-mode.png) | Saved Custom mode and native memory advice |
| [advanced-jvm.png](native/advanced-jvm.png) | Accessible retained raw custom arguments |
| [native-save-error.png](native/native-save-error.png) | Real rejected native Save; actionable persistent message and editable draft |
| [native-1024x768.png](native/native-1024x768.png) | Measured 1024×768 CSS client |
| [native-minimum.png](native/native-minimum.png) | Enforced minimum track, actual 706×512 CSS client |
| [native-minimum-text200.png](native/native-minimum-text200.png) | 200% text, mode and heap reachable by scrolling |
| [native-minimum-heap-text200.png](native/native-minimum-heap-text200.png) | Maximum-heap control at 200% text |
| [native-minimum-advanced-text200.png](native/native-minimum-advanced-text200.png) | Advanced text at 200%, native minimum |
| [native-rose-accent.png](native/native-rose-accent.png) | Real isolated native Rose preference |
| [restart-default.png](native/restart-default.png) | Restarted native host |
| [restart-custom.png](native/restart-custom.png) | Persisted Custom with exact prior heap and text |

These screenshots are configuration acceptance, not a real Minecraft launch or
performance benchmark. The synthetic record has no installed Minecraft and no
accounts. Native resizing is scoped to the exact isolated process, with no browser
viewport override. Full browser/regression PNGs remain locally retained under the
new browser/regressions folders; their JSON summaries are separately identified
as synthetic fixtures. Their hashes/paths appear in the evidence inventory.
