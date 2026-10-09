# Aurora Performance P0.1 — controlled baseline preflight

Date: 2026-10-08, America/New_York. **Verdict: BLOCKED at the owner-interaction
boundary.** This is a measurement preparation deliverable, not an optimization.

## Result and reason for stopping

**Zero new packaged lifecycle trials ran.** The owner's installed Launcher still
has a running Java child, and another Java process exists. No game was closed,
restarted, paused or profiled, and no competing Launcher was started. No UI was
activated, no account/session/credential operation ran, and no owner selection or
cache was changed.

The P0.1 request explicitly requires: "Never overwrite the owner's installed
Launcher or interfere with an active Minecraft session" and "If owner interaction,
credentials, elevation, or machine reboot is necessary, stop at that boundary and
provide a precise procedure." The first controlled restart/foreground/Play series
requires the owner to finish the current session normally. It also requires a
separate validated production test environment. These are real prerequisites,
not elapsed-time approval or permission to operate the owner's live instance.

P0's native-function measurements and gameplay JFR remain available, but do not
complete these missing end-to-end measurements. No native repeats, mocked browser,
simulated invocation count, or static call graph is substituted for a packaged run.

## Exact identities and reconciliation

| Repository | Investigated HEAD | Branch | State before P0.1 |
|---|---|---|---|
| Launcher | `8c2d7466be278d15b896589de5f50848cee43950` | `codex/client-3-production-authority` | No tracked changes; 423 tracked files, 15,143 protected untracked files |
| Client | `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d` | `codex/unified-theme-pilot` | No tracked changes; 422 tracked files, four protected untracked bytecode files |

Live remote default HEADs remain Launcher `origin/main`
`4575a0e3f1c28da6cfe072948fe91b60eeaf608f` and Client `origin/master`
`d8eecd89b9187619b3ad7a47adf97cd74d4f1c77`. Launcher has no upstream and is
4 ahead / 0 behind cached `origin/main` before P0.1 commits. Client is 0 ahead /
0 behind its own tracking branch `origin/codex/unified-theme-pilot`; that is a
different comparison from its remote default. Queries were read-only; no fetch,
push, release or publication ran.

No intervening agent changes were found. The two P0 commits add only the report,
research tools and evidence. All production source/package/native configuration
at P0.1's starting HEAD matches P0's investigated code HEAD
`fad40125f8fd87b8fa0893c97827e8e4a890081b`. Existing uncommitted tracked changes
were absent. No new optimization was incorporated into the comparison.

| Payload | SHA-256 | Bytes | What identity establishes |
|---|---|---:|---|
| Owner's installed Launcher | `15bfd5d71bbe73c9ad7f3010c052f3f1669d81abfa4790b8c48efb8b169fd868` | 19,211,264 | Same payload observed in P0; not presumed to contain current source |
| Existing local release executable | `33b07bafdb93868db68232a8a72bbcdd98dbd8c9a167bbff215d036cd13349aa` | 19,540,480 | Same local payload observed in P0; no new P0.1 build/provenance record |

Both existing payloads differ. The declared Launcher version is still 1.4.1;
version text alone cannot establish exact source identity. No executable or
installer was rebuilt, installed, replaced or run in P0.1. A future baseline
must record exact source commit, compiler/profile, frontend bundle hash, updater
trust configuration identity and executable/installer SHA. Secret build values
must not enter the record. Client source HEAD does not prove the deployed 3.0.0
JAR contains that revision; retain release artifact digest provenance separately.

Full Git statuses, tracked inventories and hashes are local in
`docs/performance-p0-1/private/`. The P0.1 inventory deliberately includes all
pre-existing P0 documents, evidence and tooling. Critical root documentation,
frontend manifests/lockfile and native build configuration remain protected.
Website was not accessed. Repository historical architecture snapshots are
interpreted against current code, not as current release/authentication policy.

## Isolation finding: a data-root override is insufficient

`src-tauri/src/paths.rs:18` honors `AURORA_DIAGNOSTIC_DATA_ROOT` only under
`#[cfg(debug_assertions)]`. Release builds ignore it. Setting the variable and
launching the production executable would still open the owner's platform
application-data directory. `application.rs:511` and native setup use Tauri's
platform resolver. No release-root mechanism was implemented here.

The debug override covers `ManagedPaths`, not every external storage domain.
There is no explicit independent WebView data directory in the current Tauri
window configuration. `auth/credentials.rs:177` derives the Windows credential
target from the account UUID under the fixed application namespace; it does not
derive it from the managed root. A temporary instance root therefore does not
isolate Credential Manager for the same Windows user. Session restoration can
rotate the credential, so copying account summaries or testing Play in a second
same-user process is not an acceptable isolation shortcut.

Launch exclusion is process-local. A fresh competing process does not inherit
the owner's supervised-child state. It cannot prove that the owner's instance
is idle just because its own snapshot says stopped. This further rules out
unisolated restart/Play testing while the owner game remains active.

**Preferred test environment:** a separate owner-provided Windows test account
or VM with its own platform application-data, WebView profile and OS credential
store. Keep `com.aurora.launcher`, production version/release configuration,
updater key/authorities and normal authentication/launch semantics unchanged.
This uses OS isolation rather than changing the production security namespace.
A VM is safer for offline and reboot tests; its timing results must be labeled
virtualized and never pooled with the original physical-machine baseline.

No environment is assumed prepared. Before use, validate isolation separately:

1. Resolve the actual app-data and WebView profile in that environment without
   exporting private paths. Confirm they differ from the owner's locations.
2. Start with no copied accounts/credentials/history. Missing authentication must
   remain an honest blocker. The owner signs in manually through the system browser
   only after the test-profile isolation passes; never copy or enumerate credentials.
3. Use distinct validated managed UUIDs/independent regular files. Validate registry,
   manifests, artifact hashes and runtime identity through existing native checks.
4. Prove UI selection/cache/metadata changes touch only the test profile; compare
   owner's protected state before/after. Refuse links/reparse escapes. Do not point
   the test profile at the owner's data through junctions, hard links or `.minecraft`.
5. Preserve updater trust and compiled authorities. Check is permitted; installing
   or publishing updates is outside the measurement task.
6. Review any development-only timing patch separately; require production-disabled
   behavior, privacy/overhead tests and the repository's full startup/native checks
   before using its packaged artifact. No patch is implemented or validated here.

A new compile-gated root mechanism is an alternative design only if OS isolation
is unavailable. It would have to isolate WebView state as well as managed storage,
fail closed for unsafe roots and retain unchanged Credential Manager semantics.
It would still require a separate credential context for authenticated Play. It
must never be added by merely enabling debug assertions on a nominal release build:
other debug behavior would then confound the comparison.

## New end-to-end timing table

All entries have **n = 0**. Dashes mean not observed; no latency or failure-rate
distribution exists. No failed attempt was relabeled a successful timing sample.

| Boundary | n | p50 | Empirical p95 | Worst | Status / required observer |
|---|---:|---|---|---|---|
| Process → visible window | 0 | — | — | — | Blocked; exact test PID → first visible nonblank window |
| Process → interactive UI | 0 | — | — | — | Blocked; first shell paint plus a harmless navigation response |
| Selection → updated UI | 0 | — | — | — | Blocked; input → new selection committed/rendered |
| Selection → verified readiness | 0 | — | — | — | Blocked; matching native result plus publication; account requirement preserved |
| Play → Java spawned | 0 | — | — | — | Blocked; real authorized Play → returned exact child handle |
| Java spawn → Minecraft window | 0 | — | — | — | Blocked; supervised-child native window observer, no title/arguments logged |
| Java spawn → usable main menu | 0 | — | — | — | Blocked; menu controls usable, not merely CLIENT_STARTED or Running |
| Mods tab → populated list | 0 | — | — | — | Blocked; expected row count / DOM commit / paint marker |
| Mods tab → available artwork rendered | 0 | — | — | — | Blocked; validated identities and successfully decoded available images; unavailable icons separate |
| Resource Packs → populated list | 0 | — | — | — | Blocked; scan/IPC/expected rows/paint |
| Shaders → populated list | 0 | — | — | — | Blocked; scan/IPC/expected rows/paint; installed shader files do not prove activation |
| Content refresh → complete | 0 | — | — | — | Blocked; explicitly name content domain; final inventory/artwork publication |
| Update check → complete | 0 | — | — | — | Blocked; signed Launcher/Client check and provider-content check are separate workloads |
| Controlled idle resources | 0 | — | — | — | Blocked; fixed Home/Workspace and visibility state, no game/build competing load |

First/subsequent process, first/repeated selection, populated/empty reconstructable
caches, online/offline and true filesystem-cold cases all remain unmeasured in
P0.1. No genuine large modpack was provisioned. A large workload is conditional on
a lawful compatible pack and the same validation criteria; duplicate/conflicting
JARs are never a size substitute.

## Readiness invocation and audit count: source model, not observed counts

There are no new packaged invocation counts. The following conditional healthy-
path model identifies what the future trace must test. It assumes ready content,
successful official plan resolution/runtime status, no concurrent mutation, no
extra explicit check and a launchable account. Errors/early blockers change it.

| Trigger | Source call pattern | Predicted game audits if all branches complete |
|---|---|---:|
| Startup with a selected ready instance | `loadInitialStatus:232` starts runtime status and readiness; runtime success refreshes readiness | 3: one runtime-status plan resolution, two readiness resolutions |
| Explicit selection | `runSelect:679` starts runtime status and readiness; runtime success refreshes readiness | 3 |
| Runtime status completion alone | `runRuntimeStatus:741` refreshes readiness after the already-completed runtime work | 1 additional |
| Home Play after completed selection | `runPlay:359` calls `play_instance`; launch preparation resolves plans and final boundary audits again | 2 additional; selection-to-Home-Play model totals 5 |
| Workspace Play after completed selection | `runWorkspacePlay:669` explicitly refreshes readiness, then same Play pipeline | 3 additional; selection-to-Workspace-Play model totals 6 |

These are **predictions, not five or six measured audits**. Startup is a separate
sequence and must not be silently added to selection trials. Workspace navigation
does not always invoke selection. Failed runtime status does not trigger its success
refresh; Play guards or authentication failure can prevent the final audit.
Concurrent jobs can overlap, so multiplying P0 duration by these counts would not
produce a valid selection/Play latency estimate.
Runtime provisioning (`runEnsureRuntime`) is a separate workload: its successful
completion also requests readiness, but installer validation is not included in
the runtime-status-only row above. Actual provisioning counts remain unmeasured.

Native mapping: `get_instance_runtime_status:4500` → `validate_instance_runtime`
→ `resolve_instance_runtime_plan:1210` → `resolve_instance_launch_plans:1229`
→ `resolve_instance_game_plan:1256` → `validate_instance:1433`
→ `install::validate_installed_game:749`. Runtime status also requests the bounded
Java diagnostic. `calculate_play_readiness:5424` reaches that same deep-game path
and validates runtime without Java execution. Play preparation (`application.rs:5572`)
does deep game/mod/runtime validation plus diagnostic/session restoration. The
locked `LaunchSnapshot::with_validated:69` repeats complete-instance validation
and compatibility before structured spawn. No additional internal double game
audit is assumed inside a single plan-resolution call.

Required security work remains: preparation validation, exact runtime/session/
compatibility checks, snapshot consistency and final locked validation/spawn.
Independent equal-generation presentation requests are a potential duplicate;
their actual number, necessity after mutations and queue cost require observation.
Do not remove security checks based on this call model.

## Per-stage attribution and instrumentation design

No causal stage attribution was measured in P0.1. Before a timing patch, attempt
safe external CPU sampling and packaged WebView timelines in the isolated profile.
WPR is available, this caller is not elevated, and WPA is not installed. No
elevation, ETW recording, new JFR or DevTools session was requested or started.

Generic process/file/network traces are not automatically suitable: Java process
arguments contain a reusable token, filesystem events contain private paths,
network events can contain server addresses, and heap snapshots can retain user
identities. Do not record raw ProcessStart arguments, File I/O filenames, request
bodies or full heap/console dumps and promise to sanitize later. Use a reviewed
allowlist/exporter whose retained schema excludes those fields at collection.

If external facilities cannot distinguish phases safely, the proposed separate
development-only patch would collect bounded, typed records in memory:

| Fields | Allowed values / purpose |
|---|---|
| run/trial/request/parent sequence | Ephemeral integers only; no UUIDs, names or hashes of private identity |
| command, trigger, stage, outcome | Closed enums: startup/selection/runtime completion/Home Play/Workspace Play; success/block/error class |
| start/end ticks, clock kind | Rust monotonic ticks and frontend `performance.now()`; calibrated correlation offset and uncertainty |
| files/bytes/requests/active/queued | Counts; no paths, URLs, payloads, DTO/debug dumps or argument arrays |
| dropped-record count | Overflow invalidates attribution; capped storage, no persistent telemetry |

Instrument native command entry/exit, actual deep-audit entry/exit, mod/runtime
checks, official metadata request roles, blocking-job enqueue/start/end, session
reuse/refresh **phase only**, final content-lock wait/audit and spawn return. Carry
correlation across async and blocking boundaries; temporal proximity alone cannot
assign overlapping work to the right request. Count every audit, including jobs
whose obsolete result the frontend rejects. Never log `LaunchSpec`, sessions or
command arguments/results.

Frontend markers surround actual invocation send/completion, reactive DOM commit
and image decode completion. Record expected/visible row and usable-image counts,
not DOM text or account display. A `requestAnimationFrame` marker is not a precise
compositor-present timestamp; label it and report observer polling uncertainty.
Cross-clock IPC residual is not pure queue time unless native entry/start and
serialization/transport phases are also measured. Attribute bytes/open/read/hash
updates separately with sanitized counters, or use validated CPU samples; summed
file-read wall time does not establish CPU hashing time. Do not subtract medians
of unmatched runs as a network estimate.

Probe unrelated harmless IPC while validation is active and while idle; compare
response distributions with matched conditions and job concurrency. One async
function containing synchronous I/O proves a scheduling concern, not measured UI
starvation. For retention, use numeric WebView heap/DOM/listener metrics and stable
process-tree private memory across matched navigation batches; working set alone
does not establish a leak. Avoid raw heap snapshots containing private strings.

Validation before use must cover compile-time production absence, bounded record
storage/overflow, secret/path field exclusion, cancellation/stale publication,
correlation across worker boundaries, unchanged outcomes/locks/argument assembly,
and all required type/Rust format/check/test/production-build/real-boot checks.
Measure enabled-versus-disabled observer overhead in paired fixture runs. This
design is not an implemented or validated instrumentation mechanism.

## Precise owner procedure and benchmark matrix

1. **Owner boundary now:** finish the current Minecraft session normally and close
   its Launcher after supervised exit. Identify the other Java application's owner
   and finish or hold its workload consistently; never terminate Java by name.
   Resume only after the owner has made a test session available. No reboot or
   elevation is needed merely to release this boundary.
2. Prepare a separate Windows test profile/VM and pass the isolation gates above.
   The owner supplies the interactive system-browser sign-in when authenticated
   Play is required. Do not share account documents or credentials with the live
   profile, change application IDs or invent authentication.
3. Rebuild the exact reviewed source without dependency/version/release changes.
   Record source/artifact/toolchain/profile identities. If timing instrumentation
   is necessary, validate and commit it separately before benchmark collection.
   Retain an otherwise matched uninstrumented build for overhead checks. Install
   into the test profile only; never replace the owner's installed executable.
4. Reuse P0's minimal game/two-required-JAR fixture as source data and a reviewed
   typical inventory of 31 mods, three resource packs and one shader pack. Build
   independent managed test instances and deeply validate them. Copy only approved
   game/content/runtime/release/ownership files, no accounts/history/worlds/logs.
   Inspect absolute ownership metadata before copying; do not make unvalidated
   provider claims. Keep original instances byte-exact. Record actual installed
   counts, digest pins, active packs/shader and enabled mods after preparation.
5. Define fixed viewport/theme/visibility, supported GPU/driver/runtime, power state,
   observer version, endpoint reachability and background processes. Freeze update
   installation and gameplay configuration for the series. A VM remains its own
   machine class; physical-machine timings need a later isolated physical session.
6. Use separate test-profile snapshots for populated and absent **reconstructable**
   caches. Preserve manifests/game/runtime and user content. Treat absent metadata/
   artwork caches separately from artifact stores; never purge the owner's caches.
   First call in a new process with warmed files is not filesystem-cold.
7. Collect first/subsequent process starts and first/repeated selection separately.
   Alternate small/typical blocks to reduce order bias. Target at least 20 comparable
   successful observations per critical boundary and state, with full settle between
   actions. Log failures/blockers separately. Use normal exact-child/game exit
   between Play trials; never issue a second Play while Starting/Running.
8. Offline tests use only the disposable VM/test network. No host firewall/proxy or
   live security-setting change. Missing cache or expired session failures remain
   failures. Fresh online session reuse versus real refresh are separate cases;
   never manipulate token expiry to manufacture them.
9. True filesystem-cold testing requires an owner-approved isolated-machine reboot
   or separately validated safe cache-control procedure. Stop before that owner
   boundary. If unavailable, report warmed filesystem only. Do not purge host caches.
10. Record at least 60 seconds per controlled idle state after settled startup,
    matching process start identities and all WebView descendants, excluding Java.
    Then compare equal navigation batches and retention after settle; require a
    trend beyond allocator/cache high-water behavior before claiming a leak.
11. Report raw sanitized durations, n, first-call result, repeated median,
    nearest-rank empirical p95 when n permits, worst **all** and worst repeat,
    sample standard deviation/variance, failures, background load and uncertainty.
    Do not pool online/offline, cache miss/hit, small/typical or profiled/unprofiled.

For game startup, use safe matching Java-21 diagnostic tools and a pre-start JFR
allowlist, explicitly disabling custom Minecraft events as P0's audit demonstrated.
Keep environment/properties/arguments/file/socket/custom-network payloads absent.
Measure JVM/Fabric/Client/resource/shader phases without treating CLIENT_STARTED
as the first usable menu. Pair with unprofiled launches and actual menu observers.

## Focused questions: current answers

| Question | Evidence-backed answer |
|---|---|
| How many full game audits selection → Play? | Actual count unknown; conditional source model is 5 for Home, 6 for Workspace; must be traced |
| Required versus presentation duplicates? | Final locked audit plus launch preparation/runtime/session/compatibility are required; equal-generation presentation jobs are candidates, not removed |
| Hash/open/metadata/network/queue shares? | Unmeasured; P0 inclusive medians cannot be decomposed by subtraction |
| Unrelated IPC delayed by synchronous validation? | Unmeasured; source shows synchronous hashing within async paths |
| Launcher preparation versus game loading? | Unmeasured; P0 recorded already-running gameplay only |
| Lists: native scan/artwork/frontend? | Unmeasured end-to-end; P0 has native component costs, not render dominance |
| Retention beyond bounded caches? | Unmeasured; P0's stable uncontrolled process tree cannot prove leak absence/presence |

## Optimization priorities and first experiment

No new verified user-impact bottleneck is established. P0's largest separately
observed local components remain game integrity (typical median 1,060.60 ms), mod
scan (301.24 ms) and runtime audit (130.05 ms, four repeats). These are carried-
forward exploratory values, not fresh controlled P0.1 timings. Whole plan resolution
overlaps integrity; do not rank it as another independent bottleneck. Cached PNG
decode was 46.45 ms for 42 objects; this does not prove artwork is the leading delay.

Priority 1 is the isolated packaged baseline and real request/audit counts.
Priority 2 is a read/open/hash-count experiment on an unchanged disposable provider
JAR inventory to quantify the second pass in `instance_mods.rs:777,817`. Priority 3
is correlation of equal-generation readiness requests and unrelated IPC waits.
Priority 4 is storage/worker-count and native/frontend attribution. Client startup
and retention require their own controlled traces before implementation proposals.

**Does one-pass hashing remain the best first implementation?** It remains the
smallest well-defined Launcher implementation candidate from P0, because the same
entry is read for expected SHA-256 and again for revision. Its saved cost is still
unmeasured. P0.1 has not established that it is the best first change by user impact.
The recommended first **measurement experiment** is the controlled provider-JAR
read-count/per-pass attribution run, followed by a separately authorized one-pass
A/B only after baseline review. Do not implement hashing or readiness coalescing
under this request. Final fresh hashing, ownership, path, compatibility, exact
runtime/session and locked launch checks remain mandatory.

## Evidence, verification and stopping point

- [Evidence index and read-only tools](docs/performance-p0-1/README.md).
- [Source/build/process preflight](docs/performance-p0-1/evidence/preflight.json).
- [Preservation verification](docs/performance-p0-1/evidence/protection-verification.json).
- Full inventories/status/history/live remote responses and path-level verification
  are local only under `docs/performance-p0-1/private/`, excluded from Git.
- [Previous P0 report](PERFORMANCE_RESEARCH_P0.md) and
  [previous native observations](docs/performance-p0/evidence/summary.json) remain
  unchanged, with their original methodology and limitations.

Only P0.1 research documents, safe preflight evidence and read-only helper scripts
are added. No production source, manifest, lockfile, dependency, release version,
identifier, updater authority/trust, credential target, launch behavior or user
instance is changed. No production checks/build/boot were rerun because no such
code is edited. Helpers receive syntax/preflight and preservation checks; these
are not application validation or benchmark success.

The independently revertible read-only tooling commit is
`6fc1887c809c1e63840b9fe030dad03a0150d40a`. It contains no runtime instrumentation.
The separate report/evidence commit is identified in the final handoff.

Final hash comparisons passed for all **15,566** pre-existing Launcher files,
**426** Client files and **11,820** managed-content/state files protected by the
retained P0 inventory: zero changed/missing files and zero removed tracked paths.
This covers the protected game/content/state set, not mutable worlds, logs or
player settings belonging to the running game. All critical repository files
remain present. The helper scripts parse, evidence JSON and document links pass,
and the private inventories are ignored; no private path-level output is staged.

The next action belongs to the owner: finish the current session normally and
provide an isolated test profile/session. This phase stops at that boundary for
owner review. No new primary timings or actual invocation counts are claimed.
