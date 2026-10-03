# Aurora Phase L — Simplified Updates Correction

## Verdict

**COMPLETE. Owner review required; do not integrate yet. Nothing pushed.**

One public release stream and one aggregate Updates action replace the original Phase L channel experience. Client and Launcher trust/activation engines remain independent. No production release or installation was changed.

## Repository

Branch: `codex/phase-l-production-updates`. Starting HEAD: `9b19f3af2acf1adfd92282d44727e5cb711ab966`. Local main: `ae17ee9f76843c0a5793d1cf7aa683d806afef8a`; origin/main: `20d7d5cdc9805c1cdbcd35217021355c1a4c745f`. Both refs remain unchanged. Starting ahead/behind: main 5/0, origin/main 43/0. Final HEAD, counts, commit subjects, and working-tree state are recorded after the report commit in `.zcode-diag/phase-l-simplified-updates/final-git.json` and the delivery message. Original tracked count 366; final count 369. Tracked deletions: **0**. Tracked worktree clean after commits; pre-existing untracked work and new diagnostics remain untracked. No push, merge, integration, or publication.

## Product simplification

Removed public Stable, Beta and Nightly choices, persisted update preference/accessors, selector UI and channel DTO/commands, channel eligibility ladder, per-channel Launcher sources, notice dismissal bookkeeping/commands, separate Download/Install controls, and the duplicate instance Client update manager. Legacy classification remains only where persisted exact release identities and fixtures need compatibility. It never chooses an update. Existing Modrinth per-item Stable/Beta/Alpha content policies are outside this product correction and remain intact.

## Final release authority

### Aurora Client

One owner-published schema-1 release truth: `https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/release-manifest/aurora-releases.json`. Choose the newest compatible intentionally published semantic version newer than the installed Client. Exact Minecraft/Loader requirements and Java compatibility remain mandatory; apply verifies resolved game-plan authority. Equal versions and build metadata alone are current; newer installed versions never downgrade; malformed remote versions reject. New entries may omit historical `channel`. Unpublished dev fixtures never join remote discovery.

### Aurora Launcher

One official Tauri static manifest: `https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/launcher-updates/launcher-update.json`. Contract: semantic `version`, optional `notes`/`pub_date`, `platforms.windows-x86_64.{url,signature}`, exact signed NSIS bytes. Compile-time diagnostic override: `AURORA_LAUNCHER_MANIFEST_URL`. No runtime/frontend URL, key or hash input.

### Manual publication

Only publishing reviewed metadata and artifacts through the owner gate exposes updates. The unchanged release workflow uses `workflow_dispatch`, exact audited source, and reviewer-gated publication; ordinary push/build/test events do not publish. This correction invokes no publication workflow and creates no public tag or release.

## Configuration migration

Schema remains **7**. Current configuration has no `updates` field. Original schema-7 stable, beta and nightly preferences validate strictly and are discarded in memory into identical production behavior. Load leaves original bytes unchanged; the next ordinary atomic save omits the obsolete field. Schemas 1–6 retain explicit existing migrations, without adding a channel. Selection, appearance, widgets and Discord/privacy preferences survive. Malformed legacy fields/documents and unknown schema versions fail without overwrite. No instance pin migration or directory move.

## Final update UX

`Check For Updates` -> disabled `Checking…` -> disabled `None Available` for exactly 5,000 ms when both are current (or Client is inapplicable) -> local `Check For Updates`. The reset makes no network request and is cleared when the view is destroyed. Valid offers show one `Update`; real operation phases show `Updating…`, `Downloading…`, or `Installing…`. Errors show `Check Failed — Retry` or `Update Failed — Retry`, with concise truthful context. If one service is unavailable while the other has a valid offer, Update remains actionable for that offer and the unknown domain stays explicitly unknown. Changed offers require a fresh check and a new Update interaction.

## Aggregated orchestration

Launcher-only: one action performs official verified download then install. Client-only: one action performs exact-instance/version preview and fingerprinted native apply, then refreshes local state. Both: Client completes and validates **before** Launcher download/install begins. No parallel mutations. Client failure prevents Launcher mutation. Client success plus Launcher failure preserves the Client commit, reports partial success, and retry touches only Launcher. Controller deduplicates repeated clicks; native aggregate checks and existing transaction/content/registry/launch locks preserve process-local exclusion. No cross-process coordination is claimed.

## Preserved security

### Client

Pre-known expected SHA-256, verified store acquisition and rehash, compatibility, native plan/fingerprint revalidation, running-instance refusal, managed containment, staged activation, recoverable old artifacts, whole-instance validation, commit-last pin/state, and byte-exact rollback remain. User mods/config/saves and credentials remain outside the transaction. HTTPS manifest discovery is honestly bootstrap metadata, **not independently signed**; no signing infrastructure is invented.

### Launcher

Official Tauri updater and mandatory minisign/signed-version verification remain. Existing >=2.12 security floor and lockfile 2.13.1 are unchanged. Compile-time public trust key only; missing key is honestly unconfigured and cannot download/install. Recheck presented version before download; recheck exact version/URL/signature before installation to reject same-version drift. Install consumes the verified retained bytes. Production HTTPS only; explicit loopback HTTP is isolated diagnostic transport. Windows passive NSIS exits and relaunches; no shell command strings or fabricated running-version success.

## Startup behavior

One bounded check per process follows local initialization. Current/offline startup is silent and nonfatal. Actual available releases show one quiet Home notice opening canonical Updates; Play authority is unchanged. Manual checks are explicit; a manual request during startup waits and performs one fresh aggregate check. No polling, watcher, background update loop or automatic install. The only update timer is the local five-second confirmation reset. Publisher request timestamps show long idle/navigation intervals without requests.

## UI

Settings retains the Phase F glass surface, compact installed version rows, one primary action, concise offer/error context, and a secondary What's New disclosure. Home has one unobtrusive availability link. Instance workspace has concise selected-instance availability plus View Updates, with no duplicate apply manager. Notes are bounded plain text rendered as escaped text nodes. Unknown-total phases use indeterminate progress; Launcher download bytes come from actual updater callbacks. No technical URLs, digests or stacks are exposed in ordinary flows.

## Deterministic acceptance

All A–BB cells are individually accounted for below. Controller tests drive the same controller as Svelte with fake timers; surface tests render actual Svelte components through SSR. Native Client/config regressions use disposable local fixtures and loopback transport. Official Windows updater trust/activation is additionally proven through the real diagnostic application because this host's mock runtime cannot load that plugin; those cells distinguish runtime evidence/source audit from unit fixtures.

| Cell | Result and evidence |
| --- | --- |
| A | PASS — initial controller action Check For Updates. |
| B | PASS — deferred check exposes disabled Checking. |
| C | PASS — both current produces None Available. |
| D | PASS — no-update action disabled; repeated activation performs no check. |
| E | PASS — fake clock stays None at 4,999 ms and resets at 5,000 ms. |
| F | PASS — fake-clock reset preserves exact calls; live timer request logs identical. |
| G | PASS — close clears timer; completion after destruction creates none. |
| H | PASS — valid availability produces Update. |
| I | PASS — Launcher-only Update and download/install sequence. |
| J | PASS — Client-only Update and native apply sequence. |
| K | PASS — both offers use one controller action and one rendered primary button. |
| L | PASS — repeated Checking shares the same promise/call. |
| M | PASS — unavailable/rejected manual check with no offer produces Check Failed — Retry. |
| N | PASS — failed mutation produces Update Failed — Retry. |
| O | PASS — Client success reflects new version/current truth; Launcher never fabricates running version. |
| P | PASS — current config serializes with no update preference. |
| Q | PASS — legacy stable strict migration test. |
| R | PASS — legacy beta reaches the same production behavior. |
| S | PASS — legacy nightly reaches the same production behavior. |
| T | PASS — migration preserves selection, appearance, widgets, privacy and original load bytes. |
| U | PASS — one exact production Launcher source test. |
| V | PASS — published historical classifications have no eligibility filtering. |
| W | PASS — newest compatible published semver wins. |
| X | PASS — separate unpublished development fixture never enters discovery. |
| Y | PASS — equal Client version current; build metadata alone not newer. |
| Z | PASS — installed-newer outcome prevents downgrade. |
| AA | PASS — incompatible Minecraft/Loader and Java newer candidates ignored/refused. |
| AB | PASS — malformed remote versions reject; malformed semver parser cases remain. |
| AC | PASS — one explicit check calls each domain once. |
| AD | PASS — offline domain preserves other's exact offer and permits its Update; unknown state retained. |
| AE | PASS — both current confirms None Available. |
| AF | PASS — Launcher available/Client current becomes Update. |
| AG | PASS — Client available/Launcher current becomes Update. |
| AH | PASS — both available becomes one Update. |
| AI | PASS — deferred Client apply proves Launcher starts only after completion. |
| AJ | PASS — exact call order Client apply/local refresh then Launcher download/install. |
| AK | PASS — Client remains committed after Launcher failure; retry only Launcher. Also live partial-success evidence. |
| AL | PASS — failed Client apply prevents Launcher calls. |
| AM | PASS — once-only current startup silent, no timer; actual Home SSR absent notice. |
| AN | PASS — startup offer survives navigation; actual Home has restrained notice. |
| AO | PASS — unavailable/rejected startup stays silent and usable. |
| AP | PASS — manual offline Retry and real loopback 503 diagnostic. |
| AQ | PASS — wrong SHA-256 native regression and live rejection. |
| AR | PASS — staging/activation/validation/persistence rollback preserve old bytes/pin. |
| AS | PASS — running-instance native regression refuses mutation. |
| AT | PASS — changed-world native fingerprint refused; controller changed offer Retry requires fresh review. |
| AU | PASS — full native transaction asserts unrelated user files byte-identical. |
| AV | PASS — official updater runtime rejects tampered diagnostic signature; no weakened verification. |
| AW | PASS — unavailable endpoint/current launcher preserved; controller offline and live 503 remain usable. |
| AX | PASS — native source audit enforces version/URL/signature recheck; live request 29 revalidated before first official install, with the final retry likewise recorded. This is runtime/audit evidence, not a mock-plugin unit fixture. |
| AY | PASS — semver/native Client no-downgrade cases; official updater greater-version policy unchanged. |
| AZ | PASS — fake timer/startup once tests and source audit: no update polling/watchers; idle request log quiet. |
| BA | PASS — narrow Rust command DTO/source audit, no updater frontend capability or runtime URL/hash input; transport validation tests reject insecure/credential-bearing sources. |
| BB | PASS — missing compiled trust test and controller unconfigured state; production builds embed no diagnostic key. |

## Diagnostic acceptance

All live checks are **DIAGNOSTIC**, not public production tests. Separate identifier `com.aurora.launcher.simplifieddiagnostic`, separately installed Aurora Simplified Diagnostic, independent copied instance/cache, no copied credentials, and publisher bound to literal `127.0.0.1:8789`. Older diagnostic evidence/keypair is read-only. Synthetic Client 2.2.x diagnostic JARs are loopback fixtures only. Launcher 1.3.0 -> 1.3.1 is signed with the existing diagnostic key, never production signing.

| Required scenario | Result |
| --- | --- |
| 1. Boot, no channel selector | PASS — actual installed diagnostic application boots normally. |
| 2. One primary Check | PASS — compact Settings area, actual accessibility/screenshot and SSR count. |
| 3. Current state sequence | PASS — Checking -> None Available -> Check For Updates. |
| 4. Reset no request | PASS — `timer-proof.json` before/after publisher logs byte-identical across 5.1 s. |
| 5. Client only | PASS — 2.2.0-diag.1 -> 2.2.1-diag.1; installed manifest/hash and UI agree. |
| 6. Launcher only | PASS — official signed download, revalidation, passive install, exit and relaunch 1.3.1; Client 2.2.1 retained. |
| 7. Both, one interaction | PASS — Client 2.2.1 -> 2.2.2 committed before Launcher download. Tampered signature deliberately demonstrated partial success; explicit retry installs only Launcher. |
| 8. Manual offline | PASS — loopback 503 yields Check Failed — Retry without instance/account/readiness corruption. |
| 9. Tampered Launcher signature | PASS — official plugin rejects, installed executable remains 1.3.0 until valid retry. |
| 10. Wrong Client digest | PASS — rejects 2.2.3-diag.1; installed Client remains 2.2.1 at that point. |

Final signed diagnostic rebuild includes the changed-offer Retry refinement and partial-service offer behavior. Final relaunch and layout are captured after its official verified install. The installed executable matches the final build except for Tauri's three-byte bundle-type marker (`NSS` in NSIS versus `UNK` in the restored build copy); the exact hashes and read-only comparison are recorded in `final-relaunch-verification.json`. Existing missing runtime/account blockers in the copied diagnostic instance are unchanged by availability and updates; no production game/account was used. The diagnostic application and loopback publisher were closed after acceptance.

## Verification

Evidence root: `.zcode-diag/phase-l-simplified-updates/`.

| Exact command | Result / log |
| --- | --- |
| `npm run check` | PASS — 0 errors, 0 warnings; `check-final.log`. |
| `npm test` | PASS — 221 tests, 16 suites, 0 failures/skips; `frontend-final.log`. |
| `npm run build` | PASS — static production SPA; `build-final.log`, also final Tauri before-build. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | PASS — `format-final.log`. |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | PASS — `cargo-check-final.log`. |
| `cargo test --manifest-path src-tauri/Cargo.toml` | PASS — 814 library + 6 icon tests = 820 passed; 29 existing gated ignores, 0 failures; `rust-final.log`. |
| `npm run tauri build` | PASS — production NSIS and MSI, unchanged version 1.3.1; `tauri-build-final.log`. |
| Real application boot | PASS — isolated installed/signed updater relaunch; screenshots and request log. |

No dependency or lockfile changes. Existing native warnings remain (unnecessary mut in Modrinth, dead helpers, Tauri STATIC_VCRUNTIME deprecation); no new frontend diagnostics. Existing Fabric, NeoForge, provider/provenance/content updates, modpacks/reconciliation, auth/runtime/process/security regressions remain green. Expensive unrelated prior acceptance was not repeated.

## Visual evidence

All screenshots below are **DIAGNOSTIC** real Windows application captures. Original helper JPEGs are retained; requested PNG files are format conversions only, with no fabricated content, cropping, compositing or progress. One transient window-targeting inconsistency after maximization was recovered by restoring the window; acceptance uses the recovered target. Release-notes and additional sequencing/partial-result captures are retained beside these twelve.

| Filename | What it proves |
| --- | --- |
| `01-updates-idle.png` | One compact Check action; versions; no channel selector. |
| `02-updates-checking.png` | Genuine disabled Checking and indeterminate progress. |
| `03-none-available.png` | Disabled no-update confirmation. |
| `04-update-available-client.png` | Client-only offer, one Update. |
| `05-update-available-launcher.png` | Launcher-only offer, one Update. |
| `06-update-available-both.png` | Both offers behind the same action. |
| `07-update-progress.png` | Real official updater byte callbacks. |
| `08-update-complete.png` | Committed Client version and truthful success. |
| `09-check-failed-retry.png` | Concise offline Retry. |
| `10-update-failed-retry.png` | Wrong-digest failure, unchanged installed version. |
| `11-home-update-notice.png` | Quiet Home notice; normal Play blockers unchanged. |
| `12-updates-final-layout.png` | Final code after signed relaunch; compact glass layout. |

## Documentation

`PHASE_L_PRODUCTION_UPDATES.md` now specifies one stream/manifest, configuration compatibility, single-action state machine, sequencing and partial success, startup cadence, trust, publication and owner requirements. `ARCHITECTURE.md` current Phase L and Distribution metadata sections align with implementation while earlier phases remain historical. `RELEASING.md` no longer falsely says no updater exists and distinguishes the still-unconfigured owner signing/publication step. Original Phase L acceptance and diagnostics remain preserved.

## Website handoff contract

One Client release truth (`aurora-releases.json`), one signed Launcher update manifest (`launcher-update.json`), no public channels. Mirror/present these authorities; never use commits/builds as release truth. Exact compatibility, expected Client SHA-256 and official Launcher signatures remain. No website work was started.

## Production owner requirements

Generate/protect the real production updater keypair; configure compile-time public verification and release-environment private signing; enable official updater artifacts; manually publish reviewed signed immutable NSIS bytes and signature plus the one Launcher manifest; manually publish reviewed Client manifest entries with pre-known expected SHA-256. Production signing infrastructure and publication were not invented or executed. No secret values are exposed here.

## Safety

Production Launcher 1.3.1 executable, launcher config/registry/accounts/history, Client 2.1.5 installed state and artifact match their exact recorded baseline hashes. Client/Fabric API also verify against the original expected hashes. Production data was only read for this audit; no production app boot/install or mutation. `.minecraft` untouched. No fake public releases, `git clean`, force operations, broad cleanup, reset, deletion, merge, push or publication. Every name-status/deletion diff reviewed: **zero tracked deletions**. All 14,944 protected pre-existing untracked files match baseline size/SHA-256, all 366 original tracked paths still exist, critical metadata/manifests/lockfiles/build configuration present. Previous diagnostics/evidence remain byte-identical. `Cargo.toml` and lockfiles unchanged. Local main and origin/main unchanged. Generated builds/game/runtime data and diagnostic keys are not staged. Safety proof: `safety-final.json`.

## Commits

- `ac26fd2bb54532ce8451266a40061ebdf864fa30` — Simplify Aurora production update authority to one release stream.
- `96f2a12320f9dde113c95d34e0c4616917b0751e` — Unify Aurora update UX with one sequential action and local reset.
- The documentation/acceptance commit containing this report is recorded by exact SHA/subject in `final-git.json` and the delivery message.

## Remaining limitations

Production self-updates remain honestly unconfigured until owner signing/publication exists. Client manifest discovery is HTTPS bootstrap trust rather than independent manifest signing. Windows real installer behavior is verified here; other OS installer acceptance was not performed. Existing process-local locks do not provide cross-process exclusion or a durable crash journal. Ordinary third-party content release policy and unrelated deferred features remain unchanged.

## Final status

```text
PHASE L SIMPLIFIED UPDATE CORRECTION COMPLETE
OWNER REVIEW REQUIRED
DO NOT INTEGRATE YET
NOTHING PUSHED
READY FOR FINAL PHASE L REVIEW
```
