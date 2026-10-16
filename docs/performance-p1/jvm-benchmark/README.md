# Offline JVM benchmark analysis

This subfolder belongs to the **2026-10-16 JVM audit P1**. The parent directory's
older `analyze.py`, README and evidence belong to startup readiness and remain
unchanged. Python 3.12+, standard library only. No installation, launcher integration,
network, profiling attachment, authentication or process launch.

```powershell
python -B -m unittest discover -s docs/performance-p1/jvm-benchmark -p test_analyze.py -v
python -B docs/performance-p1/jvm-benchmark/analyze.py docs/performance-p1/jvm-benchmark/fixtures/run.json
python -B docs/performance-p1/jvm-benchmark/analyze.py <explicit-run-manifests> --baseline current
```

The fixture output is **synthetic**, not benchmark evidence. Scripts print summaries
only; redirect stdout to a new chosen summary file if needed. Invalid input returns
exit 2 with a generic diagnostic, without dumping raw input or sensitive paths.
All captures must pass the manual provenance/clock/completeness/privacy gates in
[BENCHMARK_PLAN.md](../BENCHMARK_PLAN.md) before analysis. Tests exercise formats,
not the real Minecraft capture path. No chart is meaningful until real runs exist.

## Run manifest, schema 1

Start from `fixtures/run.json`; replace synthetic fields only after capture.
Required: `schemaVersion`, `benchmarkId`, `configId`, `blockId`, `workloadId`,
`sampleKind` (`synthetic`/`measured`), timezone-qualified ISO-8601 `timestamp`,
`status` (`complete`/`failed`/`interrupted`), `warmupSeconds`,
`measurementSeconds` (>0), `measurementStartUptimeSeconds` (Java GC clock),
`jvmArguments` (JVM-only string array), `mods` (inventory array), nonempty condition
objects `hardware`, `os`, `runtime`, `minecraft`, `loader`, `graphics`, `scene`,
`environment`, `measurementMethods`, and `sources`.

Runtime requires nonempty strings `vendor`, `version`, `build`, `manifestSha1`.
For real runs populate actual full identities, not fixture/unknown values; the
operator validates provenance. Hardware should include CPU/topology/RAM/GPU;
OS includes build/driver; mods include exact versions/hash/enable state;
scene includes snapshot/route hash; graphics includes all options/config hashes;
environment includes power/background/thermal conditions. Methods include each
tool/version, field semantics, interval, clock mapping and precision. Record
repository HEAD and raw-producer hashes as additional manifest fields.

`sources` contains run-relative `frames`, `gc`, `process` paths. Absolute, drive,
backslash, traversal and escaping-link paths are refused. Files are read only.
Manifest/normalized-input SHA-256 observations are included in output; they are
provenance observations, not publisher signatures. Store original raw captures and
their hashes separately with the adapter recipe. The analyzer accepts normalized
formats, **not arbitrary PresentMon CSV or JFR JSON directly**.

Optional `startupSeconds` is either null/absent or an object with exactly `jvm`,
`minecraftInitialization`, `mainMenu`, `worldLoad`. Values are seconds or null for
unmeasured components. Define marker origins in methods; startup metrics may overlap
and must not be summed into a score. Failed/interrupted runs require a nonempty
`failureReason`; their metrics are omitted, but identifiers remain in comparisons.
Preserve their raw evidence independently. Successful status is an operator claim;
the parser cannot prove an unrecorded OOM/early exit did not occur.

Do not put secrets, account/game argument vectors, private endpoints, or absolute
personal paths anywhere in the manifest. Output does not echo JVM arguments or
arbitrary conditions, but manifests themselves still require review before sharing.

## Normalized input formats

**frames.csv**: `timestamp_s,frame_ms`. Each timestamp is an interval **end**, in
seconds from the analysis-window start. `frame_ms` is that interval in milliseconds,
strictly positive; timestamps increase. The first interval begins at 0. Intervals
must agree with timestamp differences within 0.05 ms; final coverage within 1 ms.
Adapters should select an interval-aligned measurement window and record boundary
choices. Do not fabricate/drop frames to make a window fit. Preserve gaps and slow
frames; documented capture loss invalidates a complete run even if numeric checks
pass. Do not normalize undocumented render/CPU events into displayed intervals.

Output: type-7 mean/median/P95/P99/max, population variance in ms², P99.9 only for
>=10,000 frames, frame count, sustained FPS (frames / summed seconds). Small-tail
warning below 1000 frames; P95/P99 are descriptive, not magically reliable at that
cutoff. Stutter threshold is >50 ms: counts affected frames and distinct consecutive
bursts/events, with both per-minute rates. Exact 50 ms is not above the threshold.

**gc.log**: Java 21 **HotSpot G1 unified** logging with `uptime,level,tags`, collector
header and retained whole-run stream. Wall-clock decorators may coexist in Z/offset
formats; analysis uses uptime, never wall-clock parsing. Final `[gc] GC(n) Pause ...`
summary lines are counted once; `[gc,start]` and detailed phase records are skipped.
Young/full summaries must contain `before->after(capacity)` in B/K/M/G binary units;
remark/cleanup can omit occupancy. Durations support ms/us/s. Unknown collector,
missing header, malformed summary, duplicate event, impossible occupancy or a pause
crossing the selected measurement boundary fails. Events outside the window are
counted separately. Keep clock selection aligned; do not remove an overlapping
pause selectively to favor a candidate.

Output includes pause distributions, total pause ms/wall percent, pause frequency,
young/full collection count/frequency, full-GC count and maximum observed post-GC
used bytes. Concurrent cycles are not stop-the-world pauses. Empty valid G1 windows
report zero observations with null distribution statistics. Allocation, GC CPU and
OOM counts are null because this channel cannot reliably establish them. An explicit
OOM marker rejects a purported complete run. Missing end-of-log records cannot be
detected reliably from gc.log alone; complete capture/exit checks remain mandatory.
Other collectors/VMs require another validated parser/export path, not renaming
their lines to G1. Microsoft/Adoptium/Oracle identity is metadata, not special-cased
log parsing; fixture tests are not real-vendor compatibility certification.

**process.csv**: required `timestamp_s,cpu_percent_one_core,rss_bytes`, >=2 rows,
first timestamp 0, last at measurement duration (10 ms tolerance), strictly increasing.
Required metric columns cannot be blank. Optional columns:

```text
peak_rss_bytes,heap_committed_bytes,heap_used_bytes,system_available_bytes,
background_cpu_percent_one_core,system_cpu_percent_one_core,
paging_bytes_per_second,gc_cpu_percent_one_core
```

All sizes are **bytes**, all numeric values finite/nonnegative. CPU 100 represents
one fully occupied logical CPU, may exceed 100; do not mix whole-machine normalized
percentages. Use process CPU-time deltas with the exact child's identity, not a
process-name aggregate. Metric values at t[i] represent the following interval;
the final row is the endpoint/instantaneous peak observation. Time-weighted means
use interval duration, and min/max include the endpoint. Resample rate-derived
counters into those intervals explicitly; instantaneous memory samples are
piecewise-constant estimates and may miss peaks. Do not fill missing CPU intervals
with zero or interpolate across capture loss. Optional absent/entirely blank columns
become null; partly missing columns fail. Used heap cannot exceed committed heap.
`peak_rss_bytes` is OS-reported process peak (with its scope recorded), distinct
from sampled max RSS. `paging_bytes_per_second` must represent a documented paging/
swap byte counter; ordinary page-fault counts include soft faults and cannot be
relabeled as paging bytes.

## Repeated runs

Comparison requires identical cohort objects: sample kind, workload, hardware, OS,
runtime, Minecraft/loader/mods, scene/graphics, environment/methods, warm-up and
duration. Keep immutable intended-control fields in those objects. Record changing
observations such as actual thermal readings in a separate `observations` field;
material deviations invalidate a run or create a new cohort. JVM arguments must
be identical within one configuration; another heap/flag gets another config ID.
Unique config/block pairs define matched repetitions; synthetic/measured cohorts
cannot be combined. Run IDs should be unique across the retained experiment.

Per-run metrics are aggregated by median and sample SD across runs, with paired
relative changes and exploratory 95% paired-median bootstrap intervals (2000,
seed 20261016) for >=5 pairs. Unavailable components remain null. Zero baseline
does not have a percentage change; retain raw absolute metrics for interpretation.
The tool summarizes frame tails, GC overhead/pauses, heap/resident memory, CPU and
startup independently. It reports failures/exclusions but **does not recommend a
configuration**, correct multiple comparisons or validate a physiological/perceptual
effect. No frame-level IID significance test is used.

## Known limits

No raw capture adapters, JFR exporter, allocation estimator, full collector parser,
capture installation, statistical power guarantee, clock synchronizer or real-game
test assets. Fixture data is intentionally too short for tail performance inference.
Operator review is required for raw completeness, field semantics and privacy.
