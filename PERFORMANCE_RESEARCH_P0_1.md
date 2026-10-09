# Aurora Performance P0.1 — owner-authorized as-is baseline

Date: 2026-10-09, America/New_York. **Verdict: as-is measurements completed;
the original fully controlled P0.1 matrix remains partial.** Stop for owner review.
No performance optimization or production change is included.

## Result and revised authorization

The owner superseded the separate-account requirement with: **“we dont need
another windows account, run the check as-is.”** Measurements therefore used the
everyday installed executable, current Windows account and existing managed
instance. No second Launcher competed with it. The original BLOCKED report and
preflight are historical evidence, not the current authorization boundary.

The previous Minecraft PID was absent, no Java process remained, and Launcher
was closed before the pilot began. One bounded supervisor-log header records
exit code 0 for the earlier session; this is consistent with normal exit, but
that historical log does not retain the previous PID. No process was terminated.

New observations include 29 packaged starts, 20 sustained-readiness observations,
20 repeat entries into each content tab, three actual Play-to-menu launches,
one observed mod-refresh completion, and two approximately 60-second idle states.
All three games exited through the visible Quit button and have supervisor exit
code 0. Launcher and Java were closed at the final check.

This is a black-box baseline of the installed payload. Exact native readiness
invocation/audit counts, causal stage attribution, complete artwork-decode timing,
true filesystem-cold runs, offline runs and a source-proven current build remain
unmeasured. The report does not claim that the original controlled acceptance
criteria have all passed. Repeating black-box trials cannot recover those missing
internal markers.

## Identities and preserved state

| Item | Identity |
|---|---|
| Launcher source at resume | `1142a1bb1640dd7ac39512c63440a6362673a4f4`, `codex/client-3-production-authority` |
| Production source investigated in P0 | `fad40125f8fd87b8fa0893c97827e8e4a890081b`; production files unchanged since P0 |
| Client source | `5bdf3aa30bc7bfa9a226170a9c54ca5ec3fd8e1d`, `codex/unified-theme-pilot` |
| Installed Launcher SHA-256 | `15bfd5d71bbe73c9ad7f3010c052f3f1669d81abfa4790b8c48efb8b169fd868`; 19,211,264 bytes |
| Existing local release SHA-256, not run | `33b07bafdb93868db68232a8a72bbcdd98dbd8c9a167bbff215d036cd13349aa`; 19,540,480 bytes |
| Deployed Aurora 3.0.0 JAR SHA-256 | `43f918207a86f91b01b35045309e89d2b040951722e5ac71d01d886beec9811c` |
| UI-declared workload | Minecraft 1.21.11, Fabric 0.19.5, Aurora 3.0.0; 31 mod rows, four resource-pack rows, one shader row |

Launcher version remains 1.4.1. **The installed binary's exact source revision,
compiler/profile and embedded frontend bundle identity are unknown.** Neither
source HEAD nor version text proves its provenance. Client HEAD likewise does not
prove the deployed JAR's source. No binary was rebuilt, installed or replaced.

At resume, both repositories had no tracked changes. Launcher had 431 tracked
and 15,143 protected untracked files; Client had 422 tracked and four untracked
files. Launcher was 6 ahead / 0 behind cached `origin/main`, with no upstream;
Client was 0/0 against its own tracking branch. Read-only remote default queries
still returned Launcher main `4575a0e3f1c28da6cfe072948fe91b60eeaf608f` and Client
master `d8eecd89b9187619b3ad7a47adf97cd74d4f1c77`. No fetch, push or publication ran.

Before UI use, all 11,820 retained P0 managed hashes matched. An additional existing
resource-pack content file was protected separately; 11,821 managed files were
checked afterward. The UI's four pack rows are the observed current workload,
not P0's inherited three-pack fixture. Installed rows do not establish enabled
mods, active packs or shader activation; existing game preferences were retained
and not exported.

## Storage and environment boundary

No isolated environment is claimed. The owner-authorized as-is run shares these
domains deliberately:

| Domain | What was established |
|---|---|
| Managed application data | Existing platform-resolved owner tree; hashed protected files, no alternate root or copied account database |
| Instance paths | Existing validated managed identity and files; no clone, new instance, junction, external game path or `.minecraft` use |
| WebView storage | Same Windows account/application context; no independent profile configured; actual profile path/contents were not exported or independently instrumented |
| Windows Credential Manager | Existing user and fixed application credential namespace; no enumeration/copy/alternate store; Play exercised normal authenticated operation |

The debug-only `AURORA_DIAGNOSTIC_DATA_ROOT` remains insufficient for full isolation
and is ignored by release builds. Changing managed roots would not isolate
WebView storage or the per-user credential namespace. No such override was used.
Normal session restoration may rotate an OS credential; its phase was not traced,
so credential immutability is not claimed. No password, token, account database,
world, log or configuration was copied into another environment.

Physical host: Windows 11 Home build 26300, i7-14700F with 28 logical processors,
31.76 GiB RAM, Radeon RX 9070 XT driver 32.0.31041.1004. The existing power scheme
was Ultimate Performance; no setting changed. Desktop resolution was 2560×1440;
returned captures were 1122×791 for Launcher and 2048×1152 for Minecraft. These
capture dimensions are not an independently measured Minecraft framebuffer size.

The host kept its normal network and existing populated caches. No reboot, cache
eviction, token-expiry manipulation, firewall/proxy change, install/update, world
entry or server connection was performed. Background applications were not closed
or fully controlled. No build, preservation hashing or game ran during idle
collection. CPU/GPU utilization of unrelated applications, GPU resource usage and
the process watcher's perturbation were not quantified.

## Measurement method and limits

Windows UI actions used the Computer Use skill's `node_repl` and `@oai/sky` API.
Each input followed an inspected returned UI state. Raw accessibility/account/
server text and screenshots were not saved in research artifacts. The notebook
retains only numeric timings, closed boundary labels, counts and outcomes.

UI durations start at the **automation request**, before input delivery, and end
at the first matching accessible state. They include helper/input/capture/polling
overhead. The target polling pause was 60 ms, but actual observation intervals were
often much wider. They are observational upper bounds, not exact physical-click,
image-paint or compositor timestamps. Raw last-negative capture-start and positive
capture-end values retain uncertainty. A first positive with no negative has
the entire request-to-observation interval as its uncertainty range.

First Ready required an enabled Play control and Ready text on Home. Trial 8's
immediate full-state follow-up showed Checking again. The later cohort therefore
also required Ready/enabled Play without Checking for at least one second of
successive observations. This operational settled-readiness proxy **includes the
one-second hold** and does not prove every internal request is finished. The first
nine starts were closed after the first-Ready observation; the next 20 continued
through this stability criterion. First-Ready collection used the same prefix in
both cohorts; its combined distribution is exploratory, not randomized.

Two harmless Instances navigation actions verified input response after startup.
Their timing begins at that later navigation request, not process creation.
Window discovery can find a blank/loading WebView, so it is not substituted for
first visible nonblank paint. Native frontend-ready/paint/IPC markers were absent.

The process observer read installed-executable identity, PID, parentage and OS
creation time only, never child arguments or output. **A Java child is not always
Minecraft:** each Play also produced an earlier auxiliary Java child. The game
child was paired with the independently created supervisor log: exactly one
same-parent Java creation followed it within one second, at a rounded 0–1 ms
offset in all three trials. Earlier children were excluded. This is external
correlation, not instrumentation of the returned native child handle. Log headers
were read only to 96 bytes, excluding stdout/stderr. Public timing exports remove
PIDs and paths; exact parentage remains in ignored local evidence.

UI intervals use monotonic `performance.now()`. OS creation/log times use wall
clock, paired to each action's `Date.now()`; no independent drift calibration ran.
Millisecond-precise numbers in raw records do not justify sub-millisecond claims.
The first game trial had a gap between Play and window polling; its combined
window/loading screenshot timing is reported separately. Minecraft exposes no
usable UIA menu tree here. Screenshots distinguish the loading overlay from the
menu, and normal Quit responses establish usability afterward. Manual observation
gaps include model/tool time. The previous screenshot's return time is **not** a
strict lower bound on menu presentation; no menu p95 or tight paint latency is claimed.

## New packaged observations

Milliseconds below are observed upper bounds, except explicitly labeled OS creation
differences. Empirical p95 uses nearest rank only for n≥20. Raw sample standard
deviations/variances, first values, worst repeat and failures are in the summary.

| Boundary / condition | n | First | p50 | Empirical p95 | Worst |
|---|---:|---:|---:|---:|---:|
| Startup request → first accessible Ready; new process, warmed files | 29 | 2,245 | 3,222 | 3,499 | 3,547 |
| OS process creation → same first-Ready proxy | 29 | 2,208 | 3,195 | 3,473 | 3,516 |
| Startup request → Ready sustained ≥1 s; later cohort, includes hold | 20 | 4,677 | 4,440 | 4,734 | 4,852 |
| Later Instances navigation request → accessible list | 2 | 321 | 247 | — | 321 |
| Mods → 31 rows, repeated in final process | 20 | 223 | 234 | 249 | 252 |
| Resource Packs → four rows, repeated in final process | 20 | 160 | 257 | 266 | 267 |
| Shaders → one row, repeated in final process | 20 | 221 | 233 | 249 | 489 |
| Explicit Mods refresh, busy state observed → complete | 1 | 404 | 404 | — | 404 |

The first entries into Mods/Packs/Shaders in the final process were respectively
386/272/207 ms and are excluded from the 20-repeat distributions. Earlier-process
observations and pilot trials are also separate. Repeated-start median excluding
the first start was 3,231 ms. No filesystem-cold first call is claimed.

Median observation widths were 109 ms for first Ready and 169/194/169 ms for
Mods/Packs/Shaders. The worst shader sample's width was 422 ms; it cannot establish
a 489 ms application render stall. Tab measurements verify accessible rows and a
ready Refresh control, not all artwork decoded, viewport rows painted or each
native scan/IPC stage independently complete.

The defined startup and 20-repeat content series had no failed observations.
Five saved pilot records are excluded; pilot errors included a wrong workspace
predicate and an incorrect expected pack count. Outside the repeat series, one
30-second resource-tab attempt left the Mods page visible. It is retained as an
unsuccessful observation; a fresh screenshot and coordinate action reached Packs
within 294 ms. Attribution to application versus observer/input remains unresolved.
Resource-pack and shader refresh probes did not observe their transient busy state
within three seconds, so they are **inconclusive**, not three-second refresh times
or application failures. Geometry/input errors before valid trials are excluded
and described in the evidence index.

Settled screenshots showed both real artwork and fallback tiles in the installed
payload. Complete image decode/paint counts and timings remain n=0; they cannot
validate current source's artwork correction. No fallback was changed.

### Actual game startup, paired per trial

| Trial / trigger | Play request → correlated game OS creation | OS creation → game window observation | Play request → menu without overlay, upper bound | OS creation → menu, upper bound | Exit |
|---|---:|---:|---:|---:|---:|
| First Workspace Play in pilot process | 5,889 ms | Not separately resolved; combined window/loading snapshot by Play+19.895 s | 30.053 s | 24.164 s | 0 |
| Repeated Workspace Play, same process | 3,827 ms | 9.964–10.080 s | 25.392 s | 21.565 s | 0 |
| Home Play, same process | 2,743 ms | 10.453–10.566 s | 20.548 s | 17.805 s | 0 |

There are two Workspace and one Home trials, not 20 comparable launches per state.
No tail distribution or failure-rate assurance is inferred. Order, session/cache
reuse and trigger differ; Home is not proven faster. Java creation to window is
measured externally in the latter two trials, with observation widths of about
116/113 ms. It is a window-discovery boundary, not a first-present marker. The
loading overlay remained at the first screenshots; visible menu controls alone
were insufficient. No new startup JFR or CPU-phase recording was collected.

## Controlled idle within the as-is session

Exact Launcher ancestry plus WebView descendants were included; Java excluded.
Foreground was requested for the settled view, but continuously visible/unoccluded
state was not independently verified. The process observer remained enabled and
other owner background applications stayed in place. These are two short samples
within one process, in Workspace-then-Home order, not an isolated A/B.

| View | Duration / samples | CPU, one-core equivalent | Working set | Private bytes | Tree |
|---|---|---:|---:|---:|---|
| Workspace / Shaders | 60.53 s / six intervals | 9.19% | 551.20–551.75 MiB | 475.68–476.26 MiB | Seven processes, stable |
| Home | 60.48 s / six intervals | 8.89% | 572.05–578.87 MiB | 493.55–500.46 MiB | Seven processes, stable |

CPU is summed process CPU seconds divided by elapsed time, not whole-machine CPU
percent. Handles/threads were 5,141–5,143 / 243–244 in Workspace and 5,132–5,143 /
240–244 on Home. There was no Minecraft process in any interval. These results
do not establish a leak, retention plateau after repeated navigation, GPU cost,
or a causal memory difference between pages. No heap snapshot was taken.

## Actual readiness counts and remaining matrix

Actual command, deep-game-audit and runtime-audit counts remain **unknown**. UI
Ready/Checking transitions and auxiliary Java creations cannot count those calls.
The inherited healthy-path source model predicts three game audits at selected-
instance startup or explicit selection; selection-to-Home-Play totals five,
selection-to-Workspace-Play six. These remain conditional **predictions** about
the investigated source, not measured counts in the installed binary. Overlap,
errors and guards invalidate multiplication of P0 timings by predicted counts.

The picker had one instance. Same-instance selection is not a different-instance
benchmark; current source explicitly skips its selection command when IDs match.
The picker was opened and closed without changing selection. No artificial second
instance, duplicate modpack or copied user configuration was created.

| Requested boundary / condition | Current evidence / gap |
|---|---|
| Packaged startup / response | First-Ready and sustained-Ready proxies measured; two later navigation responses; precise first-paint/full interactive marker absent |
| Instance selection / invocation counts | One existing instance; actual selection-change n=0 and internal counts unknown |
| Play → spawn / window / menu | Three real launches; log-correlated OS child creation; two distinct window boundaries; coarse manual menu upper bounds |
| Content lists / artwork | 20 repeated row-ready observations per domain; full artwork decode/paint timing absent |
| Refresh / update checks | One observed mod-refresh completion; two inconclusive refresh probes; explicit update checks n=0, no updates applied |
| Idle / retention | Two 60-second idle states; long-term retention/navigation trend and heap/DOM metrics absent |
| Cold / warm | New-process warm-filesystem/populated-cache case only; no cache-miss or true cold series |
| Online / offline | Normal host network only; no endpoint-role attribution or offline case |
| Small / typical / large | Existing typical inventory only; no synthetic size substitute |
| Fresh auth / session reuse | Normal existing authentication; reuse/refresh phase not traced; no fresh login test |

Closing these gaps requires a source-proven payload and reviewed, privacy-safe
native/frontend markers. The existing P0.1 instrumentation design remains a design:
bounded typed correlation IDs, monotonic phase records, actual audit entries,
blocking-job queue boundaries, image-decode/DOM markers, overflow/privacy tests and
matched enabled/disabled overhead checks. Preserve production-disabled behavior,
all integrity/ownership/runtime/session/final locked launch checks and full required
startup/native verification. No such patch was implemented for this as-is run.
Future cache eviction, offline, fresh-auth or reboot work needs an appropriately
authorized disposable context; none is currently scheduled or required from the owner.

## Ranked measurement-supported roadmap

These are investigation/implementation candidates for a subsequent authorized
phase, not promised savings. Costs refer to their own boundaries and are not summed.

| Rank | Candidate and measured reason | Next discriminating experiment / security constraint |
|---:|---|---|
| 1 | Profile Minecraft/Fabric/Client startup before the window: two paired post-spawn intervals were about 10.0–10.6 s; menu observation is later | Source-proven JAR/runtime plus reviewed pre-start JFR allowlist and finer window/menu observer, paired unprofiled. Separate class loading, mixins, resource/shader work; do not infer startup causes from P0's gameplay JFR |
| 2 | Attribute Launcher preparation and startup readiness orchestration: game creation followed Play by 2.743–5.889 s; first Ready can revert to Checking; sustained proxy p50 4.440 s including hold | Count real equal-generation requests/audits, queue waits and session/metadata phases. Coalesce only proven presentation duplicates. Keep fresh preparation, exact compatibility/runtime/session and final locked validation mandatory |
| 3 | Quantify redundant provider-JAR reads before a one-pass implementation | Inherited P0 typical mod scan median 301.24 ms; current repeated UI list proxy 233.75 ms is a different, cached workload. Record actual bytes/opens/per-pass cost on unchanged disposable files, then reviewed A/B. Preserve exact hashes, ownership, paths and freshness |
| 4 | Separate idle presentation/resource cost before rendering changes | Stable short-sample tree used 551–579 MiB working set and about 9% of one CPU core. Paired fixed/static versus existing presentation in a test context; measure GPU, descendants and observer overhead. Longer navigation/settle batches before any leak claim |
| 5 | Instrument list-to-artwork decode/paint and explicit update surfaces | Warm list upper bounds were around 0.23–0.26 s, with wide observer intervals; full decode and update timings absent. Source-proven image counters/cache hit/miss and native/IPC/DOM phases before changing caching or virtualization |

P0's typical game-integrity median **1,060.60 ms**, mod scan **301.24 ms**, runtime
audit **130.05 ms** (four repeats), and cached decode **46.45 ms** for 42 PNG objects
are inherited native-component measurements. They are not new packaged timings
or independent additive costs. Whole-plan resolution overlaps integrity. No hash,
network, queue, filesystem-open or UI-blocking share was established here.

One-pass provider hashing remains a narrow, well-defined first Launcher
implementation candidate from P0, but is not demonstrated to be the largest
user-impact improvement. The new evidence puts post-spawn startup profiling and
real readiness correlation ahead of claiming that result. **No optimization is
implemented under this report.**

## Preservation, verification and review

The first post-run hash comparison detected two missing pre-existing temporary
JNA native files, a DLL and its `.x` marker. The installed JNA library is 5.17.0;
its published initialization removes marked temporary libraries and their markers.
That behavior is consistent with the observed pair, although no file-deletion
trace was captured. [JNA 5.17.0 source](https://github.com/java-native-access/jna/blob/5.17.0/src/com/sun/jna/Native.java#L1275).

Investigation stopped before commits. With Launcher/Java closed, exact missing
original paths were recreated exclusively from surviving siblings whose bytes
matched the saved original SHA-256s, verified before and after. Two files totaling
273,408 bytes were restored. No unrelated file was overwritten, deleted or reset.
The failed comparison is retained, not replaced by a passing record.

The subsequent comparison passed all **15,574** pre-existing Launcher repository
files, **426** Client files and **11,821** protected managed files, with zero missing
or changed bytes and no removed tracked paths. Report/index edits are the only
authorized changes to pre-existing repository files in the final comparison.
The installed executable SHA remains identical. New ordinary game logs/native
temporary files and reconstructable cache activity were retained; worlds/player
settings/log bodies were not inventoried or copied, so comprehensive byte equality
of mutable game/session/WebView/credential state is not claimed.

Research tools/evidence receive syntax, numeric-summary reproducibility, JSON,
privacy-field, document-link, diff/deletion and preservation checks. No application
type/Rust/build test suite was rerun: production source, package metadata, lockfiles
and native configuration did not change. UI boots here validate the installed
payload, not the current source build. No dependency, version, identifier, updater
key/release authority, integrity policy, launch argument, mod or game setting changed.

See the [evidence index and reproduction procedure](docs/performance-p0-1/README.md),
[numeric summary](docs/performance-p0-1/evidence/as-is-summary.json),
[JNA preservation record](docs/performance-p0-1/evidence/as-is-jna-preservation.json),
[post-restoration comparison](docs/performance-p0-1/evidence/as-is-after-jna-restoration-protection.json),
and [final comparison](docs/performance-p0-1/evidence/as-is-final-protection.json).
P0's [report](PERFORMANCE_RESEARCH_P0.md) and evidence remain unchanged.
The former BLOCKED report is preserved in commit `1142a1b`; its existing preflight
records remain in the evidence index. The new commits contain research only.
The independently reviewable observer/tooling commit is `2e6916a`; the separate
report/evidence commit is identified in the owner handoff.

**Stop for owner review.** No further game run, optimization, update, push,
publication or environment change is authorized by this deliverable itself.
