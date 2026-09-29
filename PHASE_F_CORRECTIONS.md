# Phase F corrections — 29 September 2026

The correction replaces background distortion with the owner-approved local H.264 loop rendered from the original Borealis artwork. The current 20-second version fluctuates within the original curtain silhouette: soft, irregular light patches brighten and fade while the overall aurora stays in place. It supersedes the rejected subtle illumination and translating-curtain studies. The asset provenance, exact hash and reproduction instructions are in [backgrounds/README.md](static/backgrounds/README.md).

Home's separate teal/violet overlay has been removed. The play card and widgets now use exactly the same shared translucent material as the approved Settings panel: identical background layers, blur, saturation, brightness, edges and shadow. The change is scoped to Home; the owner-approved glass on Settings, Instances and instance content screens is preserved. Scrolling the card over brighter aurora confirms that the background diffuses through it; the dark lower sky naturally produces a darker tint. [Visual proof](docs/phase-f/corrections-home-glass.png) shows this comparison within Home.

Play card and small widget bounds match at the checked 1280 px viewport (left 163.2, right 680.4, width 517.2 CSS px). The enlarged main logo and player keep their column alignment. Other launcher Aurora marks and the About destination are removed, the gear is corrected, and stable scrollbar space prevents Settings category width shifts.

Custom and preset accents now apply the same seven native-derived palette tokens. Native saving of a custom #ffb86b accent was exercised and persisted. Configuration schema 6 adds a validated 0–100 motion preference, with explicit migration of supported older schemas; unknown or malformed documents still fail deliberately. Playback maps to 0.5–1.5×, pauses when unfocused or hidden, uses the still for reduced motion or decoding failure, and releases its source on unmount. There is no per-frame application animation loop or new runtime dependency.

## Verification

- Frontend checks: zero errors and warnings; 136 tests passed; production frontend build passed after the Home glass change.
- Native checks: formatting and all-target checking passed; 564 unit tests and 6 icon integration tests passed, with 21 intentionally ignored tests not represented as executed.
- Final `npm run tauri build -- --no-bundle` passed with the approved video and matching Home glass; the optimized release executable was produced. Installer packaging and publishing a new launcher release were not part of this push.
- A real native application boot and native appearance persistence were exercised with isolated review application data. The production application identifier is unchanged.
- The final video decodes without errors at 1920×1080, 30 fps, 600 frames. Browser playback verified 20-second duration, 1080p dimensions and advancing playback. The raw periodic endpoint matches exactly; the seam step is comparable to an ordinary adjacent frame. The file is 4,471,527 bytes, upscaled from the original 1672×941 artwork.
- The existing fallback skin rasterizer measured 7.17 ms mean and 18.63 ms p95 over 30 software draws. This includes allocation and excludes native canvas upload; it is not a measurement of authenticated layered skins or video decoding.

## Final performance samples

The isolated native review was restarted to load the approved asset. Each CPU sample lasted 15 seconds over the same eight-process launcher/WebView tree. Visible samples used maximized Home, two small widgets and the fallback player. CPU below uses 100% for one fully occupied logical core; this machine has 28 logical processors. Working-set sums can count shared memory more than once. These are short local observations, not guarantees across hardware or long-running sessions.

| Scene | CPU, one-core % | Working set, MiB | Private memory, MiB |
| --- | ---: | ---: | ---: |
| Borealis, default speed 50 | 7.50 | 562.2 | 522.7 |
| Borealis, maximum speed 100 | 11.14 | 560.7 | 545.3 |
| OLED + Simple | 2.29 | 575.2 | 505.1 |
| Borealis, minimized | 0.00 | 560.0 | 522.5 |

Default and maximum animation measured approximately 0.27% and 0.40% of total CPU capacity on this host. Five one-second Windows GPU Engine counter samples attributed to this process tree at default speed averaged 4.49%, maximum 4.82%. This is a sum of process-engine counters, not total-device utilization; desktop compositor cost may sit outside this process tree. The video has a measurable rendering cost relative to Simple, while minimizing pauses its CPU work. The player still has no idle render loop. Previous pilot and interrupted samples are not used for these conclusions.

A fresh live Microsoft login, authenticated Minecraft launch and live Discord publication were not repeated; passing regression tests and retained action wiring do not substitute for those live checks. The owner approved the final fluctuation video before the final Home glass correction and push.
