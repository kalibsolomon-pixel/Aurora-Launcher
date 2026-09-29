# Phase F pilot acceptance — 28 September 2026

## Design direction

The pilot is ready for owner visual review, not approved for full rollout. Shell, Home and launcher Settings now use a quiet atmospheric stage, the unchanged Aurora mark, a wide Play action, restrained glass, and independent appearance choices. [The design proposal](PHASE_F_DESIGN.md) records the composition and scope decisions. The supplied third screenshot shows window controls rather than a Home sketch; the written brief guided the Home hierarchy.

## Research

[Lunar’s official launcher presentation](https://www.lunarclient.com/news/the-new-launcher-is-here) is an August 2023 historical reference, supplemented by [its launcher guide](https://www.lunarclient.com/news/how-to-use-the-new-lunar-client-launcher). Its useful principle is launch dominance with peripheral utilities. [Dawn’s official presentation](https://dawn.gg/) informs compact navigation and separation of player identity from profile organization. No artwork, branding or exact composition was copied. Aurora’s enlarged logo is the owner’s direction, not a claim that both references use the same logo placement.

## Shell

The Windows title bar uses Tauri minimize, maximize/restore, close and native drag-region behavior. Other platforms keep OS decorations. The four product destinations have familiar original SVG icons, accessible names, selected markers and focus/hover tooltips. Developer remains development-only. The account dock retains its avatar and readable username and opens the existing account dialog.

The configured default is 1120×760 and minimum 720×520. Content scrolls independently of the rail. Short windows use tighter rail spacing; the player hides below 901 px. Settings categories become horizontal and widgets become one column. The native minimum launch captured 722×551 including the host window frame; a separate 720×520 browser viewport also passed visual inspection. Native maximized capture is 2048×1152 logical pixels on this 2560×1440 desktop.

Custom button minimize/maximize/close and double-click restore were exercised. The title-bar drag action was exercised. Windows 11 maximize-hover Snap Layouts are not implemented by this custom button, and edge snapping was not independently certified. There is no claim of macOS/Linux native-chrome acceptance from this Windows run.

## Home

The existing logo is byte-identical to the supplied asset. A 78 px desktop Play action leads the composition, followed by the instance-name picker, exact installed metadata and native readiness. Manage Instance and the old Home header are removed. Existing blockers, errors, retry and process feedback remain.

The separate player retains the existing skin rasterizer and supports pointer rotation, keyboard left/right and reset. It redraws only when needed and does not animate while idle. Review fixtures intentionally display the existing no-skin fallback; they are not the owner’s authenticated account or skin.

Pencil opens direct edit mode. All eight registrations remain available; add, hide, pointer reorder, keyboard ordering, sizing, native persistence and reset survive. Done remains visible while scrolling in edit mode. Unknown/hidden widget slots survive reorder. All-hidden layouts keep the pencil entry point. Failed saves retain prior data and do not announce success.

## Appearance and Settings

Theme, Background and Accent are separate. Existing Aurora Dark, Midnight, OLED Black, seven preset accents and custom colors remain. Simple is the migration/default background; Borealis is opt-in. Schema 5 adds only the bounded `simple | borealis` native enum. Explicit older-schema migrations preserve supported appearance, instance selection, widgets and Discord preferences; malformed/current-schema invalid background data fails deliberately.

Borealis is an original generated 1672×941 WebP (92,030 bytes). It is a **still image with slow drift**, not recorded evolving aurora curtains. This asset is sufficient to review composition/glass, but the owner should decide whether final production needs licensed natural-motion footage. Provenance is in [the asset note](static/backgrounds/README.md). No video decoder, shader, remote image fetch or new dependency is added.

Pilot cards use 16 px frosted glass over Borealis and opaque theme surfaces over Simple. Settings groups Appearance, Home and Discord & privacy. Home holds an edit shortcut and reset. Desktop integration is removed from this Settings presentation; installer hooks and native shortcut ownership are unchanged. Discord retains its master opt-in, every independent detail control, and the extra address control gated by Show Server. Game details and world/server privacy are visually grouped without changing the native policy.

## Motion and accessibility

Hover/press changes are short and restrained; Play never pulses. Background motion pauses on document hiding or focus loss. CSS reduced-motion support removes background drift and decorative transitions; reduced transparency selects opaque panels. The OS reduced-motion setting itself was not toggled during acceptance, so that CSS branch has code review rather than an OS-level visual certification.

Interactive checks covered keyboard focus/tooltips, the instance picker, account dialog switching, Play feedback, direct widget drag, earlier/later controls, size selection, saved layout reload and Done. The existing modal focus behavior remains. Native buttons, radios and selects retain accessible names and selected state; color alone does not convey selection.

## Performance

Windows development native host, actual production components with synthetic review data, 7 launcher/WebView processes. Each CPU sample lasts approximately 15 seconds and uses summed process CPU deltas; **100% means one fully occupied core**, not the entire machine. Working-set sums can double-count shared pages and are not unique-memory measurements. These are local samples, not cross-device guarantees.

| Home state | CPU, one-core % | Summed working set | Summed private memory |
| --- | ---: | ---: | ---: |
| Original continuous Borealis drift, maximized | 19.16 | 549.2 MiB | 399.9 MiB |
| Final 10-step/second Borealis drift, maximized | 3.02 | 553.9 MiB | 404.2 MiB |
| OLED + Simple, maximized | 0.00 | 548.8 MiB | 354.7 MiB |
| Borealis minimized | 0.00 | 551.4 MiB | 459.2 MiB |

The minimized sample preceded the drift cap; visibility pausing was already active. An earlier sample that captured the minimized window was discarded because capture restored it. A post-reload/restored-size sample was also excluded from the comparable maximized table. The cap uses `steps(450)` over 45 seconds: small subpixel drift steps reduce repeated glass composition while retaining gentle motion.

Five one-second Windows GPU Engine counter samples summed across this process tree averaged **0.368%** for maximized Borealis Home (maximum 0.407%). Plain OLED Settings sampled 0.000%. These are sums of attributed process-engine counters, not a total-device utilization measure or a matched-scene GPU benchmark; desktop compositor work may be outside the process tree.

The existing fallback player software rasterizer averaged **6.30 ms**, p95 **14.23 ms**, over 30 measured draws after 5 warm-ups. This Node benchmark includes pixel allocation and excludes native canvas upload. The 420×600 RGBA buffer is 1,008,000 bytes; there is no idle RAF loop. Hidden/narrow previews skip drawing. Authenticated layered skins may cost more than this fallback sample. The background’s uncompressed RGBA equivalent is about 6 MiB; the actual retained memory delta depends on WebView caches.

## Verification

- `npm run check`: **0 errors, 0 warnings**.
- `npm test`: **133 passed, 0 failed**, 13 suites. Final run used the unmodified repository command. An earlier serial run also passed; initial development runs before fixes are superseded.
- `npm run build`: **passed**, static production output generated.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: **passed**.
- `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`: **passed**.
- `cargo test --manifest-path src-tauri/Cargo.toml`: **563 unit tests passed, 21 intentionally ignored; 6 icon integration tests passed; 0 failures**. No ignored live/provider cases were represented as executed.
- `npm run tauri build -- --no-bundle`: **passed**, optimized release executable produced in 3m 23s; installer bundling was not requested by this command.
- Native real-backend boot: **passed** in isolated `com.aurora.launcher.phasef-review` application data. The regular frontend on port 1420 loaded real configuration, saved Borealis as schema 5 through Rust, and retained it after reload. The production identifier remains `com.aurora.launcher`.
- Added tests cover schema-4 preservation/schema-5 background validation and round-trip, independent appearance writes/failure retention, unknown/hidden widget reorder preservation, and explicit player projection rotation. Existing launch, account, cosmetics, persistence and Discord regression suites pass.

The visual harness on port 1422 substitutes synthetic application DTOs only; native window operations remain real. Play was verified to call the existing launch action and display the deliberate fixture error. A new authenticated Minecraft launch, live Microsoft login and live Discord publication were not performed. No user installation or account was modified to obtain screenshots. [Harness instructions](tools/visual/README.md) explain reproduction.

## Repository safety

Starting HEAD, fetched `origin/main`, and merge-base: `ec30f10f7fa7224a5a9c6ff8fa67a7ab605a15d0`; ahead/behind was 0/0. Work is on `codex/phase-f-pilot`. All **208 original tracked paths** remain. All **609 protected pre-existing local files** match their initial SHA-256 hashes. Root documentation, manifests, lockfiles and build configuration exist. No tracked deletions, dependency additions, generated game/runtime data or diagnostics are staged. The original Aurora logo hash is unchanged: `faba1af24b964cfd085505c63e9e43d7abedebd70d70794e841a3251d0fa5a19`.

Changes are split into shell/appearance, Home/Settings, and documentation/evidence commits. Exact final commit IDs are reported in the delivery message. Nothing is pushed. Review screenshots are intentional small evidence assets, not generated runtime state. Build logs, original baselines and raw measurements are retained locally under `%TEMP%/aurora-phase-f-evidence`.

## Visual review

These are real component renders, not design mockups. Native fixture captures are labelled in their title bar; browser captures omit native chrome. The native smoke captures use real backend state and no fixtures.

| View | Evidence |
| --- | --- |
| Aurora Home, maximized/native | [Home](docs/phase-f/home-borealis-maximized.jpg) |
| Aurora Home, normal/native | [Normal window](docs/phase-f/home-borealis-normal.jpg) |
| OLED Home, maximized/native | [OLED](docs/phase-f/home-oled-native.jpg) |
| Minimum configured native Home | [Minimum window](docs/phase-f/home-minimum-native.jpg) |
| Home edit mode, browser | [Editing](docs/phase-f/home-edit.jpg) |
| Empty widgets, browser | [Empty](docs/phase-f/home-empty.jpg) |
| Aurora Settings, native | [Settings](docs/phase-f/settings-borealis.jpg) |
| OLED Settings, browser | [OLED Settings](docs/phase-f/settings-oled.jpg) |
| Custom title bar restored/native | [Restored](docs/phase-f/chrome-restored.jpg) |
| Keyboard focus and tooltip/browser | [Focus](docs/phase-f/navigation-focus.jpg) |
| Existing account dialog/browser | [Accounts](docs/phase-f/accounts.jpg) |
| Real native Settings save | [Native backend](docs/phase-f/native-backend-settings.jpg) |
| Real native appearance after reload | [Persistence](docs/phase-f/native-backend-reload.jpg) |

## Remaining rollout

Instances and its workspace, local/provider content management, Modrinth, account dialog interiors, About, setup and other dialogs have not received the Phase F interior redesign. They remain accessible through the new shared shell. Owner approval must precede that rollout. Review should also decide whether the still-image Borealis treatment is sufficient or requires a licensed evolving loop. Windows hover Snap Layout support and other-platform native visual acceptance remain separate follow-ups.

PHASE F PILOT STATUS: READY FOR OWNER VISUAL REVIEW

FULL LAUNCHER ROLLOUT: NOT STARTED

PUSH STATUS: NOT PUSHED
