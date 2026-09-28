# Phase E2 acceptance

## Implemented behavior

Home registers `playtime`, `recent-worlds`, and `recent-servers` with small, wide, and large sizes. A fresh default layout enables all three after the original entries; a persisted schema-4 layout is read as saved, so existing ordering and visibility do not change. Settings can add an absent ID, show or hide it, and reset to the current defaults. No schema migration is necessary because the existing layout preserves unknown IDs and permits new IDs to be added.

Playtime reads native `get_playtime_summary` and `get_daily_playtime` queries. The seven and thirty day charts show chronological UTC buckets, including zero days. All Time shows the retained aggregate without inventing older chart detail. Values are rounded to readable minutes or hours and minutes. The total and per-day labels are available to assistive technology. No playtime is synthesized in Svelte.

Recent Worlds and Recent Servers use the native E1 bounded history queries, ordered by last visit. Small shows three entries; wide and large show up to five. Labels, owning instance, relative recency, and an unavailable state are shown. Long labels ellipsize and retain full title text. Server addresses remain in native history and are absent from the widget DTO. Launcher-local history use is separate from Discord consent. No server probing occurs during Home rendering.

## Quick Launch trust boundary

The frontend passes only an opaque history ID, target kind, and selected account ID to `quick_play_history`. Native code looks up the ID in launcher-owned history, extracts the exact owning instance, and never consults the selected instance for target ownership. It does not change persistent selection. The Home page shows a separate running status when Quick Launch starts a different instance.

The world identity is parsed again as a single safe save component. The target must be a real directory with a real `level.dat` beneath the owning instance's `saves/`; both the saves root and target are canonically checked for containment. Missing or linked worlds are unavailable and fail with a structured error. A renamed world display title leaves the stable save identity intact. Invalid, removed, or stale history IDs fail rather than selecting another world.

The server target is parsed again as a canonical Minecraft host and port. The UI cannot pass an address, path, executable, JVM option, or game argument. Offline servers are left for Minecraft's normal connection behavior. A removed instance fails before launch.

Both target kinds call the ordinary Play pipeline, including the preparation guard, deep instance and content validation, mod compatibility, exact managed Java validation, session restoration, normal `LaunchSpec` assembly, and process supervision. Quick Launch explicitly rejects pinned Minecraft versions other than 1.21.11, the version whose direct-start behavior E1 verified. Only its reviewed `--quickPlaySingleplayer` or `--quickPlayMultiplayer` value plus `--quickPlayPath` is appended. The target value is marked sensitive for process diagnostic redaction. The quick-play log path is relative to the validated instance's `logs/` directory. The normal process-local same-instance duplicate guard applies. Frontend busy state also suppresses repeated clicks.

## Verification and visual acceptance

Automated checks cover opaque target lookup, mode separation, malformed IDs, canonical server values, safe world identity, missing world state, daily UTC buckets, LaunchSpec preservation and redaction, widget registration and layout behavior, readable durations, rendered populated/empty widget markup, row bounds, unavailable controls, long labels, and the exact native Quick Launch DTO. Existing Play/Discord/privacy regression suites still run. Full test and build counts are recorded in the final report.

Native Windows screenshot control is unavailable in this session. **Visual acceptance is pending.** No screenshots are claimed. To complete it, boot `npm run tauri dev` with disposable E1 history fixtures in managed app data and inspect: populated, compact, and empty Playtime; populated and empty Recent Worlds and Servers; all three together; minimum supported, normal, and wide windows; long names and duplicate instance names; unavailable world; hover and keyboard focus. Capture ten actual app screenshots for those states and pair them with DOM/state assertions for totals, row counts, disabled controls, and range switching. Confirm no clipping or horizontal overflow in compact layouts before marking visual acceptance passed.

Use a disposable Windows user profile for fixture data; do not replace a real user's launcher data. Create E1 history through the native test store with a seven-day daily sequence, more than five world and server targets, one deleted world, one deleted instance, duplicate instance display names, and long names. Include one zero-history fixture. With the actual Tauri dev window:

1. At 920×640, capture populated Playtime in 7D and switch to 30D and All; assert the text totals and UTC day count against the fixture.
2. Resize to 720×520 and capture compact Playtime; assert seven distinct bars, labels, no horizontal scroll, and no clipped controls.
3. Load zero-history data and capture the Playtime empty state.
4. Restore populated data at 920×640 and capture Recent Worlds with three rows; assert recency order, owning instance labels, disabled missing world, and an ellipsized long title with full title tooltip.
5. Load zero-history data and capture Recent Worlds empty.
6. Restore populated data and capture Recent Servers with three rows; assert no raw address is displayed, recency order, and the Quick Launch control.
7. Load zero-history data and capture Recent Servers empty.
8. Restore populated data, show all three widgets, and capture the combined Home layout with the original Play area still usable.
9. Capture the combined layout at 720×520 and inspect scroll, focus outlines, hover, disabled state, and long names.
10. Resize to at least 1440×900, expand world/server widgets to five rows, and capture the wide layout. Assert row counts and keyboard focus order.

The fixture must remain confined to the disposable profile. Quick Launch interaction must use a test account/session and a disposable world or server; verify its owning instance launches without changing saved selection and that repeated clicks do not start a second process. Do not mark visual acceptance complete until all ten screenshots and matching assertions are recorded.

## Limits

Production Aurora Client 2.1.3 remains bridge v1. A reviewed, pinned v2 client artifact is required before real production world and server identities can populate these widgets. This phase does not change the client release, launcher release 1.1.0, or production client pin. No E3 skin or cape work was started.
