# PresentMon calibration — BLOCKED

P3-A.1 safely acquired the verified portable console. **Zero actual presentation
samples**, zero controlled-workload runs, zero ETW sessions and zero overhead pairs.
The owner confirmed no Windows test user is provisioned; live validation stops at
that boundary. The help output is tool-identity evidence, not a calibration capture.

| Stage | Result |
| --- | --- |
| 1: executable/version/options | PARTIAL — official digest + valid Intel signature + actual 2.3.1 help; help exit 1 |
| 1: tracing permissions/start/stop/PID targeting | NOT TESTED — no trace attempted |
| 2: controlled presentation | BLOCKED — no approved isolated session or executed native stimulus |
| 3: observer overhead | NOT TESTED — no instrumented/uninstrumented pair |
| 4: actual CSV/import compatibility | BLOCKED — parser tests are synthetic only |

Exact executable and source: [PRESENTMON_INSTALLATION.md](PRESENTMON_INSTALLATION.md).
Local raw help SHA-256:
`30514b1e312789dafc5f0b02c224d5511d604658265c7fcf77bc0dc8deead9ba`.
Pinned license SHA-256:
`4c949341b1893c8c6ad82f7fb4eedf622cd1fd9c22a9af8f19b2dac19d1947b6`.
There is no raw frame artifact to hash. Do not fabricate a successful empty run.

After isolation passes, follow [CALIBRATION_PROTOCOL.md](CALIBRATION_PROTOCOL.md):
review the bounded native OpenGL stimulus before execution, use its fixed 30/60/120
Hz, alternating cadence and predeclared stalls, 20 seconds per stimulus with >=2 s
pre/post-roll. Preserve independent monotonic swap markers and producer diagnostics.
The native stimulus is not implemented or executed by this preparation phase.

The verified help confirms `--process_id`, `--qpc_time`, `--session_name`,
`--output_file`, `--timed`, `--terminate_after_timed`, `--terminate_on_proc_exit`
and `--no_track_input`. Use an exact authorized child PID plus creation identity,
unique trace name, explicitly new output and a bounded interval. Keep dropped rows
and display tracking. Never stop an existing session or target by process name.
An access-denied result is a stop/approval boundary, not an elevation fallback.

Record integer QPC frequency/origin and before/after monotonic/UTC pairs. Under
`--qpc_time`, `CPUStartQPC` is integer ticks; DisplayLatency/DisplayedTime are ms.
Import estimates display timestamps as `(CPUStartQPC-origin)/frequency + latency/1000`.
Compare emitted swaps/raw rows/undisplayed rows against duration, boundaries,
duplicates, missing samples and producer loss warnings. Retain unexpected gaps.
OpenGL's `Other` runtime has less instrumentation; latency-derived estimates need
actual calibration. See the pinned [console contract](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/README-ConsoleApplication.md)
and [OpenGL limitations](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/README.md#analyzing-opengl-and-vulkan-applications).

Use the existing `quality.presentmon` adapter for **frame-only calibration inspection**
with reviewed identity/QPC/stream/window settings. The full schema-1 measured-run
validator intentionally requires >=300 s warm-up and >=600 s measurement and all
three sources. A short calibration cannot become a schema-1 complete measured run;
do not label real data synthetic or weaken those thresholds. Until a separately
reviewed short-capture manifest adapter exists, retain short raw artifacts plus an
explicit frame-only/manual report and classify full-run integration INCOMPLETE.
This is an additional integration limitation discovered during source review.

No parser/threshold changed. Any real-format correction must retain its raw
reproducible fixture and regression. Overhead on another application cannot establish
Minecraft overhead. Calibration observations never authorize P3-B comparison sets.
