# Aurora Performance P2 — Background audit attribution

**Status: attribution complete; retain the audit. No production optimization.**
Collected 2026-10-09. Instrumentation and reporting are separate commits. No push,
release, installation, publication, installed-Launcher replacement or Minecraft
launch. Stop for owner review.

The remaining background audit belongs to the actual native
**`startup_update_check` command**, through the internal full Client update
preview. Six matched attributed starts prove this by request-span IDs and parent
chains. The audit validates installed state before release selection; it is not
a second official-metadata resolution or an independent provider update check.
Its median inclusive duration was **576.5 ms**, entirely overlapping pending
readiness. It finished **569–903 ms before first Ready**. There is no direct
readiness dependency on its result; the experiment does not identify an indirect
contention cost or a counterfactual latency saving.

The [evidence index](docs/performance-p2/README.md) describes reproduction,
numeric events, exclusions, privacy and all retained artifacts.

## Preparation and exact source identity

Initial HEAD was `6633a66c5c80d9af8e1cfc5b503adffe39ac5baa`, on
`codex/client-3-production-authority`. Its parent is the required P1 optimization
`0d68340f0e280978f675c6f83330a10fb97bf678`. The intervening commit adds reporting;
there is no application/native/frontend input difference. Initial tracked status
was clean. AGENTS.md, ARCHITECTURE.md, P0.2/P1 reports and both evidence indexes
were reviewed, including their attribution, preservation and authority limits.

The private baseline records status, history, the complete tracked-file set,
critical configuration presence and path-level hashes before source edits:
604 original tracked files, 15,143 protected untracked files, 124 additional
ignored diagnostic/evidence files, 24,193 protected managed files, 3,066 separate
operational WebView/server-cache files and the installed executable. Existing
diagnostics and all old performance evidence remain protected.
[Preflight](docs/performance-p2/evidence/preflight.json)

Instrumentation commit: **`b8591ae78f9ba319dfd07f2d56dbe12085c272b4`**.
Only four existing Rust files change: numeric event vocabulary and scopes in
`performance`, `application`, `updates::client` and `instances::lifecycle`, plus
one disposable-fixture correctness test. All validation branches, returned
results, command DTOs, errors, dependencies, locks and persistence remain as P1.
There is no frontend change, caching, audit suppression, offloading or reuse.

Builds use two new disposable, byte-checked source copies and separate compiler
targets, release optimization/LTO/one codegen unit/abort/stripping, unchanged
Launcher 1.4.1 / `com.aurora.launcher`, and the approved updater public-key digest
`bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`.
The [source snapshot](docs/performance-p2/evidence/source-snapshot.json) records
every compiled tracked input. Both builds verify all 604 inputs after building;
Tauri's disposable Cargo-manifest line-ending rewrite is checked canonically.
No original manifest is normalized or replaced.

| Artifact | Source | Executable SHA-256 |
|---|---|---|
| Retained P1 optimized research baseline | `0d68340` | `bcefce604ee02f72a866676fee768bf9b40bddcaa78d906cf0f3b8d6fa87aaba` |
| P2 attribution research | `b8591ae` | `44879472599960a424e5c81783efe953fa01f063842be0e1b038aa6c6c7b6d06` |
| P2 ordinary feature-off production | `b8591ae` | `24f64728e5bb6e52d1a4f3bf8398daaed9029b85a69abbf6e7c07fc6f12ba987` |

All three executables and embedded frontend files are re-hashed. Canonical
frontend source is identical to P1: 119 checked inputs, including 28 with only
checkout line-ending differences. Of 23 embedded files, 17 match byte-for-byte;
the remaining six match exactly after replacing SvelteKit's build-version
timestamp/global identifier and the consequent generated chunk filenames.
This investigated build variation is retained in
[frontend equivalence](docs/performance-p2/evidence/frontend-equivalence.json),
not treated as unexplained application changes. Tool versions and ordinary
warm/online conditions are in [environment](docs/performance-p2/evidence/environment.json).

## Exact owner and measured call chain

Frontend startup installs the update listener, reads the local overview, performs
P1's read-only `launcher.refreshState(false)`, then starts
`UpdateController.startup()` without awaiting it from readiness. The typed adapter
invokes `startup_update_check`; it does not invoke `preview_client_update` at boot.

The native chain is:

```text
application::startup_update_check                     [39, actual command]
  run_update_check(startup = true)                    [once-per-process gate]
    updates::launcher::run_check                      [46, separate domain]
    updates::client::check_client_update
      check_client_update_with_url                   [42]
        fetch_published_manifest                     [47]
        preview_update                               [43, internal helper]
          registry/configuration/Aurora pin and state consistency
          merged local release authority lookup
          lifecycle::validate_instance               [44]
            install::validate_installed_game          [20, deep game audit]
            active bootstrap/provider checks and three-way consistency
            instance_mods::scan                       [23, dependency problems]
          select_candidate                           [published semver/exact environment]
          candidate-only conflicts/preflight, if applicable
          fingerprint_update                         [45]
            instance_mods::scan                       [23, approval snapshot]
            installed game manifest and ownership/provider snapshot
        preview -> UpdateAvailability                [fingerprint not returned]
    finish_check + selected-instance update-center record
    update_overview + update-status emission          [48]
    return the same overview to the command caller
```

Concrete proof from
[attribution primary 1](docs/performance-p2/evidence/trace-attribution-01.json):

| Event | Meaning | Request/span ID | Actual parent ID |
|---|---|---:|---:|
| 39 | Startup native command | 9 | 0 |
| 42 | Client discovery | 15 | 9 |
| 43 | Internal Client preview | 17 | 15 |
| 44 | Instance validation | 18 | 17 |
| 20 | Game-integrity audit | 19 | 18 |

Every one of six primary P2 traces has this same complete ancestry. Each also
contains exactly one event 39, zero manual-check event 40 and zero explicit
preview-command event 41. The validator checks parent existence, span containment,
balanced begin/end edges and ancestry, rather than assigning ownership by clock
proximity. Ordinary command correlation remains numeric zero, but the unique
root request ID and explicit parent chain prove ownership. The other two audits
remain beneath the independent runtime-status/readiness request dispatches.

P0.2 previously instrumented the inner audit without this command/helper context,
so the audit appeared parent-zero. In each matched P1 baseline it still does.
P2 records the missing context; it does not remove an invocation. Each P2 start
has 95 numeric records versus P1's 75, with zero dropped/overflow, cancelled or
unfinished spans. The pilot proves the same chain but is excluded from timings.

## Necessity, result use and trust boundaries

**The audit is an installed-state verification gate in the existing full-preview
contract.** `preview_update` requires a ready, configuration-matching registry
record, consistent Aurora ownership/pins and resolvable installed release
metadata. It then consumes `validate_instance` and refuses non-Ready validation
before selecting a candidate. Registry `ready` alone is not evidence that game
bytes are still valid.

The game audit reads `installed-game.json`, rejects malformed/future documents,
checks every recorded file's size and SHA-1/SHA-256/observed-digest trust, and checks
the natives directory. Instance validation adds Minecraft/platform/Aurora
consistency, active bootstrap/provider integrity, dependency problems from local
inventory, and pack validation where applicable. These remain read-only and
download-free. An observed digest remains a consistency reference, never promoted
to an official expected digest. The audit verifies installed facts against their
recorded trust; it is not a fresh remote authority or independent signing proof.

Semver ordering and exact Minecraft/Loader/Java compatibility select release
eligibility. The full game hash is not an input to semver comparison. It is a
separate prerequisite for the current preview's claim that this installed
instance can be considered for an update. Replacing that contract with a lighter
"a release exists" query would be a behavior/security design change, not proved
redundancy. **Retain this audit.**

Failure is consequential. The new deterministic test
`discovery_requires_deep_game_validation_even_without_a_new_release` changes one
byte in a disposable synthetic client JAR while preserving size. For both an
up-to-date release and a newer published release, healthy discovery first returns
the expected result, then damaged preview returns `Inapplicable` with the
validation gate, and discovery returns `NotApplicable` instead of an offer or
`UpToDate`. Registry/Aurora state and damaged bytes remain unchanged. This proves
the digest result is consumed and that skipping it would change update-preview
correctness even when no new release exists. No owner file was damaged.

This is separate from a transport failure: the existing loopback offline test
returns `Unavailable` while independently confirming the instance still validates
Ready. Update-service failure never itself degrades Play. Explicit update/apply
continues to re-fetch/re-derive preview and compare the approved fingerprint,
including a fresh under-lock preview and final deep validation at commit.

At startup, `check_client_update_with_url` converts only outcome/current/candidate/
notes into availability; the update center retains that availability. It does not
retain the validation object, game audit result or approval fingerprint. Subsequent
readiness/runtime requests independently validate; a later explicit update obtains
a fresh preview. No startup result authorizes activation or launch. All primary
traces contain zero Play/preparation/final-lock/game-spawn events, so no subsequent
game/update transaction is claimed as a live workload here. Result consumption
is established by source control flow and the corruption test; traces contain no
result/error payloads and do not independently encode an availability label.

**Official metadata is correlated separately.** Every P2 startup has two game-plan
and two runtime-plan resolutions, with four Mojang, four Fabric and two runtime
index HTTP spans. All belong to the runtime-status/readiness branches; there are
**zero** such descendants under startup update discovery. Client discovery has
its own published Client-manifest HTTP interval, preceded by independent Launcher
discovery. There is no startup Modrinth candidate check implied by the two local
mod inventory scans. Those scans establish local bytes/ownership/dependencies;
they do not confer new provider authority.

Client HTTPS bootstrap discovery and expected artifact SHA-256 authority remain
unchanged. Launcher signing, public key, endpoints and official updater lifecycle
remain unchanged. Authentication/entitlement, runtime exact identity, path and
ownership checks, compatibility and final locked launch validation are untouched.

## Matched startup measurements and critical path

Six adjacent matched pairs, alternating P1→P2 for odd pairs and P2→P1 for even
pairs; 12 successful primary starts, no primary exclusions. One preliminary P2
pilot and two later feature-off controls are separate, for 15 starts total.
No old P0/P0.1/P0.2/P1 measurements are pooled into this series.

Same Windows 11 Home build 26300 host, i7-14700F, owner selection/account/settings,
warm installed artifacts/OS/WebView caches and ordinary online services. No cache
purge, reboot, account/instance change, installation or data-root isolation.
All build/test/hash-sweep work ended before the primary series. Remote response
bytes, DNS/TLS/service timing, OS page-cache temperature and owner background
activity are uncontrolled; matching does not claim identical network responses.
The fixed source endpoints and acquisition policies are unchanged.

Each start checks no competing Launcher/Java, verifies the exact executable,
sends application stdio to the OS null sink, activates its uniquely discovered
source-built window and closes it normally after observation. Poll delay is
60 ms; observe at least eight seconds; sustained Ready includes a one-second
hold reset by any later non-Ready sample. UI times are observation upper bounds,
not paint times. Native-origin and OS-creation clocks are kept separate.

| Measured invocations per primary startup | P1 | P2 attribution |
|---|---:|---:|
| Readiness / runtime-status requests | 1 / 1 | 1 / 1 |
| Total game audits | 3 | 3 |
| Background audit | 1 parent-zero/unattributed | 1 proved descendant of command 39 |
| Mod inventories, all branches | 5 | 5 |
| Mod inventories under background command | unproved by old probes | 2 |
| Game / runtime metadata resolutions | 2 / 2 | 2 / 2 |
| Official metadata descendants of background command | unproved by old probes | 0 |
| Game hash calls per audit | 4,677 | 4,677 |
| Runtime hash calls per audit | 402 | 402 |
| Mod scan hash calls per scan | 62 | 62 |
| Java version diagnostics / Minecraft spawns | 1 / 0 | 1 / 0 |

| P2 attributed interval, n=6 | Median ms | Range ms |
|---|---:|---:|
| Whole startup update command, inclusive | 1,169.7 | 1,149.9–1,217.5 |
| Separate Launcher discovery | 78.2 | 69.3–91.2 |
| Client discovery, inclusive | 1,096.6 | 1,065.0–1,125.7 |
| Client published-manifest HTTP/body/parse | 160.4 | 148.2–207.1 |
| Full internal Client preview, inclusive | 917.7 | 915.7–944.9 |
| Instance validation, inclusive | 745.6 | 740.6–771.1 |
| Background game audit, inclusive | **576.5** | **570.1–582.6** |
| Audit file-open/read/digest wall time | 568.8 | 562.5–574.9 |
| Fingerprint construction, including its scan | 172.4 | 169.3–177.5 |
| Correlated mod scans, n=12 | 161.3 | 157.9–179.3 |

Nested timings must not be summed as independent cost. File-open/read/hash wall
time occupies 98.57–98.71% of the background audit; this is not pure hashing CPU.
The fingerprint interval includes a separate inventory scan and other reads,
serialization and hashing; this probe does not divide its residual time further.

The audit overlaps the pending readiness request for its entire 570.1–582.6 ms
in all six runs. It overlaps neither displayed first Ready nor the subsequent
Ready hold. Audit completion precedes the native first Ready marker by a median
661.6 ms (569.0–903.1); whole update-command completion precedes it by 323.9 ms
(227.9–554.7). All 12 primary startups have one Ready transition and no subsequent
Checking, in both native/frontend markers and UI observations.

**Direct critical-path contribution:** no awaited dependency from readiness to
the update command, and no update-dependent Ready/Checking tail. Its result is
not a readiness prerequisite. **Indirect contribution:** synchronous validation
can contend for disk/CPU/executor capacity while other requests are pending;
this attribution-only experiment does not quantify that interference. Since all
audits complete before Ready, the traces cannot demonstrate Ready while one is
still running or prove that suppressing it would leave first-Ready latency
unchanged. Removing a 576.5 ms audit is not evidence of a 576.5 ms startup saving.

| OS/UI upper-bound metric, n=6 per research source | P1 median (range) ms | P2 median (range) ms |
|---|---:|---:|
| First Ready | 1,920 (1,833–2,236) | 2,056 (1,913–2,405) |
| Sustained Ready, includes hold | 2,988.5 (2,900–3,311) | 3,137 (2,918–3,415) |

Paired P2-probe minus P1 sustained Ready: median **+88.5 ms**, range −133 to
+459, sample SD 224.8 ms. First Ready: +109 ms, range −125 to +510, SD 227.9 ms.
These are added-probe/ordinary-variation observations, **not an optimization
benefit or a fixed instrumentation cost**. Do not subtract them from audit times.
Native first-Ready median is separately 1,785.9 / 1,893.6 ms. Worst primary UI
capture was 36.7 ms; observer attachment was at most 276 ms after OS creation.
No p95 is reported with six pairs.

Both feature-off production boots reached sustained Ready (3,060 and 3,114 ms)
and emitted no trace despite capture opt-in. Their counts are unknown, not
inferred from instrumented runs. They are boot/privacy controls, not matched
overhead inputs. Normal window close plus a completed trace is recorded;
Launcher process exit codes were not collected.

## Redundancy assessment and recommendation

Required native work includes the installed-state validation gate and its
dependency/ownership checks. The second inventory scan constructs a transaction
fingerprint. Its returned **fingerprint value is definitely unused by discovery**,
but constructing it also reads/validates ownership, retained/provider state,
inventory and the game manifest; failures propagate to discovery. The validation
scan and fingerprint scan have different consumers and observe the filesystem
at different times. They are not automatically interchangeable, and no result
reuse is justified by their proximity. A healthy-path discarded value is not
proof that the full 172.4 ms interval is safely redundant.

There is optional presentation duplication: native update discovery emits an
overview and returns that same overview; the listener and startup promise both
route to `UpdateController.receive`. Equal deliveries can repeat local snapshot
publication. That is presentation work, with no new validation request. Actual
duplicate-render counts/cost are not instrumented here. The initial local
overview refresh and read-only launcher-state refresh are also optional startup
presentation/orchestration, but this phase does not prove their removal safe.

**Recommendation: retain the audit and all native scans/checks. No production P2
optimization is justified by this dataset.** There is no measured optimization
benefit because no optimization was implemented. There is no defensible numeric
expected startup saving from deleting or caching this audit.

If the owner wants a follow-up, the smallest separate presentation-only candidate
is suppressing an exactly equal duplicate overview publication, with deterministic
event/promise ordering and changed-instance/offer tests. Expected benefit is
limited to an unmeasured local publication/render cost; it does not eliminate
this audit or either scan and is not claimed as a meaningful startup speedup.
Splitting availability from fingerprinted approval deserves a separate contract
and failure-parity study; it must retain deep validation and all required
authority/ownership failures. Neither proposal is implemented here.

## Verification, preservation and rollback

| Verification | Result |
|---|---|
| Frontend check | 0 errors, 0 warnings |
| Full frontend tests | 252 passed |
| Production frontend build | Passed |
| Rust formatting; all-target checks, ordinary/research | Passed |
| Ordinary Rust tests | 842 passed, 29 ignored; 6 icon integration tests passed |
| Research Rust tests | 846 passed, 29 ignored; 6 icon integration tests passed |
| Fresh research and feature-off Tauri release builds | Both passed; real executables booted |
| Numeric trace/bounds/privacy/parent/schema validation | 13 traces; zero overflow/cancellation; four invalid-trace cases rejected |
| Source/artifact identity and reproducible analysis | All three artifacts re-hashed; summary regenerates byte-identically |

Existing Rust test/build warnings remain; no dependency or lockfile changes.
Native buffer and frontend bounds, closed numeric contract, async context tests
and hash attribution tests all remain. Full build/test logs are retained privately;
only sanitized receipts and digests are committed. The initial notebook VM load
failed to bind functions and was corrected before any trial; no process/timing
sample was lost or overwritten. No fault was injected into live owner content.

Final preservation checks cover all 43,131 original files: **zero missing, zero
unexpected changes**. All 15,143 original untracked files, 124 additional
diagnostic/evidence files, 24,193 protected managed files and all 15 JNA-named files
are byte-identical. Only four authorized tracked source files differ. Normal
startup changed 37 existing operational WebView/server-cache files and created
six files entirely within those operational domains; these are reported
separately and never described as byte-preserved owner content. Installed Launcher
SHA-256 remains
`15bfd5d71bbe73c9ad7f3010c052f3f1669d81abfa4790b8c48efb8b169fd868`.
[Preservation receipt](docs/performance-p2/evidence/preservation.json)

Before each commit, tracked name-status/deletion and whitespace diffs, original
tracked-file membership and critical documentation/manifests/configuration are
checked. No generated build/game/runtime content or private inventories/logs are
staged. No cleanup of existing files, repository reset, credential operation,
installed app action or game execution is performed. Results apply to this one
healthy selected-instance, warm/online Windows workload, not cold/offline or
changed-instance performance. Existing failure/race/security tests remain the
correctness evidence for those unmeasured cases.

Rollback only the independently committed attribution probes/test with:

```sh
git revert b8591ae78f9ba319dfd07f2d56dbe12085c272b4
```

The separate reporting commit may remain as evidence. There is no migration,
cache invalidation, owner-data restoration or installed-application rollback.
Any future optimization requires its own isolated commit, failure/race tests,
matched measurement and owner review. This phase stops for owner review.
