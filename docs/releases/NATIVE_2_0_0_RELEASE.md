# Aurora Launcher Native 2.0.0 release record

**BLOCKED — PROTECTED APPROVAL REQUIRED**

Source integration, requested widget/icon inclusion, documentation organization,
versioning and independent validation are complete. The protected release is
waiting for its required owner review; publication has not occurred.

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
| Expected native tag | `native-v2.0.0`, not created yet |
| Public release title | Prepared as `Aurora Launcher Native 2.0.0` |
| GitHub release URL | Not published; the expected tag URL must not be treated as an available release |
| Installer filename | `Aurora.Native.Preview_2.0.0_x64-setup.exe` |
| Final public installer SHA-256 | Pending protected CI signing and publication; no public hash is claimed |
| Protected workflow | [Run 41374244252](https://github.com/kalibsolomon-pixel/Aurora-Launcher/actions/runs/41374244252), exact source above |
| Workflow inputs | Version `2.0.0`, minimum native version `1.4.2`, publication and authority advancement explicitly enabled |
| Resolve job | Passed exact-main/source/version/minimum/tag-absence guards |
| Build/sign job | Waiting for required reviewer `kalibsolomon-pixel` in `launcher-production` |
| Public signing/authority verification | Pending; local unsigned packaging is not production signing acceptance |
| Native authority | Still absent, protected from creation; no authority mutation performed |
| Authenticode | Executable and local installer `NotSigned`, under the accepted unsigned-publisher policy |

The first dispatch attempt returned workflow-not-found during GitHub registration
immediately after pushing the new workflow. Repository content and active workflow
registration were then confirmed, and the successful dispatch above was made.
There was no uncertain duplicate release or tag creation.

## Independent validation

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

The repository README and documentation index are current. The separate
[website draft PR](https://github.com/kalibsolomon-pixel/Aurora-Website/pull/1)
at `e5f68b9a443b3a057e61f397fcab1346d0e99cce` prepares the Native 2.0.0 section
using existing static styles, accurately marks publication as pending, and
preserves the WebView fallback. Local `npm run validate` passed, including ten
static tests; both GitHub validation runs passed. It adds no fabricated native
download URL or hash. Fill in actual verified release metadata before merging
and performing the separately protected website deployment. No website deployment
was dispatched.

## Exact protected actions and remaining limitations

1. The owner must approve the pending `launcher-production` environment review
   in run 41374244252. Its required reviewer and administrator-bypass prohibition
   were not changed. The agent did not approve its own protected operation.
2. Before approving native authority advancement, the owner/administrator must
   complete the separately controlled initial-ref procedure. Native ruleset
   `26335822` has **no creation bypass actors**; routine App credentials cannot
   initialize it. Preserve history-integrity rules and restore strict creation
   protection after legitimate one-time initialization, as described by the
   authority contract. No policy exception was introduced by this task.
3. The exact reviewed local initialization proposal is
   `a2e1a1c641d3d57c141abe724a82c9f14cab614a`, zero parents, canonical empty tree
   `4b825dc642cb6eb9a060e54bf8d69288fbee4904`, message
   `Initialize Aurora native preview update authority`. The workflow's
   `authority_parent` is pinned to that proposal. Its Git bundle and inspection
   record are retained outside Git under
   `C:\Dev\aurora-pandora-evidence-20261101\native-200-release-correction`.
   This proposal does not create a remote authority or authorize a different root.
4. Accept the exact protected CI artifact before approving the separate native
   publication gate, then approve the authority gate only after the intended root
   exists. Actual installer/envelope signatures, immutable public downloads/hashes,
   installed signed-byte acceptance, native authority bytes and final website
   downloads remain dependent on those protected actions. Prior test-key lifecycle
   and owner gameplay acceptance are reused, not relabelled as CI artifact results.

WebView release `v1.5.1`, its immutable artifact identity, and authority ref
`df7e824b2e49f8a5b9d2447ca2314bcafa21d119` remain unchanged. Neither
`native-v1.5.0` nor `v2.0.0` was created or repurposed. No release was falsely
labelled as published, and no private credential or generated package was committed.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
