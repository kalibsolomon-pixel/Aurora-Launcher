# Timestamp, completeness and observer decisions

No accepted PRIMARY; P3-B NOT READY / NOT AUTHORIZED.

System.nanoTime has an arbitrary origin, useful for elapsed differences, not
epoch subtraction. Short differences avoid long wraparound periods; nanosecond
units do not guarantee nanosecond precision. Valence retains long durations but
does not export the chronological frame array. Overlay exports rounded frame_ms
and truncated elapsed_ms, without benchmarkStartNs or a bounded epoch relationship.
Tied milliseconds/identical quantized rows cannot establish duplication or a
zero-duration frame. A missing callback can produce a long interval; its cause
cannot be called a game stall without independent evidence.

Overlay reset/first callback, pause policy, HUD eligibility, nonpositive-delta
skips, world exit and recording stop all affect the window. Source inspection
describes these paths; runtime boundaries/coverage/monotonicity/overflow remain
UNTESTED. Completion footer and row count catch export/summary inconsistencies,
not all suppressed HUD callbacks. No explicit raw sequence or queue watermark.

JFR events expose start/end instants and durations through supported readers,
but no accurate paired anchor exists to align this CSV. Local Date is second
precision and unzoned, sampled separately before benchmarkStartNs. Reconstructing
a relative timeline from rounded intervals accumulates uncertainty and still lacks
an epoch anchor. Precise frame-spike/GC attribution is UNSUPPORTED here; aggregate
JFR GC analysis can be considered after actual recording/coverage validation.

The new offline [collector_review.py](../../tools/benchmark/collector_review.py)
is explicitly separate from measured schema 1. It accepts only the pinned 1.0.2
CSV plus reviewed source/workload/hash/unit/window metadata. It checks format,
timestamps/interval rounding, footer counts, declared overflow/completion and
bounded windows. It preserves null missingness/alignment/observer evidence and
always labels metrics DIAGNOSTIC ONLY. Sidecar declarations are not authenticated
producer facts. Ties and indistinguishable rows are reported, not repaired.

Thirteen synthetic regressions cover missing timestamps, units, tied/duplicate
ambiguity, overflow, missing source metadata, wrong workload, unsupported format,
incomplete window/footer/summary, invalid JFR alignment, promotion, nonfinite/reversed
data. They are format/gate tests, not empirical measurements. No actual CSV was
acquired after the quiet failure, so actual-format validation remains outstanding.

Unchanged overhead screens: absolute rate <=1%, P99 <=0.05 ms, machine CPU <=1pp
on individual paired blocks, state means and off/off drift. Planned mirrored
conditions were no benchmark mod, installed inactive, active, JFR alone, combined.
Only startup/overlay interaction and JFR availability were exercised; all timing
conditions were deferred. There is no overhead estimate, zero-cost claim, A/A
equivalence bound or measured confidence interval.

Render-path record allocation/growth and writer lock/contention are plausible
interference mechanisms; static inspection does not quantify them. JFR's default
sampling and threshold costs also need on/off testing. A collector cannot certify
its own interference. An independent validated timing reference is still needed
to bound rate/P99 interference; exact-child endpoint CPU/RSS may separately help
bound resource cost. Two blocks/state or a reached cap do not establish equivalence
or uncapped JVM sensitivity. PresentMon can cross-check only when its independent
quality gates pass; it is not a mandatory source or prerequisite for recording.
