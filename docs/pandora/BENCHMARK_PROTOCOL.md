# Offline PANDORA acceptance probes

Use an optimized build and a new marked disposable root for every run. Do not
point these probes at a real launcher data root. No probe signs in or launches
Minecraft. Build/test activity must be finished before collecting timed results.

The pinned pre-change source was exported with `git archive` at
`0e2c8dd9e13ea1b1d548505d575674d9114ee386`. Both native builds use
`--release --locked --features disposable-data-root`, the software renderer and
the same compiler/toolchain. The archive/host probe is deliberately source
compatible with that baseline. Copy **only that new example** to an exported
baseline tree; do not modify the original native development worktree.

## Resource sampler

`tools/benchmark/pandora_resources.py` uses Windows process APIs and Python's
standard library. It observes the exact subprocess and live descendants. It
records working set, private committed memory, process CPU, peak working set,
sampled peak private memory and logical process I/O. It does not claim physical
disk counters or infer network transfer bytes from I/O. Native idle runs end by
terminating only the exact process this probe started; UI smoke tests separately
exercise a clean window close. The JSON records the observed descendants and
termination method. A 50 ms sampler can miss short private-memory peaks;
Windows' peak-working-set counter survives those sample gaps.

Example (PowerShell; replace placeholders with explicit absolute paths):

```powershell
python tools/benchmark/pandora_resources.py <probe.exe> <new-absolute-pandora-evidence-root> archive <pandora-bench-32m.mrpack>
python tools/benchmark/pandora_resources.py <probe.exe> <new-absolute-pandora-evidence-root> hosts
python tools/benchmark/pandora_resources.py <acceptance-native.exe> <new-absolute-pandora-evidence-root> --idle --seconds 20
python tools/benchmark/pandora_resources.py <automatic-native.exe> <new-absolute-pandora-evidence-root> --idle --seconds 75 --sync-fixture <owned-pandora-ui-fixture-root>
```

The stored ZIP fixture has one 32 MiB override (`overrides/config/large-fixture.dat`),
a valid format-1 index and no downloadable files. Write the payload in 64 KiB
blocks with deterministic bytes 0..255 repeated, using ZIP Stored compression.
It measures five complete review/parse passes, **not installation or download**.
The same unchanged ZIP is used before/after. Hash its bytes and record that hash.
The probe's 500 ms post-operation observation period is excluded from its own
`elapsedMs` and lets the sampler record retained memory.

The host probe acquires/releases 50,000 distinct `.invalid` host permits and
performs no HTTP request. It exposes lifetime retention, not real browsing or a
network workload. Do not extrapolate that workload to typical users' idle RAM.

`pandora_scheduler_bench` compares the retained ordered window and new
completion-order window in one optimized executable: 64 delay-only jobs, width
8, first job 400 ms, others 20 ms; six pairs alternate execution order. It
isolates head-of-line scheduling, with no network, file writes or hashing.
The product change applies only to the order-independent provider verification
pass. It is not an overall modpack-install speed measurement.

Collect alternating before/after process runs for each probe; report individual
results and medians. Native idle runs use fresh empty roots, default Simple
theme, no account, no installation and no active UI manipulation. Read the
existing `home-state-loaded` diagnostic checkpoint with its own monotonic
clock; it is an internal loaded-Home marker, **not physical visibility, usable
restored Home, or Ready**. Whole-process wall arrival is a different clock.

## Functional fixture and coverage limits

`pandora_sync_fixture` creates two synthetic registry entries and option files in
a new `pandora-ui-*` root. These entries deliberately have **no installed game**.
They exercise the sync controls and preservation rules, never authorize launch,
and are not evidence of an actual Minecraft installation or authenticated play.
The automatic-pool sampler accepts only that marked synthetic fixture, and
seeds only its two registry entries and bounded option files into a newly
created root. It writes a schema-2 policy with both selected and five default
categories. A 75-second run crosses one real 60-second tick. This is a tiny
options-only workload; it does not establish overhead for large pack libraries.

Current native acceptance enables syncing, selects both checkboxes, then leaves
the UI alone. A missing screenshot fixture must copy on the first real minute.
After that baseline, a unique options edit must propagate on the next minute
without Review/Apply. Virtual-clock tests separately verify the 59/60/120-second
boundaries, no overlap, and timer removal after disable. Games in another or a
previous launcher session remain outside process-local supervision.

`benchmark-results.json` retains the initial selective-integration measurements,
whose native binary still had the subsequently rejected manual UI. Final
automatic-build resource measurements were deferred when the owner asked to move
on. Future results must be recorded separately so binary identities and historical
outliers are not silently replaced. Real automatic minute propagation was tested;
that functional result does not establish resource overhead.

Cold/warm game installations, second-instance installation, large live modpack
updates, public-CDN throughput, real duplication, populated/Borealis idle and
Pandora-vs-Aurora matched runtime measurements remain NOT MEASURED unless the
acceptance report separately supplies those results. Existing deterministic
loopback installer tests establish behavior, not a performance delta.
