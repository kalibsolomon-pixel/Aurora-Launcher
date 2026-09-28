# Phase E2 acceptance

## Implemented behavior

Home registers `playtime`, `recent-worlds`, and `recent-servers` with small, wide, and large sizes. A fresh default layout enables all three after the original entries; a persisted schema-4 layout is read as saved, so existing ordering and visibility do not change. Settings can add an absent ID, show or hide it, and reset to the current defaults. No schema migration is necessary because the existing layout preserves unknown IDs and permits new IDs to be added.

Playtime reads native `get_playtime_summary` and `get_daily_playtime` queries. The seven and thirty day charts show chronological UTC buckets, including zero days. All Time shows the retained aggregate without inventing older chart detail. Values are rounded to readable minutes or hours and minutes. The total and per-day labels are available to assistive technology. No playtime is synthesized in Svelte.

Recent Worlds and Recent Servers use the native E1 bounded history queries, ordered by last visit. Small shows three entries; wide and large show up to five. Labels, owning instance, relative recency, and an unavailable state are shown. Long labels ellipsize and retain full title text. Server addresses remain in native history and are absent from the widget DTO. Launcher-local history use is separate from Discord consent. No server probing occurs during Home rendering.

## Quick Launch trust boundary

The frontend passes only an opaque history ID, target kind, and selected account ID to `quick_play_history`. Native code looks up the ID in launcher-owned history, extracts the exact owning instance, and never consults the selected instance for target ownership. It does not change persistent selection. The Home page shows a separate running status when Quick Launch starts a different instance.

The world identity is parsed again as a single safe save component. The target must be a real directory with a real `level.dat` beneath the owning instance's `saves/`; both the saves root and target are canonically checked for containment. Missing or linked worlds are unavailable and fail with a structured error. A renamed world display title leaves the stable save identity intact. Invalid, removed, or stale history IDs fail rather than selecting another world.

The server target is parsed again as a canonical Minecraft host and port. The UI cannot pass an address, path, executable, JVM option, or game argument. Offline servers are left for Minecraft's normal connection behavior. A removed instance fails before launch.

Both target kinds call the ordinary Play pipeline, including the preparation guard, deep instance and content validation, mod compatibility, exact managed Java validation, session restoration, normal `LaunchSpec` assembly, and process supervision. Only the reviewed Minecraft 1.21.11 `--quickPlaySingleplayer` or `--quickPlayMultiplayer` value plus `--quickPlayPath` is appended. The target value is marked sensitive for process diagnostic redaction. The quick-play log path is relative to the validated instance's `logs/` directory. The normal process-local same-instance duplicate guard applies. Frontend busy state also suppresses repeated clicks.

## Verification and visual acceptance

Automated checks cover opaque target lookup, mode separation, malformed IDs, canonical server values, safe world identity, missing world state, daily UTC buckets, LaunchSpec preservation and redaction, widget registration and layout behavior, readable durations, rendered populated/empty widget markup, row bounds, unavailable controls, long labels, and the exact native Quick Launch DTO. Existing Play/Discord/privacy regression suites still run. Full test and build counts are recorded in the final report.

Native Windows screenshot control is unavailable in this session. **Visual acceptance is pending.** No screenshots are claimed. To complete it, boot `npm run tauri dev` with disposable E1 history fixtures in managed app data and inspect: populated, compact, and empty Playtime; populated and empty Recent Worlds and Servers; all three together; minimum supported, normal, and wide windows; long names and duplicate instance names; unavailable world; hover and keyboard focus. Capture ten actual app screenshots for those states and pair them with DOM/state assertions for totals, row counts, disabled controls, and range switching. Confirm no clipping or horizontal overflow in compact layouts before marking visual acceptance passed.

## Limits

Production Aurora Client 2.1.3 remains bridge v1. A reviewed, pinned v2 client artifact is required before real production world and server identities can populate these widgets. This phase does not change the client release, launcher release 1.1.0, or production client pin. No E3 skin or cape work was started.
