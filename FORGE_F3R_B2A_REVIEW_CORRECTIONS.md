# F3R-B2a review corrections and Windows symlink gate

Work date: 2026-10-14 (America/New_York). Starting branch
`codex/client-3-production-authority`, verified starting HEAD
`54e8a8f576bc5e98069ec230aa4b222f049de552`.

Implementation: all three review findings corrected. Overall security verdict:
**PARTIAL**. Real Windows
file-symlink acceptance remains **BLOCKED** by error 1314 in an observed
unelevated process. No elevated process was launched or requested. Linux and
F1C authenticated gameplay remain deferred.

## Historical claim corrections

The historical B1/B2 reports remain intact, with explicit correction notices:

- The previous PowerShell `finally` deleted an unchecked caller root. Its
  naming/marker claims did not establish safe recursive cleanup. The Rust
  verify branch could also construct an automatically deleted `World` from
  caller input before validating it. Those claims were incorrect.
- Printing administrator membership and describing verify as UNELEVATED did
  not enforce actual process-token elevation. The previous success label was
  stronger than its evidence.
- The previous installer identity tuple omitted extension and never compared
  normalized destinations, despite the report describing same-destination
  semantics. Different extensions could be dropped or falsely conflict.
- Prepare invokes Cargo, potentially compiling/linking repository code and
  executing build scripts, then initializes synthetic game transactions and
  creates links. It is not merely a privileged link-creation command.

## Fixture validation and cleanup

The wrapper and direct Rust helper **never automatically remove the gate root
or its links**, on success, Cargo failure, validation failure, link-creation
failure, assertion panic, or unwind. Evidence is deliberately retained. No
prefix or marker authorizes recursive deletion; there is no cleanup fallback
to an unchecked string. `World::new` alone registers ordinary test cleanup
authority after exclusive creation. Gate/desktop worlds have no such authority,
and panicking ordinary worlds are preserved too.

Prepare accepts only the exact absolute platform-temp direct child named
`aurora-f3rb1-symlink-<32 lowercase hexadecimal UUID characters>`, with a
canonical temp parent and no traversal. Exclusive `create_dir` refuses existing
files, directories and dangling redirects. It binds the root identity into the
exact marker contents and a strict schema-1 manifest. Old fixtures are not
silently adopted.

Verify performs read-only validation before retention: root identity and
non-reparse directory; regular marker/manifest and exact marker bytes; strict
manifest schema, identity, canonical transaction UUID and SHA-256 syntax; then
a bounded whole-tree inventory of directories, file hashes and link targets.
All reparse points are refused except the two exact intended file symlinks in
the recorded backup, targeting the in-fixture outside-world sentinel and its
absent sibling. World, history, slot and control ancestors cannot redirect.
The inventory must equal the preparation observation. Manifest inventory keys
never derive mutation paths; the one path-driving field, slot, uses the existing
`journal::safe_uuid` validator before joining.

Successful real verification must observe both links, including the dangling
target, repeat real retention twice with zero retirement/reclamation, compare
the whole inventory including sentinel and authority hashes, and deeply
validate the current game. It leaves both links and the fixture intact.
This is a disposable-fixture boundary, not an OS sandbox or protection against
a malicious same-user namespace race; the existing TOCTOU limitation remains.

Five focused Rust tests execute without link privileges:

1. Unrelated directory and pre-existing prefix root; prepare cannot overwrite.
2. Thirteen malformed/missing marker/manifest/schema/identity/digest/slot and
   unexpected-inventory variants, including traversal, drive and ADS slots.
3. Real junction redirection of root, world, marker and manifest; both fixture
   and outside target inventories and exact sentinel bytes remain unchanged.
4. Valid controls but missing real links; failed verify and injected assertion
   unwinding preserve every byte and directory entry.
5. Token decisions: elevated verify and detection failure reject; supported
   prepare/single-shot modes label their actual elevation. Actual token query
   on this host reports UNELEVATED. A direct release verify of the incomplete
blocked-prepare root also rejected before retention; all 41 entries and file
hashes remained unchanged.

The three PowerShell wrapper regressions mock Cargo exit 1 and empty successful
verify/prepare runs. Each rejects without a PASS claim and compares exact
sentinel bytes and directory inventory. They are wrapper tests, not symlink
acceptance. Negative-test targets were positively created disposable roots;
production and `.minecraft` were never test targets.

## Privilege and execution proof

PowerShell and Rust independently query the current process token with
`OpenProcessToken` and `GetTokenInformation(TokenElevation)`. Elevated verify
is rejected before fixture access or retention; detection errors fail explicitly.
No new dependency or Windows setting is introduced. A real elevated-process
rejection run is **unexecuted**, because elevation is not authorized; pure
decision tests cover that branch without impersonating security acceptance.

The actual real-link helper is now explicitly ignored in ordinary suites;
direct execution without its environment fails instead of returning early.
The driver uses `--exact --ignored --nocapture` and requires the explicit
PREPARED or complete VERIFIED evidence line as well as Cargo success. Empty
filtered runs cannot pass. PREPARED is never PASS; elevated single-shot VERIFIED
does not close ordinary-process acceptance; failed creation remains BLOCKED.

The executed direct debug helper and real release wrapper prepare both reported `MODE=UNELEVATED`, failed at actual
file-link creation with OS error 1314, exited 101 (0 passed, 1 failed), and
preserved its fixture. This expected privilege-blocked attempt is excluded from
passing regression counts; none of its retention security assertions executed.

## Exact installer destinations

The shared installer-tool classifier receives the already validated repository
path used by materialization. Same destination plus agreeing official SHA-1
deduplicates; disagreeing SHA-1 conflicts. Different destinations remain even
with equal hashes, coordinate identity or basename. URLs and sizes do not define
equivalence. Forge and NeoForge callers pass `library.path()`; acquisition trust,
processor inputs, launch-library merge policy, classpath order/provenance and
the independent Minecraft plan are unchanged. No digest is invented.

Six Forge regressions normalize modified copies of the official 1.20.4 profile
and invoke the actual composer: same path/different URLs; conflicting SHA-1;
different extensions with different hashes; different extensions with equal
hashes; different versions; unrelated groups sharing the basename. Four NeoForge
regressions normalize its metadata and invoke its actual composer for same-path
dedup/conflict, versions and basenames. NeoForge's current library normalizer
accepts JAR libraries only; this phase does not add extension syntax there.

The official fixture regression still proves one jopt-simple destination for
1.20.1 and one gson destination for 1.20.4, while preserving every non-overlapping
1.21.11 installer input. Checked-in fixture bytes and provenance are unchanged.
These official plans retain the B2 output, so another full desktop replacement
or crash matrix is unnecessary for this path-comparison correction. Production
sealing, transaction authority and retention eligibility were not changed.

## Final-source verification

Raw evidence is retained under
`%TEMP%/aurora-f3rb2a-evidence-29379ea0-220c-41aa-ad4b-eaba144a8551`.
Final broad checks and builds cover implementation HEAD `2fcf811`; later
changes are documentation/evidence only. The focused safety tests first ran
during implementation and all five reran in both final full Rust profiles.

| Command / check | Exit and result |
| --- | --- |
| `npm run check` | 0; 0 errors, 0 warnings |
| `npm run build` | 0; static production build |
| `node --test --test-concurrency=1 'src/lib/**/*.test.ts'` (first run) | 1; 269 passed, 3 failed, 0 skipped; one Vite 60-second module-load timeout in appearance setup |
| `node --test src/lib/launcher/appearanceCustomAccent.test.ts` | 0; all 3 affected tests passed |
| `node --test --test-concurrency=1 'src/lib/**/*.test.ts'` (final full rerun after frontend builds settled) | 0; 272 passed, 0 failed, 0 skipped; 197.688 s; clean full-suite final-source result |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | 0 |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | 0 |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 0; 980 library tests passed, 0 failed, 31 ignored; 6 integration tests passed; main/doc tests 0 |
| `cargo test --manifest-path src-tauri/Cargo.toml --release` | 0; 980 library tests passed, 0 failed, 31 ignored; 6 integration tests passed; main/doc tests 0 |
| `npm run tauri build -- --no-bundle` | 0; normal production executable built; Rust release phase 6m47s; no bundle/installer run |
| `cargo build --manifest-path src-tauri/Cargo.toml --bin aurora-launcher --features f3ra-desktop-acceptance,tauri/custom-protocol` (diagnostic config merge) | 0; copied final-source executable booted successfully; PID 3176, then normal close |
| `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File docs/forge-f3rb1/test-symlink-driver.ps1` | 0; 3 passed, 0 failed; child rejection exits 1 |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib install::retention::tests::symlink_gate -- --nocapture` | 0; 5 passed, 0 failed, 0 ignored; observed UNELEVATED |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib installer_` | 0; 21 passed, 0 failed, 1 ignored live-network check |
| `src-tauri/target/debug/deps/aurora_launcher_lib-2ccb8a72d05df568.exe --exact install::retention::tests::windows_file_symlink_refusal_helper --ignored --nocapture` (fresh root, single-shot) | 101; BLOCKED, 0 passed/1 failed; UNELEVATED, error 1314; no security acceptance |
| `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File docs/forge-f3rb1/run-symlink-gate.ps1 -Phase prepare` | 101; real release helper BLOCKED, 0 passed/1 failed; both process modes UNELEVATED; fixture preserved |
| `src-tauri/target/release/deps/aurora_launcher_lib-cf4d0f3945c7f66c.exe --exact install::retention::tests::windows_file_symlink_refusal_helper --ignored --nocapture` (verify of incomplete prepared root) | 101; expected missing-manifest rejection; UNELEVATED; all 41 inventory entries and file hashes unchanged; no retention acceptance |

Both full profiles include Forge compatibility 13/13, NeoForge planning 9/9,
shared planning 5/5, retention 26 passed/1 ignored (including five new safety
tests), journal/recovery 11/11 and coordination 4/4. The 31 ignored tests and
other opt-in helpers' inert ordinary-suite returns are not executed security
acceptance. The one broad frontend repeat was justified by the original
full-suite setup timeout; it ran after frontend builds settled and passed
cleanly. No further broad test battery was repeated. During wrapper-test
bring-up, nested-script exit propagation and native stderr handling were
corrected before the counted three-case run. The initial PowerShell test
invocation was refused
by script execution policy; the documented process-local Bypass invocation
passed without persisting a system policy change.

## Native acceptance and preservation

Final-source native boot passed, PID 3176, from the copied acceptance executable
`%TEMP%/aurora-f3ra-F3RB-B2a-ad131484e7bc43caacbae63da925e1f4/aurora-f3rb2a-boot.exe`
(SHA-256 `782fee2ec0e223c66d1bc67a7a467cc887c3219ed2987b4bfbea901bf126e754`).
The backend logged Ready, rendered Home with "No instances yet" and "Not signed
in", and created only default configuration and its lock under the isolated
managed root. WebView storage used the sibling isolated `webview` directory.
Screenshot and logs are retained in that root's `evidence/`. Only the selected
acceptance window was closed normally with Alt+F4; PID 3176 exited. No owner
process was terminated by this phase.

The diagnostic build used the existing `f3ra-desktop-acceptance` and
`tauri/custom-protocol` features, plus a temporary `TAURI_CONFIG` merge providing
an empty updater key and a distinctive window title. This is the established
isolated boot facility; it avoids the known unconfigured diagnostic plugin
startup failure. Repository configuration, updater authority and identifier
were not modified. An initial ordinary diagnostic build also compiled (32.08 s)
but was not booted; the configuration-merged build compiled in 12.12 s.
The production no-bundle build uses normal repository configuration separately.

A boot proves only startup and the rendered native shell. This phase does not
claim desktop
replacement/sealing, protected-backup classification, backup reclamation or
authenticated gameplay from boot or planning tests. Automated retention and
recovery fixtures exercise their real engines, separately from the still-blocked
real-file-link gate. Historical B2 desktop sealing evidence remains historical.

The baseline records branch/HEAD/history/status, all 896 tracked paths, four
pre-existing nonignored untracked files and hashes, seven worktrees, critical
documentation/manifests/lockfiles/build configuration and relevant processes.
Precommit and closing comparisons: no missing tracked file, no changed pre-existing
untracked bytes, no deletion diff. The installed production executable
SHA-256 remained unchanged across verification and the isolated boot. No
generated artifacts, credentials, game
trees or runtime data are staged. The independent `codex/f3rb1-acceptance`
worktree stays unmerged at `4f2ad56`. Production 1.4.2, owner data, active owner
game processes and old evidence are outside mutation scope. Only acceptance-owned
process handles may be stopped. No push, tag, installer, publication or deployment.

Changed files (14 total; no deletions or renames):

- `docs/forge-f3rb1/run-symlink-gate.ps1`
- `docs/forge-f3rb1/test-symlink-driver.ps1` (new)
- `src-tauri/src/install/retention_tests.rs`
- `src-tauri/src/install/retention_symlink_gate.rs` (new, test-only)
- `src-tauri/src/game/plan.rs`
- `src-tauri/src/forge/plan.rs`
- `src-tauri/src/forge/compatibility_tests.rs`
- `src-tauri/src/neoforge/plan.rs`
- `ARCHITECTURE.md`
- `FORGE_F3R_B_BACKUP_RETENTION.md`
- `FORGE_F3R_B1_SECURITY_PERFORMANCE_ACCEPTANCE.md`
- `FORGE_F3R_B2_SECURITY_AND_FORGE_PLAN.md`
- `FORGE_F3R_B2A_REVIEW_CORRECTIONS.md` (new)
- `docs/forge-f3rb2a/evidence.json` (new, compact curated evidence)

Implementation commits:

- `db4e515` — fixture/driver ownership, preservation, token enforcement, tests.
- `2fcf811` — exact installer destinations and actual composition regressions.
- Final documentation/evidence commit — recorded in the final response.

## Corrected owner procedure — awaiting separate elevation authorization

Do not execute a privileged step on the authority of this report. First review
the committed driver/helper and repository build chain. Privileged prepare runs
Cargo/build scripts and initializes synthetic transactions as described above;
it is not limited to creating two links. No administrator credentials, Developer
Mode change, local policy change or elevation are authorized by this phase.

If later separately authorized, use the same checkout and account/temp directory.
If an elevated shell uses a different profile or TEMP, stop for owner review;
the helper deliberately refuses that cross-temp root. Do not copy the prepared
tree or relax validation to work around the boundary.

Procedure:

1. In the separately authorized process with file-symlink creation capability:
   `powershell -NoProfile -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1 -Phase prepare`
   Require PREPARED with real-link evidence, and retain the printed fresh root.
2. In an ordinary, unelevated PowerShell process:
   `powershell -NoProfile -ExecutionPolicy Bypass -File docs\forge-f3rb1\run-symlink-gate.ps1 -Phase verify -Root '<printed-root>'`
   Rust and the wrapper both enforce the process-token requirement. Keep the
   full transcript and preserved fixture; no automatic deletion occurs.
3. Count acceptance only with UNELEVATED mode plus the exact VERIFIED line,
   both real links, two refusal passes, no retired/reclaimed content, unchanged
   inventory/sentinel hashes/authority and deeply valid current game. PREPARED,
   BLOCKED, mocked tokens, junctions or Cargo success alone do not close the gate.

The smallest next action is separate owner review/authorization for that
privileged prepare scope, followed by ordinary verify. Only after successful
evidence closes F3R-B should the owner begin the separate F1C authenticated
1.20.1/1.20.4 gameplay acceptance. This phase does not begin F1C or merge the
parallel acceptance branch.
