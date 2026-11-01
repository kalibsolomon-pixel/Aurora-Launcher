# Aurora Launcher — NATIVE-10 functional parity and preview acceptance

**Verdict: PARTIAL — FUNCTIONAL PARITY INCOMPLETE.** Three of the four required
features are implemented and pass live end-to-end acceptance: content updates,
exact loader versions and local `.mrpack` import. Managed-modpack updates were
added with them. The fourth, **custom Java executable selection, is not
implemented**: it requires an owner decision, because `AGENTS.md` forbids it
and the WebView host never offered it (see §5). The interactive native-window
pass was declined at the desktop-control prompt; layout and input were
verified headlessly on the real Slint components instead (§12). Nothing was
published or released.

Windows 11 x64, 2026-11-01. Branch `codex/native-ui-migration`, worktree
`C:\Dev\aurora-launcher-native`.

## 1–3. Repository identity and preservation

| Item | Value |
| --- | --- |
| Starting HEAD | `e6e74ee7a9ca05cb6ab96da170b1f17bfcea2614` (NATIVE-9 final), clean index and tree |
| Tracked files at start | 3,190 |
| Final commits | listed in §19 (local only) |
| Main checkout `C:\Dev\aurora-launcher` | HEAD `9b99db3`; its 10 pre-existing changed/untracked entries untouched |
| Production executable | SHA-256 `42c34fba…2387c`, unchanged |
| Real preview data/install | no install, uninstall key or preview data root created by this phase |

No tracked file was deleted or renamed. `git clean`, `cargo clean`, resets,
recursive cleanup and process-name termination were not used. Every process
ended during this work was one this session started, identified by exact PID
and command line. All test state lives in marked `aurora-native10-*` disposable
roots under `%TEMP%`, which were kept rather than deleted.

## 4. Mod and content update controls

**Native UI** (instance Mods / Resource packs / Shaders tabs): an explicit
**Check for updates**, then a per-row badge and the actions that state allows.

| Engine status | Row shows | Action |
| --- | --- | --- |
| update available, not pinned | `Update: <version>` | **Update** |
| pinned with a candidate | `Pinned · <version> available` | **Unpin** (then re-check) |
| up to date / none under channel policy | `Up to date` / `No update in its release channel` | — |
| blocked (pack-owned, disabled, locally modified, missing, conflict) | the specific reason | — |
| provider-managed dependency | `Updated with its parent` | — |
| version unknown / provider unavailable | warning / `Check failed` | — |

A summary line counts each state. **Update all (N)** covers only eligible
items. Local files without provider identity get no badge, and no numerically
higher version is ever treated as compatible.

**Engine:** unchanged shared `check_instance_updates`,
`preview/apply_modrinth_bulk_update` and `set_provider_update_policy`. The
preview resolves dependencies against the target versions and checks
compatibility before any mutation. It refuses pinned, dependency-only,
pack-owned, disabled, modified or missing files. Apply re-resolves and requires
the reviewed fingerprint. A preview that only replaces the chosen items is
queued directly. One that adds or removes dependencies, or carries warnings,
opens a review first. Jobs run through the existing Downloads queue
(per-instance serialization, cooperative cancellation, retry). Completion
reloads the tab and re-checks it if it was checked.

**Managed modpacks** (Settings → Installation → Modpack): **Check for update**,
**Review update** (old → new version, any Minecraft/loader transition,
added/updated/removed/kept counts, conflicts, new optional files, changelog),
then **Update modpack**. Conflicts can only be resolved by keeping local
copies, a non-destructive choice valid for every conflict kind, and an explicit
checkbox is required. A merged ordinary instance stays ordinary: its
pack-origin content updates as ordinary provider content, and it is never
converted to a managed pack.

| Evidence (live, disposable root) | Result |
| --- | --- |
| Check on a merged ordinary instance | 7 update available · 17 up to date · 1 none under policy |
| Apply one (Ixeris → 4.6.11+1.21.11-fabric) | one file replaced, nothing else changed |
| Re-apply the superseded review | refused (`provider_no_update`) |
| Re-check the updated item | `upToDate` |
| Managed pack (Vulkan Optimized) | all 25 records `packOwned`; none individually updatable |
| Managed pack update check / review | update available (1.21.11-4.1 → 26.1.2-1.1); review 26 added, 10 updated, 25 removed, game transition flagged |
| Unit tests | eligibility, summary counts, row actions, review-needed rule, pack review text |

Not exercised end-to-end: applying the managed-pack update (a large download),
and an interactive cancel/retry of an update job. Both use the unchanged
NATIVE-9 queue and engines.

## 5. Custom Java path — not implemented (owner decision required)

The brief asks to "restore the existing custom Java path control". **None ever
existed:** the WebView Settings row is fixed text, "Automatic — managed by
Aurora". `ARCHITECTURE.md` records custom executables as *deliberately
deferred*. `AGENTS.md` (repository rules) says "custom Java paths … remain out
of scope" and "never use … frontend-provided paths, system Java". Implementing
it changes the launch trust model, so it was not done unilaterally.

If approved, the proposed design is:

- An optional per-instance `java` field (managed | custom absolute path). This
  is a registry schema change: older launchers fail deliberately on it, so it
  needs a schema bump and a downgrade note.
- Validation in Rust by executing the selected file: absolute path, regular
  file, no reparse ancestor, executable, a bounded structured `-version` /
  `-XshowSettings:properties` probe, Java major equal to the game plan's
  requirement, and x64.
- Launch blocked while invalid; no silent fallback; one-click reset to managed.
- `launch::resolve` accepts the separately validated external executable;
  `AGENTS.md` updated to permit it.

## 6. Exact loader-version selection

**Creation:** a loader-version selector appears for non-vanilla loaders.
"Automatic compatible version (recommended)" comes first, then the official
catalog newest first, with unstable builds labelled. Selecting a version
creates the instance with that exact pin. Aurora compatibility is rechecked
against the pin.

**Existing instance** (Settings → Game): Minecraft version, loader and loader
version selectors. They edit the draft only; **Save** persists it. If the saved
configuration differs from the installed game, a **Saved settings not
installed → Install now** row appears (the same rule as the WebView host). A
saved pin missing from the catalog stays visible as "(saved)". Changing the
version or loader resets to automatic. With Aurora enabled, the loader kind is
locked to Fabric and "automatic" means the Aurora release's loader.

| Evidence (live) | Result |
| --- | --- |
| Fabric 1.21.11 catalog | 255 versions (newest 0.20.1) |
| Forge 1.20.1 / NeoForge 26.1.2 catalogs | 1 installable / 123 versions |
| NeoForge 1.21.1 | 0. Correct: only the 26.x generation is installable by this build; none invented |
| Unsupported Minecraft version | `fabric_combination_unsupported` |
| Pin saved | configuration changed; installed loader still 0.19.3 |
| Pin installed | 0.19.2 installed, validation `ready`, user mods unchanged |
| Pin changed 0.19.2 → 0.19.5 on another instance | installed and ready |
| Unsupported pin `0.0.1-not-a-loader` | install refused (`fabric_loader_not_found`); instance still ready on 0.19.2 |
| New instance created with pin 0.19.2 | `Minecraft 1.21.11 · Fabric 0.19.2` |
| Unit tests | persisted loader JSON shapes; option order; saved pin kept on catalog failure |

## 7. Local `.mrpack` import

**Instances → Import modpack** opens the owned native picker in a new
`.mrpack` mode (same modal, single-picker, owner-validated implementation as
NATIVE-9). Rust then identifies the archive:

1. Path checks shared with local content import (absolute, regular, no reparse
   ancestor, ≤ 512 MiB), then SHA-512 and SHA-256 computed in one pass.
2. The bytes are imported into the verified SHA-256 store, and the **store
   copy** is parsed by the existing `.mrpack` parser. Malformed ZIPs,
   manifests, traversal, unsafe paths and non-Fabric indexes fail here, before
   any network call.
3. Modrinth must publish this exact SHA-512 as the **primary pack archive** of
   a modpack version (matches on secondary files are refused).
4. A SHA-512→SHA-256 alias is recorded only after re-hashing the stored bytes.
   The unchanged hosted engine then resolves that exact identity, re-verifies
   the bytes against the published SHA-512 and uses them without
   re-downloading.

The existing Discover pack review then opens for that project with the exact
version preselected, plus a banner naming the local file and match. The rest
is the established workflow: optional files, **new isolated managed instance**
or **merge into an ordinary instance**, conflicts/blockers, queued install with
cancel and retry.

**An archive Modrinth does not publish is refused** (`local_pack_unrecognized`)
with an explanation. The engine needs real provider identity for components,
updates and provenance, and none is invented. Supporting private/unpublished
packs would be a separate owner decision about local provenance.

| Evidence | Result |
| --- | --- |
| Real Adrenaline archive (copied from the verified store) | recognized as `BYN9yKrV/JBUK3TRN` |
| New-instance review / install | 27 recognized files, 7 overrides; managed pack instance, validation `ready` |
| Privately edited copy (valid ZIP) | `local_pack_unrecognized` |
| Text file named `.mrpack` | `pack_invalid_archive` |
| Merge into vanilla 1.21.10 | blocked (needs Fabric; Minecraft mismatch); instance bytes unchanged |
| Merge into Fabric 1.21.11 pinned 0.19.2 | blocked: needs Fabric Loader ≥ 0.19.5 |
| Merge after re-pinning to 0.19.5 | 27 mods added; user config sentinel kept; instance stays ordinary |
| Unit tests (loopback Modrinth) | verified cache hit with no archive download; unpublished refused; secondary-file match refused; malformed/traversal/Forge/mislabelled/relative refused before any lookup |

## 8. Native-preview process isolation

- `native/tools/acceptance-session.ps1` refuses unmarked roots and the real
  production and preview roots, and refuses distribution builds (they would
  open real data). It records pre-existing Aurora processes without touching
  them, plus the exe SHA-256, PID, creation time and the one owned app HWND.
  It verifies the data root the process itself reports.
- Before interactive work, other Aurora processes and desktop idle time were
  checked. The perf smoke ran only after 7 minutes idle.
- `native/tools/test-shutdown.ps1` is a bounded exact-process shutdown test.
  **5/5 runs**, plus 2 more in the perf smoke, closed through the exact HWND
  with "window closed" and "exited cleanly" within 15 s. The NATIVE-9
  lingering-close observation did not recur. Two harness bugs were found and
  fixed while building it: a pipe inherited by the launched app blocked output
  capture, and `Start-Process -Wait` waits for the whole process tree.

## 9–11. Owner-attended areas

- **Authenticated gameplay:** not performed. It needs your Microsoft sign-in in
  the isolated preview (the credential namespace is
  `com.aurora.launcher.native-preview/account/`). No account was signed out,
  copied or exported, and no launch arguments were reused.
- **Skins/capes:** no real profile mutation. Destructive behaviour remains
  covered by existing synthetic tests only.
- **Preview installer:** not installed on this machine; installing it needs
  your approval. NATIVE-9's disposable installer acceptance (including the
  fixed fast-exit handoff) still applies, because the NSIS script and updater
  are unchanged in NATIVE-10.

## 12. Small-window and DPI acceptance

The interactive native-window pass was declined at the desktop-control prompt
and not retried. Instead, a headless test (`native/src/layout_acceptance.rs`)
renders the **real `AppWindow`** with the shipped software renderer. It covers
8 scenes: Mods with update badges, Settings → Game, create with loader version,
update review, modpack review, import review (blocked and allowed), and the
local `.mrpack` review. Sizes are minimum 720×520, typical 1120×760 and large
1600×1000, and scales 100/125/150/200% at the minimum size (49 renders). The
same test dispatches real pointer events: Check, Update all, row Update and
Unpin each fire exactly once, and Escape dismisses the update review.

Findings from inspecting the renders: no clipping or overlap. Long filenames
and target names wrap; the summary line wraps at minimum width; Install stays
visibly disabled when blocked; Update modpack stays disabled until the
keep-local box is ticked; the create dialog fits at 200% of the minimum
window. Renders are in [`docs/native-ui/native10/`](docs/native-ui/native10/).
These are software renders of the real components, not screenshots of a
desktop window.

## 13. Accessibility findings

New controls use the existing accessible components (labelled buttons,
combobox dropdowns with values, list items). Row descriptions include the
update badge text. Dialogs keep Escape dismissal, and pickers keep owner
modality and focus restoration (verified on the native window in NATIVE-9).
No screen-reader run was done; no certification is claimed.

## 14. Updater and release readiness

Unchanged from NATIVE-9 and still fail-closed: no production key or native
update address in development builds, signed native-only offers, strict
upgrade rule, and the fixed fast-exit handoff with its regression test. Updater
unit tests pass inside the core suites. No release work was performed.

## 15. Test results (final source)

| Command | Result |
| --- | --- |
| `cargo fmt --check` (native, core) | clean |
| `cargo test --manifest-path native/Cargo.toml --locked --all-targets` | 41 + 12 + 4 + 4 + 4 passed, 0 failed |
| core `--no-default-features --features native-preview` | **1,038 passed**, 6 icon, 0 failed, 31 ignored |
| core ordinary Tauri | **1,040 passed**, 6 icon, 0 failed, 31 ignored |
| core `--no-default-features` | **1,034 passed**, 6 icon, 0 failed, 31 ignored |
| `npm run check` / `npm test` | 0 errors, 0 warnings / 301 passed |
| optimized acceptance build | exit 0, SHA-256 `f1dace8f…6494c` |
| shutdown test | 5/5 clean |
| live end-to-end driver (`native10_acceptance`) | all steps pass; one initial failure was a wrong test expectation (NeoForge 1.21.1), corrected and documented in §6 |

The 31 ignored tests are existing explicit opt-ins and are not counted.
Machine-readable live results are in
[`live-acceptance.json`](docs/native-ui/native10/live-acceptance.json).

## 16. Resource measurements

Same populated disposable root, conditions and exact-HWND sampler as the
NATIVE-9 smoke. CPU is % of one of 28 cores; the window held foreground.

| Build | Borealis foreground CPU | WS / private MiB | Minimized CPU | WS / private MiB |
| --- | ---: | ---: | ---: | ---: |
| NATIVE-9 final, same run | 0.00–2.33 | 43.7 / 18.2 | 0.00 | 35.7 / 10.2 |
| **NATIVE-10** | 0.16–3.89 | 43.8 / 18.0 | 0.00 | 35.8 / 9.9 |

There is no measurable memory change, and CPU intervals are in the same noisy
band. The window appeared in 97 ms (enumerated window, not settled startup).
Navigation, browsing, update and import resource runs need UI input and were
not measured.

## 17. Native screenshots

See §12. No desktop screenshots were taken in this phase.

## 18. Functional parity matrix

Evidence: **L** = live services on a disposable root, **S** = synthetic/unit,
**R** = headless render + input of the real UI, **N** = earlier real
native-window interaction.

| Workflow | Classification | Evidence |
| --- | --- | --- |
| Home, selection, Play readiness | VERIFIED (earlier phases) | N |
| Instance creation | VERIFIED | L (created with exact pin), R |
| Exact loader version (create + existing) | VERIFIED | L, S, R |
| Custom Java executable | MISSING (owner decision) | — |
| Managed Java install/reuse | VERIFIED (earlier phases) | N, S |
| Instance icons | VERIFIED (NATIVE-8) | N |
| Content updates | VERIFIED (apply not driven through the native window) | L, S, R |
| Managed modpack updates | IMPLEMENTED, PARTIALLY VERIFIED (check + review live; apply not run) | L, S, R |
| Local resource-pack import | VERIFIED (NATIVE-9) | N |
| Local `.mrpack` import (published archives) | VERIFIED | L, S, R |
| Local `.mrpack` import (unpublished archives) | BLOCKED (refused by design; owner decision) | L, S |
| Discover / Modrinth browsing, pack install/merge | VERIFIED (NATIVE-8/9) | N, L |
| Concurrent downloads, cancellation | VERIFIED (NATIVE-8/9) | N, S |
| Skins/capes preview | VERIFIED (preview only) | N |
| Skin/cape account mutation | BLOCKED (owner authorization) | S |
| Accounts, sign-in, authenticated Play | BLOCKED (owner sign-in in preview namespace) | historical N |
| Appearance, Borealis, widgets, media, Discord prefs | VERIFIED (NATIVE-8/9) | N, S |
| Normal shutdown | VERIFIED | L (7/7 clean) |
| Native installer | VERIFIED in disposable roots; real install BLOCKED (owner approval) | L |
| Native updater | IMPLEMENTED, fail-closed; signed E2E BLOCKED (protected) | S |
| Small window / DPI | VERIFIED headlessly (no native-window pass this phase) | R |
| Screen reader | UNVERIFIED | — |

## 19. Local commits

On `codex/native-ui-migration` after `e6e74ee`, local only (no push, merge,
tag or publication):

1. `80fe215` — shared core: local `.mrpack` recognition over the verified
   pack engine.
2. `ed4d8a0` — native: content and modpack updates, exact loader selection,
   local `.mrpack` import UI.
3. `d9e7320` — native acceptance tooling: headless layout test, live driver,
   session and shutdown tools.
4. The commit containing this report, the parity matrix and sanitized evidence
   (its SHA is reported alongside, since a commit cannot name itself).

## 20. Next recommended phase

1. **Owner decision on custom Java** (§5). If approved: schema bump, Rust
   validation, launch-path change and an `AGENTS.md` update, as one scoped phase.
2. **Owner-attended preview session:** approve desktop control or drive it
   yourself; install the preview package; sign in in the preview namespace;
   play; authorize any skin/cape mutation.
3. Optional: apply a managed-pack update end-to-end; interactive cancel/retry
   of an update job; a screen-reader pass.
4. The protected release chain from NATIVE-9 (signing, authority, CI,
   publication) remains separately gated.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
