# P3-A.4a.1 — observer interference recovery

**PARTIAL. Minecraft observer escalation stopped.** Same budgets: absolute rate
difference ≤1%, P99 difference ≤0.05 ms, machine CPU difference ≤1 percentage
point; every individual paired block, state mean and off/off drift must pass.
No outlier exclusion or threshold relaxation.

Previous 1,800 Hz both2 CPU +1.306pp remains a failure. Its raw consumer-overflow
warning also invalidates its PresentMon completeness interpretation. Independent
swap markers and process endpoint counters remain separately reviewable. The
existing 0.5-second sampler has measurable read cost; process identity and
startup/shutdown are retained rather than inferred from process names.

Recovery used one independent 500 Hz WGL stimulus, eight 10-second blocks:
none/PresentMon/sampler/both/both/sampler/PresentMon/none, inner 2–8 seconds.
This was the predeclared fallback when no cadence passed unique event matching;
it quantifies interference but cannot independently authorize Minecraft escalation.
All children exited 0. Controls retain swap-marker writes and endpoint reads, so
they are not observer-free physical ground truth. Fresh quiet CPU mean 0.958%.

| State mean minus control mean | Rate % | P99 ms | Machine CPU pp |
|---|---:|---:|---:|
| PresentMon | +0.001319 | +0.037239 | −0.323990 |
| Sampler | −0.003540 | +0.022642 | −0.518144 |
| Both | +0.000308 | +0.026058 | −0.517936 |
| Off2 minus off1 | +0.003966 | −0.008288 | +0.812591 |

All means pass, but both2 versus its mirrored off2 differs by **−1.062884 CPU
percentage points**, outside the absolute budget. A negative difference is not
an instrumentation benefit: it shows that background drift and measurement
uncertainty are large enough that the required bound is unestablished. The first
combined block was +0.027012pp. Do not discard either block or substitute only the
means. Other individual rate/P99/CPU screens pass. Nearest WGL/ETW matching also
remains ambiguous. Two blocks/state are not a statistical equivalence result.

Phase evidence retains observer CPU/RSS endpoints, total-machine busy counters,
stimulus RSS, launch latency, wall excess, sampler read overhead, ETW snapshots,
raw hashes and all block comparisons. CPU counter quantization and six-second
inner windows limit attribution. Thermal/VRR state and complete background-load
stability are unobserved; no unrelated apps were terminated or global settings
changed. Controlled gate failed again: no Minecraft observer on/off trials.
The separately authorized single-capture diagnostic matrix does not measure
observer cost and cannot establish zero cost.
