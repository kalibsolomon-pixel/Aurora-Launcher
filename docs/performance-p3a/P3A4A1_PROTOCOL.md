# P3-A.4a.1 predeclared diagnostics

No JVM, product or Aurora source changes; no P3-B. Exact installed Aurora 3.0.0
and PresentMon 2.3.1 only. Preserve every attempt, prior evidence and canonical A.
Read-only source/installed-bytecode audit precedes selection of supported caps.

## Gates and controlled trials

Before a significant group: 60-second quiet observation, AC, available RAM
recorded, unchanged display/driver/power, GPU/thermal/VRR context where observable.
Existing quiet CPU screen <=2%; a failed valid observation defers further capture,
without repeat-until-favorable attempts or unrelated-process termination.

First controlled trial: one 80-second WGL child, 20-second phases at 240/500/750/
1000 Hz, one bounded 86-second PresentMon session, 14-second inner windows 3–17 s
within each phase. Independent before/after swap QPC markers remain authoritative.
Run with existing explicit high-rate opt-in. Record raw hashes, modes, timestamps,
all unavailable fields, exit and owned ETW session counter snapshots.
Use existing strict marker reconciliation: at most one boundary-only unmatched
marker, no duplicate assignment and <=5 ms absolute nearest marker error. These
are completeness screens, not a high-rate accuracy certificate. Report interval
error separately; low sample depth, ambiguous assignments or missing spans block
primary promotion. The highest rate passing is only a candidate for observer review.

If default collection fails independent completeness, permit one targeted
two-phase WGL confirmation at the prior failing 1800 Hz: 26 seconds default, then
26 seconds with the same pinned tool's documented
`--no_track_display --no_track_gpu --no_track_input` options. This asks whether
display/GPU attribution is necessary for the missing-record failure. Its reduced
header is a separate submission-only diagnostic, never relabeled as the default
short format or a full run. This reproduces the old high-rate failure and tests a
specific attribution-path hypothesis; it is not a favorable retry. Preserve both
paths and all missing fields. If reduced
tracking improves counts, that alone does not validate its timing or observer cost
and does not silently change the default Minecraft matrix instrumentation.

If a targeted capture reports consumer circular-buffer overflow, preserve that
diagnostic independently from ETW counters. After verifying the same pinned tool's
help/source, one 26-second 1800-Hz full-tracking confirmation may increase only its
process-local present circular buffer to 8192 entries. This tests the reported
failure mechanism. No privilege/global ETW/system change; no silent switch of the
Minecraft/default capture configuration or quality thresholds.

One controlled observer recovery trial at that highest candidate (if any):
80 seconds, eight 10-second blocks none/PresentMon/sampler/both/both/sampler/
PresentMon/none, inner 2–8 s, observers 9 seconds, sampler 0.5 seconds. Same
unchanged P3-A.4a operational budgets: <=1% absolute rate difference, <=0.05 ms
P99 difference, <=1 machine-CPU percentage point, for individual mirrored controls,
block means and off/off drift. Retained markers and endpoint reads exist in controls.
No threshold relaxation. If combined interference or independent completeness
fails, stop observer escalation; no Minecraft observer experiment. If no candidate
passes, omit this recovery trial and report why. Review the prior failed block
without discarding it. Two blocks/state alone do not establish equivalence.

Prospective fallback if all rate candidates fail unique nearest-marker association:
one 500-Hz observer trial may still quantify interference using the independent
swap markers/endpoint counters. Such a trial cannot pass the separate event-
completeness prerequisite or authorize Minecraft observer escalation. This keeps
the unchanged budget test meaningful even when the ETW matching method is ambiguous.
Fresh quiet observation precedes this additional controlled group.

## Minecraft matrix

Fresh A control copy and a separate B diagnostic copy from the verified A
restoration, never writes to the frozen source. B retains the verified four mods.
This deliberately uses the accepted A scene, not a new canonical B world lock.
Copy the known canonical test options into previously unstarted B; record every
mod-introduced option/config difference. Verify normal main-menu/world execution.

One initial 16-second capture/condition, inner 4–14 s, 300 seconds stationary
warm-up after world load, debug overlay off. Keep 1920x1080 windowed, 12/8,
VSync off, Minimized inactivity limiting, locked Java, 2048 MiB, empty JVM text.
Conditions: C1 minimal A native 240; C2 minimal A native Unlimited; A1 B global cap
Off/native Unlimited; A2–A5 B global cap240/500/750/1000/native Unlimited, only if
installed support is verified. B keeps its actual PARK strategy, spin500us,
park2000us, low-CPU false and adaptive sleeping false if confirmed defaults.
If persisted configuration differs, document before deciding any interaction
setting. Among A2–A5 change only global cap; do not tune strategy/compensation.
No direct Java/game launch, renderer instrumentation or injected markers.

Save and hash actual settings before captures, exact PID/creation/focus, all CSV
fields, QPC frequency before/after, boundaries/diagnostics/counters and CPU/RSS.
Restore A canonical options after temporary controls; restore B cap Off afterward.
Normal save/exit, exact-child supervision, full frozen-material and post-exit scene
checks. One normal B restart may validate persisted settings without another capture.

Minecraft has no independently instrumented swap stream. A complete WGL stream
cannot certify Minecraft. Report observed ETW intervals/rates with this limitation,
never optical FPS or proof of no missing game events. A cap frequently reached
censors maximum throughput: no benchmark operating point becomes primary solely
because it looks regular or reaches the highest FPS. No primary promotion without
independent completeness, interference and meaningful uncensored sensitivity.

Only if the controlled observer prerequisite passes may bounded Minecraft observer
validation be considered at at most two candidate B caps. Without an independent
uninstrumented Minecraft timing source, pacing equivalence remains unmeasurable;
do not fabricate it from the PresentMon-off condition or F3 spot readings.
