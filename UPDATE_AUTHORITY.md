# Launcher update authority

This is a LOCAL implementation for owner/architecture review, intended for the later 1.4.1 release. No authority ref exists merely because this source was committed. Source pushes never publish updates. No 1.4.1 version bump or general correction work is included.

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

## Separate owner initialization (NOT performed)

1. Review the exact new ref and ensure it is absent. Never reuse the retired release/tag.
2. In a separately authorized one-time task, construct an empty tree and orphan commit (no parents), whose message is exactly `Initialize Aurora Launcher update authority`, in a disposable bare Git repository using `mktree` and `commit-tree`. This avoids assumptions about GitHub accepting an empty-tree creation request.
3. After reviewing those objects, push ONLY that new commit to the previously absent `refs/heads/launcher-update-authority`, without force. This one-time push requires separate explicit production authorization. Do not branch from main, check out into the developer tree, or add a README, workflows, keys, or placeholder manifest.
4. Verify ref, commit, empty tree and public-repository identity. Initial raw 404 is deliberate until the first approved authority publication.
5. Configure rules for this ref BEFORE publication: prevent deletion and force/rewind updates, restrict writers, and give only the reviewed publication identity necessary forward-update permissions. Do not give a general bypass that can rewind/delete. Exact compatibility of repository rules and GITHUB_TOKEN must be verified by the owner; automation fails on denied access rather than weakening rules.
6. Independently verify required reviewers/branch restrictions of `launcher-production`. An environment name alone is not reviewer protection. Build, release publication, and authority advancement each require a distinct deployment decision.

The normal publisher never creates or silently initializes a missing ref. Contents-write is needed only in release and authority jobs. Object APIs avoid checkout of the authority into a working tree. The authority tree has no Actions workflows.

## Normal publication

Manual workflow dispatch resolves the exact reviewed source. Protected BUILD/SIGN produces the private accepted artifacts; private key/password are supplied only to its signing step. Protected RELEASE PUBLICATION verifies transferred bytes, publishes the versioned immutable release, and downloads/verifies public bytes without credentials. This does not yet change discovery.

Only then approve the separate `advance_update_authority` deployment. It revalidates candidate schema, version/source/key provenance, fixed platform/installer URL, exact signature relationship, public release immutable state/inventory, public sizes/hashes/signatures and Latest identity. It reads the current ref and validates its complete minimal tree/manifest.

Older candidates and same-version different bytes fail before object creation. Identical bytes are idempotent and undergo public verification without writes. For a newer/first manifest, create its blob, minimal tree and commit with sole parent equal to the observed head. Re-read/verify the candidate objects and exact bytes. Report PREPARED. Re-read authority head and Latest. Advance the exact ref with `force:false`. This is the LAST publication mutation. Re-read the ref, report PUBLISHED, and verify unauthenticated exact canonical bytes before reporting VERIFIED.

Never routinely edit or upload the manifest manually. Never attach it to the retired release. Installer/signature/inventory must remain immutable. Object creation can leave unreachable objects after failure; they are not active authority and need no destructive cleanup.

## Concurrency and recovery

GitHub non-forced fast-forward is not a literal expected-old-SHA CAS. With forbidden rewinds/deletion and all writers creating sole-parent commits from their observed head, concurrent sibling writers cannot overwrite one another: one advance wins, the other fails non-fast-forward. Rechecks narrow races but do not replace ref protection. All authority jobs share a global concurrency group with cancellation disabled. External manual writers are prohibited.

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
