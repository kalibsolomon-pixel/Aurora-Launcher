# Aurora Launcher Native 2.0.0 release record

Status: SOURCE VALIDATION IN PROGRESS — publication has not occurred.

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

Final source SHA, workflow/run URL, public release URL, installer/executable
hashes, updater-signature verification, authority advancement and download
acceptance will be recorded from actual release results. Until then, none is
claimed as published or verified. Authenticode remains unconfigured under the
owner-approved unsigned-publisher policy. Prior signed lifecycle and owner gameplay
acceptance are reused; no new gameplay or unrelated benchmark is required.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
