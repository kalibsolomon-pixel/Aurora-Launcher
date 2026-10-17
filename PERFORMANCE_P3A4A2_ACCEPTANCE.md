# Performance P3-A.4a.2 acceptance

## Verdict

**PARTIAL. No accepted primary measurement source. P3-B NOT READY / NOT AUTHORIZED.**
Performance Overlay initialized and coexisted with the pinned Aurora/Sodium/Lithium
environment in a disposable world. Recording, completeness and overhead remain
unvalidated after the fresh title quiet screen failed. Valence's released aggregate
export is unsuitable for required raw analysis. Locked JFR command availability
is verified; actual recording/export and accurate frame correlation remain incomplete.

Recommendation: **Perform one small follow-up validation of the existing tool.**
Reuse the provisional Overlay/JFR path before considering custom instrumentation.
No custom mod, candidate fork, JVM optimization or performance improvement claim.

## Repository, local commit and inventory

Branch `codex/client-3-production-authority`, starting full HEAD
`133cdd7e9988c5e8cc70b45afa7535d04b9c750f`, exactly as requested. Initial tracked
tree/index clean; 2525 tracked files, four protected developer-local artifacts,
seven worktrees. Before mutation, full status/index/history/hashes and critical,
production, canonical and benchmark evidence identities were recorded privately
in `.git/performance-p3a4a2-baseline.json`; previous private evidence inventory
extended before runtime work to include all 666 retained P3-A artifact files.
Separate Aurora Client tracked state was recorded before any game execution.

One scoped local commit: **Evaluate existing frame collectors and locked JFR feasibility**.
Ending review HEAD is the commit containing this report; its full SHA is recorded
after commit in `.git/performance-p3a4a2-final.json` and the final response. A commit
cannot embed its own content-dependent SHA. No push, merge, tag, workflow dispatch,
version bump, publication, deployment, production installation or release.

Complete path inventory: [P3A4A2_CHANGED_FILES.txt](docs/performance-p3a/P3A4A2_CHANGED_FILES.txt).
**34 paths: 27 additions, seven modifications, zero deletions.**
Seven scoped original documentation/tool-guide modifications; new focused audits,
protocol, diagnostic-only parser/13 tests, sanitized evidence and this report.
Zero deletions/renames. No product Rust/Svelte/TypeScript, native boundary/startup,
dependencies/lockfiles, measured-run schema/importer, JVM preset, renderer, limiter
or canonical workload lock changed. No generated game/runtime/binary data staged.

## Preservation and isolation

[Preservation audit](docs/performance-p3a/evidence/p3a4a2/preservation.json): 2518
unmodified original tracked files, all four protected developer files, all 666 prior
private evidence files and 11 critical files remain hash-identical. Seven worktrees
and six other HEADs preserved. Separate Aurora Client HEAD
`5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d`, all 422 tracked files, index/status/
worktrees unchanged. Candidate source inspection occurred only in private storage;
neither candidate nor Aurora Client was rebuilt or modified.

Production Launcher 1.4.2 executable/config/registry hashes match baseline. This
limited check is not an owner-world/credential inventory or a freeze of ordinary
writes by independent owner software. Production instances/worlds/packs/credentials/
Java/release artifacts were not used or mutated by these actions. No .minecraft,
cleanup/reset, recursive removal, process-name termination or system-setting changes.

Canonical A archive `d385120835fd6b0ad0fb8757601de97b066d9e23023e32733f62e6bd56e13f4f`,
inventory `5082795f0ac5987c1eb61412102a2be5bc7a29868842dd317f985f3193258c61` and options
`efd1595619f579723159ed7bb0cbb1e06d7336dc9980662252162abd15f24ae1` preserved.
New B diagnostic copy had full pre-launch inventory equality; limited post-exit
NBT confirms camera/position/time/weather/seed. No new representative canonical B.
Historical PresentMon captures and confirmed consumer-overflow failure are retained.

Marked temp root/reparse checks, exact launcher/Java, native-reported managed root
and separate live WebView profile passed. Same-user normal authentication/readiness/
play followed the accepted credential-rotation risk and approved startup-refresh
ordering exception. No account secrets copied/read/exported, entitlement bypass,
interactive login automation or direct Java/game launch. Both exact supervised game
sessions and retained-handle launcher exited **0**.

## Candidate identity, implementation and runtime evidence

[Capability comparison](docs/performance-p3a/P3A4A2_CANDIDATES.md),
[version/compatibility audit](docs/performance-p3a/P3A4A2_ARTIFACT_AUDIT.md),
[publisher/observed digests](docs/performance-p3a/evidence/p3a4a2/artifacts.json).

| Candidate | Observed SHA-256 | Decision |
| --- | --- | --- |
| Valence 1.1.0 / K8qKDXKF | `aef9788dc393aefd07ddf4fab4cdbefc19301c5ed20c83b7b76a49cf0d38a3b0` | REJECTED for required primary role; runtime UNTESTED |
| Performance Overlay 1.0.2 / qyzs7zWa | `46c9500f67824996cd81b00a4d3da2dc09d5c6d56495cff98276c961d74804c2` | PROVISIONAL |
| Locked Microsoft JFR | Exact JDK/tool/profile hashes in JFR report | PROVISIONAL JVM telemetry; not frame collector |

[Valence findings](docs/performance-p3a/P3A4A2_VALENCE.md): no official source
located; inspect distributed metadata/bytecode instead. Default 900000-interval
preallocation, System.nanoTime render-head/fallback intervals, saturation flag,
outside-window report generation and explicit pre/post collection calls verified
statically. Reports export aggregate P99/histogram/reduced series, not raw chronological
frames. Aggregate P99 does not supply recomputation or precise GC proximity. No
installation was needed to reject the raw-data requirement; no runtime failure claimed.

[Overlay findings](docs/performance-p3a/P3A4A2_OVERLAY.md): official current and
v1.0.2 source plus released bytecode inspected. Eligible HUD-callback intervals,
not physical display/render-start timing. CSV rounded 0.001-ms intervals and
truncated integer-ms elapsed values; background 4096-row writer aborts on capacity
failure, per-frame row object allocated on render path, growing summary array
capped at 5000000. Counts/summary reconciliation do not prove missing HUD coverage.
Local second-precision Date has no bounded epoch/nanoTime anchor. No raw event
sequence, queue high-water or complete workload/runtime identity in CSV.

[Actual compatibility](docs/performance-p3a/P3A4A2_RUNTIME.md): fresh disposable
Minecraft 1.21.11/Fabric 0.19.5/Aurora 3.0.0/API 0.141.6+1.21.11/Sodium
0.8.14+mc1.21.11/Lithium 0.21.4+mc1.21.11. Native exact provider installation added
Cloth Config 21.11.153+fabric and Overlay only after the control game exited.
Independent publisher SHA-1/SHA-512 match installed bytes. Main menu, world, visible
Overlay and F7 hide succeeded; recording start/stop/CSV **UNTESTED**. Physical
client 1920x1080 confirmed; framebuffer unobserved. PARK/spin500us/park2000us,
adaptive=false, native Unlimited/VSync off/Minimized inactivity retained. Global
cap stayed OFF; no 240/1000 limiter accuracy result.

## Environment, timing, completeness and overhead

[Environmental evidence](docs/performance-p3a/evidence/p3a4a2/environment.json):
initial 60 seconds/121 samples, 0.887789% mean machine CPU, AC, >=20.37 GiB available.
Fresh no-collector title 60 seconds/121 samples: **2.979357%**, exceeding unchanged
2% screen, >=18.43 GiB available. RX9070XT driver32.0.31041.1004 and desktop
2560x1440/240Hz unchanged; GPU idle/VRR/thermals unknown. No favorable retry.
The total title CPU observation cannot identify background versus game cost.
Further frame/JFR/observer recordings were deferred; compatibility checks continued.

[Timestamp/overhead review](docs/performance-p3a/P3A4A2_TIMEBASE_OVERHEAD.md),
[attempt register](docs/performance-p3a/evidence/p3a4a2/trials.json). No valid new
frame recording exists: frame count, elapsed measurement time, mean/P95/P99/tails,
maximum, missingness, buffer state, comparative CPU/RSS/GC and output JFR hashes
are null/unavailable. HUD screenshot numbers are not benchmark data. No hidden
warm-up claim, invented frames, zero substitution or self-overhead acceptance.

Unchanged observer budgets: absolute 1% rate / 0.05-ms P99 / 1 machine-CPU pp,
individual pairs/means/off-off drift. Five timing states and 240/1000 subset all
deferred. No independent uninstrumented timing reference or overhead bound. CSV
ties can represent different frames; raw count/footer checks alone are insufficient.
Precise JFR correlation is unsupported without a bounded explicit timebase relation.
PresentMon remains optional diagnostic comparison when its own quality gates pass;
successful PresentMon is not a prerequisite for the new collector.

## JFR feasibility

[JFR report](docs/performance-p3a/P3A4A2_JFR.md): exact locked 21.0.7+6-LTS
JDK attaches successfully to the isolated Aurora child; check/help/start-stop-dump
availability/configuration queries exit 0. No actual recording. Supported installed
JFC/event metadata and default buffers inspected. Privacy-filtered default profile
prepared using standard jfr configure; only five sensitive command/environment/
process/property event settings disabled. No profiling-intensity tuning.

GC/pause/heap events enabled; allocation samples 150/s, exhaustive TLAB events off;
execution sampling20ms; parks/sleeps threshold20ms; SafepointBegin>=10ms with several
other safepoint events off. These defaults cannot capture every 1–4ms limiter wait
or all small safepoints. DataLoss enabled, actual loss/retention untested. Queried
default repository was ordinary Temp; future recording needs explicitly isolated
repository/output and age/size/coverage review. No start/stop/size/output claims.

## Tests and next decision

Existing **84 benchmark +13 P1 tests passed before changes**. Final **97 benchmark
+13 P1 tests pass**, zero failures; 13 new synthetic format/gate regressions cover
all requested missing/unit/duplicate ambiguity/overflow/metadata/workload/format/
window/alignment/promotion cases plus incomplete footer/truncation/nonfinite/reversed
values. One existing ResourceWarning remains non-failing. Actual-format acceptance
is untested without a real CSV. [Test evidence](docs/performance-p3a/evidence/p3a4a2/tests.json).
No production source/startup/native contract change; Rust/frontend/build suites
were not required. Real native startup and normal authenticated launches were exercised.

[Recommended architecture, effort and P3-A.4b prerequisites](docs/performance-p3a/P3A4A2_DECISION.md):
provisional Overlay internal intervals + JFR aggregate telemetry + existing sampler,
optional independently valid PresentMon. One quiet bounded follow-up should validate
recording/export and JFR alone before combined trials. Then independent completeness,
observer bounds, timebase scope, sensitivity, representative B and authoritative
source-contract review. Estimated 0.5–1 day recording/integration and 1–2 days
independent validation if a suitable existing reference exists; conditional estimates,
no success guarantee. Do not build a custom mod merely because limitations remain.

**P3-B NOT READY / NOT AUTHORIZED. Stop after P3-A.4a.2.**
