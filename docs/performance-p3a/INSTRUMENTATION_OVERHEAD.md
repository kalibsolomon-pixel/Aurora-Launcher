# P3-A.4a representative-rate overhead — PARTIAL

## P3-A.4a.1 current interference gate

PARTIAL. The counterbalanced 500-Hz recovery passes state means but both2 versus
its mirrored control differs by −1.062884 machine CPU percentage points, exceeding
the unchanged absolute 1pp budget. Off/off drift +0.812591pp limits attribution;
no outlier removal or zero-cost claim. Independent matching also remains uncertain.
Minecraft observer on/off escalation was not performed. See
[recovery details](P3A4A1_OVERHEAD_RECOVERY.md). Historical evidence below remains
intact; prior consumer-overflowed PresentMon captures now classify INVALID.


[Predeclared protocol](P3A4A_DIAGNOSTIC_PROTOCOL.md), [focused results](P3A4A_OVERHEAD_RESULTS.md),
[actual evidence](evidence/p3a4a/overhead.json). One 80-second WGL child achieved
1789.54–1791.75 independent swaps/sec in mirrored none/PresentMon/sampler/both
blocks. Compared with controls, block-mean rate changes are -0.01438% / -0.02112%
/ -0.10506%; P99 changes +0.003353 / -0.002911 / -0.001343 ms; machine CPU changes
+0.03975 / +0.11185 / +0.80431 points.

These means fit the numerical operational budget, but both-2 exceeds its mirrored
control by 1.306 machine-CPU points, failing the declared 1-point individual screen.
Two blocks/state provide no statistical equivalence bound. Independent marker
retention and endpoint reads exist in every control; observer startup/elapsed excess,
CPU/RSS and sampler read latency are retained with quantization/attribution limits.

More decisively, high-rate ETW rows fail independent completeness: long row holes
occur while controlled swaps continue. PresentMon process exit 0 and observed zero
ETW session counters cannot repair that limitation. The prerequisite for Minecraft
observer-on/off trials failed, so none were performed. No favorable retry, discarded
block or negligible-overhead conclusion. P3-B remains NOT READY / NOT AUTHORIZED.
Historical 120-Hz observations follow unchanged.

# P3-A.3 instrumentation overhead — bounded observations, overall PARTIAL

[Actual counter/marker evidence](evidence/p3a3/overhead.json). Two separate 80-second
native WGL series used fixed 120 Hz and the same long-lived child within each
series: off / on / on / off, four 20-second blocks, predeclared inner 3–17 seconds.
One series enabled only PresentMon; the other enabled only the independent 0.5 s
CPU/memory sampler. This is controlled instrumentation calibration, never a JVM
configuration comparison. Every successful observer/stimulus exited normally.

| On minus off block means | PresentMon only | Sampler only |
| --- | --- | --- |
| Independent presentation-call rate | -0.00513 Hz | -0.00855 Hz |
| Independent P99 interval | +0.087754 ms | -0.021433 ms |
| Stimulus CPU, percentage of one logical core | +2.7343 points | -1.8415 points |
| Whole-machine busy CPU | +0.25175 points | +0.01964 points |
| Observer sampled RSS | 13.93–14.07 MB | 16.91–17.13 MB |
| Observed observer CPU, one logical core | 0–0.22321% | reported 0% at counter resolution |
| Process creation plus identity query | 4.34–8.08 ms | 5.05–5.48 ms |
| Total observer wall time beyond requested 18 s | 26.5–48.8 ms | 69.4–73.3 ms |

The sampler retained 37 samples per on-block; maximum observed read durations
were 1.3632 ms and 0.5553 ms. These are read-latency observations, not total CPU cost.

The predeclared operational screens were <=2% mean cadence difference, <=2 ms
P99 difference and <=2 percentage points machine CPU difference. Both completed
series pass these **observed** screens. They are not statistical equivalence
bounds or thresholds for promoting JVM treatments. Off-block background CPU drifted
substantially (PresentMon series 1.971% to 0.690%; sampler 1.663% to 0.902%).
Two blocks per state in one series cannot isolate every source of drift or bound
small performance effects. Negative CPU differences do not establish a benefit.

GetProcessTimes can advance in roughly 15.625 ms quanta; a reported zero over a
14-second window does not establish zero cost. Marker retention makes stimulus
RSS grow in every block; that growth is not observer memory cost. ETW work can also
be charged outside the observer process, so its own CPU alone is not total overhead.
The wall-time excess combines startup, timer behavior and shutdown; separate
startup/shutdown CPU costs are not established. Timing variance and all long
intervals remain in the evidence. No significance test is used to claim negligible
interference.

The initial PresentMon series received a normal window-close event at about 43 s.
Its complete partial markers/metadata and first block results remain preserved;
the retry used a new directory. An earlier quiet-system sampling attempt was
interrupted and retained, then replaced by separately named complete observations.
No failed sample was silently overwritten or pooled into the completed series.

These experiments bound only the observed 120-Hz stimulus conditions. Uncapped
Minecraft emits far more presentation calls and has many unavailable ETW display
estimates. Its instrumentation interference and display-tail fidelity are not
bounded by these series. **Overall calibration remains PARTIAL**, despite passing
the limited operational screens. Full measured-run requirements are unchanged;
P3-B remains NOT READY / NOT AUTHORIZED.
