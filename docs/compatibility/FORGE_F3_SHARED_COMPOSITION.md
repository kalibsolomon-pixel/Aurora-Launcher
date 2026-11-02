# F3 — Shared loader composition and exhaustive dispatch

F3 report, 2026-10-11 (America/New_York). Branch
`codex/client-3-production-authority`, starting HEAD `91d6852` (the F1
final state), final HEAD after this phase's commits (see §C). This phase
implements the roadmap's F3 ("Finish shared game composition and exact
capabilities"); this brief's F3 identifiers map one-to-one onto the F0
roadmap's F3 — no relabeling was needed.

**F3 verdict: COMPLETE.** The shared composition boundary exists in
`game::plan`, all three implemented families compose through it with
verified behavioral equivalence, loader dispatch is exhaustive at every
selection point, F1A processor safeguards are untouched and re-verified,
no schema changed, and native desktop acceptance installed and validated
real Vanilla, Fabric+Aurora, and NeoForge instances through the new
boundary in an isolated managed root. No Forge/Quilt implementation, no
CurseForge, no pack-schema work — as instructed.

## A. Baseline

- Starting state: branch `codex/client-3-production-authority` at
  `91d6852`; tracked tree clean; F1A verified present at HEAD
  (`install/processors.rs`, `install/receipt.rs` from `d075566`).
- **F0 documentation checkpoint:** the four F0 documents remained
  untracked and byte-identical to the reviewed deliverables (line counts
  unchanged: 335/418/289/125). Per the brief they were committed as a
  documentation-only checkpoint (`9edd299`) before implementation,
  with no other untracked file included.
- Existing loader architecture (traced before editing):
  - **Minecraft base planning**: pinned piston-meta manifest → SHA-1-
    verified version document → `minecraft::metadata` DTO boundary →
    pure `minecraft::rules` → `MinecraftInstallPlan` (client, logging,
    libraries, asset index, natives, Java requirement, launch templates;
    inheritance and historical shapes deliberately rejected).
  - **Loader planning**: Vanilla = `GameInstallPlan::vanilla` (no loader
    half); Fabric = `fabric::resolve_game_plan` (Fabric Meta →
    `FabricPlan` → `compose_game_plan`); NeoForge =
    `neoforge::resolve_neoforge_game_plan` (verified installer →
    `NeoForgePlan` → `compose_neoforge_game_plan`). Aurora Client is not
    a loader: an exact Fabric-only artifact tail through the aurora
    layer, gated by `platform::required_content`.
  - **The F0-notified defect confirmed**: every shared composition type
    (`GameInstallPlan`, `LoaderPlan`, `GameLibrary`,
    `GameJavaRequirement`, `CompositionError`, the merge policy) lived in
    `fabric/plan.rs`, so the NeoForge adapter and every consumer imported
    its foundation from the Fabric module.
  - **Dispatch points**: `lifecycle::resolve_record_game_plan` (Vanilla →
    NeoForge → *else-Fabric*), `resolve_packed_game_plan` (Vanilla →
    *else-Fabric*), `require_executable_configuration` (explicit refusal
    list), `settings::validate` pinned-version arm (`_` → Fabric
    validator), `mod_compatibility::satisfies_for_family`
    (`if neoForge → Maven else → Fabric predicates` — an unknown family
    string would silently evaluate under Fabric rules; callers gated to
    fabric/neoForge, but the fallback shape was the hazard).
  - **Launch**: already composition-clean — `LaunchPlan::from_game_plan`
    consumes only composed views (platform pin, main class, classpath
    entries by role, effective argument groups); no loader branching.
  - **Installed state**: typed platform in installed-game schema 2;
    validation read-only; F1A receipts staging-local.

## B. Architecture

- **Shared composition model** — `game::plan` (new): owns
  `GameInstallPlan` (explicit containment: unchanged vanilla plan +
  `Option<LoaderPlan>` + derived views), `LoaderPlan` (closed enum,
  Fabric/NeoForge variants, **no fallback variant by design**),
  `GameLibrary` + `LibraryProvenance` (per-family honest artifact
  expectations), `GameJavaRequirement` (`without_loader_floor` for
  NeoForge; new `with_loader_floor` constructor for the shared
  max/Mojang-component derivation), `CompositionError` (now names the
  disagreeing `loader_family` instead of Fabric vocabulary), and the
  identity/merge policy (exact duplicates collapse Mojang-first;
  same-identity-different-version is a hard conflict; classifier
  coexistence preserved).
- **Loader adapters** — family composers stay home: Fabric's
  `compose_game_plan` (floor-raising derivation, Fabric entry point) and
  NeoForge's `compose_neoforge_game_plan` (Mojang-authoritative Java,
  NeoForge entry point) both assemble through
  `GameInstallPlan::from_composed_parts`. Metadata parsing, version
  conventions, and artifact expectations remain per-family. Vanilla has
  no adapter beyond the shared `vanilla` constructor — a complete plan,
  not a degenerate loader case.
- **Capability representation** — unchanged `PlatformCapability` catalog
  (vanilla/fabric/neoForge, all executable, Aurora Fabric-only): the
  smallest honest representation today. Future families are absent from
  the catalog (never falsely advertised) and refused by dispatch with
  family-naming errors; per F0, generation-level capability decisions
  belong to the adapters when they exist, not to a speculative enum now.
- **Exhaustive dispatch** — `match` over every `PlatformPin` variant in
  `resolve_record_game_plan` and `resolve_packed_game_plan` (Forge/Quilt
  → explicit "not implemented in this build" / pack-scope refusals);
  `require_executable_configuration` matches every `LoaderKind` and names
  the refused kind; `satisfies_for_family` maps exactly `fabric`/`neoForge`
  and errors on any other family (no silent Fabric evaluation; the
  `fabricloader` builtin also binds only to Fabric). Rust's exhaustiveness
  now enforces that adding a family variant touches every dispatch site.
- **Artifact ownership** — unchanged: all acquisition flows through the
  verified stores; the merge policy preserves provenance; deduplication
  collapses only exact identity+version matches.
- **Java runtime composition** — unchanged policy, cleaner types: the
  composed plan's `GameJavaRequirement` remains the single authority
  (Mojang component + effective major; loader floor recorded;
  `raised_by_loader` observable). Processor execution uses the same
  managed runtime the plan requires, ensured before install (existing
  lifecycle behavior).
- **Launch specification** — untouched code path; consumes the same
  composed views (`launch::resolve` tests all pass).
- **Installed-state verification** — untouched; manifests schema 2;
  F1A receipts unchanged and re-verified by the F1A test suites.
- **F1A processor integration** — the NeoForge processor pass runs
  exactly as hardened in F1 (same `run_client_processors`, same
  supervision, receipts at every stage); F3 changed none of its code and
  re-ran its full test suite.

## C. Implementation

Commits (local, not pushed):
- `9edd299` — F0 documentation checkpoint (4 files).
- `45f1950` — shared composition move + exhaustive dispatch
  (8 files, +982/−571): new `game/{mod,plan}.rs`;
  `fabric/plan.rs` slimmed to the Fabric adapter with reexports of the
  shared types under their historical paths (every existing
  `crate::fabric::plan::X` reference compiles unchanged — verified by
  compiling without touching any consumer); `neoforge/plan.rs` imports
  the shared boundary directly; `instances/lifecycle.rs`,
  `instances/settings.rs`, `mod_compatibility.rs` dispatch changes; plus
  the final documentation commit containing this report and the
  ARCHITECTURE.md alignment.

Important types/interfaces: as in §B. Migration sequence followed the
brief's steps: A (shared types + tests: 4 new `game::plan` tests) → B/C/D
(Vanilla/Fabric/NeoForge equivalence via their existing suites: 12
fabric-plan, 5 neoforge-plan, 34 launch, 37 install tests) → E
(exhaustive dispatch + refusal tests: 2 lifecycle, 1 compatibility,
1 settings) → F (readiness alignment — verified no code change needed;
launch/validation already consume composed views) → G (full suites,
production build, native acceptance).

Behavioral equivalence findings: all pre-existing composition, install,
launch, and lifecycle tests pass unmodified. Two deliberate, message-only
differences: `CompositionError::VersionMismatch` now names the family
("the NeoForge plan describes…" instead of always "the Fabric plan"), and
`require_executable_configuration` names the refused kind ("forge has no
installation/launch implementation…"). Neither crosses the command
boundary as a stable code (verified: no error-code mapping referenced the
message text).

Deliberately preserved loader differences: Fabric's Java floor and
intermediary/common/loader/client library order; NeoForge's absent Java
floor, installer/universal/processor artifacts, JVM/game argument
contributions, and library-directory discoveries excluded from the
classpath; Vanilla's loader-less completeness; Aurora's Fabric-exclusive
exact-digest tail. Unimplemented families (Forge/Quilt) keep
**persistable** desired configurations (shape-validated version strings
only) so no existing record can become unloadable — the executable gate
in the lifecycle is the single refusal authority. An earlier draft of
this change rejected those configurations in `settings::validate`; the
registry round-trip test caught that it would break loading persisted
future-family records, and the rejection was moved back to the lifecycle
gate where it belongs.

## D. Verification

Commands and results (this host, Windows x64):
- `cargo fmt --all -- --check` — PASS.
- `cargo check --all-targets` — PASS (only the four pre-existing warnings
  in untouched files).
- `cargo test` — PASS: **898 lib tests (0 failed, 29 pre-existing
  ignores)** + 6 icon-asset tests. The 8 new F3 tests: shared-model
  Vanilla invariants, direct merge semantics (order/duplicate), family-
  naming composition errors; Forge/Quilt plan-resolution refusal;
  NeoForge/Forge/Quilt pack-transition scope refusal; unknown-family
  compatibility-evaluator errors; unimplemented-family configuration
  persistence contract.
- F1A regression: the full processor/receipt/recovery suites ran within
  the 898 (23 processor, 9 supervision, 7 receipt, 37 install incl. the
  six crash-recovery tests) — all pass.
- `npm run check` — 0 errors / 0 warnings. `npm test` not applicable
  (no frontend change; navigation unit tests unchanged).
- `npm run tauri build` — PASS (release exe + MSI + NSIS; review
  artifacts only, nothing installed).
- Fixture comparisons: the existing serialized-shape tests (registry
  schema 2–5 chains, platform pin round-trips including
  represented-only families, installed-game manifests) all pass — no
  persisted representation changed.
- **Native desktop acceptance** — see
  `.zcode-diag/phase-f3-shared-composition/evidence-index.md` (+4
  screenshots): fresh isolated root, real official endpoints. Fabric
  0.19.5 + Aurora 3.0.0 on 1.21.11 → Ready with exact production
  digests; Vanilla 1.21.11 → Ready (loader combobox honestly disappears);
  NeoForge 26.2 → auto-resolved 26.2.0.90 → Ready with the real processor
  executed under F1A safeguards (37,321,390-byte patched client,
  `locallyGenerated` trust); deep validation passed; no staging/receipts
  left; registry schema 5 unchanged. **Limitation:** authenticated
  gameplay launch was not performed (owner-gated Microsoft sign-in);
  launch-spec assembly is covered by the deterministic `launch::` suite,
  and readiness on all three instances reports sign-in as the only
  blocker.
- Performance: second-instance creation ~90 s (dominated by
  materialization; acquisitions were cache hits), NeoForge ~45 s
  including the real processor; full lib suite runtime unchanged
  (~55 s). No new metadata fetches or verification passes were
  introduced — composition is in-memory type assembly.

## E. Preservation

- Production Launcher 1.4.2 untouched: not replaced, not launched by me,
  no update-authority or signing change; the release build produced
  during verification was not installed or published. (The owner was
  actively using the production launcher during acceptance; every
  interaction of this phase targeted the isolated debug process only.)
- Owner data: real managed data, instances, `.minecraft`, caches, and
  authentication state never accessed; all acceptance work under
  `AURORA_DIAGNOSTIC_DATA_ROOT` in a temp directory.
- Developer-local files preserved; F1 evidence
  (`.zcode-diag/phase-f1-processor-hardening/`) untouched; no `git clean`
  or broad cleanup; `git diff --name-status` reviewed before every commit
  with zero deletions.
- No push, no publish, no release mutation. Final git status: tracked
  tree clean after the documentation commit; untracked inventory
  unchanged apart from the new F3 evidence directory.

## F. Next phase (F1B readiness)

What F1B (Forge 26.2) can now reuse directly:
- the shared composition boundary (`game::plan`) — a Forge adapter adds a
  `LoaderPlan::Forge` variant and its composer, and the compiler forces
  every dispatch site to acknowledge it (no silent Fabric fallthrough is
  possible by construction);
- the merge/collision policy with per-family provenance;
- `GameJavaRequirement`'s floor representation for a Forge toolchain with
  its own Java constraint;
- the entire F1A processor execution path unchanged, including receipts,
  private installer copies, and output verification (Forge's
  multi-step mapping pipeline will exercise `ProcessorStep` chaining,
  which F1A modeled and F3 did not weaken);
- the exhaustive catalog/gate pattern for introducing Forge as an
  executable family behind explicit capability decisions.

Remaining architectural blockers for F1B (unchanged from F0/F1):
1. **Owner gate**: OS-enforced processor isolation and official-tool
   trust policy (F1's documented same-user limitation stands).
2. Forge generation specifics: spec-1 bootstrap profile parsing, embedded
   resources, `@zip`/`@txt` data kinds, ordered multi-processor chains
   with intermediate outputs, tool-only library roles, and per-step
   output contracts.
3. F3R items deliberately deferred: cross-process locks, durable
   replacement journals, installed-game schema 3.
4. Non-Linux Unix process-group parity (only relevant if that platform is
   ever claimed).

**Not started:** F1B, F2, or any Forge/Quilt/CurseForge implementation.
Stopping for owner review as instructed.
