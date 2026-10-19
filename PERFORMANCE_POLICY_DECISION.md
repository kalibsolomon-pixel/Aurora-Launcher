# Performance policy after the initiative

2026-10-19. Phases F/G retain the existing, implemented configuration system.
The [architecture audit](PERFORMANCE_CONFIGURATION_AUDIT.md),
[controlled Java results](JVM_PERFORMANCE_INITIATIVE_RESULTS.md), 992 passing
native tests and 282 passing frontend tests support retaining it. No duplicate
controls, schema change, hidden override, custom Java picker or GC flag bundle.

| Existing mode | Policy retained | Evidence and meaning |
| --- | --- | --- |
| Efficient | Conservative target of 1536 MiB, capped by fresh host safeguards; explicit apply/save | Usable in the tested lightweight workload. Neither higher FPS nor lower RSS was demonstrated; this is a maximum-heap budget, not a proven efficiency preset. |
| Balanced | 2048 MiB target, default collector and Java-selected initial heap | Established reference retained. Repeated Windows runs show stable operation; no broad hardware/modpack optimum is claimed. |
| Performance | Same policy as Balanced | No supported evidence justifies extra memory/flags or a claim of greater speed. Demanding packs require explicit workload-specific observation. |
| Custom | Explicit per-instance memory and supported advanced properties/assertions | Saves preserve manual choices. Heap conflicts and unsupported collector flags fail with explanation; no silent dropping or override. |
| Existing Configuration | Missing mode preserves legacy behavior and established advanced parser policy | Existing instances do not opt into a new policy silently. Unknown/damaged schemas still fail without overwrite. |

All modes use the exact game-plan-required managed Java component/major,
platform and architecture. The tested build is Microsoft Java 21.0.7+6; no
vendor recommendation follows from one build. No PATH/system-Java fallback.
Exactly one authoritative Xmx is generated, with no Xms override. Current Java
defaults are consistent with the [Java 21 G1 tuning guidance](https://docs.oracle.com/en/java/javase/21/gctuning/garbage-first-garbage-collector-tuning.html).
The experimental ZGC branch has no eligible timed result and cannot justify a
new ordinary-user collector control or recommended flag set.

Memory advice reserves 4096 MiB from total and 2048 MiB from available RAM, uses
architecture caps and bounded fresh observations. Missing, stale, inconsistent
or insufficient facts produce no recommendation. No CPU-model or mod-count
multiplier is inferred from this stationary workload. Host safeguards are
heuristics, not reservations, guarantees of playability or measured optima.
512–32768 MiB remains a sanity input range; 512 is not a modern-gameplay promise.

The existing UI already explains that modes provide advice, keeps saved values
visible alongside the draft, requires explicit apply/save, and supports Undo/
Discard. Manual memory changes and raw text survive failed validation. Native
save/launch boundaries revalidate deterministic structured arguments; the UI's
Ready state alone never authorizes spawn. Expanding the advanced policy needs
an independently scoped compatibility design and useful evidence.

The current native UI was exercised against a fresh test-owned copy, including
dependency rejection/recovery, setting persistence and conflict recovery; details
and measured resource observations are in the final integration report. Windows
results do not establish Linux performance or low-memory-host behavior.
