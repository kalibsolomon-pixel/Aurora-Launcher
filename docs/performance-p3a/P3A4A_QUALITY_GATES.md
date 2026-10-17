# Short measurement fidelity and future prerequisites

The measured schema-1 importer, source requirements, quality thresholds and
300/600-second durations are unchanged. This phase does not make a short capture
a complete measured run or choose a benchmark winner.

`presentation_fidelity.py` reads the same strict short manifest through the shared
input boundary. It checks pinned tool/format, hashes, plain paths, manifest fields,
bounded zoned lifetime, process/stream/runtime, integer monotonic QPC, positive
frequency, units/span, exported interval consistency, inner sample depth and edges.
Diagnostics retain strict redirected UTF-8/UTF-16 decoding. An interrupted capture,
inadequate boundaries or confirmed ETW loss is INCOMPLETE. Invalid identity, units,
bytes or fields fail. It refuses a physical-display/rendered-frame claim.
An optional separate retained-handle/marker identity record must match PID and
creation ID. Without it, independent creation-identity verification is explicitly
false; the CSV cannot prove a process creation time by itself.

Application and display channels are reviewed separately. All-NA display exports
can still contain an offline-consistent submission stream. Display metrics are null
if insufficient or unordered, with an explicit reason; numerical zero and missing
values remain distinct. Signed animation diagnostics remain signed. Every present
row stays in the hashed input; no gap filtering or old-capture overwrite occurs.

**VALID WITH WARNINGS is only offline consistency.** Missing application events can
look like long intervals. An importer cannot prove capture completeness from the
CSV alone. Unknown ETW counters stay UNKNOWN; observed zero only covers snapshots
through the window, not the unobserved shutdown tail, producer event coverage or
internal consumer loss. Independent marker reconciliation is a separate gate.

## Metric decisions for this environment

| Question / metric | Current classification | Gate before stronger use |
| --- | --- | --- |
| Uncapped application submission pacing, TimeInQPC deltas | DIAGNOSTIC ONLY | High-rate independent completeness and timestamp-error bound, then observer bound |
| Capped submission pacing | SECONDARY | Controlled lower-rate prior validation supports coarse pacing only; future condition-specific tails need a declared error budget |
| Available display-estimate intervals | DIAGNOSTIC ONLY | Condition-specific missingness/coverage and independent validation; uncapped primary use rejected |
| GPU timing/completion attribution | DIAGNOSTIC ONLY | Independent GPU timing validation, scheduling state and event coverage |
| Physical displayed frames/smoothness | UNAVAILABLE | Separately reviewed measurement apparatus/instrumentation |
| Exact render-loop/API-entry frame times | UNAVAILABLE | Reviewed independent game/API timestamps; no injection this phase |
| Exact-child CPU/RSS | SECONDARY | Counter/sample coverage and overhead; not GC CPU or Java heap |

Independent controlled swaps continue across multi-hundred-millisecond and even
1.87-second holes in the ETW row stream. Minecraft's uncapped export also has
roughly half-second row gaps, overlapping the display-estimate gaps. Neither
uncapped channel's tails can currently be called application stutters.
[Actual gap review](evidence/p3a4a/gap-review.json).

There is **no accepted PRIMARY for uncapped JVM performance comparisons**. A
submission metric may ultimately become primary for submission pacing only; that
would not validate displayed smoothness. PresentMon rows per second currently
measure the observed ETW stream, not a complete render count or optical FPS.

QPC tick quantization and display-latency formatting resolution are known. ETW-to-
API timing error at high rate, clock drift, GPU completion error and scanout error
are not bounded. Do not substitute QPC resolution for those uncertainties or use
UTC for fine timing. Game GC alignment remains unavailable. Preserve long gaps
as observations without naming them physical stutters.

P3-A.4b prerequisites: isolate the high-rate row/marker discrepancy, calibrate
timestamp fidelity with a uniquely matchable independent reference and sufficient
sample depth, bound observer interference with more independent counterbalanced
trials against an owner-reviewed effect budget, document the unresolved display/
VRR/thermal/scheduling context, and review any proposed new instrumentation before
installation. GC comparability and B/C world/gameplay readiness remain separate
future gates. P3-B still needs separate authorization regardless of calibration.
