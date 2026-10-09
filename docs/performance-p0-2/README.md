# P0.2 instrumentation and evidence

This is a development-only research harness. No validation is removed, cached,
coalesced, or authorized by a timing result. The [report](../../PERFORMANCE_RESEARCH_P0_2.md)
is PARTIAL: Launcher startup/readiness and IPC were measured; Play and startup
JFR are BLOCKED for owner review. No game was launched and no optimization made.

## Retained evidence

| Artifact | Contents and limitations |
|---|---|
| [Preflight](evidence/preflight.json) | Original HEADs, branch/remote comparisons, inventory counts and document digests; complete path-level inventories remain private |
| [Build environment](evidence/build-environment.json) | Exact profiling source/tree, toolchains, lock/config digests, unchanged identity and approved public-key digest |
| [Profile](evidence/profile-artifact.json), [control](evidence/control-artifact.json), [IPC](evidence/ipc-artifact.json) | Exact executable identities and every frontend file digest plus canonical tree fingerprint |
| [Build source verification](evidence/source-build-verification.json) | Canonical Cargo blob equals HEAD despite the CLI's LF/CRLF worktree status; 1,219 shared runtime files unchanged |
| [Client final provenance](evidence/client-final-provenance.json) | Byte-identical deployed JAR reproduction, source HEAD, 443 passing tests |
| [Initial Client build](evidence/client-provenance.json), [entry differences](evidence/client-entry-differences.json), [canonical LF build](evidence/client-canonical-provenance.json) | Retained 17-test CRLF failure and intermediate mismatching JARs; these are not accepted artifacts |
| [Runtime](evidence/runtime-provenance.json) | Exact managed Java 21 executable/manifest, bounded version diagnostic; no game execution |
| [Environment](evidence/environment.json) | Host, same-user as-is conditions, normal caches/network, uncontrolled background activity, balanced pair order |
| [All trials](evidence/trials.json) | 18 real starts; 12 primary UI trials, four excluded pilots, two IPC diagnostics; no raw UI text/screenshots or PIDs |
| `evidence/trace-profile-01.json` through `trace-profile-08.json` | Numeric native/frontend records; 01–02 are pilots, 03–08 are primary profiling runs |
| [IPC trace 1](evidence/trace-ipc-01.json), [IPC trace 2](evidence/trace-ipc-02.json) | Separate diagnostic cohort, 50 real IPC round trips per process; not primary overhead inputs |
| [Summary](evidence/summary.json) | Reproducible counts, role timings, overlaps, marker transitions, IPC distributions and matched overhead estimates |
| [Verification](evidence/verification.json) | Sanitized successful check/test/build results and private full-log digests |
| [Before instrumentation commit](evidence/before-instrumentation-commit-preservation.json), [before report commit](evidence/before-report-commit-preservation.json) | Original-file preservation; only 18 authorized Launcher source files differ; no original tracked removal |
| [Discarded stdio](evidence/raw-stdio-discard.json) | 36 exact current-test raw stdio files discarded without inspecting/exporting contents or recursive cleanup; no pre-existing file removed |
| [Failures/exclusions](evidence/failures.json) | Observation errors, guard refusal and environmental build exclusions; no hidden failed game launch |
| [Final preservation](evidence/final-report-preservation.json), [document validation](evidence/document-validation.json) | Final original-file comparison and read-only evidence/artifact/link checks |

`python docs/performance-p0-2/analyze.py` validates the numeric traces and
regenerates only `evidence/summary.json`. It opens no application and reads no
owner data. `python docs/performance-p0-2/validate.py` checks numeric schema,
metadata privacy patterns, evidence links, bundle-manifest fingerprints and
critical repository file presence without launching. Pattern checks supplement
review of the closed numeric contract and sanitized metadata; they cannot prove
that arbitrary text is safe. An optional `--artifacts-root` pointing to the retained
task-created artifact copies additionally hashes all three executables and all
69 frontend files. No private absolute path belongs in committed evidence.

The tree fingerprint hashes the UTF-8 compact JSON frontend-file list with sorted
object keys and separators `,` and `:`. List order is the collector's Windows
`Path` sort (component-based, case-insensitive), preserved in each manifest; it
must not be replaced with a case-sensitive string sort.
Evidence bytes retain their collected line endings; the local Git attribute
recognizes CRLF for whitespace checking without normalizing trace hashes.

Native invocation totals cover ten traced starts only: 20 readiness requests and
40 game audits. The six primary profiling starts have 12/24. Uninstrumented
control counts remain unknown. All 18 starts reached the actual Launcher; the
first control's later screenshot proved Ready despite its inaccessible page.
Source traces prove Ready in profiling pilots, but their UI latency records are
excluded. No OS process identifier or raw screenshot is required to regenerate
the public summary.

The `lastNegativeLowerMs` observation field is the latest non-Ready capture
request during the whole hold, not a bound for first Ready or first paint.
The first two artifact pilots lacked early window activation and had unreliable
accessibility. Recovery/fixed-settle pilots are also excluded. The primary
observer activates each returned window before polling. Use the
[notebook](ui-observer-notebook.js) only under the Computer Use skill; inspect
each observation before a subsequent input. Loading it performs no UI action.

## Build and collect

Use an isolated checkout and output directory; preserve the installed executable,
existing diagnostic outputs, and all owner-managed files. Build the frontend with
`npm exec vite build -- --mode performance-p0-2` and the native executable with
Cargo feature `performance-p0-2`. Supply the existing approved production updater
public key through the ordinary build configuration; never substitute a key,
empty trust configuration, signing secret, identifier, or release authority.
The equivalent control uses the same source/configuration and ordinary production
frontend mode with the Cargo feature absent. Record exact commits, lockfile and
bundle digests, executable hashes, toolchains, commands, and environment differences.

Each instrumented process requires a fresh existing directory below an OS temporary
directory whose first component starts `aurora-p0-2-`. Set
`AURORA_P0_2_CAPTURE_ROOT` to that directory. `native-trace.json` is created
exclusively and written once at normal application exit. Existing evidence is
never overwritten. Missing opt-in disables recording; abnormal/empty output is
not a completed trial. `VITE_AURORA_P0_2_IPC=1` enables a separate diagnostic
frontend cohort with 50 harmless application-status requests at 100 ms intervals.
Do not pool that cohort into the matched startup overhead comparison.

Use the Computer Use skill to observe the real packaged application and close it
normally. Do not retain raw UI text/screenshots, account identifiers, process
arguments, log bodies, private paths, or authentication data. Verify no competing
Launcher/game session before each trial. One existing instance does not provide a
changed-instance workload. Never fabricate one by copying owner content.

**Do not run Play over the protected JNA temporary files from P0.1.** Its previous
normal game startup deleted marked natives. A safe game collection arrangement
and reviewed Java-21 startup JFR allowlist must be established before Play. This
instrumentation does not redirect JNA or change Java arguments/instance contents.

## Record contract

All records contain eleven unsigned numeric fields: `event`, `edge`, `id`,
`parent`, `us`, `hash_us`, `hash_calls`, `correlation`, `instance_generation`,
`account_generation`, `client_us`. The envelope has only `records` and `dropped`.
The native buffer holds at most 16,384 records; overflow increments `dropped`.
The frontend sends at most 2,048 ordinary markers plus one overflow marker (114).
Any overflow invalidates counts. Edge 1 begins a span, 2 ends it, 3 means the
async future was cancelled; frontend markers use edge 0. Synchronous scope exit
means returned/unwound, not successful validation. No result/error is recorded.

| Native event | Meaning |
|---|---|
| 1 | Native builder initialization; ends before the event loop |
| 10 / 11 / 12 | Runtime status / readiness / Play preparation |
| 13 / 14 | Application status or launcher state / selection command |
| 20 / 21 / 22 | Game audit / game metadata resolution / runtime metadata resolution |
| 23 / 24 / 25 | Mod inventory / runtime integrity / bounded Java diagnostic |
| 26 / 27 / 28 | Play session obtain / ensure-session / session restoration lock wait |
| 29 / 30 / 31 | Final content-lock wait / locked game validation / exact `spawn()` |
| 32 / 33 / 34 / 35 | Blocking queue wait / blocking worker / numeric dispatch / inventory command |
| 36 / 37 / 38 | Mojang / Fabric / runtime-index HTTP document acquisition |

Frontend markers 100–113 follow the enum in `performance.rs`: initialize,
runtime request/complete, readiness request/complete, Checking, Ready, discarded
readiness, Play request/complete, instance/account generation, IPC request/complete.
114 reports frontend overflow. Generations compare existing presentation identities
in memory at request dispatch; identities never enter evidence. They are request
generations, not a new authority for account or instance state.

Async scopes install and restore their numeric context on **each poll**, including
cross-thread resumptions. Blocking workers explicitly inherit their submitting
context. Hash counters belong to the innermost active span and include file open,
read, and digest calculation wall time. They do not measure pure hashing CPU time,
archive parsing, or in-memory metadata digest operations. HTTP spans include
client construction, transport, body collection and the existing digest check;
they are not packet-level network measurements. Runtime metadata can also use
the verified artifact cache outside its index HTTP span.

Span durations are inclusive. Overlapping spans and parent/child durations must
never be summed as independent wall time. Final event 30 covers the boundary's
locked game validation; the existing callback's final mod scan is event 23 under
Play, still inside the same content lock. All original checks remain in place.

Ordinary builds use unchanged authoritative command DTOs and no-op probes.
Research-only command wrappers accept the same request plus a numeric `Probe`
and delegate to the original function; their errors retain the original shape.
The disabled wrappers reject requests. No timing path accepts arbitrary text.

## Acceptance

Run frontend checking/tests/build, Rust formatting/checking/tests for feature and
ordinary builds, inspect privacy/bounds and preserved-file comparisons, and boot
the real source-proven executable. Game instrumentation requires a usable menu
and normal supervised exit; missing evidence stays BLOCKED. No mock or isolated
native timings substitute for packaged observations. Keep raw temporary capture
roots private; publish only schema-validated numeric evidence and sanitized build
identities. No automatic cleanup of owner files or old evidence is provided.
