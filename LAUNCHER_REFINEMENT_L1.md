# Launcher refinement L1

## Result and source

**COMPLETE — local source and acceptance only.** Artwork, shared browsing controls,
Modpacks spacing and download glyphs are corrected. No publication, push, installer
replacement, owner-instance mutation or L2/L3 work was performed.

Investigation date: 2026-10-09 local (acceptance crossed midnight UTC).
Source branch: `codex/client-3-production-authority`.
Starting HEAD: `8df387bd682d7ab6c41206affcf68ba38c51dd0d`.
Prior shared artwork fix: `fad40125f8fd87b8fa0893c97827e8e4a890081b`.
Repository location, branch, remotes, history, complete status, tracked/untracked
sets and protected-file hashes were recorded before edits. `AGENTS.md`,
`ARCHITECTURE.md` and `INSTALLED_ARTWORK_RELIABILITY.md` informed this work.

## Artwork diagnosis

The owner's initial screenshots were the visual reference: 194650 Resource Packs,
194658 Shaders, 194712 Modpacks and 195045 Mods on 2026-10-09. Home and instance
Settings screenshots were references for later phases and were not changed.

There were two separate findings:

1. **Installed executable/source mismatch.** The installed executable lacks both
   the isolated WebP decoder marker and the shared `resolve_project_artwork`
   command marker. The starting source already contains the prior WebP fix and
   successfully decoded all 59 WebP sources sampled here in both debug and release
   workers. The screenshot pattern is consistent with the older executable rejecting
   newly requested WebP while previously stored PNG Mods artwork remains visible.
   This is an inference supported by binary inspection and the reproduced source
   behavior; the installed executable was neither launched nor replaced.
2. **A remaining shared GIF format gap.** Simple Grass Flowers (`ti9KkMHm`,
   440×440, five frames) and Enchant Icons (`6vhHOIKw`, 209×209, three frames)
   publish accessible GIF icons. Starting source deliberately rejected GIF.
   Enchant Icons also omits the last frame's LZW end code despite complete pixels,
   terminated image blocks and a final GIF trailer. The library's standard handling
   accepts this source; opting into stricter end-code checking rejected it.

This was not a category-specific binding problem or a reason to introduce another
cache. Search hits carry a sanitized optional icon URL, but Browse uses the exact
provider project ID through typed IPC. Rust resolves the authoritative project
document, validates its CDN locator, acquires and validates bytes, normalizes pixels
and shares the same project cache with installed content. Version normalization
does not replace project artwork identity. All four browsing destinations use
`ModrinthBrowse` and `InstalledArtwork`/`Artwork`.

The live audit queried official Modrinth search and authoritative project documents,
then fetched their official CDN icons separately from deterministic tests. The
20-result sample in each category contained **19 PNG, 59 WebP and two GIF icons**;
all 80 requests succeeded. Search/project locators matched for this sample. Examples
from the screenshot categories include Fabric API and Sodium, Fresh Animations and
Motschen's Better Leaves, Complementary Unbound and BSL Shaders, and Fabulously
Optimized and Better MC. Sanitized per-project results are in
[`docs/l1/artwork-audit.json`](docs/l1/artwork-audit.json).

### Pipeline outcomes and failure distinctions

| Condition | Evidence and behavior |
| --- | --- |
| No project icon | BadOptimizations (`g96Z4WVZ`) still returns official `icon_url: null`; native result is `unavailable`, source null, intentional local fallback. It is not included among the 80 icon-bearing successes. |
| Acquisition failure | Existing deterministic native retry/deadline tests distinguish `retryable` from absent artwork; loopback acquisition rejects wrong hosts/redirects, missing resources and oversized responses. No live upstream outage occurred in the 80-source audit. |
| Decode failure | Starting source rejected both GIFs; regression tests cover truncated streams, false PNG MIME/signatures, HTML bodies, unsupported JPEG, animation restrictions and size/pixel/work bounds. Actual GIF pixels now normalize successfully. |
| Persistence failure | New disposable blocked-cache fixture proves valid pixels remain `available` without a stored object, and persistence recovers after the obstruction is removed. Disk failure is cosmetic, not permanent negative artwork. |
| Frontend display failure | No such failure was observed for the 80 canonical images. The frontend accepts native PNG data URLs only, retains its type-specific fallback until image load and removes failed images. Browser tests verify valid pixels and absent/retry states. |
| Temporary unavailability | Typed transient results retain the existing one-minute native backoff. Deterministic native tests expire the deadline and recover; the component test demonstrates one timed retry followed by displayed pixels. No unbounded loop is added. |
| Negative-cache delay | Missing/unsupported results expire after ten minutes in memory; transport results expire after one minute. Neither is persisted. Frontend promises have matching finite deadlines and a 128-entry limit. Restart clears old unsupported-GIF negatives. |
| Corrupt positive cache | One proven review-only GIF-derived PNG was deliberately corrupted. A new process rejected it and reacquired validated pixels. Its original hash returned; the other 79 files retained their hashes and modification times. |

MIME is a diagnostic hint, not image authority: full bytes must decode under the
image policy. HTML or JPEG labelled `image/png` does not become trusted artwork.
No production host, URL-scheme, redirect, CSP or path-containment policy was relaxed.

## Exact artwork change

`cosmetic_image` now admits GIF provider icons through the existing byte-only
isolated worker and stores/displays only a canonical static PNG of the real first
frame. It preserves first-frame offsets and transparency. Later frames are decoded
and discarded to detect damaged streams; count and aggregate decoded work are
bounded. Existing WebP and PNG paths and server-favicon PNG-only policy remain.

Bounds remain 512 KiB encoded input/output, 1024×1024, 1,048,576 pixels and a 16 MiB
worker memory budget. GIF adds at most 64 frames and 16 MiB total frame pixels.
Canvas/frame containment, valid trailer and complete frame data are required.
The parent retains its ten-second deadline, bounded pipes and exact-child cleanup.
The worker receives bytes only, before Tauri initialization, with no URLs, local
paths, credentials or network access. The focused `gif` dependency disables default
features/quantization; its `weezl` dependency and lockfile are committed.

The existing cache read normalizes validated legacy WebP/GIF objects once. It does
not trust a filename, store remote bytes as browser input or persist a negative
result. Four native acquisition slots and per-project completion locks remain;
project keys do not include category, so installed/browse/category consumers reuse
the same identity and store. No schema or command DTO changed.

### Fresh, warm and recovery acceptance

Acceptance used distinct review-only application identifiers and fixture accounts/
instance metadata. Artwork IPC, project resolution, CDN acquisition, worker decoding
and persistence were real. Search results were the captured live Modrinth responses
mounted through the existing Phase F harness. No owner instance was used.

| Scenario | Result |
| --- | --- |
| Starting source, fresh sample | 20/20 Mods, 18/20 Resource Packs, 20/20 Shaders, 20/20 Modpacks; two unsupported GIFs explained every sample failure. |
| Final source, new empty review cache | 80/80 native results `available`, 80 canonical PNG objects persisted across all four categories. Genuine no-icon example returned `unavailable`. |
| Compiled decoder | Debug and final production-release workers each normalized all 61 real WebP/GIF sources. The 19 PNG sources use the existing in-process full PNG validation. |
| Restart and warm cache | 80/80 `available`; all 80 object hashes **and modification times** unchanged. Native cache-hit path bypasses metadata/CDN acquisition. |
| Corrupt one object, restart | 80/80 `available`; only `ti9KkMHm.img` rewritten, all 80 hashes equal their original validated hashes. Both GIF rows displayed real artwork. |
| Repeated/overlapping consumers | Native coalescing/offline-cache tests forbid duplicate acquisition; bounded frontend promise tests and four-surface browser tests pass. |

Narrow screenshots initially load 11 of 20 images because the existing lazy image
element defers distant rows; all 20 native results for each category and all 80
persisted objects were verified separately. This is not a missing-artwork failure.

## Controls, cards and download actions

One shared style in `ModrinthBrowse` gives Categories and Sort labels the same
placement and typography, 40px controls, padding, borders, radius, sunken background,
hover/focus treatment and existing chevron. Sort remains a keyboard-operable native
select; Categories retains its accessible dialog and Escape/focus return behavior.
Filtering, query text, sort choice and pagination contracts remain. A demonstrated
bug was fixed: changing Sort previously reset the search text passed to native;
it now preserves the active query and categories.

The standalone Modpacks browser receives the panel's existing 24px horizontal
padding. Header, search, filters and results now share one alignment. Cards use a
44px artwork column, balanced padding, a wrapping title, three-line bounded summary,
metadata and a separate wrapping action row. New Instance remains a distinct badge
and Details a button. At 900px available browser width or below the grid becomes one
column; the breakpoint measures the browser container. At smaller widths controls
and context wrap deliberately. Long multilingual titles remain visible and actions
do not overlap text.

No third-party icon library is installed: Aurora's existing `Icon` path registry
now supplies a reusable 18px `download` glyph with a horizontal tray under the
arrowhead. The shared quick-install action uses it for Mods, Resource Packs and
Shaders. Installed/Keep/Blocked/Installing labels and accessible names remain;
Modpacks retains its New Instance semantics. Unrelated arrows are untouched.

## Visual acceptance and screenshot paths

The real Tauri/WebView2 desktop app was captured through its embedded WebView, and
the native window was inspected with the existing Computer Use workflow. These are
actual Svelte components in the established review shell, with live native artwork,
not a mock image or generated redesign. Purple is the harness's default accent;
the owner's turquoise preference was not modified.

Matching before/after captures are **2000×1288 physical pixels**, with identical
fixture context and public project sample. The owner's original screenshots are
2560×1440 and remain outside Git. Source-baseline screenshots already include the
prior WebP correction, unlike the older installed executable in the owner reference.

| View | Before | After |
| --- | --- | --- |
| Mods | [before](docs/l1/mods-before.png) | [after](docs/l1/mods-final.png) |
| Resource Packs | [before](docs/l1/resourcepacks-before.png) | [after](docs/l1/resourcepacks-final.png) |
| Shaders | [before](docs/l1/shaders-before.png) | [after](docs/l1/shaders-final.png) |
| Modpacks | [before](docs/l1/modpacks-before.png) | [after](docs/l1/modpacks-final.png) |

Additional inspected captures:

- [Narrow Modpacks, 900px logical viewport](docs/l1/modpacks-narrow.png).
- [Resource Packs after warm restart](docs/l1/resourcepacks-warm-narrow.png).
- [Real GIF artwork after corruption recovery](docs/l1/gif-recovered.png).
- [Genuine absent-icon fallback](docs/l1/no-icon-fallback.png).
- [Isolated optimized production SPA boot](docs/l1/production-boot.png).

Visual inspection confirmed consistent filters, labels, padding, artwork and tray
glyphs; usable action rows; deliberate narrow single-column layout; no horizontal
overflow or text/action overlap. Deterministic long multilingual fixtures additionally
exercise four destinations at 1600, 1120 and 720px viewport widths.

## Verification

| Check | Result |
| --- | --- |
| `npm run check` | Pass, zero errors/warnings. |
| `npm test -- --test-concurrency=1` | Pass, 252 tests in 16 suites. |
| `node tools/visual/l1.test.mjs` with bundled Playwright | Pass for all four categories; retry/fallback, identical filter styles, search/sort/category retention, keyboard dismissal/focus, multilingual metadata, responsive actions and glyph rendering. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Pass. |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | Pass; four pre-existing test-code warnings. |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Pass: 847 library tests, 29 explicitly ignored, six icon-asset tests; no failures. |
| Existing installed artwork regression tests | All four pass within the full suite; installed identity/adoption logic is unchanged. |
| `npm run build` | Pass, production static SPA. |
| `npm run tauri build -- --no-bundle --config <isolated override>` | Pass, optimized native executable; no signing, bundle, installation or publication. |
| Real desktop boot | Debug review and production SPA boot under isolated identifiers pass. |

An initial default-parallel frontend test invocation encountered fixture-server
before-hook failures under concurrent load; the complete serial run passes.
An early Rust invocation hit the running debug executable's Windows file lock;
the review process was closed/copied to its own temporary executable and the complete
suite rerun successfully. Neither failed invocation is counted as passing.

## Preservation, limitations and local rollback

The preflight recorded 682 tracked files, 15,144 pre-existing untracked files and
13,341 owner files. SHA-256 comparisons preserve every pre-existing untracked and
owner file byte-for-byte; only scoped tracked source/document files changed. The
original tracked set has no missing members and critical documentation, package
metadata, both lockfiles and native build configuration remain present. No deletion
diff or generated build/game/runtime/private evidence is staged. Owner-installed
executable, instances, credentials, settings, protected `.zcode*` files and research
evidence were preserved. Review caches/WebView storage use new isolated identifiers.

Limitations: live results are a dated 80-project sample, not an assertion about every
future Modrinth format or network condition. JPEG/SVG and animated PNG/WebP remain
unsupported; GIF animation is intentionally a first-frame snapshot. Budget-exceeding
GIFs fail safely. Windows was exercised; Linux memory enforcement remains existing
code, and other platforms still reject worker formats until an equivalent allocation
boundary exists. Failed disk persistence can require later reacquisition after the
frontend cache expires. Real transient-outage recovery uses deterministic fixtures,
not deliberate disruption of the public service. The installed production launcher
still needs a separately authorized future build/release to receive these changes.

Local commits, in chronological order:

1. `98c192a` — shared browse controls, cards and Modpacks padding, plus development
   acceptance harness. No artwork or glyph dependency.
2. `1d6cfdd` — shared download glyph and its rendering assertions.
3. `5dd5ae4` — bounded GIF decoding, cache normalization, tests,
   lockfile and current architecture documentation.
4. This report and sanitized acceptance evidence.

Use `git log --oneline 8df387b..HEAD` for the report commit's identity. Revert the four commits
in reverse order to roll back L1 without touching owner data. The glyph test addition
builds on the layout acceptance test, so revert the glyph commit before layout when
rolling back both. Artwork production code is independent of layout/glyph changes.
There are no remaining L1 acceptance failures. Nothing was pushed or published.

Primary references: [Modrinth search](https://docs.modrinth.com/api/operations/searchprojects/),
[authoritative project document](https://docs.modrinth.com/api/operations/getproject/),
[GIF decoder options](https://docs.rs/gif/0.14.2/gif/struct.DecodeOptions.html).
