# Launcher update authority

This is a LOCAL implementation for owner/architecture review, intended for the later 1.4.1 release. No authority ref exists merely because this source was committed. Source pushes never publish updates. No 1.4.1 version bump or general correction work is included.

The security corrective pass isolates authority credentials and fixes UTF-8/race tests. Production App installation, environment protection and rulesets remain external owner prerequisites; local tests cannot prove that they are installed or effective. Do not dispatch or initialize until an independent review accepts both local commits and the owner separately authorizes setup.

## Identities and trust

- Immutable versioned release: signed NSIS installer, its `.sig`, and `release-assets.json` on `v<version>`.
- Mutable discovery ref: `refs/heads/launcher-update-authority`; tree contains only regular file `launcher-update.json` after first publication.
- Canonical public URL: https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-update-authority/launcher-update.json
- Trust root: existing production minisign public key compiled into Rust. The mutable manifest is NOT independently signed. HTTPS/GitHub availability never substitutes for installer signature verification.

The retired release 402692668 / tag `launcher-updates` is public, immutable, empty historical evidence. Never delete, recreate, move, upload to, or otherwise modify it. Immutable release assets cannot implement persistent replaceable metadata. The accepted public v1.4.0 release 402681397 and signed bytes remain untouched.

## Endpoint investigation and decision

2026-10-03 unauthenticated GETs of the public repository's `main/package.json`, accepted-SHA path, and a query-string variant returned 200. Direct raw URLs had no redirect, `Cache-Control: max-age=300`, ETag `"8e1c8762afe56c195784d1f9c2721b9accaac422905b8df0acb67466460718f7"`, no Last-Modified, and `Content-Type: text/plain; charset=utf-8`. Requests took about 0.17–0.31 seconds; the github.com/raw alternative redirected to raw.githubusercontent.com and took about 1.03 seconds. A query variant showed HIT, so query parameters are not treated as a guaranteed cache bypass. These observations are not a GitHub latency SLA.

Decision: use the direct raw URL and request `Cache-Control: no-cache`; do not use a redirecting wrapper or cache-busting URL override. Tauri 2.13.1 parses JSON regardless of the text/plain MIME type; its native reqwest client has no application HTTP cache by default. A request header is a revalidation request, not a guarantee of immediate CDN freshness.

No remote ref was advanced for this research. Actual GitHub propagation latency after advancement is therefore UNMEASURED. An isolated real Git ref + loopback HTTP cache test exercises old-content visibility after ref advancement and eventual exact-byte observation. Deterministic publisher fixtures cover stale/503 responses, bounded retry, exhaustion, and idempotent recovery. No zero-delay or max-300-second guarantee is claimed.

Stale prior metadata can delay an update; the official greater-version comparator does not downgrade installed newer versions. It cannot authorize unsigned bytes. Publisher verification uses up to seven public reads, separated by 60 seconds (six minutes of waiting plus bounded request time), checking the ref throughout. Exhaustion reports PUBLISHED but not VERIFIED, leaves the ref intact, and fails the job. A later separately approved retry is read-only for identical content.

## Dedicated credential and environment (owner setup, NOT performed)

Create a private, dedicated authority GitHub App and install it on **Aurora-Launcher only**. Required repository permission: **Contents: Read and write**, with GitHub's implicit Metadata read. Git blob/tree/commit creation and ref update require Contents write; release/ref/object reads require Contents read. No Actions, Administration, Secrets, Workflows, Issues, Pull requests, Packages, Deployments or organization permissions are needed. Contents cannot be scoped to one ref by token permissions; the rulesets below provide that restriction. Do not add repository-administration access to the App.

Use a separate environment named **launcher-update-authority-production**:

- Required owner-authorized reviewer(s); prevent self-review where a distinct reviewer is practical. Disable administrator bypass. Restrict deployment branches to exact reviewed `main`; prohibit tags and other branches.
- Environment variable `AURORA_AUTHORITY_APP_CLIENT_ID`: `<dedicated App client ID>` (public identification, not the installation ID).
- Environment secret `AURORA_AUTHORITY_APP_PRIVATE_KEY`: `<owner-managed dedicated App PEM>`; keep it only here, never in repository/organization secrets, source, diagnostics or chat.
- Environment variable `AURORA_UPDATER_PUBKEY`: `<same trimmed production updater public key>`; confirm its fingerprint matches signing/build provenance. No updater signing private key/password belongs in this environment.

The authority job's ordinary GITHUB_TOKEN is Contents read only. Checkout uses that read credential with persistence disabled; setup and artifact transfer occur before token issuance. After this job's independent environment approval, pinned `actions/create-github-app-token` v3.2.0 (`bcd2ba49218906704ab6c1aa796996da409d3eb1`) issues a token for the fixed owner/repository with explicit Contents write. The token is passed only to the publisher step as `AURORA_AUTHORITY_TOKEN`, never to checkout, job outputs or artifacts. The action revokes it at job completion; installation tokens also expire. A long verification can fail on expiry rather than silently refresh/fallback. See the [pinned official action](https://github.com/actions/create-github-app-token/tree/bcd2ba49218906704ab6c1aa796996da409d3eb1).

The publisher accepts only this explicit token variable and rejects missing/whitespace credentials or values equal to supplied GH_TOKEN/GITHUB_TOKEN. It does not use gh, inherited credentials or an implicit fallback. This is a wiring check, not cryptographic proof of the token's App identity: installed writer restrictions enforce identity. Environment names and local tests cannot establish owner approval or repository policy.

Read-only owner-setting inspection on 2026-10-03 still found `launcher-production` with one reviewer, self-review permitted, administrator bypass enabled and no deployment-branch restriction. Owner must harden the release/signing environment too: required reviewers, reviewed main, no admin bypass, and prevent self-review where practical. Keep existing signing material there; do not share authority App credentials with build/sign/release jobs. No setting or secret was modified by this pass.

## Layered authority rulesets (configure BEFORE initialization)

Use three **Active branch rulesets**, each with exact include `refs/heads/launcher-update-authority` and no exclusions (UI branch name `launcher-update-authority`). Do not use a general bypass covering all three.

| Ruleset | Rules | Normal bypass actors |
| --- | --- | --- |
| A — history integrity | Restrict deletions; block force pushes/non-fast-forward; require linear history | **None**, including updater App, Actions, administrators and initializer |
| B — writer restriction | Restrict updates | **Dedicated authority App only**, mode **Always** for direct API updates |
| C — creation restriction | Restrict creations | **None** after initialization; normal authority App excluded |

Do not require PRs, status checks or successful deployments on the authority branch: this direct-object protocol creates no PR and its new commit has no CI checks. Do not require signed Git commits until implemented; installer minisign verification is a separate mechanism. Ensure repository merge settings permit squash or rebase, as required for linear-history rules. Do not weaken rules after a denied API request.

Feasibility: this is a public, user-owned repository, so branch rulesets are available without organization-only push rules. GitHub supports installed Apps as bypass actors and exact name patterns before a branch exists. Active creation rules evaluate attempted creation. Multiple applicable rulesets aggregate; App bypass in B does not grant bypass in A or C. See [feature availability and layering](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets), [App bypass configuration](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository), and [creation/update/history rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets).

The read-only ruleset listing is currently empty. These capabilities support the design, but actual installed actor IDs, exact targeting, denial of ordinary Actions/human credentials, and App acceptance must be independently verified in a separately authorized setup task. Do not claim a local fixture proves server enforcement. Repository owners can deliberately change policy; this design does not prevent that administrative action.

## Separate owner initialization (NOT performed)

1. Configure the App, separate approved environment and all three active rulesets above. Verify the exact public repository and absent authority ref; never reuse the retired release/tag.
2. Separately authorize a temporary initializer exception in C, and in B if needed for the initializer's push. Never bypass A. For a manual owner push, use the Repository administrator role only after confirming it admits solely the intended owner; if it would admit others, use a distinct one-time initializer App with Contents write instead. Keep routine authority approvals disabled during this procedure.
3. In a disposable bare Git repository, construct an empty tree and orphan commit (no parents) using `mktree` and `commit-tree`, with message exactly `Initialize Aurora Launcher update authority`. Independently review both objects. Do not branch from main or add a README, workflows, keys or placeholder manifest.
4. Reconfirm absence and create ONLY that ref once, without force, under the temporary initialization authorization. Existing-ref collision is a stop condition, not an overwrite/recovery instruction.
5. Independently verify the exact ref SHA, orphan parents, message and empty tree. Record the root SHA outside ordinary mutable discovery metadata for operator audit.
6. Immediately remove all initializer exceptions from B and C. Verify normal B permits only the dedicated authority App, C permits no creation, and A has no bypass. No normal publisher can recreate a missing authority.
7. Confirm raw `launcher-update.json` returns 404 until the first approved manifest publication. Only after prerequisites are independently checked may normal authority approval proceed.

The normal publisher never creates or silently initializes a missing ref. Object APIs avoid checkout of authority into a working tree. It contains no Actions workflows.

## Normal publication

Manual workflow dispatch resolves the exact reviewed source. Protected BUILD/SIGN produces the private accepted artifacts; private key/password are supplied only to its signing step. Protected RELEASE PUBLICATION verifies transferred bytes, publishes the versioned immutable release, and downloads/verifies public bytes without credentials. This does not yet change discovery.

Only then approve `advance_update_authority` in the dedicated authority environment. It strictly decodes the original manifest bytes as UTF-8 before JSON/schema validation, then revalidates version/source/key provenance, fixed platform/installer URL, exact signature relationship, public release immutable state/inventory, public sizes/hashes/signatures and Latest identity. It reads the current ref and validates its complete minimal tree/manifest.

Older candidates and same-version different bytes fail before object creation. Identical bytes are idempotent and undergo public verification without writes. For a newer/first manifest, create its blob, minimal tree and commit with sole parent equal to the observed head. Re-read/verify the candidate objects and exact bytes. Report PREPARED. Re-read authority head and Latest. Advance the exact ref with `force:false`. This is the LAST publication mutation. Re-read the ref, report PUBLISHED, and verify unauthenticated exact canonical bytes before reporting VERIFIED.

Never routinely edit or upload the manifest manually. Never attach it to the retired release. Installer/signature/inventory must remain immutable. Object creation can leave unreachable objects after failure; they are not active authority and need no destructive cleanup.

## Concurrency and recovery

GitHub non-forced fast-forward is not a literal expected-old-SHA CAS. With forbidden rewinds/deletion and all writers creating sole-parent commits from their observed head, concurrent sibling writers cannot overwrite one another: one advance wins, the other fails non-fast-forward. Rechecks narrow races but do not replace ref protection. All authority jobs share a global concurrency group with cancellation disabled. External manual writers are prohibited.

The publisher validates the current minimal commit/tree and generated immediate parent, not the full ancestral history or a permanently pinned root. A root pin would detect an unrelated recreated history but cannot detect rewind to an ancestor or close a rewind after the final GET. Adding a mutable root setting/traversal would complicate initialization without replacing history protection; rulesets intentionally remain the primary control. A direct authorized writer could append an older valid manifest by fast-forward. Application monotonicity is guaranteed only for this validated protocol, not an arbitrary writer.

The fixture now uses ancestry-based fast-forward, accepting direct children and multi-generation descendants and rejecting sibling/unrelated/rewind updates. Deterministic barriers exercise two publishers before/after final reads, identical/drifting versions and both older/newer outcomes. Deletion fails without recreation; intentionally unprotected external rewind/recreation fixtures show that GitHub can accept the prepared descendant. Those tests demonstrate the required external control instead of pretending it is application CAS. Latest release and authority are not one transaction; an older candidate can finish if Latest moves after its final check, but cannot overwrite a newer authority head through this protocol.

- Before advancement: failures leave the previous authority active (including orphan objects).
- Rejected ref update: re-read; if still old, fail unchanged; if unrelated, fail concurrent movement. Never force or merge automatically.
- Ambiguous update response: one mutation only; GET current ref. If it is the exact prepared commit, verify public bytes; otherwise stop. If GET also fails, outcome remains ambiguous—inspect before a separately approved retry.
- PUBLISHED / not VERIFIED: retain new ref and immutable artifacts; investigate availability/cache. Do not roll back, delete or repeatedly advance it.
- Successful retry: identical candidate performs public verification without mutation.
- Emergency: pause future authority approvals and investigate. Do not rewind discovery or relax signature/URL checks. A corrective signed release must have a strictly higher version and receive ordinary acceptance/approval.

Same-version drift cannot be disguised as rollback. Do not cancel executing jobs as a recovery shortcut. Owner approval is per deployment, never inherited merely because the environment name matches.

## Runtime and bootstrap

Rust owns the fixed HTTPS raw endpoint, production public key, installer validation, and official Tauri download/install. No endpoint environment override or frontend/user setting remains. Authority redirects are rejected; verified GitHub installer redirects may reach only HTTPS release-assets.githubusercontent.com (bounded hops). Installer URL must match the production repository, versioned tag, numeric version and expected x64 NSIS name, with no credentials/query/fragment. Explicit loopback artifact acceptance is cfg(test) only.

Trust chain: mutable metadata → exact platform URL/signature → downloaded payload → compiled production key verification → retained verified bytes → rechecked version/URL/signature → explicit user install. Compromising the authority cannot produce a valid signature for an arbitrary executable. The locked CLI's authenticated filename contains the version; do not claim that every signature has an authenticated version field. Metadata itself is not signed.

Native deterministic coverage exercises the production validators without the cfg(test) loopback exception, the actual configured reqwest redirect policy against a loopback 302, CDN/hop restrictions, failed-download retention exclusion and exact install identity checks returning the original allocation. The desktop plugin builder and official signature-verifying download/Windows installation are not exercised by that boundary harness. Release-tool tests verify real diagnostic Ed25519 signatures separately; they do not substitute for future signed 1.4.1 runtime acceptance. No production key or real installer is used by these tests.

Existing 1.4.0 cannot self-update to 1.4.1. Its compile-time release-download endpoint is permanently unusable under the supported GitHub lifecycle. Keep accepted 1.4.0 bytes unchanged. Owner-reviewed signed 1.4.1 is a manual installer bootstrap. 1.4.1 → future release is the normal signed self-update path; first genuine production acceptance is expected to be 1.4.1 → 1.4.2, unless separately authorized otherwise. No synthetic production update proof is claimed.

## Review and later operator plan

Review the local corrective commit first. Push/integration, one-time authority initialization/protection, the general 1.4.1 correction pass, signed build acceptance, release publication and authority approval are separate future authorized steps. The already cancelled run 37111142571 attempt 4 cannot consume this new source. Never restart it as migration. No website work is needed by this architecture.

## Local verification record (2026-10-03)

Verification used a disposable source copy outside the repository for frontend/native outputs, preserving developer-local builds and diagnostics. No dependency, version, Client, frontend UI or production signing material changed.

| Command | Result |
| --- | --- |
| `npm ci` | Passed; existing lockfile preserved |
| `npm test` | 221 passed, 0 failed, 0 skipped; 16 suites |
| `npm run check` | 0 errors, 0 warnings |
| `npm run build` | Production SPA build passed |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Passed |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | Passed |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 816 library + 6 icon tests passed; 0 failed, 29 ignored |
| `node --test tools/release/*.test.mjs` | 66 passed, 0 failed, 0 skipped |
| `npm run tauri build -- --no-bundle --config '..\nonproduction-config.json'` | Optimized non-production executable built; no installer bundle |
| Real native boot | Fresh home view observed, no accounts/instances; isolated identifier `com.aurora.launcher.authorityverification` |
| `git diff --check` | Passed |

The external boot configuration supplies the updater plugin's required configuration object with an empty public key; no `AURORA_UPDATER_PUBKEY` was compiled. This deliberately exercises the unconfigured local build, not signed production acceptance. An initial boot without that required object failed; correcting only the external verification configuration and rebuilding produced the successful boot. Computer Use was stopped by the owner with Escape before Settings inspection completed; no further UI actions were taken. A genuine home screenshot and test/build logs remain outside Git.

The controlled Git/loopback cache test observed current bytes 160.5 ms after its local ref advancement, with cached old bytes immediately after the advance. This is a controlled test result, not actual GitHub propagation. Production initialization, rule/token compatibility, signed 1.4.1 acceptance and genuine 1.4.1 → 1.4.2 update acceptance remain future separately authorized work.
