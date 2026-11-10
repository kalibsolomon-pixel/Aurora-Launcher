# Aurora Launcher 2.1.1 release record

Status: **RELEASED**, published on 2026-11-10. [Public release](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/native-v2.1.1).

## Exact source and package

- Source and immutable tag `native-v2.1.1`: `048d0303d1db939495aa2d186bedc1ec6256de10`.
- Validated candidate: `056328ffeb4ae4575ceee19c67e981b21ea67b28`; identical source tree `4757ff62d49135744d6b627064cdca88202574d3`.
- Version `2.1.1`, channel `native-preview`, product `com.aurora.launcher.native-preview`, Windows x86_64, per-user NSIS.
- Public product name Aurora Launcher; executable `aurora-launcher.exe`; installer `Aurora.Launcher_2.1.1_x64-setup.exe`.

## Protected workflow and read-back recovery

[Protected release run #4](https://github.com/kalibsolomon-pixel/Aurora-Launcher/actions/runs/42916264341) passed source resolution, validation, build, production signing and immutable publication. The owner approved all three protected environments. The authority job prepared `d82e170f0f43b6dd73064abae775fb7b73ddf370`, advanced the native ref, and then failed its immediate read-back with `Native authority moved`. The workflow run remains failed; it is not represented as a green run.

Independent read-only verification established that the live native ref equals that prepared commit, its sole parent is the previous authority, its signed envelope equals the immutable release envelope, and the public installer and manifest signatures verify. All six public files match the protected build artifact exactly. No recovery write, forced update, tag movement, republishing or signing-key replacement was performed.

A regression fixture reproduces the exact failure when the first read after an accepted ref update returns the previous parent. Future releases now retry only bounded reads of that exact prior parent, requesting uncached API responses. They still refuse unexpected commits or malformed refs, never repeat the update write, and fail when read-back does not converge. The cause is consistent with a stale GitHub ref response; the failed job did not record the mismatching SHA, so its exact stale response is not asserted as captured evidence.

## Validation and acceptance

The protected build independently passed both formatting checks, 10 release-tool tests, 1,199 core tests with 33 ignored, 100 application tests with 2 ignored, 5 integration tests, the optimized build, package identity/version guards and production signature verification. Local validation of the identical source tree also passed both Clippy checks with the existing warning counts. Isolated installer checks passed filename/shortcut migration, ownership and ambiguity refusal, in-use files, concurrency, downgrade refusal, handoff and preservation of user content and unrelated shortcuts.

The owner confirmed in this release conversation that run #4's exact signed installer was tested in a disposable Windows profile or VM before publication approval. This is owner-reported acceptance; no additional DPI, focus or gameplay result is invented.

The owner-attended Modrinth Fabric/Forge/NeoForge matrix was **waived by the owner as a release gate, not run or passed**. No live pack installation or gameplay validation is claimed. CurseForge and Quilt modpacks remain outside scope.

## Public artifacts and production signatures

All public files were downloaded anonymously, checked against GitHub's SHA-256 digests and compared with the protected artifact. Installer and manifest signatures independently verify with the production-trusted key fingerprint `bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`.

| Public artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| [Aurora.Launcher_2.1.1_x64-setup.exe](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.1/Aurora.Launcher_2.1.1_x64-setup.exe) | 11,048,234 | `c3000b57886b78ec6311642f0d17952a905d8642fe5b75838add5302a603f8f1` |
| [Aurora.Launcher_2.1.1_x64-setup.exe.sig](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.1/Aurora.Launcher_2.1.1_x64-setup.exe.sig) | 428 | `377ad853a1df93eef514963e41d0b82e515dd6ad2700f547ce613b325684308e` |
| [native-release-assets.json](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.1/native-release-assets.json) | 1,143 | `9007d7ec6686e7f2c4212cd187cc5bc31554cbaf4e92d7d6a50bde3937d344f6` |
| [native-update-payload.json](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.1/native-update-payload.json) | 1,030 | `47858d5c5169aeb28d65288215685cb58973367481a6f9c1faac0e7d96f57758` |
| [native-update-payload.json.sig](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.1/native-update-payload.json.sig) | 416 | `9142f75a22da837a0f3e5909c03c9a4672d17ae9a8c0070bead1001e527af75a` |
| [native-update.json](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.1/native-update.json) | 1,822 | `ec92bf2df699618818d0369a0d2fbb1e768c9494722f585b128ba437138a2aa9` |

Windows Authenticode signing is unconfigured; the public installer independently reports `NotSigned`. Updater signatures authenticate the release and do not establish a Windows publisher identity. The established distribution does not include a separate portable executable or public archive. The private protected transfer ZIP independently hashes to `c591288983d9752962a62a4021498ca5063e13ea3fd4f069644c89747ccec6c4`.

## Update authority and preservation

Native authority advanced monotonically from `658da039fa15105320bbb9c3db50d64b7c5538b8` to its sole child `d82e170f0f43b6dd73064abae775fb7b73ddf370`. [Public discovery](https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-native-update-authority/native-update.json) serves the production-signed 2.1.1 offer, SHA-256 `ec92bf2df699618818d0369a0d2fbb1e768c9494722f585b128ba437138a2aa9`, bound to the exact source and installer above.

Legacy WebView authority remains `df7e824b2e49f8a5b9d2447ca2314bcafa21d119`, serving 1.5.1. Its discovery document remains byte-for-byte unchanged, SHA-256 `ac0f5170faf362969b3352076056f957de6dd7c4f804941328c7a210a3b6421b`. All 13 earlier public release inventories are preserved. Original development work and both preserved owner/modpack binaries remain intact.

Users on 2.1.0 or earlier must run the renamed installer once because their shipped updater requires the earlier filename. Later versions use the signed updater. Historical signed filenames, package/data/credential paths, signing trust and authority refs retain their compatibility identities. See [the naming contract](BRANDING_CONTRACT.md).

The published tag and assets remain pinned to the release source above. The later read-back fix changes release tooling and documentation on main; it does not replace or rebuild 2.1.1. No protected approval remains pending for this release.
