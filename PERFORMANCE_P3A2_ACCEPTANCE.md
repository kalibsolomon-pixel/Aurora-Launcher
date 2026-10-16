# Performance P3-A.2 — same-user authenticated capture acceptance

**Overall: PARTIAL. P3-B NOT READY / NOT AUTHORIZED.** Legitimate authenticated
Minecraft execution, real presentation capture, CPU/memory telemetry, clean exit
and a complete snapshot/archive restoration succeeded. Essential workload/calibration
gates remain: the captured scene paced near 30 Hz despite saved Unlimited, the frozen
world contains 19 non-player entities instead of zero, full cadence/loss/overhead
confidence is incomplete, and final live WebView verification followed owner sign-in.
These are retained limitations, not replaced by synthetic results or favorable retries.
No Minecraft performance improvement or tuning/preset change is claimed.

## Repository, HEADs, commits and changed files

Branch `codex/client-3-production-authority`; starting HEAD
`d84b5f338bebbe711740a8987486f433c65834b4`, exactly matching the request.
Initially clean tracked tree/index; **2,446 tracked paths, four protected untracked
files and seven worktrees**. Starting status/history/index/tracked-untracked hashes/
worktrees/critical-file presence are in `.git/performance-p3a2-baseline.json`.
Prior P1/P2/P3-A/P3-A.1 acceptance and native/auth/runtime/workload/tooling boundaries
were reviewed; historical acceptance artifacts remain unchanged.

| Local commit | Scope |
| --- | --- |
| `135e663` | Strict separately named short-calibration reader, actual default-CSV specimen, seven initial regressions and format documentation |
| `56b8a04` | Observed Windows redirected-diagnostic encoding support and eighth regression; original failed inspection preserved |
| Commit containing this report | Same-user safeguards, current readiness/capture/world reports, sanitized evidence, final absolute ancestor checks and acceptance |

Ending implementation HEAD before acceptance is `56b8a04`; ending review HEAD is the
local commit containing this report. Its exact content-dependent SHA is recorded
**after commit** in `.git/performance-p3a2-final.json` and the final chat response.
A commit cannot embed its own hash. No push, merge, tag, release, version bump,
production installation, deployment or release workflow occurred.

Complete inventory: [P3A2_CHANGED_FILES.txt](docs/performance-p3a/P3A2_CHANGED_FILES.txt).
**25 paths: 16 additions, nine modifications, zero deletions.**
Only nine original paths change: Authorization, Readiness Matrix, PresentMon
Calibration, Minecraft Capture Validation, World Snapshot Lock, historical Windows
Test User guidance, Calibration Protocol and Workload Spec (supersession notes),
and benchmark README. Product source, native
DTOs, dependencies/lockfiles, presets and historical workload hashes remain unchanged.
New files contain benchmark-only validation/tests/specimen, reports and sanitized
local evidence. No file deletion or generated executable/game/runtime/world is staged.

## Same-user safeguards and actual isolation

[Authorization](docs/performance-p3a/AUTHORIZATION.md) now reflects the owner's explicit
limited approval and accepted shared Windows Credential Manager continuity risk.
Normal same-account authentication may rotate the UUID-keyed refresh credential and
require production sign-in again. No credential namespace workaround, extraction,
export, backup/restoration, direct production vault inspection, account-file copying,
fabricated account or entitlement bypass occurred. The isolated account remains
persisted; it was not removed as cleanup because removal can delete the shared secret.
No passwords, Microsoft consent screens or MFA were automated.

Prelaunch inspection proved the marked direct-temp root and ancestors/descendants
had no reparse points. The actual native backend reported the intended managed root;
the live WebView process reported that tree's separate EBWebView profile. Canonical
test and production install/data roots are disjoint. Verified categories: managed
root, launcher registry/config, WebView, per-instance working/game/world/log paths,
exact shared runtime and benchmark artifacts. This is runtime/filesystem evidence,
not merely an environment-variable assertion. No unexpected production target arose.
Exact local paths/principal/private identifiers remain outside committed evidence.

The owner independently completed normal interactive sign-in while read-only native
revalidation was underway and confirmed this in chat. Prelaunch filesystem checks
passed before startup, but the final live WebView confirmation was recorded after
login. This ordering is not presented as fully observed pre-authentication acceptance.
Supported account/session/readiness commands selected the legitimate isolated account
and returned entitled/session ready without blockers. No production auth check was
performed: actual production reauthentication/rotation effects remain **unknown**.
[Native/auth evidence](docs/performance-p3a/evidence/p3a2/native-auth.json).

## Java, installations, launch and process supervision

All **three prepared workloads** passed current native complete-instance and runtime
validation. Each runtime check rehashed **402 files / 99,041,788 bytes** and ran the
bounded managed version diagnostic. Exact Microsoft OpenJDK **21.0.7+6-LTS** lock,
java-runtime-delta/windows-x64, official manifest SHA-1
`cb4394a27089d19f65d5baa6cf0482c27c3c7865`, and javaw SHA-256
`1ba6a4fb32a6a3d5c925fe837ae6717e2f75743a71e013e668e6dadb909245ba`
are preserved. [Independent mod rehash](docs/performance-p3a/evidence/p3a2/mod-revalidation.json)
confirmed A=0, B/C=4 jars, with all recorded publisher digests/sizes matching.
No Java installation, PATH or JAVA_HOME changed. B/C installation
readiness is not Aurora/Sodium/Lithium coexistence execution evidence.

Workload A's normal authenticated pipeline reached the actual Minecraft 1.21.11 /
Fabric 0.19.5 main menu with no optional mods/API. A custom 1920x1080 Aurora window
request first failed before spawn as `launch_placeholder_unresolved` for
resolution_width. The failure is retained. Restoring supported null window settings
and using Minecraft's own saved resolution override produced the independently F3-
verified **1920x1080** framebuffer. No resolver change, fake identity, direct Java
launch, shell command string or argument-validator bypass was used.

The exact supervised Minecraft child was PID **25332**, Windows creation FILETIME
**134366600133671485**, with the expected absolute managed executable/hash. No full
child argument list, LaunchSpec or game token was read/exported. Main-menu and F3
observations, private native screenshots and saved-world NBT establish actual
execution. Minecraft saved/quit normally: **exited / code 0 / no diagnostic** at
21:47:59.145 UTC. Isolated launcher then closed normally: **code 0**, retained handle,
21:48:50.829297 UTC. No owner process was terminated or sampled by name.

## PresentMon and actual gameplay capture

[Calibration report](docs/performance-p3a/PRESENTMON_CALIBRATION.md) distinguishes
bounded operational success from full calibration. Staged PresentMon **2.3.1** was
reverified against SHA-256
`364e5d98d4d134bd54dd25c22ed2ca2f4883f8bc3ed6502bee0c151e3436d30c`
and Valid Intel Authenticode before capture. Unique ETW sessions started/stopped
without elevation, security/group/policy changes, services or another download.

A real native WGL stimulus emitted 2,320 independent swap markers. Its bounded
capture produced **1,428 actual rows**, of which 424 had unavailable display estimates.
Exact PID/creation/stream, integer QPC units, raw hashes, startup/shutdown and the
retained inner window passed inspection. Marker discrepancy averaged 0.212443 ms,
max 1.7794 ms; boundary counts differed by one and are not independent loss proof.
Full 20-second-per-cadence and instrument-on/off A/A calibration were not performed.
The initial stimulus planned-stall metadata was stale; its explicit review preserves
original bytes and records actual code timings. Observer overhead remains unknown.

A single real gameplay capture followed **334.372 seconds** of stationary warm-up:
**2026-10-16 21:45:51.390345–21:46:17.513650 UTC**, 26-second tool timer, predeclared
20-second inner window with pre/post-roll, QPC frequency **10,000,000 Hz**.
It produced **776 raw presentation rows**, one Other stream 0x0, no unavailable
display rows, **598 inner intervals**, and valid boundary coverage. PresentMon exited 0.
No loading/menu samples were blended into the inspected interval.

[Short format](docs/performance-p3a/SHORT_CALIBRATION_FORMAT.md) handles the actual
default TimeInQPC/MsUntilDisplayed export separately from the historical v2-column
adapter. Direct redirected producer stderr used UTF-16LE with BOM: first inspection
was INVALID, encoding support/regression was added, and **the same capture** now
returns **VALID WITH WARNINGS**. Original failure and raw bytes remain retained.
No recapture, synthetic relabeling or measured quality-gate weakening occurred.

Actual pacing is approximately **29.924 Hz despite saved Unlimited/VSync OFF**.
Post-capture F3 likewise showed 30 FPS; later input/menu interaction showed thousands.
Saved AFK limiting makes inactivity/focus a plausible explanation, not a proven cause.
This is actual presentation plumbing evidence, **not an unlimited-FPS benchmark**.
OpenGL timing remains an ETW display estimate; producer event-loss counters and
physical-display timing are unproven. No FPS/tail improvement or causal claim exists.
[Raw-hash/quality summary](docs/performance-p3a/evidence/p3a2/minecraft-capture.json).

## Quiet-system checks and instrumentation

After the owner quieted unrelated applications, a read-only **60-second** preflight
observed mean machine CPU **0.68541%**, P95 **1.06171%**, minimum available memory
**23,094,452,224 bytes**, and AC connected. One GPU engine sample peaked around
0.2105%; this is not continuous aggregate GPU-idle proof. RX 9070 XT driver
32.0.31041.1004 and reported desktop 2560x1440/240 Hz were recorded. Thermals/VRR
remain unknown; no power settings, unrelated app state or priorities were changed.
[Preflight](docs/performance-p3a/evidence/p3a2/quiet-preflight.json).

Exact-child CPU/memory telemetry retained **61 samples over 30 seconds**, with a
complete 20-second counter window, max interval 0.501306 seconds. Sampled RSS peak
was 1,738,133,504 bytes; OS peak 1,787,535,360 bytes; process mean CPU 8.75045% of
one logical core. Capture machine CPU rose to 6.04158%, above the quiet preflight;
this interference warning remains. Maximum sampler read duration was 1.471 ms,
not a measured total observer-overhead estimate. Unknown heap/paging/GC CPU remain null.

A stayed Balanced/2048 MiB/empty custom JVM text. Game GC logging/correlation is
**unavailable** because no Existing Configuration laboratory instance was created;
no unsupported flags or hidden instrumentation were inserted. Schema-1 measured
runs still require **300/600 seconds and all three sources**. Their status is
**INCOMPLETE**; no extended session was started to satisfy them.

## Disposable snapshot and workload conformity

After supervised clean exit, the complete test world was copied, inventoried and
archived; a separate restoration was extracted with member/type/path containment
checks and passed the existing read-only checker. **56 files / 12 directories**,
**22,159,360-byte archive**. Original snapshot/archive files are read-only, retained
and rehashed; same-owner attribute changes remain possible, so absolute immutability
is not promised. The archive includes actual level/player/region/entity/POI/dimension
state and empty directories; options/config are retained separately. Private player
identifiers/raw world inventory are not committed.

| Artifact | SHA-256 |
| --- | --- |
| Complete world archive | `6e77ef39a966e4175469d06ba0f503500f0fd4f269a9bbbc0b1e03aed4354c33` |
| Full inventory manifest | `2570658c952055bceaaf8b66bac9b1b036110ce4d61b408ddfb41c896db1d7e1` |
| Actual conditions | `be3c6f81e0d2386a61e0b1d4fe21d4c6e5f63c1f3f21b7fe8c9897e98e986371` |
| Saved options | `8d6484f7cc489795a6966bdd757511447939081448b45da7ab07afcc1819d15d` |

Bounded read-only NBT/region inspection confirmed all **1,024 required full chunks**
in the 512-block square, 5,329 recorded chunks total, exact seed/camera/time/weather/
mode/difficulty/gamerules and the explicit superflat layers supporting y=66.
However, **19 non-player entities** (chicken/horse/item/pig) remain, versus specified
zero; block entities are zero. Nothing was removed to conceal this. The snapshot
is byte-reproducible and fully restored, but the canonical Workload A lock remains
blocked; its hashes in workloads.json stay null. Graphics are the actual 1.21.11
Custom individual settings, not a claimed new Fancy-preset mapping.
[World report](docs/performance-p3a/WORLD_SNAPSHOT_LOCK.md).

## Tests, quality gates and preservation

**52 benchmark tests pass: all 44 P3-A.1 tests plus eight new short-reader regressions.**
**13 P1 analysis tests pass.** Zero failures. Coverage includes real default-CSV
specimen, identity/stream/units/order, hashes/schema/unknown fields, unavailable and
invalid rows, columns, incomplete/producer-loss refusal and strict redirected encoding.
Actual controlled/game short CLI exits 0 (VALID WITH WARNINGS); actual restored-world
checker exits 0 (VALID). Full measured-run quality remains INCOMPLETE.
[Tests/log hashes](docs/performance-p3a/evidence/p3a2/tests.json).

No product/native/startup/frontend/dependency source changed, so Rust/frontend/build
checks were not rerun; prior P3-A test totals are historical only. This phase did run
the actual isolated native launcher and authenticated Minecraft. Git whitespace,
name-status/deletion, original tracked baseline, critical-file presence and staged
artifact hashes/links are checked before commit. All **2,437 unmodified original
tracked files**, four protected developer files and seven worktrees remain intact;
only this branch advances. No broad cleanup, reset, .minecraft access or generated
build/game/runtime staging occurred.

No agent action mutated production installation, owner instance/world/mod/config/Java
files or release artifacts. Late read-only observations of production executable,
configuration and registry hashes stayed identical through close. These are not a
whole-phase hash freeze of all live owner data. Shared credential continuity through
legitimate same-user login is the explicitly accepted exception; no direct vault
inspection or production sign-in test was made. No secret-bearing artifact is committed.
[Reviewed evidence inventory](docs/performance-p3a/P3A2_EVIDENCE_INVENTORY.json).

## Remaining blockers and P3-B recommendation

1. Establish a reviewed unlimited stationary capture condition; isolate the 30 Hz
   inactivity/focus discrepancy and background rise in a separately authorized follow-up.
2. Prepare a new actually zero-entity A world lock; preserve this nonconforming original.
   Review native 1.21.11 graphics terminology/options rather than assuming preset parity.
3. Complete controlled cadence/loss/overhead confidence and fully observed pre-auth
   path checks. Retain unelevated process-query and thermal/VRR limitations.
4. Resolve the supported native custom-resolution placeholder defect in separately
   scoped product work if required; the saved-game-resolution path is documented here.
5. B/C coexistence/world execution and game GC/correlation remain untested/unavailable.

**Do not begin P3-B.** Real short capture and restoration are substantial progress,
but these remaining gates prevent PASS. No repeated tuning comparisons, profile
promotion, collector/Xms change or automatic upgrade is authorized. Stop after P3-A.2.
