# Installed artwork reliability correction

Local correction and Windows acceptance, 2026-10-08. Launcher version remains
1.4.1. No push, publication, update-authority change, installer replacement or
game launch is part of this work.

## Confirmed causes

1. `src-tauri/src/cosmetic_image.rs:77` accepted static PNG only. The existing
   project cache contained 38 objects: 27 WebP, 10 PNG and one JPEG. Valid WebP
   objects were consequently rejected on every read. This reproduced the
   reported baseline: Continuity displayed its PNG while numerous recognized
   provider mods displayed cubes. The supplied screenshot itself was not
   attached to this conversation; the installed launcher provided the captured
   failing baseline.
2. `src/lib/instances/projectArtwork.ts` and the installed panels previously
   supplied project IDs only for `providerManaged` entries. Locally imported
   exact matches and verified bootstrap artifacts lacked an automatic cosmetic
   identity path. Resource Packs and Shaders shared that limitation.
3. `src-tauri/src/artwork.rs` previously returned `None` to concurrent callers
   when an acquisition was already active, rather than sharing its completion.
4. `InstalledArtwork.svelte` previously resolved once. A temporary failure left
   mounted rows showing a fallback, without retry or completion refresh.

## Complete pipeline and implemented behavior

The native inventory scanner validates the instance and direct content directory,
rejects links/reparse points and verifies persisted provider records against
installed bytes. Modified provider files receive no stale provider identity.

`src-tauri/src/installed_artwork.rs:122` prefers that verified provider provenance.
For missing provenance, a bounded scan hashes regular JAR/disabled-JAR/ZIP files
with SHA-512 and asks the existing official Modrinth version-file endpoint for
an exact match. Returned file hashes and project type must agree. Names, filenames,
declared mod IDs and fuzzy search never select a provider identity. Local files
are not adopted, rewritten, enabled, removed or given management rights.

Cosmetic identities persist as bounded schema-1 receipts under
`cache/artwork/identities/<content-type>/<sha512>.json`. A validated positive
receipt survives restart; an authoritative no-match expires after one hour.
Changed bytes select a different receipt. Malformed, future-dated, unsupported
or mismatched receipts are ignored and reconstructed. Transport/lookup failures
are not persisted and receive a one-minute process-local retry deadline. The
scan considers at most 256 candidates, each at most 512 MiB. Directories,
unrecognized file formats and damaged/unknown provider entries remain legitimate
local entries with generic artwork where no verified source exists.

Aurora Client's first-party identity requires the installed bootstrap artifact's
verified SHA-256 to match its exact release declaration. Its image comes from the
existing bundled 128px Aurora application mark, independently SHA-256 pinned in
`src-tauri/src/artwork.rs:219`. A name or metadata declaration alone cannot select
that artwork.

`src-tauri/src/artwork.rs:281` obtains the icon URL from the native official
provider project document. Acquisition is HTTPS, restricted to the official
Modrinth CDN, refuses redirects and has a ten-second timeout. The frontend
supplies only a project ID; it never supplies a URL or loads a provider image
directly. The CDN was removed from the frontend image CSP.

PNG validation remains unchanged. Static lossy/lossless/alpha WebP now passes
bounded RIFF and encoded-dimension checks before full decoding. Animated WebP,
animated PNG, JPEG, SVG and malformed images remain rejected. The existing
512 KiB input/output, 1024×1024, 1,048,576-pixel and 16 MiB limits remain in force.
Every accepted image becomes a fully decoded, metadata-free, bounded static PNG.
WebP sources are therefore **supported and saved as canonical PNG pixels**,
including one-time conversion of valid legacy WebP cache objects.

The image-webp decoder's advertised memory limit does not cover all lossless
allocations. Production WebP decoding consequently uses a disposable byte-only
child of the same executable, before Tauri initialization. It receives no URLs,
paths, credentials or inherited application environment. Windows enforces a
16 MiB process-memory Job Object; Linux uses address-space/data limits. The parent
bounds input/output, supervises only its exact child with a ten-second deadline,
and revalidates the returned PNG. Other platforms deliberately reject WebP until
equivalent allocation enforcement exists. Server favicons retain PNG-only rules.

The shared project cache is `cache/artwork/modrinth/<project-id>.img`; its contents
are canonical PNG after successful validation. Reads always decode/revalidate
bytes instead of trusting names/extensions. Invalid entries remain absent until
a full successful acquisition atomically replaces them. A corrupt cache object
is never shown merely because its filename matches a project.

Per-project native locks share the completed acquisition; at most four network
acquisitions run together. Typed results distinguish `available`, `unavailable`
and `retryable`. Missing/unsupported artwork has a ten-minute in-process negative
window; transient errors have a one-minute window. Neither is a permanent disk
failure cache. Valid positive images remain reusable across navigation, restarts
and offline startup. Provider logo freshness has no automatic revalidation TTL.

Browse and installed views use the same `InstalledArtwork` component and native
validated cache. Shared frontend caches hold at most 128 promises, coalesce
in-flight work and expire completed results after ten minutes or the native retry
deadline. Mounted consumers retry transient failures and update reactively when
identity/artwork arrives. Unmount/identity changes cancel timers and ignore stale
completion; keyed image handlers cannot attribute an obsolete load/error event
to a replacement source. Generic artwork stays visible until actual image decode
succeeds. The installed interface layout is unchanged.

## Verification

| Check | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| `npm test` | 229 passed, 0 failed, 16 suites |
| Rust format/check, all targets | Passed; four existing test warnings remain |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 841 library and six icon tests passed; 29 existing ignored tests |
| Production SPA and NSIS build | Passed |
| Debug and release WebP workers on Windows | Each decoded 27/27 real cached WebPs; each rejected oversized, truncated and animated samples |

Deterministic native tests use loopback servers, never public network. They cover
verified provider precedence, damaged-file rejection, renamed local exact matches
for Mods/Resource Packs/Shaders, no-match preservation, cross-type rejection,
receipt restart reuse without requests, expired no-match, corrupt receipt repair,
failed lookup followed by successful retry, coalesced acquisition, corrupt image
replacement, cache reuse with acquisition forbidden, WebP persistence and the
original PNG/security limits. Frontend tests cover shared promises, retry expiry,
unavailable versus transient results and rejected-promise recovery.

Evidence is outside Git at
`C:/Users/kalib/AppData/Local/Temp/aurora-artwork-2a9becf5-1240-4751-8e0f-b7270e4a7e4a`.

## Packaged runtime acceptance and artwork coverage

The NSIS build's actual optimized executable payload was copied into that
disposable evidence directory and booted against the owner's existing ready
Aurora Client 3.0.0 instance. The installed executable was not replaced, and the
installer was not run. Native readiness and the real embedded production SPA
booted successfully. Screenshots were captured from the real packaged WebView2
rendering through its loopback diagnostic port; no screenshot or content UI was
fabricated. Every row was scrolled into view to complete lazy decoding before
coverage was counted, then the views were returned to the top for screenshots.

The base Tauri configuration, without the build-provided updater object, failed
at plugin initialization. As documented in `UPDATE_AUTHORITY.md`, local acceptance
used an external temporary config providing an empty updater public key and no
compiled `AURORA_UPDATER_PUBKEY`. This is an unsigned local packaged boot with
updates unavailable, not signed release/update acceptance. No signing material,
production authority, repository updater configuration or version was modified.

| Installed view | Entries | Verified identities | Verified usable artwork | Baseline displayed | Corrected displayed | Restart displayed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Mods | 31 | 31 | 30 | 6 | 30 | 30 |
| Resource Packs | 3 | 1 | 1 | 0 | 1 | 1 |
| Shaders | 1 | 1 | 1 | 0 | 1 | 1 |
| Total | 35 | 33 | 32 | 6 | 32 | 32 |

All **32 of 32 entries with verified usable artwork display it correctly**.
BadOptimizations has a verified project identity but the official project response
has `icon_url: null`; its fallback is appropriate. The PETE UHC ZIP received no
exact provider match, and Smooth Vanilla V2 is a local folder without a provider
identity. Neither was assigned an invented icon or identity. Aurora Client now
uses its verified first-party mark, and bootstrap Fabric API resolves by exact
file hash. `native-audit.json` records every entry/project/outcome; provider
no-icon evidence is `badoptimizations-source.json`.

| View | Before | After |
| --- | --- | --- |
| Mods | [Screenshot](C:/Users/kalib/AppData/Local/Temp/aurora-artwork-2a9becf5-1240-4751-8e0f-b7270e4a7e4a/mods-before.png) | [Screenshot](C:/Users/kalib/AppData/Local/Temp/aurora-artwork-2a9becf5-1240-4751-8e0f-b7270e4a7e4a/mods-after.png) |
| Resource Packs | [Screenshot](C:/Users/kalib/AppData/Local/Temp/aurora-artwork-2a9becf5-1240-4751-8e0f-b7270e4a7e4a/resourcepacks-before.png) | [Screenshot](C:/Users/kalib/AppData/Local/Temp/aurora-artwork-2a9becf5-1240-4751-8e0f-b7270e4a7e4a/resourcepacks-after.png) |
| Shaders | [Screenshot](C:/Users/kalib/AppData/Local/Temp/aurora-artwork-2a9becf5-1240-4751-8e0f-b7270e4a7e4a/shaders-before.png) | [Screenshot](C:/Users/kalib/AppData/Local/Temp/aurora-artwork-2a9becf5-1240-4751-8e0f-b7270e4a7e4a/shaders-after.png) |

Packaged asynchronous acceptance temporarily injected one typed `retryable`
response for Continuity at the cosmetic IPC transport, with a shortened 1,200 ms
retry deadline. The next response came from the actual native validated cache.
The row initially displayed its fallback, retried 1,217 ms after the first request,
then automatically decoded/displayed the real PNG without navigation or Refresh.
The hook was restored; identity, image bytes and persistence were not mocked.
`async-refresh.json` records both calls and the before/after decoded state.

Returning Mods → Overview → Mods kept the total artwork/identity command count
at 32; it added zero artwork calls. Browse's default page contained 12 already
resolved installed project IDs among its 20 hits. Those reused shared results;
a subsequent Browse search added zero artwork calls. `navigation-first.json`,
`navigation-repeat.json` and `browse-reuse-summary.json` record this evidence.
The validated project store is the same native store in both views.

The packaged process was closed and restarted, and all three installed views
retained the coverage shown above. A separate packaged launch with a deliberately
blocked network proxy was rejected by automatic approval review as “blocked by
policy,” without a more detailed reason. No alternative traffic interception or
system network change was attempted. Offline reuse is verified by deterministic
native tests: acquisition panics if called on an image cache hit, and the identity
test checks that a now-failing provider receives zero additional requests across
restart. A fully network-disconnected packaged startup is therefore **not claimed**.

## Remaining limitations

- No image is invented for missing icons or unmatched local files/folders.
  JPEG, SVG and animation remain unsupported by design; over-limit static images
  remain rejected. WebP persists as normalized PNG, not retained original WebP.
- Windows WebP isolation and packaged rendering were exercised live. Linux's
  allocation enforcement was implemented but not runtime-tested on this host;
  other platforms currently reject WebP safely.
- Valid positive provider identity/artwork has no automatic freshness TTL or
  garbage collector. Changed installed bytes invalidate identity through the
  hash-keyed receipt; corrupt cached bytes cause reacquisition. A provider's
  changed logo requires cache invalidation rather than ordinary navigation polling.
- Locally imported exact/no-match cases and corrupt-cache recovery were tested in
  disposable loopback fixtures, not by altering the owner's installed mods.
- This correction has not been installed over the owner's executable, signed,
  pushed or published. The local packaged acceptance build has updates disabled.
- A physically/offline-network packaged boot remains unverified for the policy
  reason above; the native cache path is tested with network acquisition forbidden.

## Files changed

- `src-tauri/src/installed_artwork.rs` (new): cosmetic exact-file identities and receipts.
- `src-tauri/src/artwork.rs`: validated persistence, typed outcomes and coalescing.
- `src-tauri/src/cosmetic_image.rs`: bounded static WebP and isolated worker.
- `src-tauri/src/application.rs`, `lib.rs`, `main.rs`: narrow commands and worker entry.
- `src-tauri/Cargo.toml`, `Cargo.lock`: image-webp and platform memory-limit APIs.
- `src-tauri/tauri.conf.json`: image CSP restricted to local/native sources.
- `src/lib/backend.ts`: synchronized artwork/identity DTOs and commands.
- `src/lib/instances/installedArtwork.ts` (new), `projectArtwork.ts`,
  `projectArtwork.test.ts`: shared bounded resolution caches and tests.
- `src/lib/instances/InstalledArtwork.svelte`, `InstanceModsPanel.svelte`,
  `InstancePacksPanel.svelte`, `src/lib/shell/Artwork.svelte`: reactive resolution,
  retry, stale-result protection and decode-safe refresh.
- `ARCHITECTURE.md`, this report: implemented behavior, evidence and limitations.

## Repository protection

Baseline HEAD: `420f54c0ad48ec9aa70320786a77d678b51864f1`.
Baseline origin/main: `4575a0e3f1c28da6cfe072948fe91b60eeaf608f` (ahead one,
behind zero). The 393 tracked-file baseline and hashes for all 15,143 pre-existing
untracked files were recorded before editing. All pre-existing untracked hashes
match and all tracked files remain present. All 2,735 protected instance-file
hashes also match. No broad cleanup or deletion occurred. Generated outputs and
acceptance data remain unstaged. The final commit is the focused, independently
revertible commit containing this report; its SHA and final status are reported
in the task's final response.
