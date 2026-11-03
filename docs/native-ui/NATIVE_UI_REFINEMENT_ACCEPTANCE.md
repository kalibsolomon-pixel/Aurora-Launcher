# Native UI refinement acceptance

## Scope and baseline

The launcher job is to find and manage installed content, launcher preferences,
accounts and active operations without leaving the native interface.

- Starting commit **8dcb308059ce25946270013727ab7d08f29212d8**. This matched
  both the native checkout and the remote main branch.
- Final implementation commit **e5a98e9e4d8623ba07cfe15ea6ee8150d0a4af7e**.
- Working branch **codex/native-ui-1** in a separate worktree.
- The task attachment contained the written brief. The historical WebView
  Settings registry and presentation source supplied reference behavior for
  the drawer, list hierarchy and Settings navigation.
- The initial native checkout contained 13 release-related edits. The UI
  branch starts from committed source and leaves those edits in their checkout.

## Implementation

- **Installed content.** Mods, Resource Packs and Shaders share a rounded
  list surface, with flat 68 px rows, 46 px artwork, fine separators and
  secondary metadata. The existing actions still dispatch through their
  original callbacks. Accounts and Downloads reuse the same list surface.
- **Artwork.** Mods use the existing digest-keyed thumbnail service. Resource
  packs read a root-level PNG from a validated ZIP or pack directory.
  The decoder accepts a single static PNG up to 512 KiB and 1024×1024, with
  a 16 MiB decoded allocation limit. ZIP reads are bounded and do not write
  archive members to disk. Shader packs use cached Modrinth artwork only for
  a project identified by installed provenance. Missing or rejected artwork
  falls back to the category glyph or mod initial. The UI requests thumbnails
  from the visible row range, with at most 48 in a batch and 96 cached entries
  per open workspace. A cache entry retains a 48 px image or a recorded miss.
  Explicit Refresh retries pack artwork. Rendering the workspace performs
  no provider requests for icons. [Minecraft Java pack icon convention](https://www.minecraft.net/nb-no/article/minecraft-java-edition-1-16-2)
- **Drawers.** Accounts and Downloads share the existing native popup focus
  scope, a 460 px width, rail anchoring, matched vertical inset, scrollable
  body and close control. Accounts show the selected identity first.
  Downloads show the existing operation phases and transfer summary, with
  retry and cancellation where the operation supports them. The activity
  timer runs only during active work.
- **Settings.** The ordered, typed section registry drives the secondary
  navigation. The selected ID activates one content section. Home & widgets
  links to the existing Home layout editor. Accessibility owns Reduce Motion.
  Java and runtime preferences remain with their instance.
- **Skin favorites.** The star uses the preset UUID and existing atomic
  favorite metadata write. The Favorites only filter reads the local model.
  Removing a preset removes its favorite metadata with the record. Thumbnail
  and favorite updates keep existing row controls and focus when IDs match.
- **Toggle contrast.** The switch track uses a dedicated control token derived
  from the theme's strong border color. The shared border and hover tokens
  continue to serve their other consumers. Accent and focus tokens retain
  their existing roles.

## Headless layout and interaction evidence

The Slint software renderer used synthetic data at 720×520, 1120×760 and
1600×1000 logical pixels. The minimum layout was also rendered at 125%, 150%
and 200%. These are headless renders from the launcher component tree.

| Surface | Render |
| --- | --- |
| Mods with 120 entries and long filenames | [Minimum](ui1-evidence/headless-mods-minimum.png) · [After scrolling](ui1-evidence/headless-mods-scrolled.png) |
| Settings secondary navigation | [Typical](ui1-evidence/headless-settings-typical.png) |
| Accounts with two entries | [Minimum at 200%](ui1-evidence/headless-accounts-200-percent.png) |
| Downloads with active, queued, completed and failed rows | [Minimum at 150%](ui1-evidence/headless-downloads-150-percent.png) |
| Skin library with 120 entries and duplicate names | [Large](ui1-evidence/headless-skins-large.png) |
| Switch tracks in Simple, OLED and Borealis | [Theme crops](ui1-evidence/headless-toggle-themes.png) |

The harness checks that scrolling requests a new bounded artwork range, Escape
closes Accounts, the close button dismisses Downloads, pointer and keyboard
activation preserve the skin preset ID, and Tab and Enter select Settings
sections. It also checks the derived switch track against all curated accent
palettes across the three background cases.

The native binary suite passed 57 tests. Native type checking across all
targets and formatting checks for native and core also passed.

Five targeted core tests cover valid ZIP and directory icons, missing root artwork,
corrupt data, oversized data, and the existing PNG animation and integrity
checks. The favorite persistence test reopens a saved library and verifies
that the original skin bytes remain intact.

## Windows desktop validation

The launcher opened in the Windows desktop session against disposable data.
At 1120×760 logical pixels and 125% display scaling, the content run covered
120 mods, a ZIP pack, a directory pack, corrupt and missing pack artwork, and
a shader with cached artwork linked to its digest-matched Modrinth record.
The drawer and Settings run used the same disposable instance. Escape
returned focus from Accounts to Manage accounts. After Escape dismissed
Downloads, Enter reopened it from the restored rail focus. At the minimum
window size, Customize Home opened the existing widget editor.

The 120-entry skin test saved a favorite by UUID among duplicate display
names, showed only that entry in Favorites only, and reopened the launcher
before unfavoriting it. Deleting one duplicate left the other entry and
removed the deleted favorite's metadata. In a separate pointer and keyboard
check, the star kept focus after the click and Enter toggled the same row
back.

| Surface | Native desktop capture |
| --- | --- |
| Installed mods | [Baseline](ui1-evidence/desktop-baseline-mods.png) · [Refined](ui1-evidence/desktop-candidate-mods.png) · [Refined after scrolling](ui1-evidence/desktop-candidate-mods-scrolled.png) |
| Resource packs | [ZIP and folder artwork, corrupt and missing fallback](ui1-evidence/desktop-candidate-resource-packs.png) |
| Shader packs | [Cached project artwork](ui1-evidence/desktop-candidate-shaders.png) |
| Accounts | [Baseline](ui1-evidence/desktop-baseline-accounts.png) · [Refined](ui1-evidence/desktop-candidate-accounts.png) |
| Downloads | [Matching drawer and empty state](ui1-evidence/desktop-candidate-downloads.png) |
| Settings | [Appearance and section navigator](ui1-evidence/desktop-candidate-settings.png) · [Minimum at 125%](ui1-evidence/desktop-candidate-settings-minimum.png) |
| Skin favorites | [One favorite among duplicate names](ui1-evidence/desktop-candidate-skin-favorite-filter.png) |


## Optimized resource smoke

Both builds used the same Windows host, Slint software renderer, disposable
fixture and Rust release optimization. The final launcher binaries used
thin LTO and eight codegen units. The scenario posted 36 wheel steps into
the 120-mod list, returned to the top, cycled the three installed-content
tabs five times, and opened and closed each drawer four times. It opened
Settings in both builds and visited the candidate's nine sections before
returning Home. The initial and settled idle windows each lasted about
12 seconds.

The quiet repeat below shows baseline → candidate in MiB. Working set
counts resident pages, including shared pages. Private commit and private
working set are separate Windows counters.
[Windows memory counter definitions](https://learn.microsoft.com/en-us/windows/win32/memory/memory-performance-information)

| Stage | Total working set | Private commit | Private working set |
| --- | --- | --- | --- |
| Initial idle | 33.34 → 33.32 | 9.78 → 9.74 | 8.91 → 8.86 |
| After long-list scroll | 55.78 → 54.99 | 25.22 → 24.41 | 23.31 → 22.48 |
| After content tab navigation | 56.54 → 54.93 | 23.61 → 22.40 | 21.57 → 19.86 |
| After both drawers | 57.21 → 55.64 | 23.81 → 22.79 | 21.76 → 20.00 |
| After Settings | 63.38 → 62.69 | 24.61 → 24.09 | 22.17 → 20.73 |
| Settled Home | 63.03 → 62.37 | 23.89 → 23.54 | 21.68 → 20.36 |

Neither binary accumulated CPU time at the sampler's resolution during either
idle window. In the quiet repeat, the launcher used 4.67 → 4.95 CPU seconds
over the full smoke, which included the additional Settings visits in the
candidate. Content tab UI Automation calls had a median of 18 → 26 ms in
the quiet repeat and 16 → 15 ms in the first run. Those calls include
cross-process automation overhead and measure dispatch rather than display
latency.

The first candidate run overlapped baseline compilation and showed a larger
working set gap at initial idle (33.19 → 47.18 MiB) and settled Home
(63.42 → 73.15 MiB). The gap disappeared when both builds ran again with
compilation finished. Private commit stayed near baseline at idle in both
passes. The repeat provides evidence against a persistent working set
increase on this host.

[Baseline repeat](ui1-evidence/desktop-baseline-resources.json) ·
[Candidate repeat](ui1-evidence/desktop-candidate-resources.json) ·
[Baseline first pass](ui1-evidence/desktop-baseline-resources-first.json) ·
[Candidate first pass](ui1-evidence/desktop-candidate-resources-first.json)

## Limits and local commits

The live drawer captures use a signed-out profile and an empty download
queue. Multiple accounts and active download phases use the headless
fixtures. Desktop interaction and resource sampling cover one Windows host.
Shader artwork requires an installed association and a matching cache entry.
Other shader packs use the category icon.

- **9dc53a61ae47e70ac366aa114cea2357491d6295** refined the native launcher lists, drawers and Settings.
- **e5a98e9e4d8623ba07cfe15ea6ee8150d0a4af7e** preserved skin-row focus during thumbnail and favorite updates.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
