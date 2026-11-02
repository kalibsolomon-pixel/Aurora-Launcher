# F1 — Installer processor hardening and transaction recovery

F1 report, 2026-10-11 (America/New_York). Branch
`codex/client-3-production-authority`, implementation commit `d075566`
(from F0 baseline `9890a9c8ba2b0fb64508df60d9ea8fd860a21f64`), plus this
report and the ARCHITECTURE.md alignment in the closing documentation
commit.

**F1 verdict: COMPLETE** for the F1A scope defined by the F0 roadmap
(processor safety and output evidence on the existing NeoForge boundary,
no new loader generations, no schema migration of existing documents, no
release). Residual limitations are recorded honestly in section C — the
most important unchanged fact is that installer tools remain same-user
code without an OS-enforced filesystem/network sandbox; F1 makes their
execution *bounded, verifiable, recoverable, and diagnosable*, not
sandboxed.

## A. Baseline

- Branch `codex/client-3-production-authority`; starting HEAD
  `9890a9c` ("release: prepare gated recovery of accepted 1.4.2 draft"),
  identical to the F0 audit baseline.
- Pre-existing working tree: zero tracked modifications; nonignored
  untracked files (the four F0 documents, `.zcode-diag*`, `.zcode-perf*`,
  `.zcodeignore`, `.zcode-icon-diag.py`, one legacy diagnostic filename,
  `docs/__pycache__`) preserved byte-for-byte throughout. The four F0
  documents remain untracked and unstaged.
- Processor execution path at baseline (traced in full before editing):
  `instances/lifecycle.rs::install_instance_components` (ensures managed
  Java first) → `install::install_game` (per-instance mutex, staged tree,
  marker-last commit, one/two-rename promotion) →
  `neoforge::processors::run_client_processors` (token substitution, tool
  spawn via `tokio::time::timeout(600s, Command::output())`, names-only
  before/after snapshot, whole-file `fs::read` of generated outputs) →
  staged validation → manifest → promotion. The managed-Java diagnostic
  (`runtime/install.rs::run_process`) used the same unbounded
  `Command::output()` pattern. Game-launch supervision
  (`launch/process.rs`) is a separate boundary and was not modified.
- F0 findings addressed (audit §17): unbounded output capture; output
  bound applied only after collection; names-only snapshots (no in-place
  modification or link detection); published processor outputs discarded
  by the DTO boundary; unbounded manifest read; Unicode-panic diagnostic
  slicing; the verified-store installer path passed into tool processes;
  crash-recovery evidence gaps. F0 decisions applied as written; no
  conflict between this task and the F0 documents was found.

## B. Implementation

Files changed (commit `d075566`, 7 files, +2528/−195):

- **`src-tauri/src/install/processors.rs` (new)** — the shared bounded
  supervision boundary: concurrent stdout/stderr draining with a live
  per-stream retention cap (default 256 KiB, truncation recorded and
  rendered into diagnostics), wall-clock timeout and cooperative
  cancellation (`watch`-based signal pair) racing child exit, whole-tree
  termination on timeout/cancel/drop (Windows Job Object with
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` assigned at spawn; Linux process
  group via `process_group(0)` + group SIGKILL; other Unix builds kill
  the immediate child only, documented), bounded reader-grace shutdown,
  distinct `Exited/TimedOut/Cancelled/SpawnFailed` classification, and
  character-boundary-safe tail slicing.
- **`src-tauri/src/neoforge/processors.rs` (reworked)** — the processor
  pass now: materializes a **private, SHA-1-re-verified copy** of the
  installer inside scratch before any tool sees it (`{INSTALLER}` and
  installer-entry extraction use the copy; the shared verified store is
  never a tool argument or write surface); requires **regular-file**
  staged inputs (links/directories refused); hashes every consumed input
  (tool jar, classpath, client jar) before and after each step and fails
  on in-place modification; snapshots the staged tree with size +
  link/reparse status so destruction, in-place modification, and
  link-shaped outputs are all hard failures; stream-hashes generated
  outputs under a 2 GiB bound instead of whole-file reads; bounds the
  tool-manifest read before buffering; renders Unicode-safe bounded
  diagnostics with explicit truncation notes; and verifies **published
  output expectations** against their official SHA-1 when a profile
  supplies them (never fabricating digests). Cancellation is checked
  between steps and inside supervision; per-step timeout defaults to
  600 s and is test-overridable.
- **`src-tauri/src/neoforge/metadata.rs`** — `ProfileProcessor` gains
  `outputs: Vec<ProfileProcessorOutput>` (token + validated 40-hex
  SHA-1); the raw DTO previously discarded the upstream `outputs` map.
- **`src-tauri/src/neoforge/plan.rs`** — `NeoForgeProcessorStep` carries
  `ProcessorOutputRule`s; an expectation naming an undefined token is a
  planning-time hard error (new `NeoForgePlanError::Output`).
- **`src-tauri/src/install/receipt.rs` (new)** — schema-1 transaction
  receipts: written atomically inside `.install-staging/` at every stage
  transition (`prepared → acquired → processing → processed → verifying
  → verified → replacing → committing`), recording transaction id,
  instance id, platform, Minecraft version, processor tool identities,
  generated-output observations, and replacement reservation.
  `classify_existing` distinguishes `Interrupted` (known schema, names
  last stage) from `Unknown` (malformed or future schema — reported,
  never interpreted) from `NoReceipt`. A receipt grants no authority:
  staging disposal still requires the pre-existing derived-path proof.
- **`src-tauri/src/install/mod.rs`** — `install_game` classifies any
  existing receipt before staging disposal (with a one-line recovery
  diagnostic), begins a receipt before long work, records every stage,
  records the processor tools before the pass and the generated outputs
  after it, and journals `replacing`/`committing` inside
  `promote_staged_game` so the crash window between the two replacement
  renames is durably named. The transaction id is reused as the manifest
  installation id. `InstallContext` gains `processor_timeout` and a
  `processor_cancel` signal (defaults: 600 s, never cancelled).
  `InstallFaults` gains `fail_after_processing_receipt` and
  `fail_between_renames` crash-window injectors.
- **`src-tauri/src/runtime/install.rs`** — `run_process` (the managed
  Java `-version` diagnostic) now runs through the shared supervised
  runner: bounded capture, tree termination on timeout, and preserved
  exit classification. Timeout semantics unchanged (10 s).

### Behavior contracts preserved

No command DTO, error code, persisted schema, Tauri identifier, or
managed-data semantics changed. Installed-game manifests stay schema 2;
content/registry/pack schemas untouched (the F1 brief's smaller
"separate transaction receipt" option was chosen over installed-game
schema 3, which F3R owns). Vanilla and Fabric paths gain no processor
machinery: receipts are the only addition to their flow (a handful of
atomic ~1 KiB writes inside disposable staging). Processor retry
semantics are unchanged (processors always re-run; outputs re-derived).

## C. Security assessment

**Eliminated.** Unbounded memory growth from tool stdout/stderr
(including the managed-Java diagnostic); pipe-buffer deadlock potential
(readers drain concurrently for the process's whole life); the
Unicode-panic diagnostic path; unbounded manifest reads; whole-file
reads of generated outputs; silent discarding of published processor
output expectations; the shared verified store appearing in tool
argument vectors.

**Reduced.** In-place tampering with staged inputs by a tool (hashed
before/after per step); destruction of staged managed files (snapshot);
link/reparse-point outputs (refused, never followed); oversized
generated outputs (2 GiB bound); process-tree survival past
timeout/cancellation/crash (Job Object / process group — including the
real parent-death case, proven in native acceptance); the crash window
between replacement renames (durably journaled, deterministically
rebuilt); cache-object exposure to tool writes (private installer copy;
store objects are still re-hash-validated on every reuse).

**Still present (documented, not solved by F1).** Installer processors
are downloaded executable code running with the user's own authority:
`env_clear`, checked arguments, private paths, and snapshots bound and
verify effects — they do not prevent outside writes, network access, or
reads of user files. This is the F0 "same-user code" finding; an
OS-enforced boundary and explicit supplier-trust policy remain the
owner decision F0 recorded. Same-length in-place rewrites of
non-input staged files are detected only by the final staged
validation's per-trust re-hash, not by the snapshot (size-only
comparison); every staged file is still fully re-verified before
commit. Non-Linux Unix builds get immediate-child kill only (no group
kill without a libc dependency there).

**Deferred to later phases.** OS-level isolation and the
supported-host/trust gate (owner decision, prerequisite for F1B per the
roadmap); Forge's multi-step mapping pipelines, `@zip`/`@txt` data
kinds, and generation-specific output contracts (F1B/F1C); durable
cross-process locks and old-tree restoration journals (F3R); older
NeoForge generations (F2).

## D. Test evidence

All runs on this host (Windows x64, JDK present — no test skipped for
lack of one).

- **Unit/supervision (`install::processors`)** — 9 tests: bounded
  retention accounting; character-safe tails; default signals never
  fire; fired signals resolve immediately; spawn failure classified;
  successful exit with captured output; wall-clock timeout terminates a
  running child promptly; cancellation terminates a running child
  promptly; simultaneous stdout+stderr floods (≥256 KiB each, real
  subprocesses) retained under a 16 KiB cap with truncation markers.
- **Processor pass (`neoforge::processors`)** — 23 tests with the real
  compiled Java tool: success + recorded artifact; private installer
  copy exists, re-verifies, and the store object stays byte-identical;
  tampered store installer fails before execution; missing output;
  nonzero exit with diagnostics; invalid-UTF-8 output (bounded,
  non-panicking); dual-stream flood bounded; timeout kills the step and
  no late output appears; cancellation before and during execution
  (classified distinctly, no output); grandchild-spawning tool;
  unexpected side-effect files recorded; deterministic retry;
  destruction and in-place modification of managed files fail; symlink
  outputs rejected not followed; published-output verification (match
  and mismatch); token/path/unit-level checks (whole-argument tokens,
  manifest wrapping, escape rejection, mode coverage).
- **Receipts (`install::receipt`)** — 7 tests: stage persistence and
  `Interrupted` classification with last stage; missing → `NoReceipt`;
  malformed → `Unknown`; future schema → `Unknown`; no partial-write
  window; idempotent classification; closed stage vocabulary.
- **Transaction recovery (`install::tests`)** — 37 tests total,
  including six new: crash during materialization leaves an
  `acquired`-stage receipt, nothing validates as installed, retry
  completes and leaves no receipt; crash after verification records
  `verified` and retries; crash after the processing receipt (real
  processor path) leaves `processing` evidence and retries to a valid
  install; crash between the replacement renames leaves `replacing`
  evidence plus the retired tree and rebuilds with user sentinels
  intact; corrupt and future-schema receipts are reported and staging
  rebuilt safely; repeated interrupted attempts recover identically.
- **Full suites**: `cargo fmt --check` PASS; `cargo check --all-targets`
  PASS (only pre-existing warnings in untouched files); `cargo test`
  PASS — **890 passed, 0 failed, 29 ignored** (lib; all ignores
  pre-existing live-network/performance gates) + 6 icon-asset tests;
  `npm run check` 0 errors/0 warnings; `npm run tauri build` PASS
  (release exe + MSI + NSIS at 1.4.2 — review artifacts only, nothing
  installed).
- **Native desktop acceptance** — see
  `.zcode-diag/phase-f1-processor-hardening/evidence-index.md` and the
  six screenshots beside it. Isolated managed root; real official
  endpoints. Real NeoForge 26.2/26.2.0.90 install through the hardened
  processor path to Ready; hard `taskkill /F` at receipt stage
  `processing` during a reconfiguration install (26.2.0.88) left: no
  orphaned Java processes (Job-Object parent-death cleanup), honest
  `installing` registry state, the prior complete game tree intact, and
  a durable receipt naming the exact tool; the relaunched launcher
  detected the interruption, Retry logged the recovery line naming the
  transaction and stage, disposed staging after the path proof,
  rebuilt, and committed Ready with the regenerated patched client.
  Authenticated gameplay launch was not performed (owner-gated
  Microsoft sign-in; Phase K acceptance covers that path and launch
  assembly is unchanged by F1).
- **Performance** — processor-stage overhead is unchanged in kind (same
  one JVM spawn per step; added input hashing ≈ two streamed SHA-256
  passes over the tool jar + client jar per step, and one installer
  copy of a few MiB — the accepted real install completed in ~1 minute
  end-to-end including ~500 MB of downloads). Output capture memory is
  now hard-bounded (≤512 KiB retained across both streams regardless of
  flood size). Receipt overhead is one atomic ~1 KiB write per stage
  (≤8 per install) inside disposable staging. Full-tree snapshot walks
  were already O(files) and remain so (metadata only).

## E. Preservation

- Production Launcher 1.4.2 (installed copy, update authority, signing
  key, release assets): unchanged and untouched; the release build
  produced during verification was not installed or published.
- Owner's real managed data, instances, `.minecraft`, caches,
  diagnostics, and authentication state: never accessed; all acceptance
  work used `AURORA_DIAGNOSTIC_DATA_ROOT` in a temp directory.
- Developer-local untracked files: preserved; the four F0 documents
  remain untracked and were not committed.
- No unexpected deletions: `git diff --name-status` showed 5 modified +
  2 added files, all intentional; no deletions at any commit.
- No push, no publication, no release mutation. Final git status:
  tracked tree clean except this report, ARCHITECTURE.md, and the
  acceptance evidence index (untracked until the documentation commit).

## F. Follow-up (for the owner's F2 planning)

F1A's mechanics let later phases assume: a reusable bounded supervised
runner (`install::processors`) with cancellation and tree-kill; a
processor pass that verifies inputs and outputs and refuses links and
escapes; published-output-expectation plumbing end-to-end (metadata →
plan → verification); a staged transaction receipt with interrupted
classification; and crash-window fault injectors for every promotion
boundary.

Remaining blockers for Forge/NeoForge expansion, per F0/roadmap:
1. **Owner gate:** supported-host OS-enforced processor isolation and
   official-tool trust policy (prerequisite for F1B; the honest
   same-user limitation above stands until decided).
2. F3 shared composition (`game/plan.rs` move, exhaustive family
   dispatch) before F1B Forge 26.2.
3. Forge-generation specifics for F1B/F1C: multi-step processor chains,
   intermediate outputs consumed by later steps, `@zip`/`@txt` data
   kinds, tool-only library roles, per-step output contracts.
4. F3R for durable cross-process locks, old-tree restoration journals,
   and installed-game schema 3 (generated/native output proof and plan
   receipts) — F1's receipt deliberately stopped short of those.
5. Non-Linux Unix process-group cleanup parity (needs a small libc or
   equivalent for macOS if that platform is ever claimed).
