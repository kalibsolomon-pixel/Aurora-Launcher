# P3-A benchmark infrastructure

Python 3.12+, standard library only. No package installation. `quality.py` is
offline; `process_capture.py` is an explicitly invoked bounded read-only Windows
sampler. Nothing runs at launcher startup, downloads a tool, authenticates, launches
Minecraft, modifies presets or sends telemetry.

```powershell
python -B -m unittest discover -s tools/benchmark -p 'test_*.py' -v
python -B -m unittest discover -s docs/performance-p1/jvm-benchmark -p test_analyze.py -v
python -B tools/benchmark/quality.py docs/performance-p3a/evidence/synthetic/run.json
# Create explicitly synthetic data only in a new directory whose parent exists:
python -B tools/benchmark/fixture.py <new-disposable-directory>
# Future explicitly authorized PID; never choose an owner Java process by name:
python -B tools/benchmark/process_capture.py --pid <pid> --creation-id <Windows-FILETIME-identity> --seconds 30 --interval 0.5 --new-directory <new-disposable-directory>
```

The sampler refuses an existing output directory and holds the opened process
handle; PID reuse/exit/mismatched creation identity fails. It never terminates a
process or reads its command line. Preserve partial captures on error. At most
660 seconds per invocation, intervals 0.1–5 seconds; sustained game measurement
still requires owner authorization. A duration bound is not authorization.
Creation identity is the exact Windows creation FILETIME integer, not wall-clock
text or an inferred process name. The future native supervisor can identify the
authorized child; no production identity-export command was added in P3-A.

The historical PresentMon importer expects 2.3.1 v2-style columns with QPC,
despite its legacy `default-qpc` format tag. P3-A.2 real execution showed that the
actual default CSV differs. That adapter and measured-run gates remain unchanged;
use the separate short-calibration reader for actual default exports. GC parser supports HotSpot G1 summaries;
other collectors fail rather than using a misleading generic estimate. `quality.py`
reuses P1 statistics but fixes shrink-capacity handling in its own P3-A parser.

Exit 0: every requested manifest passes offline checks (possibly warnings).
Exit 2: at least one invalid/incomplete record; each run remains in JSON output.
The tool never declares a performance winner or independently authenticates a
producer's claim that input came from Minecraft. See
[artifact contract](../../docs/performance-p3a/ARTIFACT_FORMAT.md) and
[readiness matrix](../../docs/performance-p3a/READINESS_MATRIX.md).

Windows native acceptance helpers use the already installed bundled Playwright
and PowerShell runtimes at explicit host-local paths, not new dependencies. Build
frontend and debug native assets first, then run native-prepare.ps1 and, in a second
terminal, native-verify.mjs. They create a fresh marked direct-temp root and install
only lightweight Fabric through existing native commands. The session record is
exclusive; reruns refuse to overwrite it. native-restart.ps1/.mjs reuse only that
recorded marked root and exact executable hash. Root/executable checks do not grant
arbitrary mutation authority. These are local acceptance tools, not deployment or
an alternative game/runtime installer. Never use them with owner roots/accounts.

Evidence is retained; no broad cleanup/reset helper is provided. Private full logs,
temporary paths and raw inventories remain local under .git or the disposable root.
Small committed files explicitly distinguish synthetic/parser, actual standalone
GC, actual self-process/system telemetry and actual native installation evidence.

P3-A.1 adds `readiness_checks.py`, a read-only directory snapshot/restoration
inventory checker. It never copies, restores, extracts, authenticates or launches.
See [WORLD_SNAPSHOT_LOCK.md](../../docs/performance-p3a/WORLD_SNAPSHOT_LOCK.md) for
its strict schema/layout and provenance limits. The pure `isolation_guard` is a
refusal helper for observed SID/elevation facts, not a live credential-vault probe.
Eight synthetic tests add missing/extra/changed-file, directory, hash, metadata,
path/redirection and principal/elevation coverage; the original 36 remain unchanged.

```powershell
python -B tools/benchmark/readiness_checks.py <test-owned-evidence>/snapshot.json
```

Schema-1 `sampleKind=measured` still requires 300/600-second gameplay and all three
sources. Short P3-A.1 calibration uses frame-only adapter inspection and an explicitly
limited report; full-run integration remains INCOMPLETE. Never relabel real samples
synthetic or weaken thresholds. See [calibration status](../../docs/performance-p3a/PRESENTMON_CALIBRATION.md).
The owner confirmed no separate Windows test user exists, so this phase acquired
and verified PresentMon 2.3.1 and executed help only; no live trace/game session.


P3-A.2 supersedes the separate-user requirement with explicit owner-approved
same-user credential-continuity risk acceptance, verified isolated managed/WebView/
registry/game/world paths and normal owner-operated Microsoft sign-in. See
[authorization](../../docs/performance-p3a/AUTHORIZATION.md); this grants no P3-B.

`short_calibration.py` strictly validates a separately named bounded artifact:
[format](../../docs/performance-p3a/SHORT_CALIBRATION_FORMAT.md). It handles actual
2.3.1 default `TimeInQPC` / `MsUntilDisplayed` columns, retains NA display rows and
warnings, verifies raw hashes/identities/units/order/coverage, and always leaves
full measured-run quality INCOMPLETE. It neither captures nor authenticates.
Eight focused regressions use a real controlled OpenGL CSV excerpt and deliberately
malformed temporary copies. Original 44 benchmark and 13 P1 tests remain unchanged.

```powershell
python -B tools/benchmark/short_calibration.py <test-owned-directory>/short-calibration.json
```

Direct Windows producer diagnostic redirection uses UTF-16LE with BOM; strict decoding
is covered by the eighth new regression. Original measured gates remain unchanged.
