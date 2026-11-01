# Aurora Launcher — NATIVE-10 functional parity and preview acceptance

**Verdict: PASS — LOCAL FUNCTIONAL PARITY ACCEPTED.** All four requested
NATIVE-10 features are implemented and locally verified. This continuation
completes the owner-authorized custom Java capability over the retained work;
the earlier content-update, exact-loader and local `.mrpack` evidence below
remains valid. Actual native picker/DPI click-through and authenticated gameplay
remain owner-attended acceptance. Protected distribution remains separate.
Nothing was pushed, merged, tagged, published or production-released.

Windows 11 x64, 2026-11-01. Branch `codex/native-ui-migration`, worktree
`C:\Dev\aurora-launcher-native`.

## 1–3. Repository identity and preservation

| Item | Value |
| --- | --- |
| Original NATIVE-10 starting HEAD | `e6e74ee7a9ca05cb6ab96da170b1f17bfcea2614` (NATIVE-9 final), clean index and tree |
| Tracked files at start | 3,190 |
| Final commits | listed in §19 (local only) |
| Main checkout `C:\Dev\aurora-launcher` | HEAD `9b99db3`; its 10 pre-existing changed/untracked entries untouched |
| Production executable | SHA-256 `42c34fba…2387c`, unchanged |
| Real preview data/install | no install, uninstall key or preview data root created by this phase |

No tracked file was deleted or renamed. `git clean`, `cargo clean`, resets,
recursive cleanup and process-name termination were not used. Every process
ended during this work was one this session started, identified by exact PID
and command line. The original live acceptance used marked `aurora-native10-*` disposable roots
under `%TEMP%`. The continuation also uses unique `aurora-custom-java-*` unit
fixtures and a separate marked root for each preview smoke; none opens real data.

### Final continuation preservation (2026-11-01)

- Start: `96cc205b15dfcd8c418294894af8c8cdf3960396`.
- Ending implementation HEAD (full): `69ac4d5321a911f5660d2d92d6177eec93c1b995`.
- Branch: `codex/native-ui-migration`, `C:\Dev\aurora-launcher-native`.
- The index was empty. Twelve modified tracked Java-integration files and
  untracked `src-tauri/src/runtime/external.rs` were preserved and completed.
  Full tracked-file inventory, byte hashes, staged/unstaged patches, untracked
  names, history and all worktree identities were recorded before editing in a
  unique `aurora-native10-continuation-*` temporary evidence root. The baseline
  contains 3,206 tracked files plus the one existing untracked module; all
  3,207 original files remain. Full local scripts/logs are retained under
  `%TEMP%/aurora-native10-continuation-4f0d7e49-ef93-4706-b304-08dd65142daa`.
  Unscoped byte changes: zero. Main checkout
  status and complete binary diff match the recorded baseline exactly.
- Production EXE full SHA-256:
  `42c34fbab45ece1c88727374b52751067d5aeebb419a705f72f6e16dd6a2387c`
  Final identity/preservation check recorded below.
- Retained optimized NATIVE-10 baseline SHA-256:
  `f1dace8fcd41ad86c82c806f4ba551022662bb81928e049b7076e476f216494c`.
- The stable WebView source/working changes and its production binary were not
  edited. Both compiled update endpoints, production manifests, keys/configuration,
  signing and publication authority retain their prior identities. Native offers
  remain unconfigured without separately supplied owner build authority.


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

## 5. Custom Java — implemented and locally verified

The owner explicitly approved **“Yes, implement it.”** This supersedes the old
blanket prohibition. `AGENTS.md` now permits narrowly scoped, per-instance,
validated external executables; `ARCHITECTURE.md` records the implemented model.

**Configuration:** optional `configuration.java = {kind: "custom", path: …}`;
absent/null remains managed. Saving retains the path even if runtime eligibility
fails, so it stays editable; malformed path/configuration shapes are refused.
Use managed Java plus Save removes it. Existing atomic saves, original-value
checks, memory, JVM-argument validation, performance mode, window options and
installed identity remain intact. No global registry schema change or redundant
runtime copy is introduced. Installer tools still use their managed Java.

**Validation:** `runtime::external` checks absolute plain path/name, regular file,
access, no links/reparse ancestors, native Windows PE architecture (Unix execute
permissions elsewhere), then the existing process-tree supervisor runs a structured
properties/version diagnostic. It clears the environment, disables crash reports
and core dumps, hides the Windows console, bounds execution to 15 seconds plus
one-second drain grace, and retains at most 64 KiB per stream. Truncated,
unreadable, ambiguous, malformed, nonzero and timed-out output fails. Exact game
major (from the normalized composed Minecraft/loader plan) and host architecture
must agree. Selecting a file is a local decision to trust running it; this does
not establish that arbitrary executable code is harmless. External installations
are never modified or repaired.

**Readiness and launch:** runtime status and Play read the saved selection;
custom failures produce `launch_custom_java_invalid` and open Settings. There is
no managed fallback. Readiness caches successful diagnostics by selected path,
executable SHA-256 and required major, after fresh file/ancestor checks. Failures
are not cached. No idle polling or recurring Java subprocess is added. Every
launch runs a fresh diagnostic; assembly checks the game major and executable
identity again. The final Windows boundary holds a read handle denying ordinary
write/delete replacement through spawn, including time spent restoring the
session. Runtime DLL changes can invalidate a cached readiness verdict without
changing the EXE; Check and launch deliberately run the probe again. This does
not claim publisher authenticity, complete DLL identity, malicious same-user
namespace protection or offline metadata resolution.

**Actual process selection:** a native compiled fixture passed validation outside
managed runtimes, flowed through `resolve_launch_spec_with_java`, and was started
by the established supervisor. The fixture recorded exact argument strings and
the derived instance working directory; its supervised terminal state reported
exit 0. A token validated for another game major and a subsequently removed file
were refused. No account or real Minecraft game was used for this proof; normal
classpath ordering, memory/JVM options, token redaction and supervision remain
covered by existing launch tests.

**Compatibility tested, not assumed:** a disposable harness compiled the actual
older WebView core from `C:\Dev\aurora-launcher`, HEAD
`9b99db3ac226854c8ae96a86eed4c8d3084defa7`, with its lockfile. Its real registry
reader accepted managed configuration, rejected the added `java` field on
load/migration **and save**, preserved the custom document byte-for-byte, and
accepted it again after reset to managed. **Custom configuration blocks downgrade
until reset using a supporting launcher.** It is not silently ignored or erased.
Native DTO extra fields are preserved; unknown persisted core fields remain
errors. The updated WebView DTO round-trips `java` on unrelated edits, but this
phase provides selection UI only in the native host.

**Tests:** registry save/restart/unrelated edits/reset and damaged-state
preservation; managed default; malformed selections; native Unicode/spaced-path
fixture; wrong major/architecture; missing/non-native file; junction; malformed
and duplicate properties; nonzero exit; deadline; excessive output; removed or
same-size/same-timestamp changed EXE; probe-cache reuse; explicit custom readiness
blocker; selected executable/arguments/CWD and normal supervisor exit. Real Eclipse
Adoptium 25.0.4.1 `java.exe` and `javaw.exe` also passed an explicit local check.

**UI:** existing draft/Save semantics; managed/custom selector; full-width editable
path; owned single-click Choose; explicit Check; wrapped detected version or
problem; Use managed Java. Choosing does not execute the file. Stale async checks
are discarded on path, mode, instance or saved/draft game changes. The shared
picker preserves its owner, modality, single-picker guard, cancel/focus behavior
and lifetime checks. Missing/invalid window-owner rejection and retry after refusal pass without
opening a dialog. New Java controls were driven headlessly; actual Windows
picker cancellation and keyboard traversal remain owner-attended (see §12).

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
- In the original session, other Aurora processes and desktop idle time were
  checked before interactive work; that original perf smoke followed 7 minutes
  idle. Final continuation conditions are stated separately in §16.
- `native/tools/test-shutdown.ps1` is a bounded exact-process shutdown test.
  **5/5 runs**, plus 2 more in the original perf smoke, closed through the exact HWND
  with "window closed" and "exited cleanly" within 15 s. The NATIVE-9
  lingering-close observation did not recur in those original runs. The final
  continuation also encountered lingering processes with its first resource
  wrapper. Direct-file-handle negative controls isolated this to inherited
  output pipes in that wrapper, rather than a product shutdown defect. The
  product close path remains unchanged; corrected final measurements and
  clean-close controls are recorded below. Two original harness bugs were found and
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

## 12. Small-window and DPI acceptance (retained plus final continuation)

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

The final continuation adds valid/invalid Java Settings scenes. The complete real
Slint test renders 10 scenes at 720×520, 1120×760 and 1600×1000 plus minimum-size
100/125/150/200% scales: 70 scene renders, with minimum 100% repeated, and 3 input
frames (63 distinct files). Choose, Check and Reset dispatch on one pointer click;
End plus text input edits the long path. The mode selector, three actions and
wrapped error remain visible at minimum 150% and 200%; the full Settings page
scrolls normally. Long paths are keyboard inspectable in the shared text field.
Representative final renders:

- [Minimum 150%, incompatible Java](docs/native-ui/native10/custom-java-invalid-minimum-150.png)
- [Minimum 200%, incompatible Java](docs/native-ui/native10/custom-java-invalid-minimum-200.png)
- [Typical, compatible Java](docs/native-ui/native10/custom-java-valid-typical.png)

No new decorative treatment or control variants were added. The redundant status
pill was removed; the wrapped result carries the status. These are software
renders and dispatched headless input, **not desktop click-through acceptance**.
The retained interactive desktop-control refusal was respected. Actual dialog
selection/cancel, native focus traversal, DPI behavior and a screen-reader pass
remain owner-attended. No baseline Java screenshot was available before the
continuation; the original fixed-width clipping defect was evident in source.
Machine-readable final checks: [custom-java-verification.json](docs/native-ui/native10/custom-java-verification.json).

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

## 15. Test results — final continuation

| Command | Actual result |
| --- | --- |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` and native equivalent | clean |
| `cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets` | pass |
| `cargo test --manifest-path native/Cargo.toml --locked --all-targets` | 42 + 5 + 12 + 4 + 4 + 4 passed (71 total), 0 failed |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features --features native-preview` | 1,047 + 6 icon passed, 31 ignored, 0 failed |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked` | 1,049 + 6 icon passed, 31 ignored, 0 failed |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features` | 1,043 + 6 icon passed, 31 ignored, 0 failed |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features --features native-preview runtime::external` (after cache race guard) | 6 passed, 0 failed |
| `npm run check` | 0 errors, 0 warnings |
| `npm test` | 301 passed, 0 failed, 0 skipped |
| `npm run build` | pass |
| explicit real Java25 diagnostic (`custom_java_acceptance`) | java.exe and javaw.exe pass |
| actual older WebView core registry harness | managed accepted; custom read/save refused, bytes preserved; reset accepted |
| optimized acceptance build / isolated native boot / resource smoke | final results in §16 |

The 31 ignored core tests remain opt-in and are **not counted as passes**. No
security test was disabled. Broad suites include runtime, config, readiness,
launch, loaders, installer containment/retention and updater boundaries. The three
completed features retain the earlier live evidence; no large network install
was repeated.

Resolved investigation failures: an initial supervisor fixture used the wrong log
root (correctly refused; fixed to the derived instance logs); a native async
callback had an invalid `Option::flatten` (corrected); Vite SSR tests watched
locked native build products (`EBUSY`). `vite.config.js` now excludes those build
folders; the complete frontend suite then passed in about 26 seconds. The older
WebView harness initially resolved newer unlocked dependencies, then needed its
original lockfile and Tauri's static-runtime link settings. Those were harness
corrections in a disposable root, with no old-source or dependency-file edits.
The new PowerShell harness also normalized parsed DateTime values to UTC for
identity checks. Its initial redirected-output wrapper could keep previews
lingering; a native CreateProcess test helper with direct inherited file handles
for stdout/stderr removed the pipe dependency. The baseline then closed cleanly.
Failed wrapper runs were retained, and only their exact recorded processes were
ended after diagnostics. Native close behavior was left unchanged.

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


### Final continuation optimized smoke

The final release-profile acceptance binary was built with
`cargo build --manifest-path native/Cargo.toml --locked --release --features disposable-data-root`.
SHA-256: `bd0594696ad6c7abea9285f355c143bf4d2ac429659aafdd70a42d3eaf80c6bc`.

Matched fresh empty roots, Simple background, no account or instances. A native
CreateProcess helper gives stdout/stderr direct file handles, records the exact
executable/PID/creation time/window, verifies the app-reported disposable data
root, and closes only that window. Visible sampling is about 12 seconds,
minimized sampling about 10 seconds, after bounded settling. This is a bounded
idle observation, not a game/frame-time benchmark.

| Build | Visible CPU, one core | Visible WS / private MiB | Minimized CPU | Minimized WS / private MiB |
| --- | ---: | ---: | ---: | ---: |
| Retained NATIVE-10 baseline | 0.00% | 29.78 / 9.73 | 0.00% | 24.75 / 4.61 |
| Final custom Java build | 0.00% | 29.70 / 9.60 | 0.00% | 24.66 / 4.48 |

No material idle memory regression was observed. Reported zero CPU is limited to
these sample windows and counter resolution. These empty/Simple conditions differ
from the earlier populated/Borealis 44/36 MiB observations; those are historical
context, not targets. GPU counters returned no usable per-process values.

The final owned window was enumerated in 109 ms (not settled Home
paint or physical visibility). It exited cleanly after the resource run; two
additional exact-HWND visible/minimized close controls also exited cleanly:
**3/3 final optimized shutdowns**, with the original product close path. No Java
or other direct child was present at the end of the final idle sample. The failed initial
pipe-based resource wrappers are excluded; their evidence was preserved.


## 17. Native screenshots

See §12. The Java acceptance images are headless real-component renders. An
owned-window PrintWindow diagnostic during harness investigation was blank and
was not counted as a valid product screenshot or UI acceptance. No desktop
Settings click-through acceptance is claimed.

## 18. Functional parity matrix

Evidence: **L** = live services on a disposable root, **S** = synthetic/unit,
**R** = headless render + input of the real UI, **N** = earlier real
native-window interaction.

| Workflow | Classification | Evidence |
| --- | --- | --- |
| Home, selection, Play readiness | VERIFIED (earlier phases) | N |
| Instance creation | VERIFIED | L (created with exact pin), R |
| Exact loader version (create + existing) | VERIFIED | L, S, R |
| Custom Java executable | VERIFIED locally; native dialog/gameplay attended pass outstanding | S, R, real Java diagnostic |
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
| Normal shutdown | VERIFIED | retained L (7/7); final optimized L (3/3), corrected harness |
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
4. `96cc205b15dfcd8c418294894af8c8cdf3960396` — original NATIVE-10 report,
   parity matrix and sanitized evidence.

5. `69ac4d5321a911f5660d2d92d6177eec93c1b995` — complete validated per-instance custom Java selection,
   responsive native controls, policy/architecture, tests and native-build watcher exclusion.
6. The documentation/evidence commit containing the final continuation results
   (local only; full SHA reported in the final chat).
A report commit cannot embed its own SHA; its full hash is reported in the final
chat and can be found with `git log -1 -- NATIVE_10_ACCEPTANCE.md`.

## 20. Remaining owner-attended acceptance and production gates

1. Native preview Settings click-through: actual Choose/cancel/focus/close,
   keyboard traversal, 150/200% display scaling and screen-reader behavior.
2. Sign in through the isolated preview namespace and exercise authenticated
   gameplay with the chosen custom Java, normal exit and return to Ready.
   Skin/cape account mutations still require their own explicit action.
3. Optional retained areas: apply a managed-pack update end-to-end and exercise
   interactive update cancel/retry; no completed feature was reimplemented.
4. Preview installation on the owner's machine and the protected NATIVE-9
   signing/authority/CI/publication chain remain separately gated. This local
   acceptance does not authorize or claim production distribution.

No new development phase, client change, release action or additional experiment
is started by this report.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
