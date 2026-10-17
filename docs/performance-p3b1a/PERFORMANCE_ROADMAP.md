# Performance roadmap after P3-B.1a

Objective: improve frame-time consistency, CPU efficiency, memory behavior and
gameplay responsiveness while preserving stability and compatibility. This is a
ranked investigation plan, not authorization to implement it or a claim of gains.
Keep 2048 MiB because no superior candidate has been demonstrated; it is not a
globally optimal heap claim. P3-B.1 remains INCONCLUSIVE: B1/C1 is the only complete
pair, C2 is unpaired, B2 is excluded and B3/C3 were not run.

## Ownership and evidence

| Area | Authority / owner | Evidence and decision |
| --- | --- | --- |
| Maximum heap, initial heap, commitment, headroom | Launcher: saved configuration and structured JVM assembly | B1/C1 commitment increased 28 MiB with the smaller maximum; RSS decreased only 20.50 MiB. C1/C2 RSS range was 72.69 MiB. No repeatable saving. Do not adopt 1536 MiB or sweep heap sizes. Initial heap remains ergonomic; no evidence supports adding Xms. |
| G1 pauses, occupancy, collection frequency | Launcher: runtime/flags; Client only if attributable allocations require source changes | Valid windows had 7–10 top-level pauses, 24.29–30.17 ms summed, maxima 4.94–5.20 ms, zero >50 ms eligible intervals. Aggregate context does not identify a GC-induced frame. Default G1 stays; another collector needs a specific observed limitation and exact-build compatibility review. |
| Runtime implementation/version | Launcher, within Mojang/Fabric requirement | Exact Java 21.0.7+6-LTS is locked. No measured runtime deficiency or alternative-build comparison. Defer runtime changes; Java major compatibility is not proof of equivalent performance or mod behavior. |
| Launcher while game runs | Launcher native host and WebView, separately | Existing sampler reports native launcher CPU 0.0 in the prior valid capture windows; this does not include all WebView work or prove zero cost. Exact-child supervision awaits the child and drains streams; update controller has one local reset timer and no recurring update polling. No demonstrated competing download/poll bottleneck. Audit full launcher/WebView cost before optimizing it. |
| Efficient/Balanced/Performance/Custom | Launcher preferences and advice | Modes remain opt-in advice. Future labels must reflect validated trade-offs, workload/headroom and stability; Performance cannot promise faster gameplay based on more RAM. No changes now. |
| PARK pacing, overshoot, spin-tail, CPU cost, adaptation | **Aurora Client** | Installed-artifact audit documents one park then spin, fixed target advancement/reset, no measured overshoot estimator, and low-CPU park-threshold behavior that does not alter PARK's spin-tail calculation. These are testable implementation facts, not a demonstrated defect or speedup. |
| Render loop, UI, render-thread stalls, Sodium/OpenGL | **Aurora Client**, sometimes upstream mods/driver | No current allocation stack or render-thread attribution establishes an Aurora bottleneck. Profile before selecting code. Preserve Sodium/OpenGL and all features; Vulkan remains a separate engineering concern. |
| Temporary allocations / per-frame object creation | **Aurora Client only after attribution**; otherwise vanilla/mod/upstream owner | Prior JFR allocation sample weights total roughly 5.20–5.50 GB per 30 s, while after-GC used heap is roughly 674–692 MiB. These are weighted samples and phase-dependent used observations, not an exhaustive allocation ledger or attribution to Aurora. |
| HUD/widgets/events/background features | **Aurora Client** | Per-frame work is a plausible profiling target. No feature is proven excessive; compare supported feature states and preserve behavior before proposing caching or cadence changes. |
| GC and scheduling/render interaction | Shared investigation; assign change to actual subsystem | Launcher can hold JVM/observers/workload constant. Client owns pacing and its allocations; upstream work stays upstream. No two-repository implementation in one unreviewed task. |

Sources: [P3-B.1 results](../../PERFORMANCE_P3B1_ACCEPTANCE.md),
[installed-artifact limiter audit](../performance-p3a/AURORA_LIMITER_AUDIT.md),
[practical protocol](../performance-p3a/PRACTICAL_PROTOCOL.md), and current source
`src-tauri/src/launch/process.rs` (await/drain supervision),
`src/lib/launcher/updateController.ts` (explicit updates), and
`src-tauri/src/instances/performance.rs` (memory advice). Installed Client behavior
and present source provenance must be distinguished before any Client work.

## Ranked next three investigations

### 1. Aurora Client PARK pacing and CPU trade-off audit

- **Hypothesis:** under a supported positive frame cap, scheduler overshoot and
  spin-tail policy may offer a useful CPU/frame-consistency trade-off. Audit
  responsiveness because pacing runs at frame head before input/render work.
- **Supporting evidence:** installed audit above identifies concrete wait/reset
  mechanisms and parameter interactions. Existing heap captures had global cap 0
  and adaptive sleeping false, so they did **not** measure an active global PARK
  cap and cannot demonstrate a PARK defect. Prior positive-cap empirical work was
  deferred. High practical relevance for capped play makes this the best next
  investigation despite uncertain benefit.
- **Expected metric:** internal interval median/P95/P99, cap accuracy, severe tails,
  game CPU at equivalent cap; existing JFR execution samples for scheduling context.
  Physical display latency and precise input-to-photon latency remain unsupported.
- **Required workload:** separately authorize one supported positive cap relevant
  to the owner/display, fixed native/VSync/adaptive state and identical observers,
  repeated fresh stationary normal-terrain controls, then ordinary interaction and
  stability confirmation if promising. Do not change the frozen P3-B.1 source.
- **Effort:** medium: exact artifact/source provenance, a small supported-settings
  comparison, then at most one mechanism change if justified. No new monitor or
  adaptive subsystem merely to produce a result.
- **Main risk:** tick/input scheduling, CPU burn, cap jitter, hybrid CPU behavior,
  Sodium/OpenGL interaction and feature regressions. Preserve manual PARK mode;
  hardware-aware adaptation needs conservative fallback and its own evidence.
- **Proposed next phase / repository:** **Aurora Client C-PERF.1**, scoped
  investigation first. Launcher supplies isolated native Play, fixed JVM and the
  existing evidence pipeline; all Client-source changes require Client ownership.

### 2. Allocation and render/feature attribution before changing code

- **Hypothesis:** a small number of avoidable hot-path allocations or redundant
  feature updates may contribute to CPU cost or GC pressure; determine whether
  they belong to Aurora, vanilla, Sodium/Lithium or another observer first.
- **Supporting evidence:** existing JFR shows material allocation activity with a
  much smaller after-GC used envelope. It does not attribute it to Aurora. This has
  broader potential than altering only the heap ceiling, but attribution confidence
  is lower than the concrete PARK mechanism audit, so it ranks second.
- **Expected metric:** allocation sample weight/class/stack distribution, execution
  samples, game CPU and top-level GC totals, then repeatable P95/P99/severe-tail
  differences with no feature loss. Sampling bias and observer effect stay explicit.
- **Required workload:** first review retained privacy-filtered JFR event fields
  offline. If adequate, use separately authorized equal capped/uncapped normal-play
  workloads with supported feature-state controls. A stack missing from the current
  recording remains unknown; do not add instrumentation in P3-B.1a.
- **Effort:** medium to high, depending on whether a dominant owner-attributable
  path exists. Stop if no actionable concentration; do not optimize every allocation.
- **Main risk:** stale HUD state, event ordering, feature behavior, rendering/mod
  compatibility, and blaming Aurora for upstream allocations.
- **Proposed next phase / repository:** **Aurora Client C-PERF.2 attribution**,
  conditional on C-PERF.1 findings. Launcher can review existing aggregates and keep
  runtime/workload fixed. Any implementation follows the proven source owner.

### 3. Launcher runtime memory/GC ergonomics for an explicit constrained use case

- **Hypothesis:** a single justified memory policy may reduce real commitment/RSS
  while preserving pacing and loading headroom on a memory-constrained machine.
- **Supporting evidence:** P3-B.1 tested the mechanism but has only one mixed pair;
  1536 MiB did not meet the declared 128 MiB goal in that pair. This host retained
  at least about 16 GiB available during valid captures, so memory pressure is not
  established. Expected payoff for the present host is lower than the first two.
- **Expected metric:** commitment, weighted RSS, after/before-GC envelopes, pause
  count/duration, headroom, paging evidence if already available, normal load and
  traversal stability, and unchanged P95/P99/severe tails.
- **Required workload:** only after continuity is operationally reliable and a real
  constrained-memory use case justifies another experiment. Predeclare one candidate
  and adequate matched repetitions. A heap comparison is worthwhile only when that
  goal outranks Client findings; no automatic resumption of B3/C3 or exhaustive sweep.
- **Effort:** low to medium experimental work; new workload/stability validation may
  dominate. No new Xms, collector or generic flag bundle without a named mechanism.
- **Main risk:** more collection pressure, loading OOM, misleading short static
  headroom, and promoting advice before repeatability is demonstrated.
- **Proposed next phase / repository:** separate owner-authorized **Launcher
  P3-B.2 memory-use-case gate**, deferred. Keep current Java, G1 and presets now.

Launcher/WebView overhead is a reserve candidate, not a presumed optimization:
promote it only when existing process-family measurements show meaningful competing
work. Small CPU/RSS differences need adequate sensitivity; native 0.0% samples
cannot bound the entire launcher process family. Runtime/collector alternatives
likewise need evidence before displacing the ranked work.

## Owner handoff and standing policy

**AURORA CLIENT HANDOFF REQUIRED**

- **Subsystem:** Aurora Client frame pacing (PARK, frame-head scheduling, spin tail).
- **Evidence:** the installed 3.0.0 artifact audit describes concrete scheduling
  mechanisms and unresolved parameter effects; the heap cohort had cap 0, so no
  positive-cap benefit or defect is verified.
- **Proposed improvement:** first audit positive-cap overshoot, CPU use and variance;
  implement only a measured improvement supported by that separate investigation.
- **Expected benefit:** potentially steadier capped intervals or lower CPU cost at
  equivalent pacing; no numerical gain is promised.
- **Risks:** input/tick scheduling, increased spin cost, platform/hardware variability,
  Sodium/OpenGL and feature compatibility; preserve manual PARK and isolate Vulkan.
- **Launcher dependency:** fixed authenticated launch/runtime/workload, existing
  Overlay/JFR/resource sampling, and continuity protocol. No Launcher pacing code.
- **Recommended Client development phase:** C-PERF.1, installed-artifact provenance
  plus bounded PARK scheduling/CPU audit before implementation. C-PERF.2 allocation
  attribution is a separate conditional Client follow-up, not an approved patch.

For **every future Launcher performance phase**, assign the real subsystem owner
before implementation. When a justified recommendation requires Client source,
prominently include the exact handoff format above (subsystem, evidence, proposed
improvement, expected benefit, risks, Launcher dependency, recommended separate
Client phase) in the final owner report. Never implement it from the Launcher task.
When none is justified, explicitly report **AURORA CLIENT HANDOFF: NOT YET REQUIRED**.
Hypotheses do not become verified defects by being listed on this roadmap.

Overlay measures eligible internal HUD-callback intervals, not physical displayed
frames. JFR supplies aggregate JVM context, not individual-frame GC causality.
Use the existing pipeline; repeated meaningful practical effects can justify work
without laboratory accuracy, while one favorable run or noise-sized gains cannot.
