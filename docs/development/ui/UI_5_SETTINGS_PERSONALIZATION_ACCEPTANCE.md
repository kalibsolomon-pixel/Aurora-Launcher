# UI-5 — Settings & Personalization acceptance

**PASS — implementation and scoped browser/native acceptance complete; ready for
owner visual review.** No owner aesthetic approval is claimed. UI-4 remains
separately **PARTIAL**. A production-config hash change during the interrupted
session is explicitly reported below; blanket owner-config byte identity is not
claimed.

## Repository and local history

- Branch: `codex/client-3-production-authority`.
- Starting HEAD: `3e248361474d6de8eb39df8d50549ace1cd867a8`.
- Verified implementation/evidence HEAD:
  `c0910ae1c1c528348107c829449d9d6f1678b27d`.
- Ending work consists of that verified HEAD plus the final acceptance-document
  commit. Its exact SHA is recorded by Git and in the delivery message; resolve
  it with `git log -1 --format=%H -- docs/development/ui/UI_5_SETTINGS_PERSONALIZATION_ACCEPTANCE.md`.
  A document cannot embed its own commit hash without changing that hash.

| Local commit | Boundary |
| --- | --- |
| `7b9cb52` | Settings audit and original preservation baseline |
| `34732a1` | Cohesive Settings presentation, navigation, feedback and existing capabilities |
| `04edceb` | Focused tests and isolated browser/native acceptance helpers |
| `c0910ae` | Original/matched captures, regression logs, native verification and preservation evidence |
| Final report commit | This acceptance report and complete changed-file inventory |

All commits remain local. No push, merge, tag, publication, deployment, production
installation, package/version change or release preparation occurred. Work stops
at UI-5. All seven original worktrees remain registered and their other HEADs are
unchanged. The original working tree had only the four protected local artifacts;
they remain byte-identical and untracked.

## Audit, setting inventory and ownership

The audit was completed and committed before broad implementation:
[SETTINGS_AUDIT.md](../../ui-5/SETTINGS_AUDIT.md). It records each existing setting,
default, authoritative current-value source, allowed values, validation, save
behavior, effect timing, errors and prior coverage, plus instance/account surfaces.
AGENTS.md and ARCHITECTURE.md were read in full; actual current code supersedes
historical descriptions of older schemas and capabilities.

| Category | Implemented surface / authoritative owner |
| --- | --- |
| Appearance | Three existing themes, Simple/Borealis, bounded motion speed, seven preset accents and native-validated custom color; global native appearance preferences |
| Home & Widgets | All eight registered widgets, shared visibility state, existing Home editor and confirmed native layout reset |
| Launcher | Live Windows desktop/Start-menu shortcut status and existing capability-gated operations |
| Minecraft & Java | Saved selected-instance memory summary and clearly labeled links to that instance's Settings/Overview; Library when no instance is selected |
| Updates | Existing canonical Launcher/Client update controller and owner-published release authority; no new preferences or automatic installation |
| Discord & privacy | Existing master switch and eight independent detail permissions, native connection/setup state and automatic connection explanation |
| Accessibility | Read-only reduced-motion, reduced-transparency and forced-color system status, plus existing appearance access |
| About | Running native application version/platform and license/product information; no private paths |

No unsupported toggles, custom Java picker, global memory override, startup
behavior setting, bandwidth control, telemetry, cleanup action or OAuth flow was
introduced. Accounts and skin/cape Appearance stay separate. Existing per-instance
memory, JVM arguments and window dimensions apply on the next game launch;
Minecraft/loader changes retain Save followed by deliberate installation.

## Design and behavior

One compact category sidebar becomes wrapping section buttons below 1050px.
There is no additional global navigation rail. Existing General/Home/Updates
keys and update deep links survive; General is labeled Launcher and Home is
labeled Home & Widgets. Category reading positions are session-only. Arrow keys,
Home and End move selection and focus; ordinary Tab/Enter/Space remain native.

Shared rows associate labels and descriptions, retain suitable native inputs,
and add textual On/Off state. Errors use alerts, pending writes say Saving, and
Saved appears only after the authoritative write succeeds. Rejected toggles and
radios restore the saved state; Retry retains the requested operation. Failed
slider/color input remains visible for retry. Retrying a read never claims Saved.
No global Save button or restart requirement was invented.

Appearance retains the existing renderer and native-derived root accent tokens.
Static theme/background tiles preview existing effects; current theme/background/
accent are also textual. Motion shows its 0–100 value and explains that zero is
slowest, not Off. The direct custom picker now has real focusable geometry, commits
the completed change and marks a custom selection with a checkmark. The supported
selection/focus/primary-action accent system remains page-wide.

Widgets reuse `widgetCatalog`, `HomeWidgetsStore` and the original native layout.
Hiding preserves order, size, unknown IDs and other visibility flags. Edit Home
reuses the original editor; no second widget renderer or backend subscription was
added. Reset describes exactly what changes, defaults focus to Cancel, and returns
focus to its trigger on cancellation/close. Existing backend reset safety is intact.

Discord preserves all privacy defaults and independent permissions. Server Address
editing requires Show Server; hiding Server does not erase the stored address
choice. Effective disclosure remains enforced in Rust. Loading no longer implies
missing registration before a response arrives. Native acceptance never enabled
presence or any disclosure, connected Discord, or signed into Microsoft.

## Configuration compatibility and performance

Launcher configuration remains **schema 7**, registry 5, cosmetics 2, and all
existing serialization/default/migration policies are unchanged. No Rust source,
DTO, backend command, error code, manifest, permission, native minimum, package
metadata, lockfile or release asset changed. Unknown/malformed native documents
retain their existing refusal behavior; known migrations and unknown widget
registrations are covered by the existing deterministic fixtures.

The existing appearance, widget and Discord stores remain authoritative and
byte-identical. Appearance has no extra normal read; its explicit load retry uses
the existing command. Widget loading remains guarded by the shared store. Discord
keeps its one mounted listener and disposal. Accessibility adds only three media
query listeners while its category is mounted, with cleanup on departure. About
reuses boot status. There is no live widget preview, new dependency, background
poll, idle animation loop, duplicated configuration system or telemetry. No startup,
GPU or memory improvement beyond these source-level observations is claimed.

## Automated verification

Full commands, exit codes, initial failures and final results are retained in
[checks/exits.json](../../ui-5/checks/exits.json) and the named adjacent logs.
Commands below are relative to the repository root; `ui5-run.mjs` records their
exact invocation/output without changing their assertions.

| Command | Actual result |
| --- | --- |
| `npm run check` | Final check: exit 0, zero errors and warnings |
| `npm test` | Initial unbounded parallel run: exit 1; shared Vite SSR cache EPERM collisions/stalled workers; retained, not concealed |
| `node --test --test-concurrency=1 --test-timeout=60000 "src/lib/**/*.test.ts"` | Exit 0; **280/280** frontend tests |
| Serial bounded run of `settings.test.ts`, `appearanceCustomAccent.test.ts`, `releasePolish.test.ts`, `generalSettings.test.ts` | Exit 0; **24/24** after final appearance feedback/selection refinements |
| `node tools/visual/ui5-pilot.mjs` | **5/5** pilot cases before broader category rollout; zero type-check diagnostics |
| `node tools/visual/ui5.test.mjs` | Final exit 0; **14** functional groups, **80** layouts, zero page errors |
| `node tools/visual/ui5-minimum.mjs` | Exit 0; **16** additional layouts at measured native 712×512 and 100%/200% text |
| `node tools/visual/ui5-regression.mjs ui1` | Exit 0; **11** UI-1 cases |
| Same runner, `ui2` | Exit 0; **17** UI-2 cases |
| Same runner, `ui2b` | Exit 0; **6** UI-2B cases |
| Same runner, `ui2b1-crop` | Exit 0; **6** responsive crop cases |
| Same runner, `ui3` | Exit 0; **11** UI-3 groups |
| Same runner, `ui31` | Exit 0; **13** UI-3.1 groups; original approved non-Modpack geometry/screenshots preserved |
| Same runner, `ui4` | Exit 0; **13** UI-4 browser groups; five sizes at 100%/200% text |
| `cargo test --manifest-path src-tauri/Cargo.toml config::tests:: --lib` | Exit 0; **24/24** compatibility/preservation tests |
| Same native command filtered to `appearance::tests::` | Exit 0; **10/10** palette/appearance tests |
| Same native command filtered to `discord::` | Exit 0; **17/17** privacy/transport tests; no live Discord |
| `npm run tauri build -- --debug --no-bundle` | Final exit 0; production static frontend plus isolated debug executable; no installer |
| `node tools/visual/ui5-native-verify.mjs` | Exit 0; restart equality, widget invariants, schema, host sizes and qualified owner-hash observation |
| `node tools/visual/ui5-preserve.mjs` / `git diff --check` | Pass; no tracked deletion/rename or whitespace errors |

The serial runner was already used by UI-4; it avoids the pre-existing shared
prebundle collision without modifying tests to hide failures. Only recorded
children of this task's stalled parallel test runner were stopped, after checking
their exact PID, parent and test command. No process-name termination occurred.

The two initial UI-5 browser failures were harness issues: immediate failed-write
rollback makes Playwright `check()` correctly reject its postcondition, and the
installed Playwright media helper ignored an unsupported transparency option.
The final harness clicks the rejected radio then asserts rollback, and uses
explicit CDP media emulation with positive match/opaque-style assertions. Original
failure logs remain. Existing native warnings (STATIC_VCRUNTIME and four test-only
unused/dead-code warnings) remain unchanged. No full Rust fmt/check/test claim is
made: no native/startup boundary was changed; relevant compatibility tests ran.

## Responsive, accessibility and visual evidence

The primary browser matrix covers 1440×900, 1280×800, 1024×768, 720×520 and the
historical observed 706×512, all eight categories at 100%/200% text. The measured
712×512 native minimum adds 16 exact browser fixtures. No document/content
horizontal overflow occurred. Buttons wrap; category navigation stays reachable;
long descriptions/labels, color target, range, reset dialog and error feedback
remain usable with vertical scrolling.

Checks cover category focus, label/description associations, announcements, native
checkbox/radio behavior, reset Escape/cancel/focus return, reduced motion,
transparency, forced colors and alternative/custom accent. Forced colors use
system selection borders/checkmarks; state is not color-only. No manual assistive-
technology user study or live screen-reader certification is claimed.

Five matched comparison sheets use identical dimensions and starting preferences:
[Appearance](../../ui-5/comparison-appearance.png),
[Home](../../ui-5/comparison-home.png),
[Launcher](../../ui-5/comparison-general.png),
[Discord](../../ui-5/comparison-discord.png),
[Updates](../../ui-5/comparison-updates.png).
Original `before/`, pilot and final `after/` captures are retained. Functional,
error, system-media and scaling captures are in `docs/ui-5/browser/`; unchanged
historical assertions write new evidence only beneath `docs/ui-5/regression/`.
Historical UI-1 through UI-4 evidence was never overwritten.

Agent visual inspection found a consistent hierarchy, supported current values,
readable grouped rows and deliberate wrapping/scrolling. Narrow Appearance trades
more vertical space for usable theme targets and complete navigation. This is
evidence for owner review, not a claim of owner aesthetic approval.

## Real native acceptance

Detailed provenance, complete capture index and excluded attempts:
[NATIVE_EVIDENCE.md](../../ui-5/NATIVE_EVIDENCE.md).
Final executable SHA-256:
`034abcd1291e38a980dd9b138f2a94521e0fa2b0146a6642844db7178def76ba`.
The native process and WebView use a fresh marked direct-temp root; restart reuses
only that root. No owner configuration, accounts, credentials, game data or
appearance library was copied. The root remains retained for inspection.

| Actual native CSS client | DPR | Evidence |
| --- | --- | --- |
| Default 1120×790 | 1.25 | [Launcher overview](../../ui-5/native-final/overview.png), [geometry/state](../../ui-5/native-final/overview.json) |
| Exact medium 1024×768 | 1.25 | [Whole host window](../../ui-5/native-final/medium-host.png), [proof](../../ui-5/native-final/medium-proof.json), [Appearance](../../ui-5/native-final/medium-host-appearance-verified.png) |
| Enforced minimum 712×512 | 1.25 | [Whole host window](../../ui-5/native-final/minimum-host.png), [proof](../../ui-5/native-final/minimum-proof.json), [widgets](../../ui-5/native-final/minimum-widgets.png), [Discord](../../ui-5/native-final/minimum-discord.png) |
| Restart default 1120×790 | 1.25 | [Restored Home](../../ui-5/native-persistence/restored-home.png), [restored Appearance](../../ui-5/native-persistence/restored-appearance.png) |

The configured default 1120×760 and minimum 720×520 remain unchanged; the table
reports measured client areas at Windows scaling. Actual OS window edge drags and
System > Size established medium/minimum host measurements. Browser window-bounds
API attempts resized the embedded child view instead and are explicitly excluded.
No browser viewport simulation is represented as native host sizing.

All eight categories were inspected natively. Actual native writes selected
Borealis, Cyan, motion speed **70**, and hidden Instance Details. Native preferences
before/after restart are exactly equal; order, size and every other widget flag
match the original layout. Home no longer renders the hidden widget. Native
End/Home/ArrowDown produce matching category selection and focus. About reports
the running application's version/platform. Discord honestly reports missing
build registration and all nine booleans stay false. Java/Minecraft shows an
instance-scoped Library destination in the genuinely empty isolated registry.

The native reduced-transparency screenshot uses **emulated media preferences** in
the actual compiled WebView, with positive reduced-motion/transparency matches and
`backdrop-filter: none`; no Windows accessibility setting was changed. Actual
global Java editing with a populated instance is covered by fixtures/existing
tests, not a newly installed game. No live game, Microsoft or Discord acceptance
is claimed. Final run PID 21188 and restart PID 21000 both exited normally with
code **0**, observed through retained process handles; both are gone.

## Preservation and the observed owner-config difference

[preservation.json](../../ui-5/preservation.json) verifies all **1,984** original
tracked files still exist, no deletion/rename, all unscoped bytes unchanged,
all four protected local files unchanged, and all seven worktrees retained.
Home rendering, widget registry/persistence, native source, backend.ts, package/
lockfiles, build configuration, release assets and previous evidence remain
byte-identical. Only the five scoped pre-existing files changed: ARCHITECTURE.md,
SettingsPage, navigation store, HomeWidgetSettings and DiscordSettings. All other
changes are additive UI-5 source, tests, tools and evidence.

Owner preservation hashes are deliberately qualified:
[owner-observations.json](../../ui-5/owner-observations.json). Production executable,
accounts, instance registry and skin-library documents match their read-only
pre-native hashes. The production config hash differs. Its last write is
04:07:23 UTC (00:07:23 EDT), **after** the earlier isolated app exited at
04:07:11 UTC and **before** resumed isolated acceptance began at 07:27:44 UTC.
The production app was independently open during the task. The current owner
appearance differs from the synthetic test state; all tested native mutations
resolve through the unchanged managed-path helper to the explicitly validated
temporary root. The write's source was not directly observed. Current owner bytes
were preserved; no restoration, speculative migration or overwrite was attempted.
This is not reported as a byte-identical production configuration.

No owner instance/world/mod/pack directory was operated on or enumerated for
cleanup. No credential operation or authentication occurred. No destructive
cleanup, reset, worktree removal or production-process termination occurred.
The only remaining untracked files are the original protected developer files.
The full changed-file inventory is [changed-files.txt](../../ui-5/changed-files.txt).

## UI-4 result and remaining limitations

**UI-4 remains PARTIAL, with its owner visual approval retained.** UI-5 Settings
host sizing does not establish UI-4 full-body character/control/library-scrolling
acceptance. Those player-workspace native requirements were not exercised in this
empty, account-free root. Its original report remains unchanged.

UI-5 has no outstanding implementation or scoped automated/native acceptance
blocker. Owner aesthetic review remains pending. System media tests are simulated
preferences, not changes to Windows settings; live service/game testing was
intentionally excluded. The observed production-config difference remains a
qualified preservation observation, with evidence and no destructive response.

Recommended next step: owner review of the matched comparisons and native
medium/minimum screenshots, followed separately by the remaining UI-4 player
workspace verification. Do not begin UI-6 or release preparation.
