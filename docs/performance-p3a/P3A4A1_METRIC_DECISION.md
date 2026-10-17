# P3-A.4a.1 — metric suitability and operating point

**No reliable primary operating point established. No JVM recommendation change.
P3-B NOT READY and NOT AUTHORIZED.** A single short condition cannot establish a
performance improvement, sensitivity to JVM changes, or statistical equivalence.

| Candidate | Classification | Practical limit |
|---|---|---|
| Application presentation-call intervals | DIAGNOSTIC ONLY | `Other` ETW downstream event proxy; independent Minecraft completeness absent, consumer overflow invalidates affected captures. WGL aggregate recovery is target-specific. |
| Display-estimate intervals | DIAGNOSTIC ONLY where available; UNAVAILABLE elsewhere | Missing estimates and attribution uncertainty; not physical scanout. Conditional available-only samples have selection bias. |
| Presentation-call throughput | DIAGNOSTIC ONLY | Row count can omit events, and capped cadence censors maximum throughput. |
| High-tail application intervals | DIAGNOSTIC ONLY | A long row gap can be consumer loss, scheduling or real workload delay; cannot label every gap Minecraft stutter/GC. Short windows have weak tail depth. |
| Available GPU timing | DIAGNOSTIC ONLY | ETW attribution rather than independent GPU timestamp/optical validation. Missing and signed fields preserved separately. |
| CPU/memory | SECONDARY context for retained process-counter observations; DIAGNOSTIC ONLY for comparative effect | Exact child identity and sampled counters support usage observations, but failed observer bound and background drift prevent causal performance claims. RSS is not Java heap occupancy or GC evidence. |
| Physical visible FPS / scanout | UNAVAILABLE | No optical/display measurement. Desktop refresh and ETW estimates are not interchangeable with submission FPS. |

Mathematical cap intervals: 240→4.166667 ms, 500→2 ms, 750→1.333333 ms,
1000→1 ms. Recorded interval errors are errors of the observed ETW channel, not
independently proven limiter error. Faster/slower intervals can reflect frame
work, scheduler wake-up variation, downstream timestamp location, missed events
and observer interference. Do not attribute every deviation to PARK.

A cap frequently reached removes maximum-throughput headroom and can conceal the
JVM differences this project intends to measure. Regular pacing can still be
useful for future interruption/stability studies after completeness and overhead
validation, but is not enough to select a maximum-throughput benchmark. No cap
is promoted solely for attaining 1000 FPS or producing a dense CSV. No sensitivity
experiment or JVM preset/heap/collector sweep occurred in this phase.

Remaining P3-A.4b work, subject to separate authorization:

1. Validate a supported Minecraft event source or justify target-specific
   completeness with independent evidence; do not inject renderer hooks here.
2. Resolve unique event association and downstream timestamp error at high rate.
   Evaluate buffer capacity explicitly, including its own resource cost, without
   erasing default-tool failures or assuming aggregate recovery proves accuracy.
3. Establish observer bounds under stable background conditions using unchanged
   budgets and adequate evidence; controlled recovery failed again.
4. Validate capped sensitivity/headroom for intended JVM questions, and choose a
   metric whose completeness and cost pass. Retain optical limitations.
5. Complete a canonical representative B lock separately; this diagnostic copy
   preserves A-scene provenance but is not a final B workload declaration.
6. Resolve VRR/thermal/background uncertainties and full measured-run readiness,
   then obtain explicit P3-B authorization. Passing this phase would not grant it.

Current phase is diagnostic only; all primary decisions remain unaccepted.
