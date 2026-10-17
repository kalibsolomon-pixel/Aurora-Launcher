# P3-A.4a predeclared diagnostic protocol

Measurement validation only. No P3-B, tuning or production changes. Frozen before
new traces. Preserve every attempt and original raw bytes, even on gate failure.

## Environment and preservation

Record repository/index/worktrees/original files, prior captures and production
binary/config/registry hashes privately. Recheck canonical A snapshot/restoration,
archive, options and scene. Read-only power, CPU/RAM, display/driver and available
GPU/thermal/VRR context precede each group. Unknown sensor state remains unknown.
No unrelated application termination, system setting or priority changes.

## Controlled representative-rate overhead

One 80-second WGL child, requested 1800 Hz, swap interval requested zero; actual
markers determine achieved rate. Eight 10-second blocks: none, PresentMon only,
sampler only, both, both, sampler only, PresentMon only, none. Inner 2–8 seconds
per block. Observers request 9 seconds; sampler interval 0.5 seconds. Retained
process handles provide endpoint CPU/RSS/system counters in every block, including
controls; therefore 'none' means neither sustained observer, not zero observation.
Every swap has independent before/after QPC markers. Retained markers themselves
have unbounded overhead relative to an uninstrumented app and are equal in all blocks.

Hypothetical future minimum useful effect: 5% submission-rate change or 0.25 ms
P99 change. Calibration budget: absolute on/off rate difference <=1%, P99 difference
<=0.05 ms, machine busy CPU difference <=1 percentage point. All observed pair
differences must fit, and off/off background drift must fit the same budget.
These are conservative operational budgets, not promotion criteria or proof of
equivalence. Two mirrored blocks/state cannot establish a confidence bound.
If sampling error/background drift prevents a defensible bound, overall PARTIAL.
Report P50/P95/P99/P99.9 (only with sufficient depth), max, endpoint CPU/RSS,
observer spawn/elapsed excess and sample read latency. ETW queries use unique owned
sessions and distinguish unknown from observed zero through the inner window.

Proceed to Minecraft observer-on/off experiments only if the controlled evidence
is reliable: achieved rate 1500–2200 Hz, complete markers, matching stream/identity,
no interior row/marker ambiguity or missingness, and interference/drift screens fit.
Otherwise preserve the result and defer Minecraft overhead; no favorable retries.

## Minecraft presentation-path matrix

One normal authenticated isolated A session, fresh copy of verified restoration,
exact Java, Balanced/2048 MiB/empty custom text. Keep stationary scene/camera and
Minimized inactivity limiting. At least 300 seconds stationary warm-up. Predeclare
three 16-second traces, inner 4–14 seconds: Unlimited/VSync off, cap 120/VSync off,
cap 240/VSync off (only if actual installed UI supports each). A bounded VSync-on
condition is unnecessary unless these fail to discriminate availability behavior.
No player motion. Options only through disposable game UI; restore Unlimited/VSync
off and verify canonical option bytes after each capped diagnostic. Verify live
framebuffer/camera before/after; normal save/exit, post-exit scene check. Frozen
world/archive/options are never written. Saved live-copy world bytes may change
through normal game saving; require scene equivalence, not live-copy byte identity.

Retain all columns and summarize stream/mode/flags, app timestamps, missing display
fields, available estimate intervals, GPU fields, raw hashes, exact-child/focus,
ETW counters and boundaries. Application timestamps are ETW submission proxies,
not API-entry, rendered completion or optical display measurements. A failed
display-quality gate must not invalidate an independently complete submission stream.
No physical-display claim is authorized by CSV consistency alone.
