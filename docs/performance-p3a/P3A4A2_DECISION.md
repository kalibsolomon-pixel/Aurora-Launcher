# Candidate decision and remaining P3-A.4b work

**PARTIAL. Perform one small follow-up validation of the existing tool.**
Performance Overlay is the single selected PROVISIONAL frame collector. Its raw
interval format and actual coexistence justify completing bounded validation
before considering custom development. Valence is REJECTED for this raw-data
primary role, not declared runtime-incompatible. JFR is PROVISIONAL JVM telemetry;
actual attach commands work, actual recording remains untested. None is an
ACCEPTED PRIMARY CANDIDATE. PresentMon remains optional DIAGNOSTIC ONLY.

Minimal proposed architecture:

| Layer | Proposed reuse / gate |
| --- | --- |
| Internal pacing/tails | Overlay raw eligible-HUD callback intervals, only after recording/completeness/observer validation |
| JVM context | Exact locked JFR, privacy-filtered default profile; aggregate GC first; no precise frame attribution without bounded clock relationship |
| System context | Existing exact-child CPU/memory sampler, subject to its interference limits |
| External comparison | PresentMon only for conditions independently passing its existing quality gates |
| Offline analysis | Diagnostic source adapter now; explicit review/authorization before any authoritative measured-run source contract change |

One follow-up should have an owner-quiet window and frozen protocol, not repeated
tries until favorable. Validate manual recording start/stop, hidden-overlay
recording, complete footer/raw bytes/error handling and boundary timestamps in
the same disposable B copy. Then JFR-alone supported start/check/dump/stop and
coverage, before combined recording. Retain the initial cap subset 240/1000 and
unchanged PARK/native Unlimited/VSync/inactivity/JVM configuration. Counterbalance
the five observer conditions, retaining all blocks and off/off drift.

This follow-up is necessary but may not suffice for primary acceptance: independent
timing/overhead uncertainty is still unbounded. Predeclare an independent reference
and error budget or classify results provisional/diagnostic. Do not infer physical
display FPS, exact render duration or uncapped treatment sensitivity from callback
cadence at a reached cap. A failed independent reference does not prohibit useful
compatibility/raw-data tests, but does prohibit primary promotion.

Estimated remaining effort, conditional on a quiet host: 0.5–1 engineering day for
bounded recording/export/JFR integration and offline format review; another 1–2
days for independent completeness/observer evidence if a suitable existing reference
is available. These are planning estimates, not measured execution time or a promise
that validation will pass. No candidate fork is presently justified. Improving raw
export/clock/coverage inside a candidate is outside this phase; do not implement it.
If a necessary gate ultimately requires source changes, report a narrowly scoped
estimate and request separate authorization then.

Remaining P3-A.4b prerequisites: successful quiet controls (plus explicit unknown
GPU/VRR/thermal limits); actual collector output/coverage; JFR-alone then combined
recordings; event filters/retention/loss; explicit timestamp relation or aggregate-only
GC scope; independent observer bounds; operating-point sensitivity; representative
canonical B lock/full validation; unchanged full-run durations/source requirements;
source-contract review and separate phase authorization. Prior PresentMon overflow
history remains preserved and unresolved matching remains diagnostic.

**P3-B NOT READY / NOT AUTHORIZED. Stop after P3-A.4a.2.** No heap/Xms/GC preset,
renderer, limiter, release or production behavior changed.
