# Representative-rate instrumentation overhead — PARTIAL

[Frozen protocol](P3A4A_DIAGNOSTIC_PROTOCOL.md),
[actual counter/marker evidence](evidence/p3a4a/overhead.json).
One 80-second native WGL process, requested 1800 Hz, achieved 1789.54–1791.75 Hz
in eight 6-second inner windows. Order: none / PresentMon / sampler / both /
both / sampler / PresentMon / none. 'None' still has equal retained swap markers
and two endpoint counter reads. It is not an uninstrumented absolute baseline.

| State versus no sustained observers, block means | Rate change | P99 interval change | Machine CPU change |
| --- | --- | --- | --- |
| PresentMon only | -0.01438% | +0.003353 ms | +0.03975 points |
| CPU/memory sampler only | -0.02112% | -0.002911 ms | +0.11185 points |
| Both | -0.10506% | -0.001343 ms | +0.80431 points |
| Last control versus first control | +0.00099% | +0.004008 ms | +0.26554 points |

These averages fit the predeclared 1% / 0.05 ms / 1-point operational budget for
a hypothetical future 5% rate or 0.25 ms P99 effect. **The full screen does not
pass**: both-2 machine CPU is 3.743%, versus its mirrored control 2.437%, a
1.306-point difference. Background/OS/ETW work is not isolated by process CPU.
The original mean-only private review is preserved; its corrected revision applies
the declared individual-block screen as well. There was no second experiment.
Two blocks/state do not give a defensible statistical equivalence bound.

Independent call P50/P95/P99/P99.9/max, process CPU on a one-logical-core basis,
machine CPU, endpoint RSS, observer CPU/RSS, creation+identity time, combined wall
excess and sampler read latency are retained for every block. Counter quantization
means observer zero CPU is not zero cost. RSS growth of the stimulus includes its
retained marker list. Endpoint RSS is not a sampled peak or Java heap.
Observer elapsed excess combines startup, scheduling/timer and shutdown; separate
startup and shutdown CPU costs remain unknown. GPU/thermal/VRR context is incomplete.

## Completeness prerequisite failed

| PresentMon-active block | Independent inner swaps | CSV inner rows |
| --- | --- | --- |
| PresentMon-1 | 10745 | 7731 |
| Both-1 | 10738 | 7806 |
| Both-2 | 10739 | 6079 |
| PresentMon-2 | 10751 | 6667 |

All sustained observers and the controlled stimulus exited 0. Observed ETW session
loss counters are zero through the inner windows. The large row/marker discrepancies
are far beyond an edge allowance. Nearest-marker matching also has duplicate
assignments at this cadence; its absolute matching errors cannot certify precise
API timing. Missing CSV records, ETW buffer loss and missing display estimates are
three different questions. Reduced Other-runtime instrumentation, driver event
production and consumer tracking remain possible causes; this experiment does not
identify the missing-record mechanism conclusively.

[Gap review](evidence/p3a4a/gap-review.json) is decisive about one limitation:
both-2 contains a **1873.352 ms gap between CSV rows while 3356 independent swaps
continue**, with a maximum independent interval inside that gap of 1.7741 ms.
Other blocks also show long row gaps with continuing swaps. These are not real
pauses in the stimulus's swap calls. No benchmark tail may be inferred from those
CSV gaps; independent marker intervals are the timing reference for this overhead
report. The cause of absent ETW rows remains unresolved.

The Minecraft-overhead prerequisite therefore **fails**. No Minecraft observer-off/
on performance series was performed. The small observed independent cadence changes
do not rescue the incomplete PresentMon measurement path. No inconvenient block,
tail or failed screen was removed. P3-B remains NOT READY / NOT AUTHORIZED.
