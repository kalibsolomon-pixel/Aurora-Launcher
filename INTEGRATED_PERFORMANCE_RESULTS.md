# Integrated Client and Launcher validation

2026-10-19, Windows. Native Play launched locally built Client `9513af4` through
the current embedded Launcher debug build using normal installation, managed-Java,
authenticated-session and structured-process authorities. Client was an unmanaged
local test jar in a fresh copy of a test-owned Fabric instance. No release artifact
was replaced/published; no credential or argument array was extracted or logged.

Minecraft 1.21.11, Loader **0.19.5**, API 0.141.4, Sodium 0.8.14, Overlay 1.0.2,
Cloth 21.11.153, managed Microsoft Java 21.0.7+6, G1, Xmx2048 MiB and Java-selected
initial 510 MiB. Exact VM fields were checked. AMD RX 9070 XT/OpenGL 3.3 core,
2560×1440 fullscreen, RD4/SD12, VSync off, global PARK/500 at 240, native Unlimited,
Low CPU off, Low Latency on, adaptive sleeping off. Discord remained disabled.

## Combined observation

Original copied-world camera matched at entry/pre/post capture and in normally
saved NBT. Recorded stationary warmup 93.798s; complete automatic 60s CSV and 121
samples per exact retained read-only process handle, nine processes total.
No screenshots/builds/exports/other benchmarks inside the resource window.
Family membership unchanged, no sampling failures.

| Metric | Measured observation |
| --- | --- |
| Internal eligible HUD callback FPS | 239.994 |
| Median / P95 / P99 interval | 4.162 / 4.782 / 5.147 ms |
| Maximum; >2T; >50ms | 8.443 ms; 2; 0 |
| >1.25T; largest >2T cluster in one second | 90; 1 |
| Overlay aggregate GC counter delta | 45 ms; no individual-frame attribution |
| Game CPU | 44.4375 CPU seconds / 60s, 74.062% of one logical core |
| Background Launcher family CPU | 0 observed counter increment; quantized, not zero-cost proof |
| Combined CPU | 2.645% of 28-logical-processor host capacity |
| Game RSS range | 1399.199–1399.949 MiB |
| Background Launcher family summed RSS | 486.762–486.867 MiB |
| Combined summed RSS | 1885.961–1886.816 MiB |
| Host busy / available RAM minimum | 4.947% / 17219 MiB |

This is compatibility/resource evidence, **not** an end-to-end speed gain. Native
classpath, Loader 0.19.5, real session, absence of startup JFR and launch route
differ from the dev heap matrix's Loader 0.19.2. Do not subtract RSS between them
as an optimization gain. Callbacks are not presentation/input latency; summed RSS
can double-count shared pages; process CPU is not active renderer/GC CPU.
[Measurements](docs/performance-initiative/combined-results.json) retain VM fields,
windows, identities, counts and CSV hash. No large-pack/low-memory/long-session claim.

## Failure, persistence and recovery

- Native readiness rejected absent Fabric API in the copied dev mods (Gradle had
  supplied it outside mods). Adding/hash-checking the same cached API jar to the
  isolated instance and explicit revalidation restored Java/Play without a bypass.
- UI saved 1536 MiB/Balanced and retained it across normal app restart. Custom
  `-Xmx3g` save failed explicitly, preserving text/draft and persisted values.
  Clearing the conflict and saving restored 2048/empty arguments and Ready.
- Primary gameplay saved normally, retained exact pose and exited **0**, observed
  in the native UI. No OOM/incompatible-mod crash observed. Ready returned.
- A declared pre-world abnormal-exit check hit Computer Use helper exit 3221225477
  and foreground UIA/accessibility errors. Fullscreen captures then showed non-target
  desktop frames even after reset. The mislabeled title checkpoint/event was
  explicitly invalidated, remains private, and is not accepted menu evidence.
- Logs showed no server-thread/world-entry activity; every saved-world file matched
  its post-primary hash. Only the exact isolated child, checked by parent, executable,
  creation identity and retained process handle, was terminated. Native supervision
  reported unsuccessful exit **-1** and offered recheck. This is an induced exit,
  not a naturally occurring Client crash.
- Subsequent smoke changed **only copied options to windowed mode**, after saving
  primary fullscreen options/config/logs. Native Play spawned a fresh process;
  Client and Sodium/OpenGL initialized; actual Aurora title was observed; normal
  close returned **0**/Ready. World files remained byte-identical. No secondary
  performance capture. All test processes were closed after validation.

The automation helper's fullscreen failure is a test workaround/limitation, not
an attributed Client/Launcher defect. Preferred repair: reliable fullscreen target
capture in that helper. Native/frontend tests also cover exact-child failures,
persistence, corrupt/unknown schemas and validation. External interactive sign-in
and production managed-Aurora artifact installation were not repeated: normal saved
account and existing verified runtime were used. Unattended first-Ready latency
is unaccepted; see [startup/idle limits](LAUNCHER_OVERHEAD_RESULTS.md).

## Evidence and boundary

Private `.git/performance-initiative/integration-evidence/` contains 51 protected
files (3,191,321 bytes before its manifest), supplemented by saved level and final
acceptance records: family CSVs, game CSV, event journal, failures, screenshots,
separate logs and primary config/options. Logs were copied before later launches
could rotate them. The Temp test root remains; its path is private in
`.git/performance-initiative/integration-root.json`. Accounts/browser/world contents
are not tracked report artifacts. Original inputs were never mutated.

Windows first fullscreen gameplay and windowed recovery passed. Automated second
fullscreen observation remains unreliable. Linux/WSL is absent, so no privileged
installation or Linux acceptance; Vulkan remains separate and untested.
