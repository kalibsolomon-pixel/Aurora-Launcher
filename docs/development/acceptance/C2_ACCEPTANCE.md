# Phase C2 acceptance — 2026-09-26

## Git and scope

Starting HEAD, fetched `origin/main`, and merge-base were all `6f51a9c6bb4633f1c08cb210b092b44decbcad43`, ahead/behind **0/0**. A final fetch during acceptance still returned that exact remote baseline. Implementation commit: `1afa7e0ec74d907254bafbce2da7fb5871b21053` — `feat: execute Vanilla and transact optional Aurora configurations`. This report is a separate local documentation commit; its final hash is reported in the delivery message. **No C2 commits were pushed.** The completed branch is two commits ahead and zero behind the unchanged remote.

The native command contract, install-state platform representation, transition engine, and their frontend consumers form one coherent implementation commit. Splitting those would leave incompatible native/frontend contracts. Acceptance evidence is separate. No dependencies, lockfiles, Tauri identifier, registry schema 4, or authentication architecture changed.

## Implemented lifecycle

Vanilla has a dedicated normalized `GameInstallPlan::vanilla`: Mojang libraries, main class, and Java requirement, with no Fabric plan or invented loader identity. The existing verified executor installs base Minecraft into isolated managed storage. Installed-game schema 2 records typed Vanilla and omits a Fabric version; legacy schema-1 Fabric remains readable without migration. Validation checks the complete base game plus exact registry/platform consistency. Reinstallation resolves the actual configured platform and cannot inject Fabric/Aurora. Launch uses Mojang contributions, deterministic classpaths, validated managed Java and authenticated Rust session, structured arguments, redaction, and exact-child supervision. Rust capabilities now expose the implemented Vanilla lifecycle.

Fabric without Aurora composes genuine Mojang/Fabric plans and pins an official loader. It remains fully Ready without Aurora state or automatic API installation. Validation, reinstallation, Java and launch use the existing generalized paths. Modrinth compatibility derives Minecraft plus Fabric, independently of Aurora. Creation offers backend-authorized Vanilla/Fabric; Aurora is off by default. Vanilla hides loader selection and receives backend Aurora incompatibility. Enabling Aurora requires the reviewed exact Minecraft/Fabric combination; no pinned version is silently substituted.

## Optional Aurora transaction

Plain settings saves reject changes to Aurora state. Rust previews current/target configuration, requirement additions/removals, exact files, retention reasons, warnings and blockers. Approval supplies only the instance ID, enabled state and SHA-256 fingerprint. The fingerprint includes registry settings, installed-game manifest, Aurora and retained ownership documents, provider provenance/dependencies, local inventory identities, release identity and plan. It is checked before acquisition and inside mutation locks before activation. A stale preview fails deliberately.

Enable resolves authoritative reviewed release metadata against the installed pins and checks Java compatibility against the normalized game plan. Missing bytes use the SHA-256 verified store, unique staging and rehashing. Exact verified former launcher artifacts can be reused. Independent provider/manual content is never adopted, overwritten or duplicated; conflicting ownership blocks enabling.

Disable verifies expected release identities, hashes, sizes and contained regular paths before retirement. Tamper, ambiguous ownership, active Aurora lookalikes, and dependencies on Aurora fail closed. API is retained when provider descriptive dependencies, local dependencies or conservative ambiguity require it. The strict `aurora-retained.json` preserves former launcher ownership without inventing provider provenance or active launcher requirements.

Verified rollback copies survive retirement, activation, state changes, candidate validation, registry persistence and cleanup. In-process errors restore the old files/documents/configuration. Rollback failure is explicit and preserves recovery files. This does **not** implement a durable power-loss/process-crash journal or automatic crash recovery. Cross-process locking remains deferred.

## Fabric API ownership

| Configuration/content | Behavior |
|---|---|
| Fabric without Aurora, no API | No automatic API requirement or download |
| Provider API selected directly | Real provider provenance, `explicitlyRetained: true` |
| Provider dependency-only API | Real provider graph, `explicitlyRetained: false`, derived Required By and shared-dependency/orphan protections |
| Provider API explicitly promoted | Existing verified provider bytes retained through the C1 lifecycle |
| Fabric with Aurora requiring API | Verified launcher Required/Protected; providers cannot adopt, duplicate, replace or remove it |
| Disable with external provider dependency | Former launcher API retained, loses Required/Protected; descriptive dependency and provider graph unchanged |
| Re-enable retained launcher API | Exact verified reuse restores Required/Protected, one file, no provider ownership transfer |
| Enable with independently provider-owned API | Fails closed; no unsafe adoption or duplicate installation |

## Live Vanilla acceptance

Created **C2 Vanilla acceptance** through the release UI: Minecraft **1.21.11**, ID `778aa81f41344051a7f124c960aab876`. Vanilla selected, no loader selector, Aurora unavailable. Persisted Ready with typed Vanilla, no loader version, no Fabric library paths, no Aurora state and an empty mods directory. The manifest records 4,669 managed files.

Managed `java-runtime-delta`, Java **21.0.7**, and the existing authenticated account enabled Play. Running disabled duplicate Play. Genuine Vanilla menu reached; normal Quit produced supervised **exit code 0**, Exited and Ready. Launcher restart preserved selected Vanilla and readiness. The launch log has no Fabric Loader or Aurora initialization:

`C:\Users\kalib\AppData\Local\com.aurora.launcher\instances\778aa81f41344051a7f124c960aab876\logs\aurora-launch-1790453973-0.log`

Repair/corruption behavior is covered deterministically, without altering this live installation.

## Live Fabric without Aurora and Modrinth

Created **C2 Fabric without Aurora** through the release UI: Minecraft **1.21.11**, pinned official Fabric Loader **0.19.5**, ID `e3ee1470ba894aa0a38ab759ab35a160`. It reached Ready with no Aurora state, no Aurora JAR and initially no API JAR.

Modrinth correctly filtered Minecraft/Fabric compatibility. Installed AppleSkin **3.0.8+mc1.21.11**, version `59ti1rvg`, 180,107 bytes. Its required dependency installed Fabric API **0.141.6+1.21.11**, version `6qAuTtLR`, 2,426,039 bytes. Both files matched recorded published SHA-512 identities and local SHA-256. AppleSkin is explicitly retained; API is dependency-only with the real `requires` edge and displayed Required By AppleSkin. Neither is launcher Required/Protected. The existing provider acquisition verifies published size before activation; the final provenance remains schema 2.

Managed Java 21.0.7, Running with duplicate Play disabled, Fabric menu, normal Quit, supervised **exit 0**, and Ready passed. Restart preserved the installed configuration and readiness. Log confirms `Loading Minecraft 1.21.11 with Fabric Loader 0.19.5`, no `Aurora Client ready`, and normal stopping:

`C:\Users\kalib\AppData\Local\com.aurora.launcher\instances\e3ee1470ba894aa0a38ab759ab35a160\logs\aurora-launch-1790454753-0.log`

## Mandatory controlled roundtrip

Existing **Aurora performance acceptance**, ID `6cd0cc88e87b43159051ab7b52edcdf3`, began with Minecraft 1.21.11 / Fabric 0.19.5 / Aurora 2.1.2 stable / API 0.141.6+1.21.11 / managed Java 21.0.7.

| Stage | Persisted configuration and ownership | Validation, restart and launch |
|---|---|---|
| A: baseline | Fabric + Aurora; Aurora/API launcher Required/Protected; existing provider content | Ready, menu, `Aurora Client ready`, normal Quit, exit 0 |
| B: disable preview | Remove both requirements; remove exact Aurora JAR; retain API because installed content requires it; no blockers | Native preview reviewed before approval |
| C: disable | Fabric + no Aurora; Aurora JAR/state absent; API recorded as former launcher Retained, no Required/Protected | Restarted launcher; Ready; provider bytes unchanged, unrelated mod hashes unchanged, no duplicate API or stale stages |
| D: disabled launch | Same no-Aurora state; provider content intact | Fabric initialized, menu with Mod Menu reached, no Aurora initialization, normal Quit, supervised exit 0, Ready |
| E: enable preview | Restore exact Aurora/API requirements; acquire Aurora, reconcile exact retained API without adoption | Compatible release 2.1.2, native preview, no blockers |
| F: enable | Fabric + Aurora restored; Aurora/API Required/Protected; retained marker removed; one API JAR | Restarted launcher; Ready, provider document byte-identical, three unrelated mod hashes unchanged, zero stale stages |
| G: restored launch | Original production pins restored | Running/duplicate Play disabled, Aurora menu, `Aurora Client ready`, normal Quit, supervised exit 0, Ready |

Controlled logs beneath that instance's `logs` directory: baseline `aurora-launch-1790454941-0.log`, disabled `aurora-launch-1790455114-0.log`, restored `aurora-launch-1790455305-0.log`. Provider schema-2 file was compared byte-for-byte at disable, enable and final audit. AppleSkin, Mod Menu and Placeholder API matched their pre-transition SHA-256 snapshots. Resource packs, shaders, config, saves and other user content were outside the transition mutation set.

The original selected production instance (`413e831999bd444a8e0df2d22fc6d134`) was restored through the UI. Acceptance instances remain available. Repository release processes booted and closed normally with captured **exit 0** after Fabric acceptance, disable, and restoration.

## Supported matrix

| Configuration | Resolve | Install | Validate | Repair/reinstall | Launch | UI |
|---|---|---|---|---|---|---|
| Vanilla/no Aurora | Yes | Yes | Yes | Yes | Yes, live | Yes |
| Fabric/no Aurora | Yes | Yes | Yes | Yes | Yes, live | Yes |
| Compatible Fabric+Aurora | Yes | Yes | Yes | Yes | Yes, live | Yes |
| Vanilla+Aurora | Rejected | No | Unsupported | No | No | Disabled |
| Forge | Unsupported | No | No | No | No | Not offered |
| NeoForge | Unsupported | No | No | No | No | Not offered |
| Quilt | Unsupported | No | No | No | No | Not offered |

Repair is the existing deliberate reinstallation path, not a new generalized repair feature.

## Verification

| Check | Final result |
|---|---|
| Rust full suite | **461 passed, 16 ignored, 0 failed** (455 library + 6 integration passed) |
| Frontend tests | **56 passed, 0 failed** |
| Svelte/TypeScript | **0 errors, 0 warnings** |
| Rust formatting | Passed |
| Rust all-target checks | Passed |
| Frontend production build | Passed |
| Tauri production build | Passed; MSI and NSIS built |
| Version contract | **1.0.0**, passed |
| Whitespace | `git diff --check` and staged check passed |
| Release boot/normal close | Passed, captured exit 0 |

Seven native additions cover genuine Vanilla lifecycle/corrupt-client reinstallation, Vanilla LaunchSpec, persisted Aurora roundtrip, stale previews and staged/activation/persistence fault rollback in both directions, local API retention/reuse, descriptive provider API dependency retention without provenance mutation, and tamper/manual collision/incompatibility rejection. Existing C1 provider tests retain direct API, dependency-only/shared dependency, promotion, launcher protection, and Vanilla rejection coverage. Five frontend additions consume authoritative capabilities/compatibility, render native preview rows, send the approved fingerprint, refresh success and preserve stale/failed errors.

An existing transport test briefly exposed a parallel-test port reuse race: it bound then dropped its supposedly unavailable loopback port, allowing another fixture to reuse it and return 404. Its test-only server now holds that port without responding and uses a bounded timeout. The targeted test and final full suite pass; production networking behavior was unchanged.

## Screenshots

All paths below are beneath **`C:\Users\kalib\AppData\Local\Temp\aurora-c2-evidence-XzkJs0\`**, outside the repository and unstaged. They are native UI captures; the initial `.png` filenames contain the tool's original JPEG payload unchanged.

| Filename | Evidence |
|---|---|
| `01-platform-choices.png` | Vanilla/Fabric creation choices |
| `02-vanilla-creation.png` | Vanilla, absent loader selector, Aurora unavailable |
| `03-vanilla-ready.png` | Installed Vanilla Ready |
| `04-vanilla-running-menu.png` | Vanilla menu and launcher Running/disabled Play |
| `05-vanilla-exit-ready.png` | Vanilla exit 0 / Ready |
| `06-vanilla-restart.png` | Vanilla persisted readiness after restart |
| `07-fabric-creation-no-aurora.jpg` | Exact Fabric 0.19.5, Aurora unchecked |
| `08-fabric-ready-no-aurora.jpg` | Fabric Ready with no Aurora |
| `09-fabric-modrinth-appleskin.jpg` | Fabric-compatible Modrinth installation |
| `11-provider-api-required-by.jpg` | Dependency-only provider API, Required By AppleSkin |
| `12-fabric-running-menu-no-aurora.jpg` | Fabric menu and launcher Running |
| `13-fabric-exit-zero-ready.jpg` | Fabric exit 0 / Ready |
| `14-fabric-restart-ready.jpg` | Fabric no-Aurora restart persistence |
| `15a-aurora-baseline-menu.jpg` | Working Aurora menu before transition |
| `16-aurora-disable-preview.jpg` | Configuration delta, exact Aurora removal, API retention |
| `16a-disable-preview-approval.jpg` | Retention warning and explicit approval |
| `17-disabled-restart-ready.jpg` | Roundtrip disabled persistence / Ready |
| `18-disabled-api-retained.jpg` | API Retained, no Required/Protected |
| `19-roundtrip-disabled-menu.jpg` | Roundtrip Fabric menu without Aurora |
| `20-disabled-exit-zero-ready.jpg` | Disabled configuration exit 0 / Ready |
| `21-aurora-enable-preview.jpg` | Requirements restored, exact retained API reuse |
| `22-restored-restart-ready.jpg` | Restored Aurora persistence / Ready |
| `23-restored-required-protected.jpg` | Restored Aurora and API Required/Protected |
| `24-restored-aurora-menu.jpg` | Restored Aurora menu |
| `25-restored-exit-zero-ready.jpg` | Final Aurora exit 0 / Ready |

## Repository safety and deferred work

Baseline inventory: `C:\Users\kalib\AppData\Local\Temp\aurora-c2-baseline-5ca91fb4-f125-4dd1-a11a-345ca438bd17`. Complete tracked-file and untracked diagnostic hash snapshots were recorded before implementation. Commit audits found **0 tracked deletions, 0 missing baseline files, 0 changes among all 606 protected diagnostics**, and intact root docs, manifests, lockfiles, native configuration and source trees. No unexpected file mutations occurred. Generated build/game/runtime data and screenshots were not staged. Final untracked state is only the pre-existing `.zcode-diag/`, `.zcode-diag2/`, `.zcode-diag3/`, `.zcode-diag4/`, `.zcode-diag5/`, `.zcode-icon-diag.py` and `.zcodeignore` set. No `git clean` or broad cleanup ran.

Deferred: Forge, NeoForge, Quilt, CurseForge, Home/account/skin restructuring, cross-process mutation locking, automatic dependency conflict resolution, durable crash recovery and instance deletion. No next-phase work was started.

## C2 publication-blocker correction — 2026-09-26

Starting HEAD was `66e85af80301b68f56598c1ee51ffb244bedfb1b`; fetched `origin/main` was `6f51a9c6bb4633f1c08cb210b092b44decbcad43`, ahead 2 / behind 0. This correction is a separate commit above the existing implementation `1afa7e0ec74d907254bafbce2da7fb5871b21053` and acceptance commit; neither was amended.

### Root cause and corrected invariant

Previously `play_instance` deeply validated through `resolve_instance_launch_plans`, then awaited managed-Java validation/diagnostic and possibly authenticated-session restoration. The process remained Stopped until `spawn_supervised` established Starting. An Aurora transaction could acquire the content/registry/process locks and commit during those awaits, leaving Play's already resolved assumptions stale before its later spawn.

Play now reserves preparation once per instance in Rust. Immediately before argument assembly/spawn it takes the existing instance-content lock, then registry lock, compares the complete captured registry/game/Aurora snapshot, and repeats deep, read-only local validation. A changed snapshot returns `launch_configuration_changed` without Starting or spawn; retry uses the current configuration. Damage returns `launch_instance_not_ready`; competing content work returns `launch_instance_busy`. The same exclusion remains held through synchronous LaunchSpec assembly, native Starting reservation, structured spawn and exact-child supervision establishment. Aurora commit uses these same locks and cannot commit underneath that section. Home/workspace Play controls also reflect Starting/Running, with native reservations remaining authoritative.

Network metadata, runtime diagnostic and session work stay outside the mutation locks. Ordering is content -> registry -> short process-state accesses. The preparation mutex briefly reads process state, releases both before awaited work, and is never acquired by transition commit or process-state holders. RAII releases preparation on error/cancellation and releases mutation locks on callback errors. No reverse lock acquisition was found in the reviewed paths.

### Deterministic concurrency and regressions

Five added native tests passed. Channel-controlled tests commit disable and enable while Play is paused after initial validation: both reject stale preparation and never invoke spawn; a fresh snapshot validates the new configuration. Reverse-order tests hold the final boundary and attempt each actual transition: both return conflict with unchanged registry, then succeed after the controlled spawn section releases. Concurrent preparations permit exactly one supervised fake-child spawn/log; a second attempt returns the existing duplicate error. Cancellation releases the reservation. Vanilla final validation rejects damaged client bytes, replaced installed revisions and competing content work, then validates without Fabric/Aurora contributions after restoration.

All prior C2 rollback, stale-preview, tamper, ownership reconciliation, Vanilla lifecycle/LaunchSpec and Fabric/provider tests remain passing. Fabric/no-Aurora and Fabric+Aurora composition are unchanged by the correction; previous live Vanilla and Fabric/no-Aurora launch evidence above remains applicable.

### Corrective live acceptance

The optimized release launched Minecraft 1.21.11 / Fabric 0.19.5 / Aurora 2.1.2 / Fabric API 0.141.6+1.21.11 using managed Java 21.0.7. Starting and Running were observed, the genuine Aurora menu was reached, and the supervised log recorded `Aurora Client ready.`. The user completed normal Quit; supervision reported exit 0 and the launcher returned Ready.

The approved native transition roundtrip completed Fabric+Aurora -> Fabric/no-Aurora Ready -> Fabric+Aurora Ready. Disable removed exact Aurora ownership and retained the former launcher Fabric API for installed content. Enable reused that exact verified API and restored active Aurora ownership. All seven provider artifact SHA-256 values and the provider ownership document remained byte-identical. The acceptance instance remains restored with Aurora enabled.

After the small Play-button presentation correction, a final optimized release rebuild produced both MSI and NSIS successfully. That exact binary booted, launched the restored acceptance instance (supervised child 17884), reached the Aurora menu and recorded `Aurora Client ready.`. Home's Running button was confirmed disabled in the native accessibility tree. Normal menu Quit produced supervised exit 0 and Ready, recorded in `logs/aurora-launch-1790457880-0.log` beneath the acceptance instance. No screenshots were added to the repository.

### Verification and safety

The complete corrected native suite passed **466 tests (460 library + 6 integration), 16 ignored, 0 failed**. Frontend tests passed **56, 0 failed**; Svelte/TypeScript reported **0 errors, 0 warnings**. Rust formatting and all-target checking passed; frontend production build, Tauri production MSI/NSIS build, production boot/normal Quit, version contract **1.0.0**, and whitespace checks passed. Final publication identifiers are recorded in the completion report.

Correction audit baseline is `C:\Users\kalib\AppData\Local\Temp\aurora-c2-correction-840f42de-7a5c-4857-ae50-78892a7a9a60`. Audits found 0 tracked deletions, 0 missing baseline tracked files, and all 606 protected diagnostics unchanged. Generated build/game/runtime data and screenshots are excluded from the commit. The combined C1-to-C2 candidate retains the artifact trust boundaries, strict persisted-state handling, provider provenance, contained mutation and exact-child supervision.

This correction implements no Phase D, cross-process mutation locking, durable crash/power-loss recovery, Forge, NeoForge, Quilt or CurseForge.
