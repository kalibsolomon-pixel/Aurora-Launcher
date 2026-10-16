# Performance P3-A.1 acceptance

**Overall: PARTIAL. P3-B NOT READY / NOT AUTHORIZED.** Exact PresentMon 2.3.1
portable acquisition and identity verification succeeded. The owner confirmed the
separate Windows test user is **not provisioned** and requested documented owner
setup. That stop condition prevents authentication, calibration/gameplay capture and
world freezing. Zero presentation rows, zero game launches, zero world snapshots.
No Minecraft performance improvement or complete validation PASS is claimed.

## Repository, commits and changed inventory

Branch `codex/client-3-production-authority`; starting HEAD
`3f709a6af0a25be4eea019833a58c0fb6e956862`, matching the request. Tracked tree and
index initially clean; 2,433 tracked paths, four protected untracked files and seven
worktrees. Pre-change hashes/index/history/worktrees/benchmark inventory and critical
file presence are recorded privately in `.git/performance-p3a1-baseline.json`.
Input document identities: `.git/performance-p3a1-input-documents.json`.
Initial process/principal/tool-discovery observations are retained separately.

Ending review HEAD is the local commit containing this report; its exact SHA is
recorded after commit in `.git/performance-p3a1-final.json` and the final response.
A commit cannot embed its own content-dependent hash. All commits remain local.
Full inventory: [P3A1_CHANGED_FILES.txt](docs/performance-p3a/P3A1_CHANGED_FILES.txt).
**15 paths: 13 additions and two modifications; no deletions.**
Only two pre-existing files change: active readiness matrix and benchmark README.
Other changes add phase-specific reports/status, a read-only checker and its tests.
No production source/configuration/dependency, version, release asset or preset change.

## Tool acquisition and actual calibration

[Installation report](docs/performance-p3a/PRESENTMON_INSTALLATION.md): official
[Intel/GameTechDev v2.3.1](https://github.com/GameTechDev/PresentMon/releases/tag/v2.3.1),
asset `PresentMon-2.3.1-x64.exe`, 421,312 bytes, SHA-256
`364e5d98d4d134bd54dd25c22ed2ca2f4883f8bc3ed6502bee0c151e3436d30c`.
Expected GitHub release-asset digest matches raw downloaded bytes. Windows
Authenticode reports **Valid**, Intel Corporation, timestamped by Sectigo. Pinned
MIT-style license reviewed and retained. Fresh direct-temp tools staging only;
no MSI/service/driver/system installation and no committed executable.

Actual execution: `--help`, banner **PresentMon 2.3.1**, exit 1. PE version fields
are absent; version is established by release identity/digest and actual banner.
The current token is unelevated with pre-existing Performance Log Users membership;
future test-user tracing privileges remain unproven. No trace or elevation attempted.

[Calibration report](docs/performance-p3a/PRESENTMON_CALIBRATION.md): Stage 1 PARTIAL,
Stages 2–4 BLOCKED/NOT TESTED. No start/stop/PID targeting operation, controlled
presentation data, dropped/lost-row reconciliation or overhead experiment occurred.
Help/license hashes are recorded; no raw frame hash exists. OpenGL timing limitations
and calibration-vs-Minecraft evidence remain explicit.

## Isolation, authentication and Minecraft

[Windows procedure/checklist](docs/performance-p3a/WINDOWS_TEST_USER_VALIDATION.md)
starts from the established authorization document. Actual current principal is the
owner; no approved separate session/profile/vault exists. No user account, group,
policy, credential, browser profile or security setting was created/modified.
Credential Manager UUID targets are not namespaced by launcher root. A separate
managed/WebView root alone is not sufficient; sandbox accounts are not substitutes.

The procedure covers owner-created standard local user, live SID/profile/elevation
verification, distinct non-imported credential namespace, narrowly reviewed staging,
debug-only managed-root semantics, native path/runtime revalidation, legitimate
owner-operated browser sign-in, normal shutdown and safe return to owner session.
No passwords, tokens or recovery codes are requested in chat or logs.

[Minecraft report](docs/performance-p3a/MINECRAFT_CAPTURE_VALIDATION.md): legitimate
authentication, entitlement, actual Java/Fabric/mod execution, main menu, disposable
world, exact target process, gameplay samples and clean exit all **NOT TESTED**.
No native test launcher was booted. Prior three isolated installations and exact
Microsoft OpenJDK `21.0.7+6-LTS` evidence are preserved; neither becomes current
test-user readiness without native revalidation. No Ready injection or auth bypass.

## Worlds, metrics, quality and quiet-system gate

[World lock report](docs/performance-p3a/WORLD_SNAPSHOT_LOCK.md): A/B/C actual locks
BLOCKED; seeds/proposed scene settings are not frozen snapshots. Workload versions/
mod hashes and null world hashes remain unchanged. New read-only directory checker
validates strict metadata, full file/directory inventory and an independently prepared
restoration; it never copies/extracts/restores/deletes. Tests use fake world bytes and
do not prove NBT scene claims, provenance, clean exit or immutability.

No real game GC/CPU/memory/frame correlation, clock anchors, drift/uncertainty or
capture overhead is available. Unavailable metrics are null, not zero. P2 logging
rejections stay unchanged; any future instrumented short session needs a deliberate
disposable Existing Configuration lab instance. Temporal GC/frame proximity remains
observational and cannot establish causality.

Full real-run quality: **INCOMPLETE**, absent all real sources; no dummy successful
manifest. Original synthetic CLI: **VALID WITH WARNINGS**, synthetic scope only.
Source review exposed an additional limit: measured schema 1 requires 300/600 s and
all three sources. A short session needs frame-only adapter inspection/manual report
until a separately reviewed short-manifest adapter exists. No threshold/schema/parser
was weakened, real capture relabeled synthetic, or long run started to satisfy it.

[Quiet observation](docs/performance-p3a/P3A1_QUIET_PREFLIGHT.md): current instant
CIM CPU 2%, free RAM ~21.2 GiB, RX 9070 XT / 32.0.31041.1004, reported desktop
2560x1440/240 Hz. Production launcher remains running; no Java was found in the
bounded current inventory. AC/GPU load/thermals/VRR/observer overhead are unknown.
No 60-second quiet test-session window: gate **BLOCKED**, no game capture attempted.

## Tests and native validation

[Test matrix](docs/performance-p3a/P3A1_TEST_MATRIX.md): **44 benchmark tests PASS**
(all 36 existing + eight new), **13 P1 tests PASS**, zero failures. Coverage includes
PresentMon CSV/QPC/process identity/completeness/metadata/digests, G1 parser, actual
Windows self-process telemetry, invalid/incomplete classification, synthetic world
inventory/restoration validation and principal/elevation refusal. Synthetic CLI exits
0. Test logs/hashes retained. No importer correction was needed without real samples.

No Rust/frontend/native/startup/build change, so those regressions/build/boot were
not rerun. Historical P3-A 992 Rust / 40 frontend / native restarts are inherited
evidence, not current executions. Native Minecraft and actual ETW acceptance remain
blocked. Git whitespace, scoped changes/deletions, local links and original-byte/
protected-file/worktree/index preservation are checked before commit.

## Preservation and production isolation

All **2,431 unmodified original tracked files** and **four protected untracked files**
match baseline hashes. Only the two scoped original documents change. All seven
worktree paths/branches are retained; other worktree HEADs unchanged and only this
branch advances. Critical root docs/manifests/lockfiles/frontend/native configuration
and original P3-A artifacts remain present. No deletion, cleanup/reset, process-name
termination or generated binary/game/runtime data staged.

Production Launcher 1.4.2 was observed and left running. No owner instance/world/mod/
settings/account/credential/Java/release mutation was performed. No authentication
operation, owner vault read, production build installation, power/security change,
push/merge/tag/publication/deployment/release workflow or P3-B execution. Existing
owner software can make normal independent writes; blanket live-data hash identity
is not claimed. Private identity/local paths stay outside committed evidence.

## Exact next owner actions and P3-B decision

1. Provision/select a dedicated standard local Windows user and sign into its
   interactive session; make it available for validation. The owner chose setup
   documentation, so agent account creation remains unauthorized.
2. Verify distinct SID/profile/vault, unelevated token and all test-owned paths.
   Stage only reviewed benchmark materials and revalidate the development binary,
   exact game/mod/runtime identities. Do not copy accounts, browser data or secrets.
3. Confirm PresentMon tracing works without elevation. If it needs a group change,
   install or privilege, stop for that specific separate approval; do not bypass it.
4. Owner completes legitimate system-browser sign-in/entitlement and chooses a quiet
   short-validation window. Validate A first, freeze disposable worlds and retain
   bounded real captures/diagnostics; B only if A succeeds, C may remain pending.
5. Review actual quality/world/calibration evidence and the short-manifest limitation.
   Keep failed attempts. **P3-B remains BLOCKED and requires a new explicit decision.**

[Updated readiness matrix](docs/performance-p3a/READINESS_MATRIX.md) and
[machine status](docs/performance-p3a/evidence/p3a1-status.json) distinguish actual
tool identity, synthetic checks, inherited standalone/native evidence and unavailable
gameplay. This phase claims safe preparation only. It does not authorize repeated
benchmarks or changes to heap advice, collectors or performance presets.
