# P3: local Minecraft startup investigation

The verdict, measured distributions, attribution, and proposed next changes are in
[PERFORMANCE_RESEARCH_P3.md](../../PERFORMANCE_RESEARCH_P3.md). These tools implement
a Windows research workflow for the exact Java 21 / Minecraft 1.21.11 / Fabric
0.19.5 environment studied. They are not a production launch mode or a general
benchmark service. No application source or installed executable was changed.

## Evidence

| File | Meaning |
| --- | --- |
| `evidence/inputs.json` | Source/artifact identities, 31 top-level mod hashes, deidentified pack inventory, graphics settings |
| `evidence/trials.json` | All 16 successful trials, native spans, deterministic JVM markers, exit statuses |
| `evidence/exclusions.json` | Three failed setup pilots, kept out of the primary cohort |
| `evidence/statistics.json` | Cohort means, medians, ranges, sample standard deviations |
| `evidence/jfr.json` | Per-recording sanitized CPU, blocking, GC, class loading, compilation, I/O and allocation summaries |
| `evidence/attribution.json` | Recomputed phase, ownership, native cost and paired-control aggregates |
| `evidence/jna-isolation.json` | Three exact-JNA cleanup positive/negative controls |
| `evidence/agent-tests.json` | Overlay/two-frame menu test, callback timing, denied-event test |
| `evidence/preservation.json` | Full before/after preservation comparison, including operational-cache exceptions |
| `evidence/verification.json` | Check log hashes, repository membership, Java windowed executable, profiler/tool identities |
| `evidence/source-patch-hashes.json` | Four changed tracked files in the disposable copy; not changes to this checkout |
| `evidence/mapping-provenance.json` | First-party Mojang mapping verification |
| `evidence/privacy.json` | Export checks; no raw recording, owner identifiers, or absolute private path |
| `evidence/reproduction.json` | SHA-256 equality for two independent reductions of the five core outputs |
| `evidence/injected-method-provenance.json` | Exact-jar proof of the hot Sodium method injected into a Minecraft class |
| `evidence/research-patch-provenance.json` | Compiled copied helper and build configuration identities |
| `evidence/commits.json` | Base and local instrumentation commit; report commit discovery |
| `startup.jfc` | Exact reviewed JFR configuration used for the measurements |

`jfr.json.eventCounts` describes the **whole recording**, including time after the
menu. `preMenuEventCounts`, sample attribution, durations and CPU load are limited
to marker origin through usable menu. In particular, baseline-03's long idle
period after its menu is excluded from all startup attribution. `NativeLibrary`
is emitted at chunk end and used as whole-recording corroboration only.

CPU sample fractions describe observed samples across threads; they do not measure
wall-time shares. Compilation, class-loading, mixin and callback totals are
inclusive and can overlap. The six `partitionMs` intervals are disjoint **within
each trial**; medians of those intervals need not sum to the median total.

## Private recordings

The ignored `private/research-root.txt` contains the exact local research root.
Its `raw-index.json` records the absolute local path, byte size, SHA-256 and role
of every retained `.jfr` file, including failed pilots and fixtures. The index's
SHA-256 and recording count are in `evidence/verification.json`. Raw recordings,
game logs, copied account metadata, native process IDs, clock correlation points,
and screenshots remain outside Git in that root. No upload is part of this workflow.

The root is in the owner's local temporary storage; preserve it separately before
OS temporary-file maintenance. This investigation does not schedule cleanup or
delete original files. Never attach the raw index or research root to a public issue.

## Reproduction prerequisites

Read both repositories' operating guides and architecture documents, the P0,
P0.1, P0.2, P1 and P2 reports, and the P3 report first. Preserve the P2 startup
Client-update audit and all final launch validations. Verify the selected ready
instance and its settings rather than selecting a directory by enumeration order.

Use Python 3.12+, the existing Node/Rust toolchains and a JDK with `javac`, `jar`
and `jfr` available to the shell. This study used the existing JDK 25.0.4 build
tools with `javac --release 21`; every game and isolation probe used the exact
managed Microsoft OpenJDK 21.0.7+6 runtime. Install no replacement runtime into
the owner's managed tree. `node_modules` must already be available for copying.
Build utilities deliberately stop on source-boundary drift, mismatched hashes,
competing Launcher/Java processes, missing credentials or incomplete state.

Use a **fresh** research root for reproduction. Do not rerun `build.py` against
the measured root: it would replace research binaries and invalidate their
recorded identity. The tools are tailored to the recorded layout/component;
review runtime and instance selection if reproducing on another machine/version.

```powershell
# Prints a newly created private root. Substitute that result below.
python docs/performance-p3/preflight.py <client-repository>
$p3Root = '<newly printed research root>'
python docs/performance-p3/isolate.py $p3Root
python docs/performance-p3/prepare.py $p3Root
python docs/performance-p3/build.py $p3Root
python docs/performance-p3/test_agent.py $p3Root
python docs/performance-p3/mappings.py $p3Root
python docs/performance-p3/preserve.py $p3Root before-pilot
python docs/performance-p3/test_analysis.py
```

Inspect every receipt before the first Minecraft launch. `preflight.py` hashes
documents and inventories; it does not replace reading their instructions.
`isolate.py` must demonstrate real JNA extraction and marked-object cleanup inside
the new root, unchanged outside sentinels and unchanged protected originals.
`prepare.py` copies files, never links them, and patches only a new source tree.
It initializes that copy's private Git index because source-contract tests inspect
HEAD. Review the four source patches and generated helper before building.

The new build enables existing P0.2 numeric probes and the copy-only P3 feature.
The root override requires a temporary research directory and baseline receipt.
The updater public key retains the already-reviewed P2 identity; the normal OAuth
registration, OS credential store, trusted caches and entitlement flow remain in
use. The copied data root is isolated; refreshed credentials are still rotated in
the ordinary OS credential store by the normal authentication implementation.

## Run one trial

```powershell
python docs/performance-p3/start.py $p3Root pilot-01
```

This starts exactly the copied executable and an observer of the returned child
handle. The Launcher must become Ready before Play. Start each trial at least six
seconds after Launcher creation, as in this study. Keep other workload stable.
The guard refuses to start while any Launcher or Java process is present; it does
not stop competing processes. Close research windows normally before continuing.

Before clicking Play, write the following JSON to the fresh trial's
`play-request.json`, using an exclusive create and a current test-harness clock:

```json
{"requestUnixMs": 0, "harnessMonotonicMs": 0}
```

Replace both zeros with actual values taken immediately before the normal UI
click. The clock receipt is an external input boundary, not a synthetic native
command or a substitute for authentication. The supplied Python tools do not
click or manipulate the Launcher. Use ordinary input or a reviewed UI harness.

Wait for `markers-menu.json`. Require `menuReady=true`, exactly one `menu.usable`,
exactly one `jna.isolation-confirmed`, and no `error.*` markers. The readiness
marker means two consecutive completed `Minecraft.runTick(boolean)` frames with
`AuroraTitleScreen` active and no loading overlay. Verify the actual menu as a
secondary UI check, then write `ui-proof.json` with a boolean observation receipt.
Quit Minecraft through its normal menu or normal window close; require
`exit.json.exitCode=0`. Close the research Launcher to flush `native-trace.json`.
Never terminate Java by process name. An observer timeout requires investigation,
not a new trial.

After a successful pilot, use labels `baseline-01` through `baseline-10` for the
primary JFR cohort. `control-01` through `control-05` disable startup JFR while
retaining the same agent, marker bridge, data copy and per-trial native isolation.
Interleave matched controls; record the actual order and any interruption. Keep
failures and pilots. Do not discard slow successful runs. These are warm filesystem
launches with fresh Launcher/JVM processes, not cold-disk trials or repeated Play
inside one long-lived Launcher.

## Reduce, verify and review

```powershell
python docs/performance-p3/analyze.py $p3Root
python docs/performance-p3/summarize.py
python docs/performance-p3/inventory.py $p3Root <validated-selected-instance-id>
python docs/performance-p3/preserve.py $p3Root final
python docs/performance-p3/audit.py $p3Root
```

The analyzer uses `jfr summary` to reject unexpected enabled event types before
export. It reads allowlisted JFR JSON in memory and exports only static code
symbols and numerics. It validates marker uniqueness, normal exit, zero native
trace drops, clock correlation and JNA isolation. It refuses incomplete trials
as primary evidence. The official mappings are expected-SHA-1 verified; methods
whose obfuscated name is ambiguous remain intermediary rather than being guessed.

`analyze.py` expects private `client-mappings.txt`. `mappings.py` obtains the exact
version document from the pinned official Mojang manifest, verifies its
manifest-provided SHA-1, then downloads `downloads.client_mappings` and verifies
that object's published SHA-1. It records provenance and refuses to overwrite a
differing existing input. The public reference
receipt gives this study's exact mapping URL and digest. Never substitute scraped
or third-party mappings. Do not modify a product artifact cache for analysis.

Run `analyze.py` and `summarize.py` twice, comparing SHA-256 for `trials.json`,
`statistics.json`, `jfr.json`, `exclusions.json`, and `attribution.json`. They must
reproduce byte-for-byte. Review outputs for secrets, private paths, server/user
names and resource-pack filenames before staging. `audit.py` additionally checks
original repository bytes/membership, critical files, staged deletions and known
private identifiers. It does not claim a general proof against every possible secret.

Open a retained recording in portable JDK Mission Control 9.1.2 using a workspace
and Java/JNA temporary directories under the research root. Review Method
Profiling, GC, Class Loading and Compilations. Do this after benchmarking, since
Mission Control itself consumes CPU. Never export/upload an unsanitized JMC report.
The study verified the portable ZIP against its published release-asset SHA-256.

## Scope and rollback

Instrumentation is a separate local commit from the report and sanitized evidence.
There are no production changes to roll back. To remove the research additions,
revert the report commit and then the instrumentation commit after preserving any
desired local evidence. Identify them with `git log --oneline -- docs/performance-p3
PERFORMANCE_RESEARCH_P3.md`. Do not delete the private root or owner files as an
automatic rollback step. Nothing was pushed, published, installed or uploaded.
