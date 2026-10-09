# Performance P0 evidence and isolated tools

The [research report](../../PERFORMANCE_RESEARCH_P0.md) describes the results,
limitations, architecture and owner-review gate. These tools are research-only;
they do not instrument the normal application. No optimization is implemented.

## Evidence index

| File | Meaning |
|---|---|
| `evidence/native-typical.jsonl` | Sequential current-core local trials on the actual instance; 31 mods, three resource packs, one shader pack |
| `evidence/native-small.jsonl` | Sequential local trials on the disposable minimal derivative; two required mods, no packs |
| `evidence/native-online.jsonl` | Separate local observations plus five official launch-plan resolutions and runtime validations, if successful |
| `evidence/summary.json` | First observed call, repeated p50/p95/max/min/sample standard deviation, sample counts and verified workload facts |
| `evidence/pilot-*.jsonl` | Eight-trial exploratory runs; the two processes overlapped. Retained for transparency, excluded from aggregates and conclusions |
| `evidence/resources.jsonl` | Six uncontrolled observations of the already installed Launcher/WebView process tree, excluding Minecraft |
| `evidence/jfr-start.json` | Safe allowlist and duration for the temporary existing-child recording |
| `evidence/jfr-event-summary.txt` | JFR event inventory; sensitive event types have zero records |
| `evidence/jfr-aggregate.json` | Static stack/class sample counts and GC pause durations; allocation weights are estimates |
| `evidence/jfr-pilot-*.json` / `jfr-pilot-event-summary.txt` | Initial recording aggregates/audit; default custom Minecraft events prompted explicit exclusion and replacement. No custom payloads exported; excluded from conclusions |
| `evidence/runtime-version.json` | Existing child executable's Windows version resource; no executable path or arguments |
| `evidence/protection-verification.json` | Hash comparisons against pre-work repository and instance inventories |

Private local evidence is excluded by this directory's `.gitignore`:
`launcher-baseline.json`, `client-baseline.json`, `instance-protection.json`,
`small-root.local.txt`, and compiler logs. The disposable OS-temporary root contains
the raw `allowlisted-steady-state.jfr`; its exact root is recorded only in
`small-root.local.txt`. The original pilot `steady-state.jfr` also remains local.
No owner instances or diagnostic directories are deleted by these tools.

## Reproduce native measurements on Windows

Prerequisites: the exact reviewed Launcher source, its already installed Rust/MSVC
dependencies, Python 3, an existing valid managed instance, and at least 2 GB free
temporary space. These scripts describe this machine's one-instance configuration;
`prepare-small.py` deliberately refuses a multi-instance registry. Review paths and
version assumptions before using them on another host. Never replace an earlier
safety baseline while relying on it to prove preservation; use a new evidence
directory for a new investigation.

1. Capture repository baselines **before** any edits, with `snapshot.py` and a fresh
   evidence directory. The original P0 inventories remain local.
2. Run `python docs/performance-p0/prepare-small.py` once. It hashes the owner content,
   allocates a new temporary root, copies installed game content and only the two
   digest-checked required JARs, and creates empty pack directories. It copies no
   accounts, credential material, worlds, logs or player settings.
3. Build the independent harness. Its own manifest/lockfile and release profile
   isolate tooling dependencies from production manifests/lockfiles.

```powershell
cargo build --offline --release --manifest-path docs/performance-p0/harness/Cargo.toml --target-dir src-tauri/target
```

On the P0 host, Tauri dependency output generated an invalid 82-byte `msvcrt.lib`
that shadowed MSVC's real CRT. The successful research-only build supplied the
genuine library by absolute linker argument, after establishing the MSVC shell:

```powershell
cmd.exe /d /c 'call "C:\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul && cargo rustc --offline --release --manifest-path docs/performance-p0/harness/Cargo.toml --target-dir src-tauri/target -- -C "link-arg=C:\BuildTools\VC\Tools\MSVC\14.51.36231\lib\x64\msvcrt.lib"'
```

This workaround is host-specific. Do not modify production build scripts or remove
generated/pre-existing files to reproduce it. The successful build log is retained
locally. Rust formatting validation applies only to the independent harness:

```powershell
cargo fmt --manifest-path docs/performance-p0/harness/Cargo.toml --package aurora-performance-p0-readonly -- --check
```

4. Run each workload **sequentially**, with no compilation or other research scan
   in parallel. Finish each process before starting the next. The default is one
   first-observed call plus 20 repeated calls per local stage. `AURORA_P0_TRIALS`
   accepts 2–100 trials. Direct calls exclude Tauri startup, IPC and frontend work.

```powershell
& .\src-tauri\target\release\aurora-performance-p0-readonly.exe |
    Out-File -Encoding utf8 docs/performance-p0/evidence/native-typical.jsonl

$env:AURORA_P0_BENCHMARK_ROOT = (Get-Content docs/performance-p0/evidence/small-root.local.txt -Raw).Trim()
& .\src-tauri\target\release\aurora-performance-p0-readonly.exe |
    Out-File -Encoding utf8 docs/performance-p0/evidence/native-small.jsonl
Remove-Item Env:AURORA_P0_BENCHMARK_ROOT
```

The alternate root must canonicalize beneath OS temporary storage with the fixed
research prefix. Neither filenames nor display names choose instance paths. The
harness uses validated registry identifiers and production validation functions.
It reports aliases/counts instead of instance names, paths or identifiers. Errors
become a boolean rather than raw diagnostic messages. PNG-only artwork reads avoid
the production legacy-WebP conversion write. No install, authentication, selection,
update, ownership migration, or process-launch APIs are invoked.

5. Optional online resolution uses official pinned metadata endpoints and populated
   caches. It can populate verified metadata caches, but never changes instances.
   It also repeats local stages in its own process; do not pool these observations
   with the local-only series. Five online trials do not support a p95 claim.

```powershell
$env:AURORA_P0_ONLINE = '1'
& .\src-tauri\target\release\aurora-performance-p0-readonly.exe |
    Out-File -Encoding utf8 docs/performance-p0/evidence/native-online.jsonl
Remove-Item Env:AURORA_P0_ONLINE
python docs/performance-p0/summarize.py
python docs/performance-p0/verify-protection.py
```

`summarize.py` uses median and nearest-rank empirical p95 only with at least 20
repeated trials. Worst means worst observed repeat; first calls have their own
column. Native timings include minimal result assembly and exclude stdout writing.
Sub-millisecond values include uncalibrated harness overhead. Filesystem data was
already warmed by protection hashing/copying. These are observational samples,
not controlled A/B improvement tests or disk-cold startup measurements.

## Resource and JVM observations

`sample-resources.ps1 -LauncherPid <verified-pid> -OutputFile <new-evidence-file>`
collects six ten-second intervals by default. It enumerates parent IDs and process
names, never command lines/window titles. It matches process start times to reduce
PID reuse errors. Only Launcher/WebView descendants enter the totals; Java is counted
as background load. Shared working-set pages may appear in several processes.

The already running Minecraft child was identified by parentage before attachment.
`record-jfr.py <verified-exact-child-pid>` uses the locally available JDK 25 tools
and auto-stops after 45 seconds, bounded to 32 MiB. It starts with `settings=none`
and explicitly enables execution sampling, allocation sampling, GC and pause
events. `settings=none` alone permits default application-defined events: the
pilot event audit discovered Minecraft FPS/tick/network summaries. The replacement
explicitly disables every registered custom event type from that inventory, and
the aggregator refuses any nonzero custom-event count. Do not export those payloads.
Its raw diagnostic output is neither printed nor persisted. On another host use
tools matching the actual managed runtime; do not infer Java 25 from the tool alone.

After auto-stop, `summarize-jfr.py` reads the raw recording in memory and exports
only selected static class/method counts, allocation estimates and pause durations.
It does not export thread names, launch arguments, environment, file/socket payloads
or usernames. Retain the raw recording locally. The recording is uncontrolled
steady gameplay, not startup; no FPS, total allocation-rate, leak, heap tuning or
loading-speed claim follows. No persistent telemetry, JVM flags or user configuration
changes were introduced. Profiling overhead was not calibrated.

For the missing packaged UI/startup/large-instance series, use the report's observer,
isolation, privacy and acceptance procedure. These scripts intentionally cannot
claim or synthesize those end-to-end timings.
