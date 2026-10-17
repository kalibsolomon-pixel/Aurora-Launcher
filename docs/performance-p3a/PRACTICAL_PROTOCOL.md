# Practical Performance Protocol — fresh-process supplement P3-A.4a.4

The [bounded trial plan](P3A4A4_TRIAL_PLAN.md) replaces the earlier within-process
validation sequence for this phase only: A–B–C–C–B–A, two fresh processes per
observer condition, equal fresh world/configuration copies, >=300s stationary
warm-up and one30s primary resource window per process. Historical within-session
variation is context, not this workload's restart sensitivity. Retain warm-up
resource series separately from the six primary windows.

Classify each metric family independently: practical frame-loop screening, GC/heap
diagnostics, CPU and RSS may have different readiness. Two repeats establish only
an observed range. Effects comparable to repeated-control or within-condition drift
remain unresolved; do not subtract an unrelated observer/control pair. A sufficiently
large repeatable effect may be screenable even when smaller effects are not.
No universal percentage threshold or statistical detection guarantee is implied.

Controls have no active CSV recorder, so active/inactive internal timing overhead
cannot be measured directly with Overlay itself. JFR remains optional for a primary
pacing cohort when its resource effect is unresolved. Matching observers between
future baseline/candidate treatments is mandatory. Preserve all strict-research
contracts below; authorization is still a separate owner decision.

# Practical Performance Protocol v1 — P3-A.4a.3 (historical sequence)

This is a separate evidence class from the **Strict Research Protocol**. Historical
reports, schema-1 measured-run importer, completeness requirements, 2% quiet screen
and strict observer budgets remain unchanged. A practical result never upgrades a
historical failed capture. P3-B still requires separate owner authorization.

## Purpose and scope

Screen one reversible JVM change for repeatable, user-relevant improvement. Use
Performance Overlay 1.0.2 eligible HUD-callback intervals as internal frame-loop
diagnostics, never physical display intervals or proof of every rendered frame.
JFR supplies separate aggregate JVM context. Precise GC/frame causality requires
demonstrated clock alignment and is unavailable with this CSV.

Keep canonical A archive/options immutable. B's canonical specification requires
normal terrain and its own complete identity/runtime audit. The previously
compatible B mod stack on a fresh, inventory-equal **A scene copy** is explicitly
`workload-b-a-scene-diagnostic-v1`, a stationary screening workload; it is not
canonical representative B. No inference to traversal, chunk generation, combat,
servers or heavily modded gameplay. Record all world/options/mod/runtime hashes.

## Bounded validation and baseline plan (declared before capture)

Use exact isolated launcher, native create/provider install/readiness/play and
Microsoft OpenJDK 21.0.7+6-LTS. Rehash Overlay to
`46c9500f67824996cd81b00a4d3da2dc09d5c6d56495cff98276c961d74804c2`.
Reuse the conservative privacy-filtered default JFC from P3-A.4a.2. No new VM
tuning, forced GC, exhaustive allocation profiling or production mutations.

1920x1080 windowed; native Unlimited (260 sentinel), VSync off, inactivity Minimized;
Aurora global cap OFF and adaptive sleeping false; render/simulation distances
12/8. Retain actual options/configuration and physical client evidence. Unknown
framebuffer/VRR/thermal facts stay unknown. Hold focus/camera/HUD state constant.

1. One <=30-second Overlay pilot: start, bounded settling interval outside the
   accepted measurement, stop, inspect raw CSV and successful footer. One JFR-only
   <=30-second validation before combined recording. These are tool validation.
2. After at least 300 seconds in-world warm-up, take three 30-second Overlay-only
   baseline recordings when integrity/environment permit. Same process/scene is
   a within-session baseline, not restart or cross-day reproducibility. Log actual
   key/OS times and use CSV elapsed time for the recorded duration.
3. Bounded observer subset: inactive Overlay, JFR with Overlay inactive, active
   Overlay, combined. Use 30-second CPU/RSS windows, an inactive control before
   and after, and at most one repeat per state if needed to resolve a named issue.
   No-mod timing is unavailable in this subset; do not call inactive zero cost.
4. Preserve every attempt. Stop after two recorder failures, corrupt export,
   compatibility failure or unexplained substantial interference. No favorable
   retries, unrelated-process termination, power/priority/affinity changes.

## Environmental attribution and practical gate

The existing Reader uses Windows GetSystemTimes: total busy = kernel + user - idle;
total capacity = kernel + user. Deltas include **all processes**, including game,
launcher and observers. The quiet scripts opened their own Python PID but gated
on the separate global counters. Their process selection never filtered total CPU.
2.979357% was title-screen total machine utilization, not measured unrelated load.
Retain its strict failure, without inferring a background culprit.

For each practical window sample at 0.5 seconds, retaining exact game PID/creation
identity, launcher PID and sampler PID with read-only handles. Calculate each
process share against the same machine-capacity denominator. Report total machine
CPU, game CPU, launcher and sampler CPU, and residual = total minus those measured
shares. Residual includes WebView, attach tools, OS/kernel and unattributed work;
it is an **estimate with attribution uncertainty**, not certified unrelated CPU.
Report one-core-equivalent utilization separately where useful (28 logical CPUs
on this host; hybrid threads are not equal performance units).

Use weighted endpoint deltas for window means and distribution/range of interval
values for changes. Half-second values are scheduler/accounting-noisy, especially
for low-cost processes; reject reversed counters and never clamp negative residual
into a reassuring zero. Synchronized reads are sequential, so small residual
errors are expected. Processor groups >64 remain unsupported by this Reader.

No universal total-CPU ceiling: expected game cost is the workload. Review residual
time series, before/after controls and memory together. A sustained new residual
step, competing GPU work, low-memory/paging pressure, changed display/power/focus
or known maintenance makes comparisons INCONCLUSIVE (INVALID for a known capture
failure). Stable ordinary residual work is tolerable. Unknown GPU/thermal/frequency
telemetry prevents a strong environmental-equivalence claim, not raw export tests.
If unexplained variation approaches the proposed benefit, no candidate verdict.
Available RAM must exceed heap +4096 MiB; record AC and read-only display/driver/
power facts. Do not build a new hardware monitor to close unsupported telemetry.

## Recording integrity and metrics

Require pinned source identity, complete export, positive finite plausible
frame_ms, nondecreasing elapsed_ms (ties are legal), timestamp/interval rounding
coherence, matching exported/summary counts and successful EndReason. Detect
writer abort via log/footer, inspect all outputs, retain raw hashes. Missing
footer/window/rows is a failed capture, not an artificial stutter. Missing HUD
callbacks and queue high-water remain unknown even with coherent counts.

Recompute type-7 median/P95/P99, maximum, count/rate of intervals >50 ms and
throughput = count / sum(intervals). Never average instantaneous FPS. Reconcile
producer summary using **its own** percentile/low definitions and rounding; report
differences rather than silently replacing them. 0.001-ms CSV precision and
integer-ms elapsed precision limit reconciliation. Do not interpret sampled
MXBean GC deltas as individual pauses.

JFR: explicit isolated repository and output, named bounded disk recording,
maxage/maxsize exceeding the window, normal stop/export, installed jfr summary/
print reader. Inspect ActiveRecording/ActiveSetting, duration, loss and retention.
Use top-level GCPhasePause only for pause sums/distribution; nested levels would
double-count. Heap summaries are before/after GC, allocations/execution are sampled.
Zero eligible events is not proof of zero work; no forced collection to fill data.

## Observer and comparison decisions

Crashes/export failures or substantial disturbance reject a configuration. Stable
measured overhead may be acceptable when identical in both treatments. Unmeasured
timing interference remains unknown. Without a reference, CPU/RSS and runtime
checks cannot prove negligible frame-time overhead. Variable observer effects or
environment drift as large as the intended benefit yield INCONCLUSIVE. No invented
percentage budget replaces the preserved strict observer limits.

Report each run and baseline min/max/range relative to median. Three runs only
establish observed variation, not a population confidence bound. A first experiment
uses at least three matched baseline/candidate pairs with counterbalanced order,
equivalent warm-up/settings/world and equal observer state. A promising effect must
repeat, exceed ordinary baseline and paired drift, matter for pacing/resources,
and survive stability/compatibility tests. Higher average FPS never conceals worse
severe tails or memory pressure.

Classifications: **PROMISING**, **INCONCLUSIVE**, **REGRESSION**, **NO MATERIAL CHANGE**,
**INVALID**. No material change requires sensitivity sufficient to rule out the
predeclared practical benefit; failure to detect significance alone is inconclusive.
Synthetic parser tests never constitute performance evidence.

Gates: A = coherent Overlay/JFR plus adequate observer review; B = three comparable
runs and useful sensitivity; C = one exact hypothesis, matched conditions and
reversal/regression rules; D = separately authorized candidate acceptance. Recommend
P3-B only after A–C pass. This protocol alone grants no authorization.

References: [Windows system counters](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes),
[Java 21 diagnostic recording commands](https://docs.oracle.com/en/java/javase/21/docs/specs/man/jcmd.html),
[installed-reader interface](https://docs.oracle.com/en/java/javase/21/docs/specs/man/jfr.html).
