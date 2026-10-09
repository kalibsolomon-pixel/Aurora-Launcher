# Aurora Performance P0.2 — Targeted instrumentation and causal profiling

Status: **PARTIAL. Launcher startup/readiness and IPC were measured; Play and
Minecraft startup/JFR acceptance are BLOCKED.** Collection: 2026-10-09.

There were 18 real source-built Launcher starts: six matched profiling/control
pairs, four observation/recovery pilots, and two separate IPC diagnostic starts.
Ten starts have numeric native traces. They contain **20 executed readiness
requests and 40 full game-integrity audits**: two and four respectively on every
traced start. The six primary profiling starts contribute 12 requests and 24
audits. No Minecraft game was launched. No performance optimization was made.

The main verified cost is repeated file verification. Each game audit called
the file-verification helpers 4,677 times; file open/read/digest wall time occupied
98.45–98.77% of that audit interval, median 98.69%. This is **I/O plus hashing**,
not a measurement of pure hashing CPU. The later readiness refresh adds a
sequential request with median duration 1,158 ms after runtime status completes.
That refresh explains the observed Ready → Checking → Ready behavior.

The [evidence index](docs/performance-p0-2/README.md) describes record contracts,
reproduction, exclusions, and the exact retained inputs. P0/P0.1 remain historical
baselines; their installed-binary and isolated native timings are not pooled here.

## Source and artifact provenance

| Source | Exact revision |
|---|---|
| Launcher before P0.2 | `36bda4f1f1c8a1b0830ad1bfac1ec295f2e4470d` |
| Launcher instrumentation and all three measured builds | `6cbabd38bc1c5268b9f6c80034f18da3a26f1e21` |
| Client, unchanged | `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d` |

The instrumentation is one independently revertible commit. The subsequent
report/evidence commit is separate; the final response identifies that final
Launcher HEAD. No branch was pushed, release published, version bumped, or
installed Launcher replaced.

Initial complete status/history, tracked inventories, and per-file protected
untracked inventories were retained privately for both repositories. Launcher
had 452 tracked and 15,143 protected untracked files; Client had 422 and four.
Neither had pre-existing tracked edits. Launcher remained on
`codex/client-3-production-authority`, without an upstream: the cached comparison
was eight ahead/zero behind `origin/main`. Its separately queried live default
HEAD was `4575a0e3f1c28da6cfe072948fe91b60eeaf608f`. Client remained on
`codex/unified-theme-pilot`, zero ahead/behind its cached tracking branch; its
separate live default `master` HEAD was
`d8eecd89b9187619b3ad7a47adf97cd74d4f1c77`. These cached comparisons are not
claimed as fresh graph comparisons against those live default HEADs.

Builds used a task-created detached Launcher worktree, separate output targets,
`npm ci`, and Tauri `build --no-bundle`. The control has the native research
feature absent and production frontend mode. The profile has Cargo feature
`performance-p0-2` and the same-named frontend mode. The IPC artifact additionally
sets `VITE_AURORA_P0_2_IPC=1` at frontend build time. All use the ordinary release
profile: optimization 3, LTO, one codegen unit, abort panic, stripped symbols.

| Artifact | Executable SHA-256 |
|---|---|
| Profile, 19,629,568 bytes | `21cf2b08bfcd95af8a78e9ad33dabbb3a276b9a6543f06abb3ff78fbb82993e9` |
| Feature-off control, 19,591,680 bytes | `b23182d171a491ae180ee88ea8a848a899e47d059e5f7d6ae030606017445ce9` |
| Separate IPC diagnostic, 19,629,568 bytes | `bd198762e20307272f132ae9f92f213a6798d8294d9caba70385558789067e55` |

| Frontend | Canonical file-list/tree SHA-256 |
|---|---|
| Profile | `6fa20eb305f8a20bb403951c18d0264e834e2c6ffe3fada54d13a7d3b43ebaf2` |
| Control | `8416f5f958a375ae66764429a9a09cebaa44c1c1cb87660866d8ff9b590a4725` |
| IPC | `b3af09c1530f44d99d5176e8f05fa5b70a2adcce2cadff95f0074a5882a8a9ba` |

The artifact manifests retain every relative bundle filename, size and digest,
plus the tree-digest algorithm. Input manifest/lock/configuration hashes and
toolchains are in [build environment](docs/performance-p0-2/evidence/build-environment.json).
Rust was 1.98.1 / LLVM 22.1.8 for Windows MSVC; Node 24.21.0 and npm 11.19.0.
No dependency or lockfile change was needed.

Release identity remains Launcher 1.4.1 / `com.aurora.launcher`. Builds used the
existing approved `launcher-production` updater public variable, both in native
build environment and external public-only Tauri configuration. Its SHA-256 is
`bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`.
There was no empty/replacement trust key, signing-secret workaround, registration
change, installer, or updater publication. Tauri CLI left its disposable
`Cargo.toml` with LF in a CRLF checkout, producing a Git `M` status despite an
empty HEAD diff. Its Git-clean-filtered blob exactly matches HEAD; this observation
is retained in [source verification](docs/performance-p0-2/evidence/source-build-verification.json).
The owner's original manifest was not normalized.

The deployed Aurora Client 3.0.0 JAR was reproduced **byte-for-byte** from the
unchanged Client source and original tracked-file bytes in a disposable checkout:
3,077,377 bytes, SHA-256
`43f918207a86f91b01b35045309e89d2b040951722e5ac71d01d886beec9811c`.
All 435 archive entries match, and all 443 tests pass. The first automatic CRLF
checkout had 17 source-text assertion failures and ten resource line-ending
differences. A fresh canonical LF checkout passed the tests; copying only the
422 baseline-hash-verified original tracked files into that task-created checkout
then reproduced the deployed JAR exactly. No Client source fix or deployed-JAR
replacement occurred. [Final proof](docs/performance-p0-2/evidence/client-final-provenance.json)
and the earlier failed/mismatching observations are retained separately.

Client builds used Gradle 9.4.1 / Loom 1.16.3 and the system Temurin JDK 25.0.4.1,
targeting Java 21. Source compile dependencies include Fabric Loader 0.19.2 and
Fabric API 0.141.4 for 1.21.11. The existing instance remains Minecraft 1.21.11 /
Fabric Loader 0.19.5 / Aurora 3.0.0, with its existing content. Compilation
dependencies were not substituted for installed requirements.

The resolved managed runtime is `java-runtime-delta`, Windows x64, manifest
SHA-1 `cb4394a27089d19f65d5baa6cf0482c27c3c7865`. Its executable SHA-256 is
`0afb1170b83c156dfdb7f54bb34c9b058064175c329a5d80b782bcc9dbaa2164`;
the bounded diagnostic reports Microsoft OpenJDK 21.0.7+6-LTS and exits zero.
System Java was used for the Client build, not selected for game execution.

The installed Launcher remains SHA-256
`15bfd5d71bbe73c9ad7f3010c052f3f1669d81abfa4790b8c48efb8b169fd868`.
Its exact source revision remains unestablished. There is no claimed source-level
improvement over its P0.1 measurements.

## Instrumentation and privacy

The [native probe module](src-tauri/src/performance.rs) uses closed event enums,
numeric request correlation and instance/account generations, monotonic clocks,
and eleven unsigned numeric fields per record. Native storage is capped at 16,384
records with an explicit dropped counter. Frontend markers are capped at 2,048
plus one overflow marker. All ten captured traces pass numeric schema checks,
contain no cancellations/unfinished spans, and have zero overflow/dropped records.

Ordinary builds compile timing call sites to no-ops; research wrappers reject
calls when the feature is absent. All eight control starts were given the capture
environment variable and produced **no trace file**. Research wrappers delegate
to the same authoritative readiness/runtime/Play implementations and preserve
their errors. Existing DTOs, validation results, policy and launch arguments are
unchanged. Timing results authorize nothing.

Async scopes restore correlation on every poll, including a resume on another
thread. Blocking workers inherit their submitting context. Tests cover numeric
serialization/bounds, rejection of private event vocabulary, nested context/hash
ownership, and pending/cross-thread restoration. Hash timers aggregate within the
innermost span, without per-file paths, digests, names or sizes.

Recording requires a compile-time feature and a fresh opted-in temporary capture
directory; output creation is exclusive. The bounded buffer is written once on
normal application exit, without filesystem writes in validation loops. Native
timestamps, frontend clocks, and OS process-creation times are kept distinct.
Span durations are inclusive; parent/child or simultaneous operations must not
be summed as independent wall time.

Retained research records contain no account IDs, usernames, tokens, launch
arguments, request bodies, raw errors, private paths, server addresses or raw UI
content. Initial ordinary application stdio redirection created 36 task-private
disposable files. Their contents were never inspected/exported; those exact
current-test files were discarded after exit, without recursive cleanup, because
ordinary app logs are outside the numeric-only retention contract. No pre-existing
diagnostic, evidence, log, or JFR file was removed. No raw startup JFR was created.

## Collection and distributions

The owner-authorized same-user **as-is** arrangement was used for Launcher
initialization; no separate account, data-root override, or credential isolation
is claimed. Source builds used normal approved authentication/updater configuration.
No tool enumerated or manipulated Microsoft credentials. Normal startup did not
execute the instrumented session-restoration phase.

Host: Windows 11 Home build 26300, i7-14700F (20 cores / 28 logical processors),
Radeon RX 9070 XT driver 32.0.31041.1004, Ultimate Performance power scheme.
Build/test activity ended before measurement. Caches and networking were ordinary
and warm; no reboot, cache purge, firewall, power, content, selection or settings
changes were made. Discord/ordinary owner background activity was uncontrolled.

Each start checked that no competing Launcher/Java existed. Windows were closed
through the normal window shortcut; subsequent process checks and complete
exit-written traces confirmed completion. Launcher exit codes were not collected;
this is not a claim of a supervised Minecraft exit. The two IPC runs settled for
eight seconds, completed 50 real status requests each, and visibly reached Ready.

The first profile pilot's accessibility observer reported Ready after 29.117 s,
although native markers show the later Ready at 4.033 s. The first control pilot
timed out without an accessible page, while a screenshot later showed Ready.
Those are **observer failures**, not app latency samples. A fixed-settle profile
pilot and an early-activation control recovery pilot are also excluded. All four
remain in `trials.json`, with reasons. No missing pilot duration was invented.

The corrected primary protocol activates each newly returned unique window
immediately, polls accessibility with a 60 ms delay, and requires Ready to remain
observed for one second. Six pairs alternate order: P3/C3, C4/P4, P5/C5, C6/P6,
P7/C7, C8/P8. All twelve primary observations succeeded.

| Packaged UI observation | n | Median ms | Range ms | Sample SD ms |
|---|---:|---:|---:|---:|
| Profile first observed Ready | 6 | 3,195 | 2,248–3,374 | 414.3 |
| Control first observed Ready | 6 | 2,089 | 1,916–3,284 | 649.4 |
| Profile one-second sustained Ready | 6 | 4,305.5 | 4,180–5,017 | 313.8 |
| Control one-second sustained Ready | 6 | 4,442 | 4,200–4,738 | 204.8 |

The sustained metric **includes its one-second hold**. Values are observer upper
bounds, not exact paint times. Largest primary capture durations were 29.0 ms
profile / 28.8 ms control; polling, scheduling and accessibility delivery add
uncertainty. The retained `lastNegativeLowerMs` field is the last non-Ready
capture request during the entire hold procedure, **not a first-Ready lower
bound**. Brief Ready flashes can be missed entirely.

Matched overhead estimate, profile minus control: sustained-Ready paired median
**−89.5 ms**, range −343 to +279 ms, sample SD 225.2 ms. This small noisy sample
does **not resolve a fixed instrumentation overhead**, and the negative value is
not an improvement claim. First-Ready paired median is +608.5 ms, SD 726.8 ms;
completion races and missed flashes make it unsuitable as a fixed-overhead
estimate. No startup-cohort p95 is reported with only six pairs. No Play/JFR
overhead estimate exists.

## Actual request counts, overlaps, and latency ownership

Every primary traced startup has one runtime-status request, two readiness
requests, four game audits, three game-plan metadata resolutions, three runtime
metadata resolutions, seven mod inventory scans, three runtime integrity checks,
and one Java version diagnostic. Across ten traced starts the corresponding
totals are 10 / 20 / 40 / 30 / 30 / 70 / 30 / 10. Control invocation counts are
unknown and are not inferred by multiplying their startup count.

Three audits carry the runtime/first-readiness/later-readiness correlations.
The fourth is a measured parent-zero background audit, not attributable to a
root command with this probe set. Source startup update discovery calls Client
preview, which validates the instance and scans mods; that is consistent with
the fourth audit, but **its command ownership is not proved by the trace**.
It needs an additional root probe before changing that work.

| Primary operation, same role across six runs | Median ms | Range ms |
|---|---:|---:|
| Runtime-status request | 1,647.1 | 1,517.9–2,271.4 |
| First readiness request | 1,531.1 | 1,403.2–1,761.1 |
| Later readiness request | 1,158.2 | 1,143.3–1,206.4 |
| Game audit under runtime status | 635.5 | 624.8–652.9 |
| Game audit under first readiness | 611.2 | 595.6–621.2 |
| Later readiness's game audit | 563.5 | 556.2–582.5 |
| Unattributed background game audit | 571.1 | 550.1–587.7 |
| Initial game metadata, runtime-status branch | 383.5 | 284.3–1,025.9 |
| Initial game metadata, readiness branch | 377.9 | 292.0–630.8 |
| Later readiness's game metadata | 122.8 | 115.7–146.1 |
| Runtime metadata, runtime-status branch | 30.0 | 27.8–45.1 |
| Runtime metadata, first readiness branch | 51.6 | 24.5–128.8 |
| Runtime metadata, later readiness branch | 28.5 | 27.4–43.7 |
| Runtime integrity under runtime status, including diagnostic | 382.1 | 373.8–385.3 |
| Java version diagnostic, included above | 284.6 | 277.6–288.6 |
| First readiness's runtime integrity | 93.7 | 92.0–98.4 |
| Later readiness's runtime integrity | 95.7 | 94.4–100.9 |

Inventory scan wall times were 156.8–223.6 ms; role medians were about 161–183 ms.
Each scan made 62 file-hash helper calls. Runtime checks made 402 such calls.
Game-audit wall time is almost entirely file I/O/digest work; most inventory-scan
time lies outside its hash timers and is not further divided into ZIP parsing,
filesystem enumeration, or compatibility work here.

Runtime status overlaps first readiness by 1,403–1,761 ms. The background audit
also overlaps initial work. Later readiness begins after runtime-status completion
and forms an additional sequential tail. Native recorder origin to final Ready
marker has median 3,159.6 ms, range 3,050.1–3,832.1 ms. Frontend initialization
markers arrive 282–294 ms after that origin; the native builder scope itself is
small. These origins are not substituted for OS creation or first paint.

HTTP acquisition probes recorded six Mojang document requests, six Fabric
requests, and three runtime-index requests per traced start. Metadata intervals
include transport/body handling and existing verification/parse work; they are
not pure network packet time. Available official hashes and verified-cache
semantics were preserved. Network variation, particularly initial Mojang
acquisition, contributes to the range; no DNS/TLS/server sub-attribution was made.

Five of six primary runs recorded Ready → Checking → Ready. The Ready flashes
lasted 28.7–461.9 ms; the remaining run recorded an obsolete readiness result
discarded before display. Source and numeric ordering agree:

1. Startup requests runtime status and first readiness independently.
2. A first readiness response can make the Home label Ready.
3. `runRuntimeStatus` completes and requests readiness again for the same selected
   instance/account generation. `refreshPlayReadiness` sets busy, displaying Checking.
4. The later authoritative response makes the label Ready again. If the later
   request began first, the serial guard discards the older response instead.

This is measured duplicate **presentation refresh work** and a source-confirmed
trigger. It is not a failure of final launch validation, an account change, or
permission to trust frontend readiness for launch. No request was coalesced or
cancelled by P0.2.

### Remaining Track A questions

Changed-instance selection is **BLOCKED/unmeasured**: only the existing selected
instance was used, there were zero selection-command events, and no owner instance
was copied or created to manufacture a workload. Request generations remained
the initial instance generation; no changed-selection count is claimed.

Four audits occurred by settled Home **before pressing Play**. Additional audits
inside Play preparation, session restoration, final content-lock wait, final
locked validation and exact game spawn are **BLOCKED/unmeasured**, not zero-cost
operations. Their dormant probes remain feature-gated. The existing Play path
still deeply validates the current instance, exact runtime, compatibility and
usable authenticated/entitled session, then revalidates under the final lock.
None of these launch-security checks is an optimization candidate for removal.

No standalone inventory blocking worker ran on Home, so there are no blocking
queue-wait samples. This does not establish absence of scheduler/IPC queueing.
No session-restoration span ran during the measured Home path; actual Play session
work cannot be inferred from that absence.

## Unrelated IPC during readiness

Two separate diagnostic processes each issued 50 real `get_application_status`
requests at 100 ms intervals. These are excluded from matched startup overhead.
Frontend request/complete markers measure round trip on one frontend clock.

| IPC sequence | n | Median ms | Empirical p95 ms | Worst ms |
|---|---:|---:|---:|---:|
| Run 1, all requests | 50 | 2.1 | 6.0 | 114.4 |
| Run 2, all requests | 50 | 2.2 | 6.3 | 110.8 |
| Run 1, no validation request pending | 23 | 2.2 | 2.8 | 2.9 |
| Run 2, no validation request pending | 22 | 2.3 | 3.0 | 3.1 |

The large tails occurred while runtime/readiness requests were outstanding;
pending groups had 27/28 samples and maxima 114.4/110.8 ms. The native unrelated
IPC spans themselves were at most 2.950/2.836 ms in those processes. Most of the
large round trip therefore lies outside that service body's execution. Exact
native IPC-to-frontend correlation and separation of dispatcher delay, WebView
scheduling/rendering and executor contention are missing. This establishes a
startup-associated IPC tail, **not an isolated causal estimate of validation's
queue delay**. Each p95 describes a correlated timed sequence; the 100 requests
are not 100 independent startup trials.

## Minecraft / Fabric / Client startup — BLOCKED

No new game process, startup JFR, Fabric/mixin/class-loading measurement, Aurora
initialization timing, resource reload/pack timing, active shader timing, window
creation timing, usable Minecraft menu, or supervised game exit was collected.
All Track B startup attribution and matched profiling overhead remain BLOCKED.
An installed pack/mod/shader was not assumed enabled. P0's late-attached JFR and
P0.1's coarse menu upper bounds do not satisfy these missing observations.

P0.1 documented that normal startup of JNA 5.17 removed two protected marked
native temporary files, then retained the failed comparison and explicitly
restored their original bytes. All four protected JNA temporary objects matched
before and after P0.2. JNA's source confirms that its temporary-file cleanup uses
the configured temporary directory and deletes marked old libraries. Running
the unchanged as-is game again would repeat a known preservation risk.
[JNA 5.17 source](https://github.com/java-native-access/jna/blob/5.17.0/src/com/sun/jna/Native.java)

Under the user's protection and stop conditions, Play was gated for owner review.
No protected files were silently normalized, removed, moved, or restored. A
possible next arrangement is a separately reviewed development-only JNA temporary
directory under a fresh disposable root, identically applied to profiled and
unprofiled games while preserving managed natives, integrity, runtime selection,
classpath and final validation. That launch-configuration deviation is **not
implemented or accepted here**.

A Java 21-compatible explicit safe JFR allowlist must then be reviewed and tested
before real collection. `settings=none` alone is insufficient evidence that custom
events are disabled: event annotations can supply enabled defaults.
[Java 21 event configuration](https://docs.oracle.com/en/java/javase/21/jfapi/configuration.html)
No unvalidated startup JFR configuration is presented as accepted, and no sensitive
custom/process/environment/file/socket event data was captured by P0.2.

There are **no newly verified Minecraft/Client startup bottlenecks to rank**.
Client recommendations stay separate from Launcher findings: first establish
safe matched startup JFR/menu/exit observations, then rank only supported JVM,
Minecraft, Fabric, Aurora and third-party attribution. No rendering/gameplay
change or Client optimization is justified by this incomplete Track B evidence.

## Ranked Launcher findings and future experiments

These are measured costs and source-confirmed duplicate work; **measured
optimization improvements: none**.

| Rank | Verified finding | Narrow future candidate; mechanism / risk / validation / rollback |
|---|---|---|
| 1 | Four full game audits per traced Home startup; 4,677 verification calls each; roughly 0.56–0.64 s per primary role, almost all file I/O/hash time | First experiment: avoid the unconditional second startup presentation readiness refresh when the initial request already belongs to the current instance/account generation and no relevant mutation occurred. Potentially avoids its ~1.16 s sequential request, including one audit and two inventory scans. This is an expected mechanism, not a measured gain. Risk: stale generation or incorrectly hidden invalidation. Test current-generation/obsolete-result races, instance/account changes, runtime failures and all mutation invalidations; prove final Play/locked checks still reject tampering. Repeat matched packaged trials. Roll back one focused commit. |
| 2 | Seven inventory scans per start, 157–224 ms each; substantial work outside hashing | After request ownership is fully traced, test sharing a single presentation inventory within one unchanged-generation readiness operation. Do not share across mutation or use it to authorize Play. Risk: stale compatibility/provider decisions. Test mutation/race invalidation and final locked rescans; rollback separately. No persistent inventory cache is proposed from these data. |
| 3 | Repeated official metadata resolution; initial branch median ~0.38 s with a 1.03 s observed maximum | A later experiment could reuse a resolved presentation plan only within an explicitly unchanged operation/generation. Preserve hashes, exact version/platform rules and provenance; never use frontend URLs or unverified metadata. Risk: stale pins/selection. Test version/rule changes and trust failures; isolated commit. First measure whether request deduplication already removes the relevant work. |
| 4 | Runtime-status integrity interval ~382 ms includes a ~285 ms `java -version`; readiness separately performs two ~94–96 ms runtime checks | Consider presenting one authoritative runtime result to both status surfaces, with explicit invalidation and independent launch-time validation. No diagnostic/security-check removal based on a frontend flag. Test damaged/exact-runtime and stale-generation cases; separate rollback. |
| 5 | IPC tails ~111–114 ms while validation requests were pending; native service bodies <3 ms | Add precise IPC dispatch correlation and root coverage before considering blocking-work isolation. Offloading synchronous work is an untested hypothesis and may change lock/TOCTOU behavior; require lock-order, cancellation, damage, IPC and real-launch verification. |

The parent-zero background audit needs its owning update-command probe before any
change. Its existence and cost are verified; its necessity/redundancy is not.
Removing file verification, weakening cache reuse, relaxing ownership/path/hash
rules, bypassing final locked validation, trusting UI Ready, changing authentication
or updater authority, or modifying Minecraft behavior are excluded candidates.

## Verification and preservation

Frontend: `npm run check`, zero errors/warnings; all 229 tests pass. Native:
format check passes; all-target checks pass both ordinary and feature builds;
ordinary tests 841 passed / 29 ignored plus six icon tests; feature tests 845
passed / 29 ignored plus six icon tests. Four existing Rust test warnings remain.
All three Tauri production-profile builds completed, and all three actual artifacts
booted. Client: all 443 tests and build pass using preserved original source bytes;
deployed JAR identity is reproduced. Early frontend import/test and Client CRLF
checkout failures were corrected/retested or retained as environment exclusions.

Evidence validation passes: all ten strict numeric traces, sanitized metadata
patterns, local links and 22 critical repository files. The retained copies of
all three executables and 69 frontend files match their artifact manifests.
Regenerating the analysis produces a byte-identical summary; the observer notebook
passes JavaScript syntax checking. [Validation record](docs/performance-p0-2/evidence/document-validation.json)
records these checks without opening an application or changing owner data.

Before both commits, name-status and whitespace/deletion diffs were inspected.
No original tracked file was removed. The preservation comparison checks 15,595
original Launcher files (only 18 authorized source files changed), 426 Client
files (unchanged), 11,822 protected managed files (unchanged), and the installed
Launcher (unchanged). A further pre-boot baseline verifies all 1,219 files across
the existing shared managed runtimes unchanged after collection. Root documentation,
manifests, lockfiles, frontend package metadata and native configuration remain.
No generated build/game/runtime tree is staged. Existing diagnostics, private
inventories, P0/P0.1 evidence and all protected JNA files are retained.

Acceptance remains **PARTIAL**, because changed-instance selection, actual Play
preparation and game-launch instrumentation, a reviewed startup JFR, usable
Minecraft menu/normal supervised game exit and Track B attribution/overhead were
not satisfied. No requirement is silently waived. Stop for owner review.
