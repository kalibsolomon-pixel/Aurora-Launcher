# RR1-A — Native startup configuration closure

Acceptance date: October 14, 2026. **PASS. RR1-01 is closed.** RR1-B publication
reliability can begin as a separately scoped phase; it was not started here.
This verdict is native startup closure, not next-installer upgrade acceptance or
overall publication readiness.

## Repository and correction

- Branch: `codex/client-3-production-authority`.
- Starting HEAD: `57a3ef316e595a06e669a56c10400820f34cb1da`.
- Implementation commit: `020ff838f6a448452a96fbc391ed6c5842478c2f`.
- Initial inventory: 900 tracked paths, four nonignored untracked files, seven
  worktrees, no tracked edits. All original paths remain present.
- Implementation changes: `src-tauri/tauri.conf.json` (+5 lines) and
  `src-tauri/src/updates/launcher.rs` (+27 test lines). No deletions or renames.
  This acceptance report is a separate documentation addition.
- All four original untracked files retain their SHA-256 values. Other worktree
  paths, HEADs and refs are unchanged. No cleanup, reset, merge or cherry-pick.

Inspected the entire correction `e4c25cb7dc02e00ca433c7df5b759cef62730d48`, its
parent `420f54c0ad48ec9aa70320786a77d678b51864f1`, the historical native boot
report, and RR1's startup/packaging findings. The current lockfile still resolves
Tauri 2.12.1 and updater 2.13.1, matching that correction's dependencies.

Native initialization unconditionally registers the updater. Its configuration
must deserialize before application setup/window creation; absent
`plugins.updater` becomes JSON null, and the locked plugin requires a `pubkey`
string. Historical release `panic = "abort"` caused `0xc0000409`. RR1 inspected
the defect but did not rerun the failing executable; this phase also avoids
deliberately launching a broken build.

Ported the original configuration object and positive regression test unchanged.
Added a deterministic negative fixture that removes only the in-memory updater
entry and confirms the real plugin deserializer rejects null. The positive test
fails if the actual bundled configuration is absent or malformed. The current
CSP is preserved. Historical acceptance documentation was not imported as new
evidence; this report records only this phase's execution.

## Updater trust and production packaging

The empty baseline key supplies structure, not trust. Plain builds have no JSON
endpoint or key authority and remain honestly unconfigured for updates. The Rust
wrapper still requires compiled `AURORA_UPDATER_PUBKEY` and the fixed
`PRODUCTION_MANIFEST_URL`. Production implementation, signature verification,
exact artifact/redirect checks, TLS/certificate/hostname policy, no-downgrade
defaults, capability restrictions and manual publication gates are unchanged.
The existing signed-version compatibility policy is unchanged.

The production workflow still generates `plugins.updater.pubkey` from the
owner-provided key, compiles that same trimmed key, requires signing material,
and enables updater artifact generation. Locked Tauri codegen uses JSON Merge
Patch for the additional configuration: the injected key replaces the empty
baseline string. There is no additional endpoint or competing authority.
The 83 offline release-tool tests include signing-prerequisite refusal and
manual-workflow boundaries. No real signing material was read; no production
workflow or live installation transaction was invoked.

## Executed automated verification

All commands below returned **exit 0**, from the starting HEAD plus the exact
implementation patch subsequently committed as `020ff838`. No implementation
edits followed these tests/builds. External evidence records source file hashes.

| Command | Result |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml updates::launcher::tests -- --nocapture` | 12 passed; includes both configuration tests and existing native updater boundary tests |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Passed |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | Passed |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 982 unit tests passed, 31 ignored; six icon integration tests passed; zero failures |
| `npm run check` | Zero errors/warnings |
| `node --test tools/release/updater-release.test.mjs` | 83 passed, zero failures |
| `npm run tauri build -- --debug --no-bundle --config <evidence>/isolation.json` | Debug native build and production static frontend build passed |
| `npm run tauri build -- --no-bundle --config <evidence>/isolation.json` | Optimized native build and production static frontend build passed |
| `git diff --check` and staged diff checks | Passed |

The repository guide requires broader format/check/test/build/boot verification
for startup changes. No opt-in F3R-B campaign was repeated. Four existing Rust
test warnings and the existing STATIC_VCRUNTIME deprecation remain in the logs.
Historical RR1/B2a results (980 Rust passes, 31 ignored, six integration tests per
profile, 272 frontend tests) are historical, not newly executed results here.
No new complete frontend-test-suite or release-profile Rust-test result is claimed.

## Actual native Windows acceptance

Both builds embed the normal static frontend and use no research/desktop-fault
features. An external override changes only application identifier, product name
and window settings; it supplies no updater object, endpoint, key or security
override. Neither build was installed or bundled into an installer.

The unique application identifier is `com.aurora.launcher.rr1a20261014`. Debug
managed data uses the existing debug diagnostic override into the evidence root.
Release ignores that override and uses the platform-resolved data directory for
the unique identifier, confirmed by its native backend log. Initial account and
instance state are empty. No owner account, credential, instance or runtime was
copied. WebView data is also separated by the unique identifier.

| Observation | Debug | Optimized release |
| --- | --- | --- |
| Native build | Exit 0 | Exit 0 |
| Retained process PID | 26300 | 25736 |
| Native main window handle | 1640084 | 27330360 |
| Window title | Aurora RR1-A Acceptance | Aurora RR1-A Acceptance |
| Process/window responsiveness | True | True |
| Frontend | Home rendered; Settings and Updates navigation worked | Home rendered; Settings and Updates navigation worked |
| Native integration | Backend ready; isolated default config initialized | Backend ready; isolated default config initialized |
| Updater | Initialized before window creation; unconfigured without compiled key | Initialized before window creation; unconfigured without compiled key |
| Shutdown | Application Close button; exit 0 | Application Close button; exit 0 |
| Panic / `0xc0000409` | None | None |
| Unexpected update installation | None | None |

Acceptance combines retained process handles/exit codes, native window handles,
responsiveness, native IPC/backend logs, Windows accessibility document text and
real navigation. Screenshots supplement these observations; visual judgment is
not the sole evidence. The Updates page in each profile explicitly states
“Launcher updates are not configured in this build.” No Check/Update/Install
action was requested. Plugin initialization is established; a signed live update
installation is intentionally outside this phase.

Debug executable SHA-256:
`a77d2f263fc5cb3b90293a3fddaf7640b475fa39a6bf66ad132b3ef5f7b818ec`.
Release executable SHA-256:
`6470424a4d5f9106d8620a698e3386cabafa662b86adb8d6e4c7523f947373db`.

One premature release-launch invocation failed its executable-presence preflight
while the build was still running. It launched no process. The subsequent
completed build's retained process and exit observations are the release result.
The first debug screenshot was occluded; activating the exact target window
resolved it. Both diagnostics were investigated without changing application code.

## Production and owner preservation

The independently located installed production executable reports version 1.4.2.
Its SHA-256 before and after is identical:
`711df646d834588b404c7170db76b6546b409433aecfd882b026299412d13818`.
The uninstaller hash also matches. All nine pre-existing files in the owner's
launcher-state directory, including accounts/config/registry metadata, retain
their starting hashes. No authentication flow or credential-store operation was
invoked. Owner instance content, saves, mods, configuration and `.minecraft` were
not targeted; this is not an exhaustive rehash of all owner game data.

Only the retained acceptance processes were launched and normally closed. No
process-name termination, production launch/replacement, unrelated worktree
mutation, push, tag, release, workflow dispatch, publication or deployment occurred.

## Evidence and recommendation

External evidence directory basename: `Aurora-RR1-A-Evidence-20261014`.
The final response links its absolute location; machine-specific paths remain
outside tracked source. It contains baseline inventories/history, original
untracked/production/owner hashes, inspection and RR1 reference, exact patch,
test/build logs and exit summary, tested source hashes, both preserved binaries,
process startup/shutdown JSON, native output, Home/Updates screenshots and
accessibility text, preservation comparisons, final Git state and evidence hashes.

No required RR1-A acceptance remains incomplete. **RR1-A PASS — proceed to RR1-B
publication reliability.** Exact next signed package/isolated upgrade acceptance
and the other RR1 release prerequisites remain separate work.
