# Native 2.1.1 protected release candidate

This is the historical preparation record before the owner confirmed permanent public naming on 2026-11-10. Its earlier filename decision is superseded by [the naming and migration contract](BRANDING_CONTRACT.md). Protected run 42814502513 was canceled before signing; the replacement must use the corrected source and newly built artifacts.

This records source preparation, not publication. The owner authorized source integration, commits, protected signing, immutable publication, and monotonic native authority advancement on 2026-11-10. GitHub environment approvals and production credential controls remain required.

## Source reconciliation

- Public Native 2.1.0 is immutable, published at `native-v2.1.0`, source `8190c821a3f7c8551d3edeb97a3ed5f2f31c00a5`. No higher native release was public during preflight. Candidate 2.1.1 is the next patch version; its tag must be created by the protected publication job.
- Integration starts from current main `7edfcff797656508bb7b297599ef4c4ed7daabb5`. The original development worktree remains at `8dcb308059ce25946270013727ab7d08f29212d8` on `codex/native-2.0.1-release-record`, with all 77 modified tracked files and 10 untracked files preserved.
- The final modpack source's full SHA-256 stamp is `fef3c96ad2c5bdf54c7e7f45e392f7319c5eb44b34ef7ab39f21c666796e61b8`. The stamp is the SHA-256 of the Git Bash `git ls-files -co --exclude-standard -z core native | xargs -0 sha256sum` inventory stream. Independent comparison confirms exactly the reported 28 phase-owned files changed, with no additions or missing files, against the 3,193-file baseline. That baseline equals the corrected Forge frozen-source manifest.
- The modpack release binary is `49c9a91325aa162a6226111c8cec11771a32767c9b8e49828dd0e5d566107588`. The previous owner-validated binary is `2bf8e0bdaf80918baa3c7388bb125b9adc36424d999d9ad7b1b9e74843587ab5`. Both were independently hashed and preserved. These are development evidence, not the newly versioned production artifact.

Evidence is retained under `C:\Users\kalib\Aurora Launcher\work\native-release-20261110`, including the original complete diff, staged diff, Git state, full source inventory, original changed files, source-scope audit, integration conflicts, public release downloads, authority documents, and validation logs. Earlier evidence remains untouched under `work\modpack-20261109`, `work\forge-followup-20261109-143701`, and `work\ui-review`.

## Included scope and integration

The source includes the reviewed native interface/Borealis/artwork/cache/scroll/shutdown work, gameplay-history process coordination, corrected Forge and legacy Minecraft/runtime compatibility, and Fabric/Forge/NeoForge Modrinth new-instance and existing-instance operations. Provenance comes from the UI integration and follow-up reports, the corrected Forge frozen source and validation results, and the modpack phase reports and patch series.

The approved native instance-content and widget implementations are integrated as coherent modules rather than mixing their cache/state models with older UI implementations. Current-main core saved-data readers/APIs and historical release/evidence files are retained. Skin row updates retain main's stable models and favorites. The history merge retains main's saved-favorite removal behavior and routes its saved-favorite writer through the reviewed process lock; a regression test checks that this writer preserves another launcher's updates.

Thirteen pending branding/installer-filename edits are excluded and left untouched in the original worktree. The release retains the established `Aurora.Native.Preview_<version>_x64-setup.exe` protocol, which published 2.1.0 clients require. No package/data/credential identity, `native-preview` channel, production trust key, protection rule, or historical WebView authority is changed. No tracked file deletion is intended.

## Owner-approved matrix waiver

The owner-attended Modrinth Fabric/Forge/NeoForge matrix is **waived by the owner as a release gate**. It was not run or passed. No live modpack installation or gameplay result is inferred from automated tests or headless renders. The historical matrix proposal remains preserved unchanged.

## Protected workflow and remaining acceptance

Use `.github/workflows/native-release.yml` from exact current main with version `2.1.1`, minimum installed version `1.4.2`, `publish=true`, and `advance_authority=true`. Reobserve the authority parent immediately before dispatch. Preflight native authority is `658da039fa15105320bbb9c3db50d64b7c5538b8`, offering signed Native 2.1.0. Historical WebView authority is `df7e824b2e49f8a5b9d2447ca2314bcafa21d119`, offering 1.5.1.

Signing requires approval of `launcher-production`; publication requires separate approval of `launcher-native-preview-publication`; authority advancement requires approval of `launcher-update-authority-production`. Never approve these on the owner's behalf, extract protected secrets, or publish outside the prescribed jobs.

The production public key's trimmed encoded SHA-256 is `bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`. Public 2.1.0 installer and manifest signatures verify against it, and fixed native discovery matches that immutable release. New signatures and package hashes exist only after the protected build/sign job. The established workflow publishes an installer and signed manifest inventory, not a separate portable binary/archive asset.

The matrix waiver does not waive the established acceptance of the exact protected signed installer in a disposable profile/VM, installed identity/restart checks, or production update acceptance. Those remain owner-attended where required by the protected procedure. Authenticode remains unconfigured; updater signatures must not be described as Windows publisher signing.
