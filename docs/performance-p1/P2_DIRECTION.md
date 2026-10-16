# Proposed P2 direction — review only

No preset is implemented or justified as faster by this P1. No production schema,
default, runtime selection, command DTO or Settings layout changed. P2 requires
owner scope approval and a successful measurement program, not automatic continuation.

| Candidate profile | Intended use | Heap and GC approach | Trade-offs and required evidence |
| --- | --- | --- | --- |
| Efficient | Lower-memory/shared-GPU systems and ordinary workloads | Smallest tested adequate per-workload maximum with explicit native/OS/background reserve; ergonomic initial heap and default collector initially | Reduced footprint must not buy persistent stutter, paging or failures. Needs 8/16 GiB/iGPU tests, live-set/burst data, CPU/startup guardrails |
| Balanced | Broad default-eligible use, including unknown hardware | Conservative existing behavior until validated alternatives exist; no speculative Xms, workers or collector change | Current 2048 MiB remains a compatibility baseline, not a universal modpack recommendation. Requires A/B/C, multiple hosts and held-out regressions |
| Performance | A hardware/workload combination with a demonstrated frame-tail benefit | Only independently useful measured heap/pause/collector deltas on a supported exact runtime | May increase native/heap/CPU costs. No maximum-RAM shortcut. Needs repeated P99/stutter gains beyond A/A noise, compatibility/failure checks and bounded resource regressions |
| Custom | Explicit per-instance user control | Preserve current saved maximum and raw custom text; any future initial-heap control must be typed and deliberately supported | Advanced switches can break startup or override properties. Requires clear conflicts/errors, effective JVM-only explanation, exact runtime support and regression tests |

Keep policy in Rust, alongside the existing per-instance configuration and game
Java requirement. Svelte presents a native recommendation/explanation, draft and
explicit Save; it does not detect hardware, choose Java, concatenate flags or authorize
launch. Hardware facts are optional evidence with source/age/unknown state, not launch
authority. No new detection dependency is approved by this P1.

Prefer a small typed policy resolver for **implemented measured settings**, not a
generic optimization framework. It must expose which heap/flags are effective,
their source and the reasons/limits of advice without exposing session arguments.
Keep recommended versus applied state distinct. Existing instances and raw custom
text retain their current meaning by default; reading a registry must not tune it.
Only add persisted profile/version fields when selected implemented behavior needs
them, with explicit migrations and malformed/future-state refusal.

Selection should first produce a reviewable per-instance draft showing exactly
which values would change. Existing custom arguments survive selection. If a
profile conflicts with them, keep Custom and explain the conflict, or request an
explicit confirmation of the concrete replacement before applying it. Never clear
text silently, silently merge contradictory collectors, change another instance,
or rewrite saved maximum heap during a hardware probe.

The current lexical filter permits JVM indirection and property overrides and
does not validate all equivalent classpath spellings. Before a preset engine claims
ownership, decide whether to allowlist preset-emitted switches and strengthen
custom conflict checks (argument files/Flags files/agents/property overrides).
Such hardening may affect existing Custom behavior and needs a distinct reviewed
compatibility policy and tests. P1 documents it without changing the launcher.

Fallbacks must be explicit: missing/unsupported hardware yields conservative advice
or Custom, with no write. Unsupported candidate flags fail review before spawn;
do not silently switch collector/runtime/architecture. A failed launch keeps its
configuration and user data, reports the native failure, and offers a separately
reviewed return to the last user-approved configuration. Never silently retry with
different flags or repair/remove mods. Initial-heap work requires a dedicated typed
decision; do not make the custom parser accept Xms solely to run a benchmark.

Evidence gates before implementation/default promotion: exact validated Java build
and switch support; calibrated capture overhead; >=5 usable paired repetitions
per selected candidate/cohort with preserved failures; multiple scenes/hardware
classes; frame-tail and stutter benefit above noise; memory/CPU/startup/compatibility
guardrails; privacy-safe raw provenance; held-out confirmation; explicit Windows
versus Linux support claims. An unmeasured class remains unmeasured. No numeric
heap recommendation or collector winner can pass those gates today.

Owner decisions still needed: workload/mod/route locks, capture tool and resource
window, isolated sign-in authority if required, alternate-Xms laboratory path,
minimum worthwhile improvement/resource budgets from A/A data, platform scope,
and custom-policy compatibility. Stop after P1 and review this proposal.
