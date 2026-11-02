# UI-3 visual review

Start with the [acceptance report](../development/ui/UI_3_DISCOVER_LIBRARY_ACCEPTANCE.md).
All comparison sheets use the same baseline (`7c76626`), deterministic fixture,
viewport and background on both sides. They are browser evidence, clearly labeled.
Original full-resolution captures are in `before/` and `after/`.

| Surface | Matched comparison |
| --- | --- |
| Mods | [Compact rows](mods-comparison.png) |
| Modpacks | [Artwork-led grid](modpacks-comparison.png) |
| Resource Packs | [Medium artwork cards](packs-comparison.png) |
| Installed Mods | [States and management](installed-mods-comparison.png) |
| Installed Packs | [Bounded descriptions and controls](installed-packs-comparison.png) |
| Empty, error and loading | [Feedback states](states-comparison.png) |

Each surface includes 1440×900, 1280×800, 1024×768 and 720×520. The top of a
minimum-size page intentionally requires vertical scrolling to results.
`states/` includes exact pack review, pending/completed installation, failure,
200% text, cyan and reduced-transparency fixtures. Populated installed rows are
synthetic; native Installed inventories were empty.

## Actual native application

These are screenshots from the compiled Tauri frontend at `http://tauri.localhost/`,
using actual Rust commands and a fresh isolated data root. They are **not** the
browser fixture entry. The same root's genuine Fabric installation completed
through the ordinary verified native installer before live Mods/Resource browsing.

| Native capture | Evidence |
| --- | --- |
| Startup | [1120×790](native/startup-client.png) |
| Live Mods | [1016×768](native/mods-client.png) |
| Live Modpacks | [1120×790](native/modpacks-client.png) |
| Live Resource Packs | [2048×1152](native/resource-packs-client.png) |
| Installed Mods, empty | [1016×768](native/installed-mods-client.png) |
| Installed Resource Packs, empty | [2048×1152](native/installed-packs-client.png) |
| Search + category empty result | [Sodium + Adventure](native/filtered-empty-client.png) |
| Medium sizing | [1016×768](native/medium-height-client.png) |
| Actual minimum, page top | [706×512](native/minimum-client.png) |
| Minimum controls/results after scrolling | [Controls](native/minimum-controls-client.png), [results and actions](native/minimum-results-client.png) |
| Alternative accent | [Cyan Resource Packs](native/cyan-resource-packs-client.png) |
| Restart persistence | [Borealis/cyan retained](native/restart-home-client.png) |

The configured window minimum remains 720×520. At this host's 125% display scale,
resizing to that enforced limit yielded a measured **706×512 CSS client**. Browser
checks cover that measured client too, on all five surfaces at 100% and 200% text.
Do not relabel configured/attempted sizes as measured client sizes. Measurement
JSON accompanies each native PNG. `medium-attempt` remained 1120×790; it is a
failed sizing attempt, not medium acceptance. `medium-frame` measured 1016×790;
the following top-edge resize produced `medium-height` at 1016×768.

The first app's exit-code helper returned null; it is not a recorded zero exit.
The corrected helper retained its process handle. Restart PID 23316 exited
normally with **code 0**, recorded in [restart/exit.json](native/restart/exit.json).
No account sign-in, game launch, mod/pack installation or owner content was used.
The only real product installation was the disposable base Minecraft/Fabric game.

## Review limits

No Note/Lunar screenshots were attached; the supplied written design principles
guided the work. No proprietary reference artwork or layout was copied. Exact
per-hit compatibility versions and update dates are not in the search DTO and
were not invented. Pack versions/loader requirements stay in Details/review.
Owner aesthetic approval is still required. No UI-4 or release work began.
