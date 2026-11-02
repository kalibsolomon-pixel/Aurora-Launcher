# Pre-release mod compatibility acceptance — 2026-09-26/27

This pass is local only. No publishing, pushing, rewriting history or website work is authorized. Prior acceptance reports remain unchanged. This report distinguishes deterministic/native acceptance from production UI acceptance and does not guarantee arbitrary mod compatibility.

## Starting repository and evidence

Starting HEAD: `bb091521f76c69d2126a1f808c98acc16510dc66`. Fetched `origin/main` and merge-base: `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`; ahead 25, behind 0. The actual baseline was **177 tracked files**, rather than the historical 161 count, and **607 protected untracked diagnostics** (606 `.zcode*` files and one retained acceptance text file). All were hashed before implementation.

Session evidence is outside Git at `C:/Users/kalib/AppData/Local/Temp/aurora-compatibility-74704f3e-179d-4fa8-8db8-25fbd62f4ba9`. It contains original tracked/untracked inventories and diagnostic SHA-256s, test/build outputs, official provider metadata, immutable archive-inspection copies, primary mod/registry hash baselines, and a disposable managed-root copy. No accounts or refresh credentials were copied to that test root.

## Proven root causes

| Fixture | Evidence and outcome |
| --- | --- |
| Mod Menu / Placeholder API | Mod Menu 17.0.1 (`mOgUt4GM`, version `cLkYaVuL`) lists unpinned required provider projects Fabric API and Text Placeholder API. Its root Fabric metadata requires Fabric API modules, not a root Placeholder API predicate. It declares a nested Placeholder API 2.8.2+1.21.10 JAR identical to the published dependency. The old path planned another dependency and raised two root/nested collisions against the local UserManaged file. Corrected reconciliation satisfies the provider dependency with the local exact published artifact; equal root/nested versions no longer create a collision. |
| Iris / Sodium | Iris 1.10.7+1.21.11-fabric (`YL57xq9U`, `fDpuVzVr`) pins Modrinth Sodium version `UddlN6L4`, display mc1.21.11-0.8.7-fabric. Installed Sodium is 0.8.14+mc1.21.11 (`rkdTcxoT`). Iris's verified Fabric predicate is Sodium `0.8.x`, which alone accepts 0.8.14; the provider pin is stricter. Independently, installed Sodium's verified metadata declares `breaks iris <=1.10.7`. This is a genuine incompatible pair, not the Mod Menu false duplicate. No downgrade, replacement or forced Iris install occurred. |
| Java 25 | The exact retained Remove Resource Loading Bar 1.0.1-1.21.x-fabric JAR (26,952 bytes, SHA-256 `db9446e3956ac7d7527e470bc8fae89fd48f36605ce792165401135465f18e7c`) says Fabric `java >=21`, but both declared common/client Mixin files say `JAVA_25`. Native bounded inspection produces two precise config/mod/managed-Java-21 blockers. It was never put in the primary instance or executed. |
| Chloride / Cubes | Verified Chloride 1.8.1 (`rViW2Ciw`) requires Sodium >=0.8.13 and breaks only `smoothskies`. Installed Cubes 3.0.0-build.14+mc1.21.11 declares no breaks/conflicts with Chloride. Neither inspected descriptor declares this pair conflict. The reported Sodium fullscreen override is a code/configuration interaction, proven diagnostically with a sanitized exception fixture, not a static blacklist or a fresh destructive crash. |

Provider fixture SHA-256s: Placeholder API `975c7e575c27bf6fa82baaf8cd7645051a298f74bf95cf58092b104dd21de5ed`; Mod Menu `2eb2d6db9964fcf2e44e2be1e4d172b8c4e829e385c8f31636de94a1708c2076`; Iris `58c55da18189c91a49f847d3cee451633a23b575fb69c0c5b65ddb274436cb19`; Chloride `75fc33fa191fe9c8a407c284c61a1848dabd59e04c552bdf4159ad4537e4c475`. Modrinth acquisitions used published SHA-512 plus expected size before entering Aurora's SHA-256 store. The Java-25 JAR is retained local diagnostic evidence, not newly claimed verified provider provenance.

## Resolver and lifecycle acceptance

The old resolver treated capability overlap as an ownership collision. The new reconciliation first separates ownership from active capability satisfaction, checks parent/transitive Fabric predicates, preserves exact provider pins and reuses compatible installed provider versions. Root/root collisions, unsafe names, malformed state, disabled dependencies, changed receipts and integrity failures remain blocked. Equal root/nested overlaps are allowed; differing root/nested versions remain conservative conflicts. Nested-only candidates must share one version satisfying incoming top-level constraints.

Default selection is bounded to 16 candidates, release-first then descending publication order, with MC/loader filtering followed by installed-environment requirements and known collisions. Explicit choices are never substituted. The launcher does not silently orchestrate dependency transitions or downgrade Sodium. Existing explicit transactional update remains the reviewed transition route.

The real-provider ignored acceptance test runs only against a temp-root copy. Before correction, `before-resolver.txt` records the Mod Menu collision and independent Iris blocker. After correction, `after-resolver.txt` records:

- local Placeholder API 2.8.2+1.21.10 as UserManaged, no provenance, and satisfied in preview;
- no dependency acquisition on exact local SHA-512 satisfaction, no additional top-level Placeholder API, and no parent collision;
- normal SHA-512/size verified Mod Menu activation through the native provider transaction;
- a fresh content-state/inventory reload with correct Mod Menu provider ownership and unchanged local bytes/ownership/provenance;
- normal dependency-aware Mod Menu removal, leaving Placeholder API byte-identical;
- a precise Iris required/installed Sodium error with no mutation.

Mod Menu's bundled Placeholder API remains available if the separate local file is removed; that particular file is therefore not an indispensable runtime dependency. A generic fixture whose active parent really requires the separate local capability proves local removal is blocked before parent removal, survives restart, and becomes allowed afterward. Provider schema remains 2: only real provider records become `requires` edges; local predicates are rebuilt from JAR metadata, never fake provenance. A changed update requirement is rejected without touching the local JAR. Inventory approval revisions are rechecked at final install/update activation under the existing lock.

## Deterministic coverage

| Required cases | Verification |
| --- | --- |
| Absent, disabled, compatible local/provider/bootstrap/nested dependencies; incompatible local/provider and unknown artifacts | `mod_compatibility::tests`, content reconciliation/receipt tests |
| Actual predicates, prerelease/build, wildcard, AND/OR, exact, caret/tilde, extended numeric and nonsemantic literals | `fabric::versions::tests::fabric_predicates` |
| Root/root, root/nested, nested/root, shared nested-only identities and conflicting incoming constraints | Existing nested inventory/content collision fixtures, real Mod Menu, `one_selected_version_must_satisfy_every_parent` |
| Optional recommends/suggests/conflicts and fatal breaks | `advisory_relations_do_not_block_but_breaks_do`, inventory recommendation warnings |
| Cycles/shared graphs, provider removal blockers, restart and rollback | Existing deterministic Modrinth/content lifecycle suites |
| Local survives parent removal, local removal blocker, restart reconstruction and changed update requirements | `local_dependency_satisfaction_install_reload_removal_and_changed_update` |
| Changed approved inventory, no activation | `reviewed_inventory_revision_excludes_external_mod_changes` |
| Java depends satisfied/unsatisfied, JAVA_21/25, multiple/server declarations, missing/malformed/oversized/traversal resources, undeclared JSON ignored and disabled JAR excluded | Mod compatibility and bounded Mixin archive fixtures |
| Gson line/block comments and URL/string preservation; malformed JSON still rejected | `mixin_comments_preserve_strings_and_do_not_mask_invalid_json` plus real More Culling copy |
| Arbitrary Sodium override source names, Java/Mixin/Fabric failure, unknown/incomplete logs, harmless Iris ClassNotFound warnings | `launch::diagnostics::tests` |
| Bounded initialization/fatal tail, Unicode, redaction, unsuccessful-only diagnostics, child supervision/exclusion | `launch::process::tests` and diagnostics tail fixture |

The first strict-JSON experiment identified More Culling's legitimate commented declared Mixin resources in the known-good copy. The parser was corrected with bounded lexical comment handling, then the entire copied active environment passed with **zero compatibility issues**. No mod-specific exception or safety bypass was added.

## Runtime and failure boundary

Provider reviews evaluate the projected mod set against the Java major from the official game plan. Readiness and both preparatory/final locked Play checks evaluate current local metadata. These added checks are local, synchronous, read-only and network/download-free. Existing official launch/runtime resolution retains its prior network behavior. No system Java, managed Java policy, account authority, mod code or class loading was changed.

The Mixin bounds are 64 declarations per archive and 256 KiB per resource, with existing nested archive depth/count/aggregate-byte limits. Only declared client/common resources are read. Unsafe paths, missing/duplicate/oversized/malformed metadata fail conservatively. Managed Java 21 is retained.

Failed supervised exits expose structured categories: mod configuration conflict, Java compatibility, Fabric dependency resolution, Mixin initialization and unknown crash. The Chloride/Cubes fixture reports both arbitrary IDs and `sodium:general.fullscreen`; Java-25 fixture reports requested JAVA_25 and actual managed Java 21. This layer never disables/removes/replaces mods. Successful exits have no failure diagnostic. Original bounded redacted exception output remains in the instance launch log, retaining initialization and fatal tail rather than only the first bytes.

## Automated verification

Final full Rust: **531 passed, 17 ignored, 0 failed** (525 library plus 6 integration); the live-provider ignored acceptance was additionally executed successfully. Frontend: **115 passed, 0 failed**. Svelte/TypeScript: **0 errors, 0 warnings**. Rust format, all-target check, production frontend, version contract and final Tauri/MSI/NSIS results are recorded in session evidence. Final `cargo fmt --check`, all-target Rust check, frontend production build, Tauri production build, MSI, NSIS, and version contract **1.0.0** all passed. Final artifact hashes are saved in `final-build-hashes.json`.

Authentication/account chooser/multiple accounts/dialog/profile/avatar/3D player, Home widgets/picker/schema 3, Discord Settings/nonfatal behavior, defaults, Vanilla/Fabric/Aurora lifecycle, ownership, disable/toggle/trash/removal/update, artwork, managed Java, synchronization and exact-child supervision retain their existing regression tests. Their unrelated implementations were not edited. This is automated coverage, not a claim that every account or visual control was clicked during this pass.

## Production UI and final launch

The final executable was launched explicitly from `C:/Dev/aurora-launcher/src-tauri/target/release/aurora-launcher.exe`, with its actual process path verified. An initial app-discovery launch selected an older installed executable that rejected schema 3; it was closed without changing configuration. Acceptance uses the freshly built repository executable, not that stale installed copy.

Production Browse loaded with artwork and the existing Installed/Keep controls. Mod Menu Details selected 17.0.1 and resolved without a false blocker. Because the primary instance no longer contained the reported local Placeholder API, a single session-created local fixture was briefly copied from the rehashed verified store using CreateNew. The actual preview showed **1 file will be installed**, Placeholder API **Already satisfied** by its real UserManaged filename, exact declared version, no ownership/byte change, and only Mod Menu queued. This is captured in `modmenu-satisfied-preview.png` and its text evidence.

A new provider-owned Mod Menu installation was observed during the interactive UI checks, with the expected SHA-256 and no owned Placeholder API edge. It was preserved as a new user-visible installation. The temporary local Placeholder API was removed only after proving the exact session-created path, regular-file status, containment and unchanged digest. A final audit confirms all **28 original mod files** and captured account/instance registry files remain byte-identical. Final state has **29 active JARs**, including Mod Menu; no separate Placeholder API JAR. Mod Menu's declared bundled module supplies Placeholder API. The requested keep/remove preference received no answer during this run, so no destructive assumption was made about the new installation.

Iris Details displayed its precise required Sodium 0.8.7 versus installed 0.8.14 relationship, not a generic Conflicts message. Iris was not installed and no Sodium transition or shader gameplay test is claimed. `iris-sodium-blocker.png` records the native error. Installed Mods retained visible toggle/trash controls and dependency removal blockers. Settings appearance/widgets and the Discord application-setup-required/nonfatal description were inspected without changing preferences.

The final executable was restarted after all native corrections. Its freshly reconstructed Home inventory showed 29 active mods and **Ready**, proving restart persistence rather than a stale cached count. Final launch was:

**Ready → Play → Starting → Running → Aurora main menu → normal Quit → exit 0 → Ready.**

`final-client-ready.txt` records Fabric Loader 0.19.5 starting Minecraft 1.21.11, Mod Menu 17.0.1, and `[Render thread/INFO]: Aurora Client ready.` Sodium 0.8.14 loaded its configuration (36 options, zero overrides); Sodium Extra likewise loaded (26 options, zero overrides). The custom main menu displayed Aurora Settings and normal Quit. Quit used the observed game button, never process-name termination. Native supervision wrote `Exit code: 0` in `C:/Users/kalib/AppData/Local/com.aurora.launcher/instances/8a59a5fe0a354c3fab444b38fc33d73f/logs/aurora-launch-1790482037-0.log`; Home displayed Last game exited with code 0 and Ready. Discord's unavailable application registration remained nonfatal.

Useful screenshots, all outside Git:

- `modmenu-satisfied-preview.png`: honest local dependency satisfaction and one-file install.
- `iris-sodium-blocker.png`: exact required/installed relationship.
- `installed-mod-controls.png`: existing toggle/trash layout.
- `settings-appearance.png` plus `settings-discord.txt`: unchanged Settings and nonfatal Discord explanation.
- `final-ready-restart.png`: reconstructed 29-mod Ready state.
- `final-starting.png`: Starting.
- `final-running-and-menu.png` / `final-aurora-main-menu.png`: Running and real Aurora menu.
- `final-exit-zero-ready.png`: normal exit 0 and Ready.

No unsafe Java-25 or Chloride fixture was ever activated or executed in the primary instance.

## Safety and deferred limits

Every commit received a saved full status/name-status audit, whitespace checks, all baseline files present, critical manifests/docs/locks present, no deletions or generated staging, and all 607 protected diagnostics untracked and byte-identical. The final tracked set is 181 (177 originals plus three native modules and this report). No baseline file is missing, no tracked deletion exists, and prior acceptance documents remain unchanged. Build/game/runtime/screenshot/evidence files are unstaged and outside the committed source changes.

New code commits: `a072b19` (capability/runtime/diagnostic model), `3c36714` (preview explanations), `83f5db3` (built-in breaks). The final documentation commit adds architecture and this acceptance report. Remote was fetched again and remained `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`; merge-base remains that commit. After the documentation commit the relationship is ahead 29, behind 0. The final response records the documentation commit's exact HEAD (a document cannot embed its own Git object ID). Nothing was pushed or published.

Static facts cannot prove arbitrary mod interactions, Sodium extension hooks, dynamic Mixin plugins, undeclared bytecode/runtime needs or shader/gameplay correctness. Full optional-nested dependency SAT selection, automatic dependency transitions, all Gson leniencies and cross-process/external-writer exclusion remain deferred. When a nonidentical older/nested local artifact needs project identity established, the verified candidate may first be acquired into cache; it is not activated if the local capability satisfies. These limitations are explicit rather than a claim of complete Fabric or arbitrary-mod compatibility.
