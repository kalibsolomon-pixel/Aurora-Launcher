# Aurora Launcher 1.4.2 — controlled release acceptance

Status: LOCAL CHECKS PASSED; ready to initiate protected build/sign. No 1.4.2 installer, release or update authority has been published.

## Authorization and subjective R1 acceptance

On October 10, 2026, the owner reported that the latest manually reviewed prototype, including scrolling, seems fine and requested release. The owner selected 1.4.2. This resolves the subjective R1 review gate for final application source `db078f20d8d2eaa0c632b276e68c63179872acde` and final evidence `0e9eb4f473f61a99ccb256f3a0a6c38e051a26f9`.

The severe scrolling symptom was not reproduced in the earlier diagnostic runs. Owner acceptance does not convert compositor measurements into physical frame-presentation/GPU evidence or establish a replicated before/after fix. Those limits remain in `LAUNCHER_PRERELEASE_UI_CORRECTIONS.md`.

## Scope and identity

Target: Windows x64 NSIS, single public production stream. Stable identifier: `com.aurora.launcher`; product: Aurora Launcher; executable: `aurora-launcher.exe`. Launcher version is synchronized across npm and Cargo metadata, with Tauri resolving package.json. Aurora Client remains independent, with current compatible new-instance selection 3.0.0 and historical pins preserved.

Candidate starts at `0e9eb4f473f61a99ccb256f3a0a6c38e051a26f9`. Production main at inspection: `4575a0e3f1c28da6cfe072948fe91b60eeaf608f`. The 37 intervening local commits include Client authority/artwork correction, isolated performance research, startup coordination, L1/L2/L3 and UI corrections. Research tools are outside the production frontend and opt-in native profiling is absent from default release features.

## Current production and trust inspection

Public v1.4.1 is immutable release 404112749 from `13d8d2aec27e2871926e90f1ce3c215089841672`. Accepted v1.4.0 and retired launcher-updates release remain untouched. Current authority head is `959bf5fd833744abbdcce5a610ef4c5aac0fb62a`, published by the dedicated authority App after successful recovery run 37404093664.

Read-only inspection found both production environments restricted to main, with required owner reviewer and administrator bypass disabled. Self-review is allowed for the sole owner. The three active authority rulesets target the exact authority branch: history/deletion/force-push protection without bypass; update restriction allowing only Integration 5182303; creation restriction without bypass. Build/sign and authority public keys match. Only names/presence of signing/App secrets were inspected; their values were never retrieved.

Authenticode remains unconfigured, as documented by the established release baseline. Minisign updater authentication and Authenticode are distinct. The unchanged production key must verify exact transferred and public installer bytes.

## Verification and preservation

Refreshed candidate checks passed in a disposable source copy: npm ci, numeric version contract 1.4.2, 265 frontend tests (16 suites, serial), 100 release-tool tests, frontend type check (zero errors/warnings), production SPA build, Rust formatting, locked all-target check and complete deterministic Rust suite. Rust check retains four pre-existing test-helper warnings. Ignored live tests remain unexecuted and are not counted as passed. Exact suite totals are recorded in the final operator status accompanying the workflow run.

Unauthenticated public v1.4.1 installer/signature/inventory downloads were rehashed and verified with the unchanged production minisign key. The canonical authority bytes exactly match its Git blob and reference that signature. Public-key SHA-256: `bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`. Authority manifest SHA-256: `089e04f4d0f6bfd1c727a99e005f27bbf117897a59cac2164d9aeb7a402e2ded`. Existing installer SHA-256: `aaba35418887a615b1892ef351cd089e28812c39c6ae5b3764d2a725ac7a02f5`. These are existing 1.4.1 hashes, not a new 1.4.2 artifact. The published Client 3.0.0 artifact's GitHub digest/size match the source catalog.

A private baseline outside the repository records all 803 tracked files, 15,144 untracked files, 175 protected ignored files and 24,200 production files (excluding WebView operational cache). All 40,322 files were rechecked: none missing, no unexpected byte changes, and only the five intended tracked version/report files changed. Existing builds and diagnostic files are preserved. Tests/builds run in a disposable source copy; production installation, accounts, instances and Minecraft are not used for acceptance. The latest pre-version-bump isolated optimized smoke remains prior application-code evidence, not acceptance of a signed 1.4.2 installer.

## Remaining protected gates

The established manual launcher-release workflow must run from exact current reviewed main. Its protected build/sign job produces a seven-day private artifact. Acceptance must use those exact signed bytes in a disposable Windows user/profile or VM before the protected immutable publication job is approved. The dedicated authority environment independently gates manifest-last advancement. Do not approve publication based on a debug/isolated executable or prototype review.

No existing production installation may be replaced during this task. No owner instance/account mutation is authorized. The owner will test the signed artifact in a VM or disposable Windows profile. Live account/game acceptance required by the historical installer procedure must use that disposable environment and explicit owner participation; fixture results cannot establish it. A genuine production 1.4.1 to 1.4.2 updater test requires the public canonical authority after manifest-last publication; prepublication artifact signature/manual-install checks are not that test.

## Rollback

Before publication, hold approvals and preserve the source/artifacts; source changes may be reversed through reviewed revert commits, never reset/clean or tag replacement. After publication, retain immutable releases and authority history, pause future authority approvals and issue a strictly higher signed corrective release. Never force, delete or rewind authority.

L2 gameplay history and L3 skin library write schema 2 after explicit edits. Older binaries deliberately refuse those documents. Source revert does not downgrade data: retain byte-exact pre-migration metadata/assets backups and make any restoration a separate owner decision. Do not edit schema numbers, remove instances/accounts or restore over user content. No production migration is performed by this release preparation.
