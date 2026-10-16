# JVM/gameplay baseline — NOT MEASURED

Recorded 2026-10-16. **No real Minecraft performance benchmark was run in this P1.**
There are no measured frame-time, GC, gameplay memory/CPU or startup comparisons,
and no supported performance winner. The older startup-readiness evidence in this
directory measures a different question and is not a JVM tuning baseline.

## Actual host observations

Read-only built-in Windows CIM inventory at **2026-10-16 06:04:27 UTC**
(02:04:27 America/New_York). No hardware stress test or power-setting write.

| Field | Observation and limits |
| --- | --- |
| CPU | Intel Core i7-14700F; CIM reports 20 physical cores / 28 logical processors; architecture code 9 (x64). P/E breakdown and effective affinity were not queried |
| Installed DIMMs | Two 17,179,869,184-byte DIMMs, 32 GiB total; reported speed 6000 (CIM value, not independently measured bandwidth) |
| OS-visible RAM | 33,304,812 KiB total; 18,276,076 KiB free at this one instant. Free RAM is transient, not a recommended heap |
| GPU | AMD Radeon RX 9070 XT; driver 32.0.31041.1004. CIM's 32-bit AdapterRAM field is unreliable for modern VRAM; no VRAM capacity claim |
| OS | Windows 11 Home, 10.0.26300 / build 26300, 64-bit |
| Form factor | CIM PCSystemType=1 (desktop classification); no battery returned. Actual AC/thermal/power-limit telemetry unavailable |
| Power scheme | Existing active scheme label: Ultimate Performance. Scheme was read, never changed; label does not establish clock/thermal behavior |
| Existing load | An Aurora launcher and two javaw processes were present before work. They were neither profiled nor terminated; other applications and background load were uncontrolled |
| Java observation | Both existing javaw executables expose Microsoft company metadata, ProductVersion/FileVersion 21.0.7.0; adjacent release files report JAVA_VERSION=21.0.7. This is file metadata, not executed `java -version` or benchmark identity |
| Exact benchmark runtime vendor/build | **NOT ESTABLISHED**. Full VM `+build`, runtime manifest SHA-1, deep validation and benchmark executable identity remain required before a usable run |
| Linux | No Linux host measured |

The sanitized raw host inventory is retained in
[host-inventory.json](jvm-benchmark/host-inventory.json). Original inventory and
repository/process preservation snapshots are local-only at
`.git/performance-p1-hardware.json` and `.git/performance-p1-baseline.json`.
They contain no process argument lists, token values or account contents. Host
inventory is the only new machine measurement here; it is not gameplay evidence.

## Unmeasured matrix and blockers

| Workload/configuration | Status |
| --- | --- |
| A lightweight, B representative Aurora/modded, C demanding | NOT MEASURED on Windows or Linux |
| Current Aurora, Java 21 explicit-bound reference, heap alternatives, G1 targets | NOT MEASURED |
| Alternative initial heap / collectors | NOT MEASURED; initial-heap changes cannot pass current production custom-argument policy |
| Frame time / GC / heap / resident memory / CPU / startup | NOT MEASURED for all configurations |
| 8 GiB / 16 GiB / iGPU / laptop / nonlocal CPU classes | Hardware unavailable in this session; NOT MEASURED |

No validated external presentation capture or approved installation of such a tool
was established. There is no prevalidated disposable workload/world and dedicated
authenticated benchmark instance. Existing owner Java processes and background
applications preclude a controlled quiet baseline without interfering with owner
activity. No sign-in, private-server connection, owner-world copy, account mutation
or production-instance use was attempted. Full Java build and collector support
also remain to be recorded from the chosen isolated runtime.

The safe next measurement session needs a quiet resource window, approved capture
tool installation if needed, disposable pinned workloads, and explicit owner permission
for any Microsoft sign-in. See [BENCHMARK_PLAN.md](BENCHMARK_PLAN.md). These are
measurement prerequisites, not an invitation to skip validation or begin presets.

## Synthetic verification is separate

`jvm-benchmark/fixtures/` contains intentionally tiny **synthetic** frame, process
and HotSpot-shaped G1 data. It validates formulas, parsers and error handling;
it never launched Java or Minecraft. Parser test success is not runtime/vendor
acceptance. No synthetic metrics or charts are reported as baseline results.
Real charts are deferred until real raw captures exist and pass the quality gates.
