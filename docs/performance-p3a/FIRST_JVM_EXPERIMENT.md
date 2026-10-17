# First JVM experiment — design only, separate P3-B authorization required

P3-A.4a.4 assessment: see [fresh-process acceptance](../../PERFORMANCE_P3A4A4_ACCEPTANCE.md).
The normal-terrain stationary baseline is now frozen and restored independently;
it is a practical subset, not canonical B or a memory-pressure workload. Its
committed/live heap remains well below both proposed maxima. Do not expect changing
only Xmx to reduce RSS, CPU or callback intervals. Prefer a bounded, more
memory-demanding representative workload for this resource hypothesis before
requesting the unchanged 2048→1536 comparison. Do not silently switch candidates.

Use the metric-specific sensitivity and optional JFR rules in that acceptance,
not the older within-session ranges below. Small CPU/RSS benefits and matched
GC-frequency repeatability are unresolved. The existing three-pair design remains
a proposal; no candidate was executed here and P3-B remains NOT AUTHORIZED.

One change: saved `memoryMib` **2048 → 1536**, using the existing native instance
configuration path. Baseline is Existing Configuration (`performanceMode: null`),
2048 MiB, empty additional JVM text, null launcher window override. Candidate has
the same fields except 1536 MiB. No Xms, collector, pause target, pre-touch, worker,
compiler, renderer or limiter change. Default G1 stays selected by locked Microsoft
OpenJDK 21.0.7+6-LTS, manifest `cb4394a27089d19f65d5baa6cf0482c27c3c7865`.

The exact tuning difference is one launcher-generated argument: `-Xmx2048m` becomes
`-Xmx1536m`. All normalized Mojang Windows JVM groups, validated native paths,
launcher brand/version, ordered managed classpath and log4j property remain equal
by semantic role; Fabric contributes no JVM tuning arguments. The instance/game/
runtime/mod/options hashes must accompany each run. The source-derived baseline
vector is documented in [JVM launch audit](../performance-p1/JVM_LAUNCH_AUDIT.md);
never export a live command line, token, session arguments or LaunchSpec to prove it.
Compare current assembly/configuration rather than blindly trusting that old audit.

Hypothesis: a smaller maximum may constrain G1's heap growth and reduce process
memory under a repeatable allocation workload while retaining adequate live-set
headroom. Maximum reservation alone is not committed memory or RSS, and lowering
Xmx need not reduce either. This is a resource-efficiency hypothesis, not an FPS
prediction. The P3-A.4a.3 JFR pilot supplies real before/after GC context to review
headroom; it is a short stationary scene, not evidence that 1536 MiB suffices for
normal terrain, traversal, servers or every owner instance. Proceed only after the
accepted workload's baseline confirms headroom; defer if it does not.

Expected outcomes: RSS/heap committed and high-water may fall; GC frequency may
rise. Main trade-offs are more frequent pauses, worse P99/severe spikes, OOM or
longer loading under bursts. Monitor median/P95/P99 callback intervals, >50-ms
frequency, uncapped callback throughput, game CPU/RSS, heap before/after GC, top-level
pause sum/distribution, GC frequency and stability. Allocation samples are context,
not exact allocation throughput. No per-spike GC attribution without clock alignment.

Use a newly frozen representative B identity before product-facing conclusions;
the B-stack/A-scene diagnostic copy can screen mechanics only. Preserve canonical
A as a separate minimal Fabric control; do not call an API/Overlay-added copy
canonical A. Use the same native Unlimited/VSync-off/Minimized/custom-cap-off
settings and 1920x1080 target in every pair. Keep all observers identical.

Predeclare three matched pairs with alternating order (baseline/candidate,
candidate/baseline, baseline/candidate). Each run uses a fresh disposable world copy
and fresh supervised game process, >=300-second in-world warm-up and the same
30-second screening window. Three short pairs are initial screening, not endurance
acceptance; any promising candidate also needs normal-world loading/traversal and
longer stability/compatibility confirmation in separately scoped testing. Do not
add heap sizes or other collectors to this first comparison.

Use Overlay-only for the primary pacing cohort if its practical overhead review
passes. Use a matched separately labeled JFR diagnostic cohort (same profile in
both treatments) for heap/GC if combined interference is not stable. Never compare
an instrumented candidate with an uninstrumented baseline. If the observer or
environment variation is as large as the intended benefit, classify INCONCLUSIVE
and stop; do not solve it by adding undocumented flags or unlimited repetitions.

Repeatability: publish each result and all paired differences. A promising memory
benefit must repeat across pairs, exceed observed baseline/control drift and be
large enough to matter to the intended memory-constrained use case. Predeclare that
use-case goal after baseline headroom and practical sensitivity are established,
before running a candidate; no post-hoc favorable threshold. A practically neutral
result requires enough sensitivity to rule out that goal, otherwise INCONCLUSIVE.

Regressions override isolated throughput gains: crash/OOM, incompatibility, repeated
new >50-ms bursts, P99 worsening beyond baseline/control drift, or new memory/paging
pressure prevents acceptance. A material CPU/GC disadvantage must be weighed
explicitly against any memory benefit. Keep failures, including loading failures.

Reversal: stop the exact supervised disposable game normally, restore `memoryMib`
to 2048 via native Save, verify the other configuration fields and Ready/runtime
identity, then use a fresh disposable run. No world/config/mod deletion or rollback,
production recommendation change, preset activation or owner-instance mutation.
Efficient/Balanced/Performance advice remains unchanged; Performance still matches
Balanced. **No candidate has been executed or accepted in P3-A.4a.3.**
