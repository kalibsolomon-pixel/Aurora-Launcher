# Aurora Launcher Native 2.1.0 release record

## Source and distribution

- Release source `8190c821a3f7c8551d3edeb97a3ed5f2f31c00a5`, integrated through [PR #4](https://github.com/kalibsolomon-pixel/Aurora-Launcher/pull/4).
- UI-2 `f80af44f5fe04237e157395a643e28cdb078b9d7` contains approved UI-1 `bc355b5e9617d22df96c587e3d46c272d7606aa0`. Both descend from the published 2.0.1 source `82620e57b5c8b933dc970fae02200635df8cfa7c`. The integrated application source matches UI-2. Release preparation changed version metadata, the workflow version default and release documentation.
- Stable and Latest, published at 2026-11-03 18:51:19 UTC as immutable release 433092492. [GitHub release and tag native-v2.1.0](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/native-v2.1.0)
- Windows x64 NSIS per-user package. Slint runtime, application identity `com.aurora.launcher.native-preview` and update channel `native-preview`. [Installer](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.0/Aurora.Native.Preview_2.1.0_x64-setup.exe)

## Artifact integrity

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `Aurora.Native.Preview_2.1.0_x64-setup.exe` | 10,487,420 | `4add95991bc81c8415b15c7c5c237b7361e7041be0ef44ba321635a39d3083fc` |
| Extracted x64 executable | 34,911,744 | `25696c9e85364838029def63cd8d8293efcadf2faad18cfb4f5489fb2d47d654` |
| `native-update.json` | 1,838 | `d521aa9612af813bca82754b6752625ee7ee7c1d646f344e8b3381d70e833166` |

All six files downloaded without authentication and matched the signed CI
inventory. The installer and manifest payload updater signatures verified with
the established production public key on both CI and public bytes. The SHA-256
of the trimmed encoded key is
`bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`.
The executable was extracted without running the installer. Both files report
version 2.1.0 in their Windows metadata. The executable contains the production
key and native endpoint. Windows Authenticode independently reported
`NotSigned` for both files. [Published signed manifest](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.0/native-update.json)

## Protected CI and regressions

The required protected workflow is `.github/workflows/native-release.yml`.
Source resolution, build/signing, publication and authority advancement all succeeded through the existing review gates. Artifact
13261031704 has GitHub ZIP digest
`sha256:dbbc97ebe4c240335a4b57a928808c91bba9d3136fd83d2c3fb307b51d5c1048`.
Version checks, eight release-tool tests, both Rust format checks, the locked
optimized build and NSIS packaging passed. Core ran 1,132 passing tests with 31
ignored. Native ran 64 passing tests with one ignored. CI passed the existing
runtime-mutable modpack, deterministic content verification, interrupted
promotion and recovery regressions, and the Discover Show More interaction
regression with a bottom-right download notification. [Protected run 41688781531](https://github.com/kalibsolomon-pixel/Aurora-Launcher/actions/runs/41688781531)

## Authority and preservation

- Native authority advanced from `0146fba27066b10166c2444e78d9d44798e4cb05` to `658da039fa15105320bbb9c3db50d64b7c5538b8` in one non-forced fast-forward. The unauthenticated native discovery envelope matches the public release manifest byte for byte. [Native discovery](https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-native-update-authority/native-update.json)
- The extracted public 2.0.1 executable embeds the same production key and endpoint. The verified 2.1.0 offer matches the updater identity and package requirements, and `1.4.2 <= 2.0.1 < 2.1.0` satisfies the minimum and upgrade checks.
- The legacy WebView authority remains `df7e824b2e49f8a5b9d2447ca2314bcafa21d119`.
- All six immutable 2.0.1 assets were independently downloaded and hash-checked. Their signatures and inventory passed, and their remote metadata remained intact after advancement. The previous installer SHA-256 is `8598d38b9ab7e0e1cff8ba42acb0124374e826d4c1b89a19f359176e7cbf28dd`. [Native 2.0.1 release](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/native-v2.0.1)
- The website download URL, size and SHA-256 were updated in its source and passed local and main-branch CI. Cloudflare production deployment is disabled for the site. The public installer is distributed through GitHub. [Website PR #5](https://github.com/kalibsolomon-pixel/Aurora-Website/pull/5)

## Limits

Extremely rapid gameplay transitions can overwrite Minecraft's built-in activity
record before Aurora observes it. Native fallback history and Quick Play cover
the validated Minecraft 1.21.11 launch path. The existing development boot and
UI evidence were reused. A live production-signed install or upgrade and the
owner-attended multiplayer/world scenarios were outside this release verification.
[UI-1 acceptance](../native-ui/NATIVE_UI_REFINEMENT_ACCEPTANCE.md) · [UI-2 acceptance](../native-ui/NATIVE_UI_2_ACCEPTANCE.md)
