# P3-A measurement audit

Audited 2026-10-16 from `514fa088ee33ea99cccc63432adac1a40e5d99b4`.
P1/P2 evidence and implementation are authority; earlier launcher-startup studies
in similarly named directories measure a different subject. Gameplay remains NOT
MEASURED. No production source, schema, dependency, validator or profile changes.

| Source | Data / clock / sampling | Overhead and platform | Access/dependencies | Presentation / modification / authentication |
| --- | --- | --- | --- | --- |
| P1 `jvm-benchmark/analyze.py` | Normalized interval CSV; Java uptime G1 summaries; time-weighted process CSV; repeated paired statistics | Offline Python standard library; Windows/Linux format design, Linux execution untested | Explicit local files; no attachment | No capture, no game modification, no authentication |
| Older P0/P1/P2 startup tools | Native preparation/install/spawn spans and browser rendering | Feature-gated launcher timings, separate historical studies | Existing Rust/browser tools | Neither Minecraft displayed frames nor main-menu timing |
| Native `launch/process.rs` | Exact child PID, Starting/Running/exit; bounded redacted streams | Production process supervision; no per-frame sampling | Existing validated runtime/session/instance | Spawn success is not first frame; game launch requires normal entitlement |
| Native activity bridge | Eligible Aurora menu/world state notifications | Delivery overhead and timestamp mapping not calibrated | Aurora bridge; authenticated capability stays Rust-only | Lifecycle markers, not frame presentation; Minecraft required |
| PresentMon console candidate | ETW frame CSV and QPC timestamps | Windows; actual overhead unknown here | Third-party binary and ETW permissions; not installed by this phase | Candidate display estimates; no mod injection; standalone calibration can be unauthenticated; Minecraft validation requires legitimate gameplay |
| CapFrameX candidate | PresentMon-based capture plus monitoring/analysis | Windows; greater dependency surface, no local calibration | Separate installation/permission review | Does not remove PresentMon's OpenGL uncertainty |
| MangoHud candidate | OpenGL/Vulkan interception and logged timing/telemetry | Linux; configuration/version-dependent sampling | Approved overlay installation and loader environment; none performed | Internal/swap timing until display semantics established; not equivalent to physical presentation |
| F3 debug/internal recorder candidate | Internal game/render-loop timing | Unknown overhead/version compatibility | Would require reviewed implementation/mod approval | Separate metric; not display presentation; no such recorder found in this repository |
| New `process_capture.py` | Cumulative process CPU, working set/OS peak, system available RAM and system CPU; QPC-based `perf_counter_ns`; default 0.5 s intervals | Read-only Windows calls; actual 30 s self-process plumbing observation retained | Python stdlib/ctypes; one explicit PID plus creation identity, retained handle | No frame timing, Java attachment, owner-process sampling or authentication |
| New `quality.py` / `gc_metrics.py` | Bounded raw import, hashes, clocks, quality classification, G1 pause/heap/logged-phase CPU summaries | Offline; no production overhead | Python stdlib; reuses P1 statistics/regex without modifying historical tools | Parser validation only, never authenticity or Minecraft compatibility certification |

Inspected native authority: `runtime/metadata.rs`, `runtime/plan.rs`,
`runtime/install.rs`, `instances/settings.rs`, `instances/performance.rs`,
`instances/mod.rs`, `application.rs`, `launch/resolve.rs`, `launch/process.rs`,
`launch/diagnostics.rs`, `paths.rs`, auth credential/account boundaries and the
existing native acceptance helpers. Exact official game-plan component/major/
architecture still selects Java. Native Save and final launch validation still
own configuration. P2's optional mode is registry 6; launcher config stays 7.

Effective order remains generated Xmx, normalized Mojang JVM groups, loader
contribution, Mojang logging, original custom order, main class, game arguments.
No Xms/collector/pause tuning is generated. Full live arguments and LaunchSpec
are never exported; neither process command lines nor owner settings were read.

Important newly observed constraints:

* P2 modes reject `-Xlog`, whereas Existing Configuration retains the established
  lexical policy. For future instrumented comparisons use explicitly created
  **Existing Configuration laboratory instances** with identical logging and
  explicitly chosen heaps, recording the source advice mode separately. This
  tests effective configurations, not a new product mode. Do not weaken P2's
  validator, relabel a stored mode, or turn logging on in existing instances.
* `auth/credentials.rs:entry_target` uses `com.aurora.launcher/account/<UUID>`.
  A managed-data root does not isolate Credential Manager. Same-account sign-in
  under the owner's Windows user risks refresh-credential rotation. See the
  [authorization procedure](AUTHORIZATION.md); no credential access was attempted.
* Actual short Java plumbing showed `25M->2M(14M)`: the last capacity is after
  collection. P1's before-used <= end-capacity assumption rejected valid shrinkage.
  New P3-A G1 ingestion checks after-used against end-capacity and retains a
  regression. [OpenJDK logging implementation](https://github.com/openjdk/jdk21u/blob/master/src/hotspot/share/gc/shared/gcTraceTime.cpp)
  separates saved pre-collection usage from the current heap capacity.

Gaps remain: no approved frame tool, actual presentation calibration, authenticated
Minecraft capture, frozen benchmark worlds, B/C compatibility execution, game A/A noise,
GPU/thermal idle validation or Linux execution. Available compiler/JFR/jcmd modules
are runtime observations, not evidence that game profiling has been validated.

Counter definitions: [process times](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes),
[working-set counters](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters),
[Python monotonic timer](https://docs.python.org/3.12/library/time.html#time.perf_counter).
The sampler intentionally rejects unvalidated Windows processor-group accounting.
