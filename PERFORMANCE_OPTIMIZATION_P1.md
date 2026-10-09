# Aurora Performance P1 — Startup readiness orchestration

**Status: COMPLETE for P1 implementation and matched startup performance acceptance.**
Minecraft/Play acceptance remains **BLOCKED** by the existing JNA temporary-file
preservation issue; no game was launched and no end-to-end Play verification is claimed.
Prepared 2026-10-09. No push, publication, release or application installation.

Across **20 alternating matched pairs**, the candidate reached sustained Ready
**1,253.5 ms earlier at the paired median** (all 20 pairs improved; range
997–1,695 ms). Every baseline startup made **2 readiness requests / 4 game audits**;
every candidate made **1 / 3**. The remaining background audit was preserved.
This is an actual before/after observation, not the historical 1.16-second
duration of the removed request.

## Source and artifact identity

Initial owner checkout: `e62129cc6cacdaffbc15d323d2af669dc3f3ab5d`, branch
`codex/client-3-production-authority`, with no tracked modifications. The
pre-existing 15,143 untracked files were inventoried and protected before editing.
P0/P0.1/P0.2 reports, evidence index, architecture and native contracts were read.

Baseline reference: `6cbabd38bc1c5268b9f6c80034f18da3a26f1e21` (unchanged P0.2
instrumentation). The baseline executables are the retained, provenance-recorded
P0.2 artifacts, re-hashed before use. The intervening owner HEAD adds P0.2 reporting;
it introduces no native source difference for this comparison.

The single independently revertible optimization commit is
**`0d68340f0e280978f675c6f83330a10fb97bf678`**, source tree
`53d12b25bd1dfd3a1f0b75f8d5c5c18e675c749b`.
Research/reporting is committed separately. All 493 candidate tracked source
inputs were hashed and checked against each of three disposable build copies.
Post-build comparisons found zero semantic differences and no native changes.
Source-list canonical SHA-256:
`0ec6604331d22332c1d69de491e438575c3fdda636d5dd23d75b1dc39c182ee2`.

| Executable | SHA-256 |
|---|---|
| Baseline research | `21cf2b08bfcd95af8a78e9ad33dabbb3a276b9a6543f06abb3ff78fbb82993e9` |
| Candidate research | `bcefce604ee02f72a866676fee768bf9b40bddcaa78d906cf0f3b8d6fa87aaba` |
| Baseline production feature-off | `b23182d171a491ae180ee88ea8a848a899e47d059e5f7d6ae030606017445ce9` |
| Candidate production feature-off | `3b064d5e8977c9c49b46b271836f3600f35b94823dc8c3c6f697906f1c712ec0` |
| Baseline separate IPC diagnostic | `bd198762e20307272f132ae9f92f213a6798d8294d9caba70385558789067e55` |
| Candidate separate IPC diagnostic | `ddf18043c0b75f28c066e27906e7e97b9d377f1240eba23e2a1588150a72b11b` |

Research frontend trees are respectively
`6fa20eb305f8a20bb403951c18d0264e834e2c6ffe3fada54d13a7d3b43ebaf2` and
`29f6baf08cf420f0c27a93c9ee9ea0de674b6e9a60332948c8b3c2c72e9e45a8`.
The [evidence index](docs/performance-p1/README.md) links every artifact manifest,
individual embedded-frontend input hash, toolchain identity, source snapshot,
numeric trace and sanitized trial observation. Private full inventories, logs,
temporary executables and build inputs are retained locally.

## Root cause and implemented change

Startup loads application status and launcher state, starts the selected
instance's runtime-status request, then loads releases/accounts and dispatches
presentation readiness. Previously the runtime-status method unconditionally
dispatched another readiness request after successful status completion. Both
readiness calls could overlap and publish out of order. The later call set
Checking again, although `get_instance_runtime_status` is read-only and
`get_play_readiness` independently validates the runtime and other prerequisites.

`src/lib/launcher/store.svelte.ts` now separates the private read-only status
request from the public explicit status action. Startup retains the initial
readiness promise and its serial, selected IDs, mutation revision and complete
in-memory launcher/account/process DTO values. It waits for runtime status and
the initial readiness result, whichever completes first. It omits the follow-up
only if that exact request remains current, its decision is Ready, all recorded
inputs remain equal, and runtime status is Ready. A newer readiness request owns
publication and startup does not supersede it.

Readiness results and errors now use the same serial/revision/input/identity
guard. Stale results cannot overwrite newer state; only the latest serial clears
the busy state. Runtime responses have their own serial and selected-context
guard. Disposal invalidates pending readiness and runtime serials.

`refreshState()` conservatively increments the mutation revision at both entry
and completion. This covers content mutations absent from summary DTOs and a
check started during the reload. `updates.svelte.ts` passes `false` only for its
optional, read-only startup discovery reload. The returned DTO values are still
compared: an actual input change invalidates readiness even there. Comparing
values matters because a harmless reload produces new object identities.
The comparison string exists only in memory; it is neither persisted nor a
native trust decision. No native command, cache, executor or validation rule changes.

## Invalidation, failure and race analysis

| Condition | Result |
|---|---|
| Runtime completes before readiness, or readiness before runtime | Wait for both; one healthy current initial result is sufficient. No redundant Checking tail. |
| Runtime finishes before accounts dispatch readiness | The initial readiness request still runs; startup waits for it. |
| Instance/account IDs or same-ID account/configuration/pin DTOs change | Old publication fails its input guard. A successful runtime completion triggers a fallback unless a newer request already owns publication. Existing selection/account actions dispatch fresh checks. |
| Mutation callback reload, including equal summary DTOs | Revision invalidates checks both before and during reload. Existing callbacks continue requesting fresh readiness. |
| Newer explicit refresh during startup | It owns publication, including busy state. Startup never starts another request over it. Repeated explicit actions are not coalesced. |
| Initial readiness fails, is blocked, or is invalidated | A successful current runtime-status operation retains the necessary follow-up. Genuine Checking remains visible. |
| Runtime status returns damaged/non-Ready | Initial Ready alone does not qualify for omission; follow-up remains. |
| Runtime-status request fails | Its error remains visible; independently returned readiness is preserved. No success-triggered follow-up is invented. |
| Account load fails | A successful runtime-status result retains the original fallback readiness path with the available account context. |
| Old response/error after newer selection/check | It cannot replace the current result. An old runtime response cannot trigger another refresh. |
| Navigation or disposal | Navigation uses the existing root-owned store; disposal discards pending publications/follow-up. |

Existing explicit runtime checks, runtime installation/replacement, account
refreshes, instance selection/configuration installation, validation/repair and
mod mutation callbacks retain their fresh-readiness calls. Provider/content
operations are not converted into cached checks. P1 does not create a filesystem
watcher or claim that a DTO detects arbitrary external disk writes. The existing
explicit check actions and native launch validation remain responsible for those.

## Matched trial protocol and actual counts

One Windows 11 Home build 26300 host, i7-14700F (20 cores / 28 logical processors),
RX 9070 XT, same power scheme and owner session. Same existing selected
instance/account, settings and managed content; ordinary warm caches and online
network. No cache purge, reboot, profile/data-root isolation or account change.
Rust 1.98.1, Cargo 1.98.1, Node 24.21.0 and npm 11.19.0. Release optimization,
LTO, one codegen unit, abort and stripping remain matched. Both primary builds
use unchanged P0.2 instrumentation; the IPC workload is a separate artifact.
All builds/tests/hash sweeps finished before primary collection. Owner background
activity and network service variation remain uncontrolled.

Primary order: odd pair `baseline → candidate`, even pair `candidate → baseline`,
20 pairs / 40 successful starts, zero failed or excluded primary observations.
Every start refuses competing Launcher/Java processes. The exact research
executable starts through structured process APIs, with capture opt-in only in
that child. The observed source-built window is activated and normally closed
after observation. Complete exit-written traces are required. A window close
and complete trace are recorded; child exit codes were not collected.

UI polling has a 60 ms delay. First Ready requires Play/Ready without Checking;
sustained Ready requires a one-second uninterrupted hold, reset by any later
non-Ready observation. Observation continues at least eight seconds, allowing
late startup work to complete. Only numeric traces and sanitized UI observations
are retained; raw UI text, screenshots and application stdout/stderr are not saved.

| Startup operation, actual count per primary run | Baseline | Candidate |
|---|---:|---:|
| Readiness requests | 2 | 1 |
| Runtime-status requests | 1 | 1 |
| Game-integrity audits | 4 | 3 |
| Parent-zero background game audit | 1 | 1 |
| Game metadata resolutions | 3 | 2 |
| Runtime metadata resolutions | 3 | 2 |
| Mod inventory scans | 7 | 5 |
| Runtime-integrity audits | 3 | 2 |
| Java `-version` diagnostics | 1 | 1 |
| Mojang / Fabric / runtime HTTP spans | 6 / 6 / 3 | 4 / 4 / 2 |
| Native dispatch spans / unrelated ordinary IPC spans | 3 / 3 | 2 / 3 |
| Play / Java game spawn spans | 0 / 0 | 0 / 0 |

Each individual game audit still made **4,677 file-hash calls** and each runtime
audit **402**, in both builds. Total calls fall only because redundant audits
are no longer requested: game 18,708 → 14,031; runtime 1,206 → 804. These are
per-span counts, not a claim about unique files across overlapping audits.

The remaining parent-zero audit has no proved command owner in the P0.2 probe
set. Source inspection is consistent with background update preview work, but
that is an inference, not causal attribution. P1 leaves it intact. Inclusive
span timings and nested hash timings are not added into a fictitious critical path.

Native/frontend markers record Ready → Checking → Ready in **19/20 baseline**
and **0/20 candidate** starts. The remaining baseline discarded an obsolete
initial response. Accessibility polling saw only **5/20** baseline flashes,
demonstrating that brief Ready intervals can be missed; candidate had zero.

## Matched latency distributions

All values below are milliseconds, n=20 per build. p95 is the nearest-rank sample
percentile; SD is sample standard deviation. OS/UI observation upper bounds,
native-origin time and frontend-init time are distinct clocks.

| Metric | Baseline median | Baseline range | p95 / SD | Candidate median | Candidate range | p95 / SD |
|---|---:|---:|---:|---:|---:|---:|
| UI first Ready upper bound | 3,199 | 1,870–3,426 | 3,408 / 554.7 | 2,000 | 1,887–2,127 | 2,072 / 59.6 |
| UI sustained Ready upper bound, including hold | 4,328 | 4,096–4,619 | 4,490 / 138.8 | 3,056.5 | 2,924–3,206 | 3,129 / 67.4 |
| First Ready marker after native origin | 1,936.4 | 1,729.7–3,006.5 | 2,174.4 / 266.2 | 1,856.8 | 1,776.6–2,023.3 | 1,962.2 / 65.4 |
| Final Ready marker after native origin | 3,181.1 | 3,006.5–3,460.1 | 3,337.0 / 122.1 | 1,856.8 | 1,776.6–2,023.3 | 1,962.2 / 65.4 |
| Final Ready marker after frontend init | 2,886 | 2,707.7–3,138 | 3,035.9 / 119.1 | 1,561.5 | 1,481.8–1,700.2 | 1,641.5 / 60.4 |

| Paired candidate minus baseline | Median | Range | p95 | SD |
|---|---:|---:|---:|---:|
| UI first Ready upper bound | −1,189.5 | −1,472 to +257 | +101 | 569.3 |
| UI sustained Ready upper bound | **−1,253.5** | **−1,695 to −997** | −1,113 | 169.8 |
| Native first Ready marker | −106.5 | −1,068.9 to +293.6 | +140.7 | 272.3 |
| Native final Ready marker | −1,348.8 | −1,677.1 to −1,068.9 | −1,165.8 | 146.9 |
| Frontend final Ready marker | −1,356.4 | −1,651.5 to −1,066.2 | −1,169.9 | 140.0 |

The paired sustained improvement is the main acceptance measurement. The
difference of cohort medians is separately 1,271.5 ms; it is not the paired
median. The first-Ready UI difference overstates what can be inferred about
first validation because polling misses brief baseline Ready intervals. Native
first-Ready evidence shows a much smaller and variable difference. P1 primarily
removes the late Checking tail, rather than proving a one-second acceleration
of the first necessary validation.

Worst primary UI capture duration was 122.6 ms; observer attachment was at most
110 ms after OS process creation. Poll delay, capture duration, scheduling and
the one-second hold affect observation bounds. No exact paint time or fixed
subtraction for observer overhead is claimed.

## Instrumentation and unrelated IPC diagnostics

After primary trials, four adjacent, order-balanced feature-on/off pairs were
run per source (16 starts total); they are excluded from primary timings.
Instrumented minus feature-off sustained-Ready delta:

| Source | n | Median | Range | SD |
|---|---:|---:|---:|---:|
| Baseline | 4 | −30.5 | −506 to +339 | 347.1 |
| Candidate | 4 | +224.5 | −86 to +257 | 163.3 |

All eight feature-off starts reached Ready and emitted **no research trace**
despite the child capture opt-in. The small noisy sample does not identify a
fixed instrumentation cost or justify subtracting one from primary results.
Both primary sides were instrumented equally. Feature-off candidate sustained
Ready median was 3,057 ms (range 2,955–3,202); baseline was 4,341 ms
(4,182–4,743), descriptive n=4 only, with no p95.

Separate IPC artifacts ran in balanced `baseline1 → candidate1 → candidate2 →
baseline2` order. Each issues 50 ordinary application-status IPC calls at a
100 ms schedule during startup (in addition to three ordinary startup IPCs).
No diagnostic IPC workload enters the main 40 starts.

| Run | n | RTT median | Range | p95 | SD |
|---|---:|---:|---:|---:|---:|
| Baseline IPC 1 | 50 | 1.751 | 1.199–112.601 | 3.700 | 16.342 |
| Baseline IPC 2 | 50 | 1.600 | 1.201–3.400 | 2.899 | 0.524 |
| Candidate IPC 1 | 50 | 1.700 | 1.200–110.199 | 2.699 | 15.340 |
| Candidate IPC 2 | 50 | 1.599 | 1.300–109.399 | 2.500 | 15.234 |

Pending-validation IPC medians were 1.7 / 1.5 ms baseline and 1.5 / 1.5 ms
candidate; settled medians were 2.099 / 1.800 and 1.749 / 1.600 ms. The numeric
traces retain each subset's n/range/SD and p95 only where n≥20. Both builds have
outliers; two runs per source do not establish a general IPC latency improvement
or an executor diagnosis. No executor changes were made.

## Verification and production security

| Verification | Result |
|---|---|
| Frontend type/check | 0 errors, 0 warnings |
| Complete frontend tests | 252 passed, 0 failed; includes 23 new deterministic orchestration tests |
| Rust formatting | Passed |
| Rust all-target checks, ordinary and `performance-p0-2` | Both passed |
| Ordinary Rust tests | 841 passed, 29 ignored; 6 icon integration tests passed |
| Research Rust tests | 845 passed, 29 ignored; 6 icon integration tests passed |
| Production feature-off release build | Passed; real executable booted four times |
| Instrumented release and separate IPC build | Both passed; real executable startup traces collected |
| Evidence/schema/privacy/source/artifact checks | Validated by the accompanying read-only validator |

Tests exercise the actual Vite-compiled store and typed native adapter with
deferred command completion, rather than a duplicate state-machine model. They
cover both completion orders; runtime before account dispatch; obsolete results
and errors; changed instance/account and same-ID inputs; content mutation with
equal summaries; checks started during mutation reload; newer callback ownership;
failure/blocked/non-Ready decisions; account/runtime failures; repeated actions;
rapid navigation; explicit runtime check/install; process changes and disposal.
Failure acceptance is deterministic and fixture-based: owner game files and
credentials were not damaged or revoked to manufacture live failures.

Native source, DTOs, error contracts, lockfiles, dependencies, Tauri identifier,
release endpoints, authentication registration and updater authority are
unchanged. Existing four Rust library warnings and the deprecated
`STATIC_VCRUNTIME` build warning remain. Rust checks ran before the final
frontend correction; exact unchanged native source makes their results applicable.
All final frontend tests/checks and all three release builds use the final candidate.

SHA/file-integrity verification, ownership/path containment, runtime and provider
compatibility, authentication/entitlement, launch preparation, final content/session
locks and locked validation/spawn remain in Rust. `play_instance` still performs
fresh authoritative checks; frontend Ready cannot authorize native launch alone.
Zero startup Play/session-lock/spawn spans mean those paths were not exercised,
not that checks were removed. No persistent validation cache, timestamp trust,
system Java selection or Minecraft Client optimization was introduced.

Both sides reuse the same approved updater public-key configuration (SHA-256
`bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`),
without changing signatures or release authority. Build output is an executable
with embedded static frontend, built with `--no-bundle`; no installer was run.

## Preservation, excluded work and limits

Initial private inventory records HEAD/history/remotes/status, all 492 tracked
files, all 15,143 untracked files, 27,238 existing managed files and the installed
executable. Final hash comparison checked 42,874 original files: **zero missing,
zero unexpected changes**, all original untracked files byte-identical, all
24,193 protected managed files byte-identical. Only the three requested existing
tracked files changed; the new deterministic test is an added file.

Normal packaged startup changed 44 existing operational files under WebView
profile/cache/log storage and derived `cache/server-enrichment`. These changes
were paused for investigation and are reported separately, never described as
byte preservation. Instances, artifacts, runtimes, accounts/configuration and
other managed content are not exempted. The installed executable is unchanged,
SHA-256 `15bfd5d71bbe73c9ad7f3010c052f3f1669d81abfa4790b8c48efb8b169fd868`.

The historical P0.2 protected-content baseline independently matched all 11,821
files. Seven JNA-named managed files, including both existing temporary DLL/marker
pairs, were explicitly hash-checked unchanged. No broad cleanup, recursive
deletion, `git clean`, application installation or owner data rollback occurred.
Original diagnostics and benchmark evidence remain unchanged. Before each commit,
tracked name-status/deletions, critical files and whitespace were inspected;
generated build/game/runtime data and private inventories are not staged.

An earlier object-identity implementation failed in real candidate trial 7:
an equivalent read-only reload produced two readiness requests. All **23** starts
from that source and all six artifact manifests are retained in
`evidence/pre-correction`, excluded from final acceptance. Superseded source
`f1e42c29531e88c37587058e3d4153c84d769567` remains on local branch
`codex/performance-p1-pre-correction`. The implementation was corrected and the
entire final 20-pair cohort restarted. No successful-looking UI result was used
to excuse the duplicate native request.

Notebook setup encountered an unsupported eval path, a vm/binding mismatch and
an old-root closure. Exclusive trial directories prevented evidence overwrite.
The final corrected pilot attached about 43 seconds late and is excluded from
latency acceptance. The final dataset has **61 starts: 40 primary, 16 overhead,
4 IPC, 1 pilot**, with 53 numeric traces (eight feature-off controls have none).
Pre-correction evidence is separate; no historical P0/P0.1/P0.2 timings are pooled.

Results apply to healthy startup of this existing selected instance/account on
this one warm, online Windows host. Cold starts, offline behavior, multiple live
instances, changed accounts/content and other platforms were not benchmarked.
Those orchestration transitions are deterministic-test covered, not claimed as
live performance measurements. The existing JNA preservation blocker prevented
Minecraft/Play acceptance; it remains an explicit owner review gate.

## Rollback and P2 recommendation

Revert only the optimization with:

```sh
git revert 0d68340f0e280978f675c6f83330a10fb97bf678
```

This restores prior frontend orchestration and removes the added tests/architecture
note without touching native behavior or managed data. The separate evidence commit
can remain as a record. No persistence migration or cleanup is required.

For P2, first add causal attribution for the parent-zero background audit and
repeat a bounded matched experiment. Its owner is not established by P1, so its
removal is not justified. Inventory caching, metadata reuse, hash deduplication,
runtime changes, executor changes and Client optimization require their own phase,
correctness model and acceptance evidence. Stop here for owner review.
