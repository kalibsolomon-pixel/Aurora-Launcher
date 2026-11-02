# Phase H.1 acceptance: recent server enrichment

Focused post-Phase-H pass. The Home → Recent Servers widget renders real Minecraft server presentation (favicon, name, MOTD, player counts, online/offline) for servers already in local gameplay history. History, Quick Launch authority, Phase F visuals and every frozen Phase H surface are unchanged.

## What shipped

- `src-tauri/src/server_enrichment/` — a new launcher domain:
  - `protocol.rs`: bounded Minecraft Java status ping (VarInt framing, handshake with `-1` protocol and Status next-state), SRV resolution through the Windows system resolver (`DnsQuery_W`), strict malformed-packet handling.
  - `motd.rs`: hostile MOTD conversion into bounded, validated presentation segments; legacy `§` codes supported; control/bidi characters stripped by the shared `activity_bridge::is_presentable_char` predicate.
  - `mod.rs`: endpoint normalization (canonical `host:port` → SHA-256 cache key), favicon validation (PNG fully decoded ≤ 512², JPEG magic, ≤ 128 KiB, `data:` URLs only), the schema-1 presentation cache with TTL (10 min) and failure backoff (60 s → 30 min cap), bounded-concurrency refresh (4), per-endpoint dedup, and the presentation DTO.
- `refresh_recent_server_status` command: takes opaque recent-target IDs, resolves endpoints natively from history, returns display-only presentations. New error code `server_enrichment_invalid` for oversized requests.
- Widget: `RecentTargetsWidget.svelte` server rows render favicon (letter-tile fallback), resolved name, clamped single-line MOTD with validated colors, instance/last-played/player-count meta and the unchanged Quick Launch button; `serverPresentation.ts` owns pure merge/label logic. World rows are untouched.
- Tests: 26 Rust tests across protocol/motd/service plus 7 frontend widget/authority tests; one gated `#[ignore]` live SRV+status diagnostic.

## Deterministic coverage (protocol-level guarantees)

Normalized identity and default port; malformed endpoint rejection; bounded payload; malformed VarInt/packet id/length/trailing bytes; malformed JSON; valid and hostile MOTD conversion (depth/width/segment/color bounds, `§` handling, control/bidi stripping); valid, malformed, oversized and non-image favicons; cache key isolation, persistence, damaged/future-schema/wrong-identity documents; TTL fresh (no network) and stale (one bounded refresh) behavior; timeout retaining cached presentation plus backoff bookkeeping; one failing server not failing neighbors; duplicate endpoints and duplicate IDs queried once; bounded concurrency (peak ≤ limit); existing-history enrichment without a new visit; presentation never influencing `quick_target`; offline history fallback. Frontend: hostile MOTD renders as escaped text (no markup), one-line clamp, world mode unchanged, Quick Launch continues to send only the opaque target ID/kind/account.

## Live acceptance (real app, real servers, owner's existing history)

The owner's persisted history contained two multiplayer visits — `minemen.club:25565` and `pvphq.com:25565`, both stored with the vanilla default display name "Minecraft Server". Both are real public servers reachable only through SRV records (`geo.minemen.club`, `play.pvphq.com`), which the enrichment resolved through the Windows system resolver.

1. **Existing history enriched without a new session.** With the real app running and no Minecraft launch, both entries acquired favicons, MOTDs, player counts and status purely from the persisted `ServerTarget`. The history document was never written (9 sessions before and after; mtime unchanged from the owner's last real play).
2. **Favicon persistence.** A full app restart re-rendered both favicons with `savedAt` unchanged in the cache — proving the fresh-TTL path served purely from disk with zero network traffic.
3. **MOTD safety.** minemen's legacy `§`-coded MOTD and pvphq's deeply nested colored component tree both rendered as clamped single-line styled text; no markup interpretation.
4. **Offline retention.** Covered deterministically (`favicon_round_trips_through_refresh_and_survives_offline`: expired TTL + unreachable listener → cached favicon/MOTD retained, offline reported, backoff recorded). A live no-cache failure state was also observed during development (before a handshake framing fix) and degraded to name-only rows without breaking the widget.
5. **Multiple entries.** Both entries enriched independently through separate bounded queries; a third dead endpoint (deterministic test) degrades alone.
6. **Rejoin intact.** Both Quick Launch buttons remained present and enabled with enrichment live; the launch path still receives only the opaque target ID.
7. **Responsive.** Captured at maximized (~2560×1440), 1440×900, 1100×720 and the smallest supported 720×520: favicons legible, text clamped, rows restrained, Play reachable, no collisions.
8. **No idle work.** No timers, watchers, polling or startup scans exist in the feature; refreshes occur only on widget mount under TTL/backoff gates. Borealis behavior untouched.

Evidence: `.zcode-diag/phase-h1-server-enrichment/` (01 before, 02/03 enriched, 04 multiple + restart-cached, 05–08 responsive sizes, 09 rejoin state).

## Privacy

Status queries go only to the stored server endpoints via the system resolver. Nothing is sent to Aurora infrastructure, Modrinth, or any third party; Discord preferences are untouched; no telemetry.

## Verification

`npm run check` (0 errors/0 warnings), `npm test` (178 pass), `npm run build`, `cargo fmt --check`, `cargo check --all-targets`, `cargo test` (686 pass, 26 ignored gated live tests), `npm run tauri build` (MSI + NSIS bundles produced).
