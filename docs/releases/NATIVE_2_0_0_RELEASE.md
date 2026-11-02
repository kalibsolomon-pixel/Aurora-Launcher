# Aurora Launcher Native 2.0.0 release record

**RELEASED — AURORA LAUNCHER NATIVE 2.0.0**

**Subsequent owner decision:** Native 2.0.0 is now GitHub's stable **Latest**
release (`prerelease: false`). The active Tauri/WebView UI was retired and the
retained Rust core moved to `core/`. The immutable 2.0.0 tag and signed installer
are unchanged. See [native-only transition](../native-ui/NATIVE_ONLY_TRANSITION.md)
for the source changes and validation; preview-policy statements below record
the original publication sequence.

Source integration, requested widget/icon inclusion, documentation organization,
versioning and independent validation are complete. Protected CI has built and
signed the exact 2.0.0 installer and update envelope. Their downloaded bytes pass
independent signature, inventory and executable-identity verification. Publication
passed its owner gate. The immutable public release and every public asset have
been independently verified. The owner reports manually running the installer
and that it looks fine. Native update authority is active and independently
verified against the immutable release. The protected workflow completed
successfully on attempt 2, reusing the original signed artifacts. Distribution
works through GitHub and the repository README. Standalone website deployment
remains unavailable because its Cloudflare credential/account/project are absent.

The owner authorized Native 2.0.0, tag `native-v2.0.0`, and explicitly included
the existing responsive-widget and icon improvements. The original in-progress
merge was completed in place as `f226e013080dfb0323e95e2b83c7da28a6cd0e92`,
with parents `fcde5828a6b77ada8eb9f37cf6918a5754f3b0bb` and current WebView main
`4f9a923d1e23f05d74170b4925bf90134c0002e3`.

Native retains `com.aurora.launcher.native-preview`, `native-preview`, x64,
`nsis-per-user`, and its separate `launcher-native-update-authority/native-update.json`
discovery endpoint. The trimmed encoded production public-key SHA-256 is
`bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`.

The existing reviewer-gated `launcher-production` signing environment and
`launcher-update-authority-production` App environment are reused. Their required
owner reviewer, exact-main deployment policy and administrator-bypass prohibition
match the newly prepared native environments; no secret is copied or exposed.
Native publication retains its separate `launcher-native-preview-publication`
review gate. Both authority refs retain the three layered protection rules.

124 historical root documents were moved with Git into documentation folders.
Their complete mapping is [documentation-moves.json](documentation-moves.json).
Active Markdown links, agent/architecture references and release notes loading
were corrected. Historical content and intentionally tracked evidence remain.
The root retains README, license, agent instructions, canonical architecture,
release instructions and required source/build files. No generated application,
Minecraft state, private credentials or download cache is added.

Preserved WebView release: `v1.5.1`, release ID `418145573`, immutable;
source `4f9a923d1e23f05d74170b4925bf90134c0002e3`; installer
`Aurora.Launcher_1.5.1_x64-setup.exe`, SHA-256
`b668837752ccc50ca7654669b31080a970040fc2bce3df47cb6f3e35f79945bd`.
Native's independent version is 2.0.0; retained WebView/core package metadata
stays 1.5.1. No legacy tag is repurposed.

## Release identity and actual execution

| Item | Result |
| --- | --- |
| Reviewed release source commit | `33af6a6b619c375ed84b7ff495ff125c282386cd` |
| Integrated source branch | `codex/native-2.0.0-release`; normally fast-forwarded to GitHub `main` |
| Public native tag | `native-v2.0.0`, verified at `33af6a6b619c375ed84b7ff495ff125c282386cd` |
| Public release title | Aurora Launcher Native 2.0.0 |
| GitHub release URL | [Immutable release 431029795](https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/tag/native-v2.0.0) |
| Publication time | November 1, 2026, 10:05 p.m. America/New_York (`2026-11-02T03:05:05Z`) |
| Installer filename | `Aurora.Native.Preview_2.0.0_x64-setup.exe` |
| Public installer SHA-256 | `b72888bb1a57921e94b0fdbdec895c834c2f1522b4437f346827e1de3e28e09f`; independently downloaded without credentials and verified |
| Signed CI installer size | 10,222,028 bytes |
| Native executable SHA-256, extracted from that installer | `55ed5d536777aa09a30b535b001924f7aa206b746f6eff940cbad5dbf8521462` |
| Protected workflow | [Run 41374244252](https://github.com/kalibsolomon-pixel/Aurora-Launcher/actions/runs/41374244252), exact source above |
| Workflow inputs | Version `2.0.0`, minimum native version `1.4.2`, publication and authority advancement explicitly enabled |
| Resolve job | Passed exact-main/source/version/minimum/tag-absence guards |
| Build/sign job | Passed after owner approval: core/native tests, optimized build, NSIS packaging, production installer/envelope signing and signed inventory verification |
| Publication job | Passed after required owner review; immutable native prerelease, WebView latest preserved |
| CI signing verification | Passed independently on the exact downloaded CI installer and envelope using the established production public key |
| Public signing verification | Passed for all six immutable public assets, exact source/tag, installer signature and signed update envelope |
| Installed-release smoke | Owner reports manually running the installer and that it looks fine; environment and detailed startup/update results were not supplied |
| Authority job | Passed on attempt 2 after owner-authorized bootstrap; no build or publication repeated |
| Native authority | `c663d66f28fee33d611021200fae5f00b42668e8`, sole parent `a2e1a1c641d3d57c141abe724a82c9f14cab614a` |
| Public native discovery | Exact accepted envelope bytes and production signature independently verified; SHA-256 `63e1187c2528aaff4344e6f034815d675324799c7e193d2d46690658b1b0ba10` |
| Authenticode | Executable and local installer `NotSigned`, under the accepted unsigned-publisher policy |

The first dispatch attempt returned workflow-not-found during GitHub registration
immediately after pushing the new workflow. Repository content and active workflow
registration were then confirmed, and the successful dispatch above was made.
There was no uncertain duplicate release or tag creation.

## Independent validation

- Exact protected CI artifact `13110968951`,
  `native-signed-2.0.0-33af6a6b619c375ed84b7ff495ff125c282386cd`, was downloaded
  and verified with `native-update.mjs verify`. Both real minisign signatures,
  payload identities, sizes, hashes and all six inventory entries pass. The
  artifact ZIP digest recorded by GitHub is
  `6ddce3fe8a006fb0954a93da6388a983f2c29a1ef0268801ed1925a43ed48a1c`.
- The exact CI installer was extracted without executing it. Its payload EXE
  reports product/file version 2.0.0 and PE x64. The native package identity,
  production public key and fixed native authority endpoint are present;
  diagnostic-root/probe/acceptance-product markers and WebView/debug payload
  files are absent. Installer and EXE Authenticode remain `NotSigned` under the
  accepted publisher policy. See [CI artifact audit](native-2.0.0-ci-artifact-audit.json).
- The owner confirmed manually running the installer and reported that it looks
  fine. That report supplies manual installer acceptance; no isolated-profile,
  configuration-preservation, exact installed-byte or production-trust update/
  restart result is inferred. The agent did not install over owner data.
- All six immutable assets were independently downloaded without authentication
  and matched the protected CI byte inventory. `verifyPublicNative` and
  `verifyNativeDirectory` passed on those public bytes. The public native tag
  points to the reviewed source, and GitHub confirms an immutable prerelease.
  [Public verification record](native-2.0.0-public-verification.json) preserves
  release identity, installer/envelope signatures and all signed-content hashes.
- The public native discovery endpoint now returns that same accepted envelope.
  Its signature, version 2.0.0 and reviewed source SHA were independently checked.
  [Authority verification](native-2.0.0-authority-verification.json) records the
  result. Workflow attempt 2 is green for all jobs. Existing native test-key
  update/restart evidence is retained; no real strictly-newer production-trust
  installed update/restart is claimed from this first native publication.
- Core: 1,117 tests and six integration tests passed; 31 existing opt-in tests
  remained ignored. A merge-introduced duplicate WebView test block was removed,
  retaining the original feature-gated tests; default-host all-target checking
  also passed.
- Native: 52 tests plus five picker integration tests passed, including responsive
  widget layout, scaling and playback-pointer activation. Representative compact
  and large captures and the approved icon were visually inspected. The SVG
  matches the owner-approved source SHA-256
  `780039369189022782249d46daf69c0eaf779c7bfe7599c1b683080b5cf9c205`.
- Both Rust format checks, frontend type checking (zero errors/warnings), and
  production frontend build passed. No unrelated gameplay or performance
  investigation was repeated.
- All 127 release-tool tests passed, including native 2.0.0 publication identity,
  signature, inventory, channel separation, stale-parent and authority protections.
- Optimized native build passed with the existing production public key and
  fixed native authority opt-in. Executable and NSIS product/file versions are
  `2.0.0`. Binary audit confirms x64/native identity and production key, and rejects
  diagnostic-data-root/probe features. No test signing identity is distributed.
- Local NSIS packaging passed. The **local, not updater-signed or public**
  installer is 10,240,479 bytes, SHA-256
  `7554de7d5f80491d5f3cff782019b0c7d3cd5dd6c5e942c0eebf2dfc2ce4ea46`.
  Local native EXE SHA-256 is
  `811a5a17d53c0811fa8c1c2e7c54fc44512b74bf1ba8867565e5479e730f36a5`.
  CI's final signed inventory must replace these provisional local hashes in
  public distribution records.
- Documentation audit accounts for all 3,309 baseline tracked paths via the move
  map, with zero missing files and zero broken local Markdown links. Raw evidence
  inventories retain their original bytes rather than rewriting historical facts.
  No unrelated tracked file was deleted.
- Twenty-one pre-existing changed/untracked files in the other checkouts,
  including the original responsive-widget work, retain their recorded hashes.
  Approved native artwork was committed from the existing patch; unrelated
  developer changes were excluded. No owner installation or game data was changed.

## Website and public presentation

The repository README links the verified public native installer. The separate
[website PR](https://github.com/kalibsolomon-pixel/Aurora-Website/pull/1), source
`10713d5bb44c0132100fef76fab37364e7357d47`, was merged as
`19fb07f74acaa71eb58a135de99eab8bd62c3e0b`. It links the real immutable installer,
checksum, detached signature and release notes within existing styles, retaining
the WebView fallback. [Follow-up PR 2](https://github.com/kalibsolomon-pixel/Aurora-Website/pull/2)
removes the obsolete pending-authority notice and adds the owner's attribution
policy to the website's agent guide. Both GitHub checks passed; it was merged
as `f55fa4d5c11d7a20226fce78bc9c4f350917ae21`.
Local `npm run validate` and both GitHub validation checks passed, including ten static tests. The native download
section was visually inspected in the local browser. No dependency/style/runtime
change or new performance claim was introduced. Production deployment is not
configured: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID` and
`CLOUDFLARE_PAGES_PROJECT_NAME` are absent from `aurora-website-production`, and
`AURORA_PRODUCTION_ENABLED` is false; local Wrangler also reports no authenticated
Cloudflare session. No deployment or invented hosting URL is
claimed. GitHub's public release page already provides working distribution.

## Completed authority setup and remaining limitations

1. Build/sign and publication reviews were approved and both jobs passed in
   [run 41374244252](https://github.com/kalibsolomon-pixel/Aurora-Launcher/actions/runs/41374244252).
   The owner supplied manual installer acceptance. Review protections remain
   intact; the agent did not approve its own protected operation.
2. The owner explicitly authorized completing the documented one-time setup and
   protected review. After confirming that the owner is the sole administrator,
   initialization used temporary exceptions for the exact owner user ID
   `288523859` in native rulesets B/C only. Both exceptions were removed
   immediately after the non-forced creation. B again permits only authority App
   `5182303`; C has no bypass actors. A remained active and unchanged, with no
   bypass, deletion restriction, non-fast-forward blocking and linear history.
3. The exact reviewed and now initialized root is
   `a2e1a1c641d3d57c141abe724a82c9f14cab614a`, zero parents, canonical empty tree
   `4b825dc642cb6eb9a060e54bf8d69288fbee4904`, message
   `Initialize Aurora native preview update authority`. The workflow's
   `authority_parent` is pinned to that proposal. Its Git bundle and inspection
   record are retained outside Git under
   `C:\Dev\aurora-pandora-evidence-20261101\native-200-release-correction`.
   Remote commit inspection confirms its exact tree, zero parents and message.
4. The first approved authority attempt encountered the absent ref and failed
   with HTTP 404 before any authority write. After root/protection verification,
   only the failed authority job was retried. Its normal environment review was
   approved under the owner's explicit instruction. The dedicated App advanced
   the native ref without force, verified the public CDN bytes and completed
   successfully. No immutable asset or tag was rebuilt, replaced or moved.
5. A production-trust installed upgrade/restart has not been newly exercised;
   the owner supplied manual installer acceptance, and prior isolated test-key
   lifecycle evidence remains the update-mechanics acceptance. No safe isolated
   production-update environment was available here. Website hosting also needs
   a real Cloudflare credential/account/project before its established protected
   deployment can run. The signed GitHub installer remains publicly accessible.

## Repository attribution

There are no AI accounts among repository collaborators. Historical assistant
author aliases and co-author trailers were found in existing Git history.
Repository-local author/committer configuration now uses the owner's verified
GitHub identity. `.mailmap` canonicalizes the observed assistant aliases for Git
tools that honor it, and `AGENTS.md` forbids future AI author/contributor credits
or attribution trailers. Genuine human attribution and licenses are preserved.
Raw published commit metadata/messages are intentionally unchanged: removing
those historical fields would rewrite history and disrupt immutable release
provenance. No GitHub contributor-display refresh or historical erasure is claimed.

WebView release `v1.5.1`, its immutable artifact identity, and authority ref
`df7e824b2e49f8a5b9d2447ca2314bcafa21d119` remain unchanged. Neither
`native-v1.5.0` nor `v2.0.0` was created or repurposed. Only `native-v2.0.0`
was published. No private credential or generated package was committed.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
