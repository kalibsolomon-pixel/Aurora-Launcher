# Existing collector evaluation — predeclared bounded protocol

Scope: P3-A.4a.2 only. No JVM tuning, product/client changes, candidate fork,
custom mod, full-run schema change or P3-B. Starting branch/HEAD must match the
assignment. Preserve original evidence, frozen A and all seven worktrees.

Static audit uses publisher metadata, verified released JARs, pinned official
source and read-only bytecode. VERIFIED means the stated static fact was inspected;
it does not imply runtime verification. Developer claims remain documentation only.

Selection: Valence 1.1.0 is unsuitable for the required raw per-frame analysis:
its report builder exports frame aggregates/600-point series, not its raw interval
array. Do not install it merely to demonstrate this limitation. Select Performance
Overlay 1.0.2 for bounded compatibility work. Its eligible HUD-callback intervals
are potentially useful internal pacing observations; they are not physical display
intervals. Per-row allocation, integer-millisecond timestamps, HUD eligibility and
independent observer uncertainty remain explicit gates.

Before significant gameplay captures, observe 60 seconds at 0.5-second cadence,
AC, minimum available RAM >= heap + 4096 MiB, mean background CPU <2%, unchanged
driver/display/power and available GPU/VRR/thermal context. Failed CPU screen defers
further captures, with no favorable retries or unrelated-process termination.
Unknown GPU/VRR/thermals prevent a strong environmental acceptance claim.

Use established marked isolated managed root, separate WebView profile, exact
launcher/runtime, supported authentication/readiness/play and exact child identity.
Same-user credential rotation risk and approved startup-refresh sequencing remain.
Owner handles any interactive sign-in. A archive/options are immutable; only new
disposable copies may be used. B retains exact Aurora 3.0.0/API/Sodium/Lithium pins.

Initial compatibility is distinct from performance: normal startup/world entry,
one manual bounded recording, normal stop/output/exit. Preserve failures; no
Aurora patch or Sodium downgrade. Use 1920x1080 windowed, stationary canonical A
scene copy, distances 12/8, VSync off, Minimized inactivity, same JVM vector and
unchanged Aurora PARK configuration. Verify native Unlimited and adaptive=false.

If execution/environment gates permit, first test JFR alone on the no-collector B
copy using the exact installed default.jfc, then collector inactive/active and
collector+JFR; cap subset 240 and 1000 only. Short captures <=30 seconds, 300-second
stationary warm-up before performance interpretation, no full 600-second runs.
Predeclare paired mirrored states where feasible; preserve all attempts/deferred
conditions. Installation changes require normal exit/restart and disposable files.

Unchanged overhead budgets: absolute rate <=1%, P99 <=0.05 ms, machine CPU <=1pp
for individual pairs, means and off/off drift. Two blocks cannot establish
equivalence. Without a separately validated timing reference in observer-off
conditions, rate/tail overhead is UNKNOWN; do not derive zero overhead from the
collector's own timing or plausible FPS. JFR alone precedes combined tests.

JFR: standard exact-PID jcmd JFR.start/check/dump/stop, named disk recording,
default profile with five privacy-only event exclusions, bounded maxsize and age,
path beneath new private evidence root. Use the installed `jfr configure` to
disable JVMInformation, InitialSystemProperty, InitialEnvironmentVariable,
InitialSecurityProperty and SystemProcess. Compare all event settings to prove
these are the only differences; retain both profile hashes. Default otherwise
records potentially sensitive launch arguments and environment/process values.
No allocation-stack intensification, changed JVM startup vector or leaked
command-line/system-property events. Review installed JFC event selection before
recording; retain raw privately and publish sanitized counts/hashes only.

Integrity: retain raw bytes/SHA-256, format/version/workload/runtime/mod identities,
producer success and complete footer. Distinguish elapsed_ms ties from duplicate
rows; frame_ms has 0.001-ms formatting precision, elapsed_ms is truncated to integer
ms. Require nondecreasing elapsed timestamps, positive finite intervals and bounded
rounding consistency; no invented absolute nanosecond anchors. Missing footer,
queue overflow, summary truncation or incomplete windows block primary promotion.
Warm-up/window boundaries need external evidence. JFR epoch timestamps cannot be
subtracted from System.nanoTime; without explicit bounded paired anchors, precise
GC/frame attribution is UNSUPPORTED. Aggregate JFR analysis may remain useful.

PresentMon is optional external diagnostics only. Existing capture failures remain
in history. No successful PresentMon prerequisite for candidate testing.
