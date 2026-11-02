# UI-3 audit and proposal

Audit established before application edits, 14 October 2026. Branch
`codex/client-3-production-authority`, HEAD `7c76626464a8e0c1c581ddbf976cfb5b6c71e6f3`,
exactly the requested baseline; no intervening commits. No tracked changes.
Seven worktrees and four protected developer-local files are present. Initial
inventories/processes are retained under `%TEMP%/aurora-ui3-20261014`; committed
baseline.json records tracked paths and hashes, critical files and prior UI evidence.

## 1. Current architecture

The static SPA keeps one typed navigation store. InstanceWorkspace owns the
instance header and Mods / Resource Packs / Shaders tabs. There is no global
Discover or Library destination. “Library” in this phase means the existing
instance-local Installed views, not a new route.

InstanceModsPanel owns local query/filter/sort, native inventory, lifecycle,
recognition, updates, disclosure and confirmation. InstancePacksPanel owns the
corresponding pack presentation and native capability-driven controls. Both mount
ModrinthBrowse with their existing installed identities and callback. InstancesPage
also mounts that browser in standalone modpack mode. ModrinthBrowse owns search,
categories, sort, pagination, details and reviewed installation presentation.
backend.ts and Rust own all actual operations. No frontend network provider calls.

InstalledArtwork shares the bounded project resolution cache and native normalized
PNG boundary with Browse. Artwork supplies a local type-specific fallback and
contain-sized image, with reserved geometry. Inventory identity resolution is
independent of ownership. No provider URLs or new artwork sources are needed.

Read AGENTS.md and ARCHITECTURE.md in full, docs/development/ui/LAUNCHER_DESIGN_LANGUAGE.md,
docs/development/phases/PHASE_F_DESIGN.md, UI-1, UI-2B and UI-2B.1 acceptance, review-harness documentation,
and current source. Historical architecture descriptions are superseded by source
and the current sections (notably pack activation, modpack installation and loaders).

## 2. Feature inventory

| Capability | Mods | Modpacks | Resource Packs |
|---|---|---|---|
| Entry | Workspace Mods, Installed/Browse | Instances Browse Modpacks; shared type switch | Workspace Resource Packs, Installed/Browse |
| Search | Explicit submit; clearing triggers default search; no debounce | Same | Same |
| Provider filters | Live type-scoped category multi-select; relevance/downloads/newest/updated | Same, Fabric-only packs | Same |
| Compatibility | Native instance Minecraft and loader facets; version/dependency/collision review | Pack declares its new Minecraft/Fabric instance; no selected-instance filtering | Native Minecraft/version/file restrictions |
| Pagination | Initial 20, append Load more; 45-second default-page cache by context | Same | Same |
| Details | Inline project panel, exact version, license/loaders, dependency/conflict preview | Exact version, optional files, explicit review/create new instance | Existing project and install preview |
| Installation | Quick native install or reviewed exact install; direct/dependency distinction | Reviewed exact .mrpack transaction; new independent instance | Existing native quick/review install |
| Progress/error | Quick spinner and transient notice; details/install status; error code | Native phase events and retry by review/apply | Same as mods |
| Installed management | Snapshot search/filter/sort; switches; local or provider confirmation; details, recognition and explicit updates | Instance collection and existing PackUpdatePanel; no installed-pack row or fictitious toggle | Snapshot search/filter (no sort control); authoritative resource activation; safe local/provider removal; details, recognition/updates |
| Truthful metadata | Search title/summary/author/downloads/categories; no per-hit game versions/date/rating | Same; exact versions only in details/preview | Same; installed provider version or ZIP/folder fallback, pack format in details |

Shaders already exist; preserve their shared shell and in-game activation wording.
Do not expand shader functionality. Missing context/Installing instances remain gated
by existing native capabilities; no duplicate instance selector is appropriate.
Search result installed identities are supplied by the host content inventory;
cross-type discovery does not perform a new inventory scan just to decorate cards.

## 3. Visual weaknesses

Every content type currently shares a two-column icon/card arrangement, including
modpacks. Mods spend an entire second line on actions at wide widths. Modpacks
have only a 44px icon and little discovery emphasis. Resource packs lack a distinct
artwork scale. UI-1 improved only Mods controls; other filters retain fixed heights.
Muted metadata and per-row transparent backing can lose clarity over bright Borealis.
Installed packs show lengthy descriptions/warnings while Mods use disclosure.
Switch/trash/details geometry differs between installed types.

## 4. Usability weaknesses

No result count, explicit clear-search control or error Retry exists in Browse.
Context repeats a hard-coded Fabric label even for NeoForge; correct this display
from the existing loaderLabel without changing native facets. Install is icon-only;
pack details do not advertise review as the primary step. No-compatible-version
copy incorrectly mentions Fabric packs for all types. Installed list counts and
enabled-state text are inconsistent. Pack removal Escape clears confirmation but
does not restore focus. Narrow and 200% controls need independent verification.

## 5. Proposed component changes

Keep ModrinthBrowse as the shared structure and state owner. Refine its header,
target context, search/clear, content-type selection, filters, result count, status,
retry and pagination. Preserve explicit-submit/clear semantics, cache keys, native
requests, event subscription and callbacks. Keep inline project and preview panels.

Mods: one compact horizontal list, 44px cached artwork, two-line summary, author
and authoritative download/category facts, aligned visible Install/Details actions.
Modpacks: responsive grid of smaller artwork-led cards, fixed-ratio artwork frame
with enlarged existing project icon (no invented banners), title/author/summary,
explicit Review pack action and truthful New instance label. Exact compatibility
stays in details/preview because search hits do not provide it.
Resource packs: medium artwork cards with 64px icons and aligned install/details.

Extract only the two-button Installed/Browse view switch and visual library CSS;
they have identical semantics. Keep provider/install flows in their current owners.
Add a presentation-only size input to InstalledArtwork without touching resolution.
Library rows retain confirmations/capability checks, show truthful version/source,
counts and text enabled state, and align 40px action targets. Pack descriptions and
warnings are bounded with full details available. No new persisted state/dependency.
Use existing Borealis tokens with localized backing, no per-row backdrop filter.

No Note/Lunar screenshots were supplied with this attachment. Apply the written
density/artwork principles; do not claim inspection of unavailable references.

## 6. Preservation risks

Category/search sequencing, quick-install duplicate exclusion, dependency-only Keep,
exact pack preview fingerprint/optional selection and host instance callbacks must
remain intact. Do not use an installed flag to authorize operations. No Rust, DTO,
package, lockfile, min-window, production identity or release edits are planned.
All old screenshot/test artifacts must retain hashes, so regression scripts use
UI-3-specific output overrides. Never stage the four original untracked files.
Native tests use a new debug-only managed root and WebView profile, no accounts or
owner content. An incomplete synthetic registry must never be called Ready.

## 7. Test strategy

Capture deterministic before states with actual Svelte components, blocking public
browser requests. Add fixture tests for explicit search/clear/categories/sort/page,
install states, duplicate clicks, exact pack review, local controls and confirmation,
empty/loading/error/retry, long text, focus, accents, reduced motion/transparency,
200% text and 1440x900/1280x800/1024x768/720x520. Run npm check, serial full frontend
suite, UI-1/UI-2/UI-2B, content/large-list/artwork regressions, and production frontend
plus no-bundle debug native build. No native source change means no new Rust tests
are required. Record exact command exit codes/counts. Compare matched captures.

Actual compiled Tauri acceptance must capture startup, Browse kinds, local empty
libraries, medium/minimum, alternative accent and clean exit with measured clients.
Populated native library cannot be claimed without installed fixtures; labeled browser
fixtures cover it. Native providers may be unavailable for an incomplete fixture;
report limitations instead of fabricating data. Owner visual review remains required.

## 8. Implementation order

1. Baseline/audit and matched before captures.
2. Shared controls and Mods pilot; check/capture before continuing.
3. Artwork-led modpack grid; exact review flow tests.
4. Resource-pack presentation and responsive tests.
5. Installed Library visual consistency and focus preservation.
6. Full regression, native boot/captures, comparisons, preservation and report.

Use focused local commits at these boundaries, adapting shared-source overlap.
Stop after UI-3; no release or UI-4 work.
