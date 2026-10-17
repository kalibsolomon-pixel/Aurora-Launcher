# Candidate comparison and selection

P3-A.4a.2 verdict: **PARTIAL**. No accepted primary collector. Evidence labels
apply to each specific fact, never to a whole product. VERIFIED includes inspected
static implementation facts; runtime facts are identified separately. UNKNOWN
means no sufficient evidence, not failure. UNSUPPORTED means the audited format
cannot provide the quantity. FAILED is used for the actual environmental gate.

| Requirement | Valence 1.1.0 | Performance Overlay 1.0.2 | Locked JFR |
| --- | --- | --- | --- |
| Minecraft 1.21.11 | VERIFIED declaration; runtime UNKNOWN | VERIFIED declaration and startup/world | VERIFIED exact-JVM attach; recording UNKNOWN |
| Aurora 3.0.0 | UNKNOWN runtime | VERIFIED startup/world/HUD coexistence | VERIFIED attach to Aurora process; recording UNKNOWN |
| Per-frame timing | VERIFIED internal interval collection; runtime UNKNOWN | VERIFIED callback-delta implementation; live sampling observed | UNSUPPORTED as primary frame collector |
| Raw export | UNSUPPORTED individual-frame export in audited report builder | VERIFIED CSV implementation; actual export UNKNOWN | VERIFIED standard event-export capability; actual recording UNKNOWN |
| GC telemetry | VERIFIED aggregate sampler implementation; runtime UNKNOWN | VERIFIED cumulative MXBean delta implementation; live HUD observed | VERIFIED event definitions/settings; actual event coverage UNKNOWN |
| Timestamp precision | VERIFIED ns intervals in memory; raw export unavailable | VERIFIED rounded 0.001-ms intervals, integer-ms elapsed timestamps | VERIFIED event metadata; effective timestamp accuracy UNKNOWN |
| Sample-loss detection | VERIFIED saturation flag; no independent frame coverage | VERIFIED queue abort/footer counts; missing HUD callbacks UNKNOWN | VERIFIED DataLoss enabled; actual loss/retention UNKNOWN |
| Low overhead | SUPPORTED BY DOCUMENTATION ONLY; unbounded | UNKNOWN; render-path row allocation verified | UNKNOWN; built-in status is no overhead proof |
| Long-run support | VERIFIED finite buffer/report code; actual long run UNKNOWN | VERIFIED bounded queue/summary cap, disk grows; actual long run UNKNOWN | VERIFIED supported disk/age/size controls; retention UNKNOWN |
| Automation | VERIFIED command interface in bytecode; runtime UNKNOWN | VERIFIED F10/timer code; runtime recording UNKNOWN | VERIFIED jcmd check/help/query; start/stop/dump UNKNOWN |

Valence is **REJECTED for this primary role**, because required raw chronological
frame data is absent. It may be useful for aggregate diagnostics, but was not
installed. Performance Overlay is **PROVISIONAL**: actual initialization and world
coexistence succeeded, while recording, completeness and observer gates remain.
JFR is **PROVISIONAL for JVM telemetry**, never a primary frame collector.
PresentMon remains **DIAGNOSTIC ONLY**, optional and subject to its existing gates.

Select only Performance Overlay for the initial runtime check. The failed title
quiet screen deferred recordings and overhead conditions. Neither developer
claims nor synthetic parser tests close any empirical gate.

Details: [artifacts](P3A4A2_ARTIFACT_AUDIT.md), [Valence](P3A4A2_VALENCE.md),
[Overlay](P3A4A2_OVERLAY.md), [JFR](P3A4A2_JFR.md),
[actual execution](P3A4A2_RUNTIME.md), [decision](P3A4A2_DECISION.md).
