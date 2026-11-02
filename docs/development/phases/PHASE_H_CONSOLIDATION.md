# Phase H / H.1 Consolidation — Main Integration Acceptance Record

Date: 2026-10-01
Operator: consolidation checkpoint (automated review)

## Verdict

READY — the accumulated Phase H / Phase H.1 branch was accepted as the new
local development baseline. Phase I does not start here; this record only
documents the integration.

## Repository positions

| Reference | SHA |
| --- | --- |
| Starting `origin/main` / local `main` (merge-base) | `20d7d5cdc9805c1cdbcd35217021355c1a4c745f` |
| Reviewed correction-branch HEAD before consolidation | `17cb5817b4c8c5d6ceb52ab21465a53aacb5d7dc` |
| Consolidation correction commit | `f785585` (comment-only: ModrinthBrowse kind-seeding doc alignment) |
| Final local `main` HEAD | `main @ f785585` (fast-forward, all 23 commits preserved) |
| `origin/main` after integration | `20d7d5c` — NOT pushed, unchanged |

- Integration method: fast-forward of local `main` to the branch HEAD
  (merge-base equals `origin/main`; zero divergence; no squash, no rebase,
  no history rewrite).
- Reviewed range: `20d7d5c..17cb581` = 22 commits (Phase G provenance
  foundation through the accent swatch fix) plus the one consolidation
  correction commit.
- Working tree: clean of tracked modifications; protected untracked
  diagnostics (`.zcode-diag*`, `.zcode-perf/`, `.zcode-icon-diag.py`,
  `.zcodeignore`, Temp evidence) preserved untouched.
- Push status: NOTHING pushed. No tags, no releases, no version bump.
  Production 1.3.1 and all historical releases untouched.

## Integrated scope

- Phase G content identity and provenance foundation (`content_recognition`).
- Phase H managed Modrinth updates: discovery, release-channel policy,
  pinning, preview/apply, single combined multi-root transaction (Update All
  is one transaction, not per-item iteration), dependency-graph handling,
  rollback.
- Phase H corrective UX: compact installed-content rows, Details expansion,
  actionable updates, Update All, provider browsing with provider-derived
  categories and sorts, browse-only modpacks, repaired shader installs,
  custom accent, Home skin presentation.
- Installed-content reliability: persisted artwork, UUID-keyed account
  avatars, launcher-level Skin Library, Home player corrections (no drag
  rotation, arrow-direction fix, always-visible controls).
- Interface corrections: instances-first hierarchy, repaired category
  popover, responsive player controls.
- Resource-pack/shader management: backend-owned capability DTOs,
  options.txt `resourcePacks` activation (byte-exact splice, quoted 1.21+
  entries, CRLF preservation), local/provider removal, recovered/adopted
  content, truthful in-game shader activation, enabled-reference migration
  across renaming updates.
- Phase H.1 Recent Server enrichment: presentation-only domain, Java status
  protocol (bounded reads), Windows SRV resolution, TTL/backoff-cached
  favicon/MOTD with hostile-data normalization, enriched Recent Servers UI;
  gameplay history remains the sole rejoin authority.
- Final accent correction: circular selected swatch ring; `:focus-visible`
  keyboard distinction; mouse clicks produce no rectangular pseudo-selection.

## Integration audit summary

All areas PASS with no blocking findings:

- Content lifecycle: provider-managed / direct / dependency / recovered /
  local categories consistent end to end; capabilities backend-owned
  (`ContentManagement` DTOs); no frontend capability guessing.
- Managed updates: PLAN → STAGE → VERIFY → REVALIDATE → ACTIVATE → VALIDATE
  → COMMIT intact; Update All is one combined multi-root transaction with
  full rollback; enabled pack references migrate across renames.
- Persistence: `content-managed.json` schema 4 (v1–v3 migrations additive
  and conservative; unknown versions fail deliberately); config, registry,
  gameplay history, skin presets unchanged and fail-closed; avatar/artwork/
  enrichment caches damage-tolerant by design; no fabricated provenance or
  timestamps; no secrets in JSON.
- Accounts/skins: avatar cache UUID-keyed and restart-persistent; removal
  retires exactly one account's cache; Skin Library launcher-level with
  managed copies (no retained source paths); Home never substitutes a
  generic skin.
- Provider browsing: structured provider-neutral queries only; stale-response
  guards intact; modpacks browse-only — zero `.mrpack` functional code
  (two documentation mentions only).
- Player preview: no drag/pointer-move paths; arrows rotate in their
  direction; Reset restores the canonical angle; controls usable at all
  supported sizes; hides below the 900px breakpoint.
- Server enrichment: presentation-only; history authoritative; no
  gameplay-history mutation; bounded TTL/backoff network work; hostile
  MOTD/favicon limits enforced.
- Appearance: circular ring verified at pixel level in the native app;
  Custom picker functional; persistence unchanged.

## Consolidation corrections

1. `f785585` — corrected a stale comment in
   `src/lib/instances/ModrinthBrowse.svelte` that described a nonexistent
   `$effect` re-sync of `browseKind`. Comment-only; no behavior change.

No other corrections were required. No speculative cleanup was performed.

## Verification results (final branch state)

| Check | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| `npm test` | 178/178 pass |
| `npm run build` | pass |
| `cargo fmt --all -- --check` | pass |
| `cargo check --all-targets` | pass |
| `cargo test` | 692 passed, 0 failed, 26 ignored (live-network diagnostics, excluded by policy) |
| `npm run tauri build` | pass (MSI + NSIS bundles) |

The historical cache-parallelism flake did not appear.

## Native smoke test (Tauri dev, real backend)

- HOME: logo/Play hierarchy, Ready instance card (1.21.11 · Fabric 0.19.5 ·
  Aurora 2.1.5), real player skin with left/right/reset controls exercised,
  enriched Recent Servers (live MOTD + player counts for pvphq.com and
  minemen.club), Playtime widget, account card with persisted avatar.
- INSTANCES: existing-instance-first hierarchy with Create below.
- CONTENT: Mods installed (31 entries, per-row Details/Remove/Disable,
  dependency/conflict metadata); Resource Packs installed (honest
  enable/disable per activation state, managed provenance rows); Shaders
  honest empty state with in-game-loader note; Browse Modrinth live against
  the real provider (real projects/downloads, installed badges, install
  actions, dependency-only "keep" actions); category popover verified
  (single column, no horizontal overflow). No "Unavailable" regression.
- SETTINGS: Appearance complete (theme trio, Borealis background, motion
  slider, accent presets + Custom); circular selected ring verified at 4×
  zoom; owner accent (Aurora violet) restored after the check.
- No destructive actions; the owner's Minecraft skin and content were not
  modified.

## Performance / background-work audit

No regressions: no provider/update polling, no per-mod timers, no
server-status polling (single TTL-gated enrichment on widget load), no skin
animation loop, no pointer-move rendering, no unexpected startup network
(bundled release manifest, cache-first avatars), no minimized-Borealis
regression (CSS byte-identical since `a678b32`; event-driven pause intact).

## Known deferrals (carried forward)

1. Phase I: provider-aware Modrinth `.mrpack` installation.
2. Phase J: modpack update/reconciliation.
3. Linux compatibility: non-Windows Minecraft SRV resolution for Recent
   Server Enrichment is required before Linux production acceptance
   (currently literal-endpoint fallback only, documented in
   `src-tauri/src/server_enrichment/protocol.rs`).
4. Shader activation remains in-game-managed until Aurora deliberately owns
   supported loader configuration.
5. User-facing historical rollback: still deferred.
6. Minor (documented here, no action): a pinned record that later becomes a
   non-retained dependency (pin → remove-with-surviving-dependents) is not
   honored by the supersede check in `updated_state_multi`, which gates on
   `explicitly_retained` only. Not reachable through the UI pin control;
   candidate hardening for a future phase.
7. Minor (cache-only): server-enrichment presentation cache and
   artwork/avatar caches have per-object bounds but no directory-wide
   eviction of stale objects; degrade to verified re-fetch by design.
