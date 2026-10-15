# UI-5 Settings architecture audit

Audit completed against `3e248361474d6de8eb39df8d50549ace1cd867a8` on
`codex/client-3-production-authority`, before UI implementation. Repository code
is authoritative; historical ARCHITECTURE sections describe older capabilities.

## Architecture and ownership

`SettingsPage.svelte` uses the existing global shell and a local category nav.
Its five session-only keys are General, Appearance, Home, Updates and Discord &
privacy. `navigation.settingsCategory` survives leaving Settings. General reads
live shortcut state; Appearance uses the boot-initialized appearance store;
Home loads the shared widget store; Updates mounts the existing update controller;
Discord mounts one listener, refreshes once, and disposes on category departure.
`HomeWidgetSettings.svelte` exists but is not mounted by Settings. Home has the
authoritative layout editor and the same persistence store.

Launcher config is **schema 7**, `launcher/config.json`: selectedInstanceId,
appearance, homeWidgets, discord. Native code owns validation, atomic writes,
revision conflict refusal and known migrations. Schema 1 adds appearance defaults;
2 adds widget/privacy defaults; 3 adds world/server/address false; 2–4 add Simple;
2–5 add motion speed 50; 4–6 promote to 7. Legacy schema-7 updates.channel is
validated then discarded in memory. Unknown versions/malformed required shapes
fail without replacement. Known-schema unusable cosmetic values normalize by
the existing policy. Unknown widget IDs survive. No UI-5 schema change is needed.

Registry **schema 5** owns desired per-instance configuration. Its editor retains
drafts per instance and saves the complete proposal explicitly; install-affecting
changes require a separate install. Accounts are independent, selected through
the shell dialog, with no account preferences in global Settings. Secrets remain
native. Game/runtime installation and release authority stay outside this phase.

## Complete existing setting/action inventory

Sources: `src-tauri/src/{config,appearance,home_widgets,discord}.rs`,
`instances/settings.rs`, `src/lib/backend.ts`, launcher stores and the actual
Settings/instance/Home components. In tables, **G** = launcher config (global),
**I** = registry desired configuration (one instance), **S** = session-only,
**OS** = system state. All G edits take effect after native success, are atomic,
need no launcher restart and preserve prior authoritative state on error. Errors
are sanitized native messages, rendered with role=alert; pending edits cannot
issue overlapping writes. No existing global setting requires launcher restart.

| Display name / what it changes | Owner; default; current-value source | Allowed values and validation | Effect / errors / existing coverage |
| --- | --- | --- | --- |
| Theme / surface palette | G appearance.theme; aurora-dark; appearance.state | Native catalog aurora-dark, midnight, oled; reject invalid save | Live; appearance_invalid/write errors; appearance Rust and phaseF/appearanceCustomAccent tests |
| Background / shared shell atmosphere | G appearance.background; simple; appearance.state | simple or borealis | Live; same native errors; phaseF and borealis tests |
| Aurora motion / Borealis playback rate | G appearance.auroraMotionSpeed; 50; appearance.state | Integer 0–100; store clamps/rounds finite requests; Rust rejects invalid | Live when active Borealis; zero is slowest, not Off; OS reduce overrides playback; config and borealis tests |
| Accent / selections, focus and primary actions | G appearance.accent; preset violet; native catalog/palette | violet, blue, cyan, green, amber, rose, neutral or custom six-digit hex; custom luminance >= .08; native contrast derivation | Live everywhere via root tokens; invalid color never claims saved; appearance Rust/custom-accent tests |
| Show Instance Details | G homeWidgets; on/small; shared layout by registered ID instance-details | boolean; small/wide/large in Home editor | Live; hide preserves size/order; home_widgets/releasePolish tests |
| Show Content Summary | G homeWidgets; on/small; content-summary | boolean; small/wide/large | Local counts; same persistence/coverage |
| Show Session | G homeWidgets; off/small; session | boolean; small/wide only | Process snapshot, not saved history; same coverage |
| Show Playtime | G homeWidgets; on/small; playtime | boolean; small/wide/large | Local durable totals; historyWidgets tests |
| Show Recent Worlds | G homeWidgets; on/small; recent-worlds | boolean; small/wide/large | Native local history/Quick Launch; historyWidgets tests |
| Show Recent Servers | G homeWidgets; on/small; recent-servers | boolean; small/wide/large | Local history plus existing bounded enrichment; serverEnrichmentWidgets tests |
| Show Skin Library | G homeWidgets; on/wide; skin-manager | boolean; small/wide/large | Account/library access; cosmeticsWidgets tests |
| Show Cape Selector | G homeWidgets; on/small; cape-selector | boolean; small/wide/large | Account-owned cape access; cosmeticsWidgets tests |
| Widget order/size / arrangement | G ordered layout; above defaults; homeWidgets.layout | Registered size vocabulary; unique bounded lowercase IDs, <=64 entries; unknown registrations preserved | Saved from Home editor; native damage/write errors preserve layout; home_widgets/releasePolish tests |
| Edit Home | S editing=false; homeWidgets.editing | Navigation action, no config write itself | Opens existing Home editor; no second renderer |
| Reset layout | G layout; native defaults above; reset_home_widgets result | Existing native reset, intentionally changes all layout choices | Add explicit confirmation describing order/size/visibility; error retains prior layout |
| Desktop shortcut | OS live status, no default preference; desktopIntegration.state | Windows supported; create/remove only for installed production executable and proven owned slot | Immediate OS action, no restart; conflict/unknown remain noneditable; shortcuts Rust/generalSettings tests |
| Start menu shortcut | OS read-only, installer-owned | Available/not created/conflict/unknown | Refresh reads actual state; no create/delete action |
| Refresh shortcut status | S busy/error; native get_desktop_integration | Read-only | No saved preference; native unavailable shows error |
| Check For Updates / Update | S update controller; idle; native overview | Current owner-published offers only; signed launcher updater; explicit application | Existing startup check, no poll/auto-install; failures/retries and progress; updateController/productSurfaces tests |
| Discord Rich Presence | G discord.enabled; false; discord.state.preferences | boolean | Enable connects automatically; off clears and disconnects; errors never block Play; discord worker/releasePolish tests |
| Minecraft version disclosure | G discord.minecraftVersion; false | boolean | Exact installed game version only; all details opt-in |
| Platform / loader disclosure | G discord.platform; false | boolean | Native projected installed platform (not limited to Vanilla/Fabric); native discord tests |
| Aurora active disclosure | G discord.auroraActive; false | boolean | Verified active-at-launch bootstrap, not live health |
| Elapsed play time disclosure | G discord.elapsedTime; false | boolean | Supervised process start; not saved playtime totals |
| Instance name disclosure | G discord.instanceName; false | boolean | User display name; sanitized and bounded natively |
| Show World | G discord.world; false | boolean | Compatible authenticated Client activity only |
| Show Server | G discord.server; false | boolean | Saved/display name; addresses suppressed absent address consent |
| Show Server Address | G discord.serverAddress; false | boolean; editing disabled when Show Server off | Effective disclosure requires both flags; turning off server preserves address preference; discord privacy tests |
| Discord status/application/assets | Native runtime/build, read-only; configured/connection DTO | Aurora-owned build application ID, fixed asset key; no user credential/config field | Truthful setup-required/not-detected/closed/failed/connected; no OAuth/link/manual connection UI; existing tests |
| Reduced motion | OS media query; current match | Follow OS only; no persisted override | Static Borealis and removed nonessential transitions; borealis/windowActivity tests |
| Reduced transparency | OS media query; current match | Follow OS only; no persisted override | Opaque panels/solid accent through existing CSS; UI-2/3/4 browser tests |
| Contrast / text size | OS/browser presentation; no stored choice | Existing OLED theme and browser/OS preferences; no scaling/high-contrast preference field | Text scaling is acceptance input, not an invented slider; current forced-colors coverage needs inspection |
| Launcher version/platform | Native application status, read-only | launcher.status; never package-version fallback | Existing boot read reused; unavailable remains unavailable; no private path needed |

### Instance and account surfaces (kept in their existing owners)

| Setting/action | Default/source, values and validation | Persistence and effect / errors / coverage |
| --- | --- | --- |
| Instance name | Registry displayName, generated independently from ID; nonblank bounded name | Explicit rename modal; metadata only; stale-original refusal; instanceManagement tests |
| Minecraft version | Exact desired version; creation uses official catalog | I; explicit Save then install if changed; stale state; configurationChoices/native lifecycle tests |
| Include snapshots | S false | Discovery filter only, no persisted global default |
| Mod loader | I tagged kind; backend capability catalog | Vanilla/Fabric/NeoForge/Forge where implemented; Aurora association restricts changes; install required |
| Loader version policy | I automatic or exact pinned; native compatible list | Explicit Save then install; no silent installed upgrade; configurationChoices tests |
| Memory (MiB) | I 2048, current draft/config | Whole MiB 512–32768; explicit Save, next game launch; instance settings Rust tests |
| Additional JVM arguments | I empty raw text | Native parser, bounded text, disallow heap/classpath/executable conflicts; Save then next launch; native argument tests |
| Custom window size | I null; enabling draft initializes 854×480 | Width/height integers 100–7680; Save then next launch; no fullscreen control |
| Managed Java | Resolved game plan, no user-selected runtime | Read-only required major/status; Check/Install/repair actions remain native in workspace; no custom executable or system fallback |
| Instance validation, recovery backups, reinstall, deletion | Existing workspace maintenance and confirmed danger actions | No global duplicates, no new cleanup; lifecycle/retention tests |
| Aurora association/content, content release policy/pins, resource-pack activation | Existing Mods/content workspace native ownership | Reviewed transitions/provider transactions only; no global copy |
| Selected account / session checks / removal | Native accounts schema 1; shell account dialog | Independent of instances; secrets OS-backed; no new Settings preference |
| Skin/cape library, favorites/name/model, Apply/Equip | Native cosmetics schema 2 and selected account | Dedicated Appearance destination; local library vs explicit remote mutation; UI-4/L3 tests |
| Player camera/model/layers/cape preview, search/sort/tab | S; dedicated Appearance page | Transient, never launcher-theme configuration; UI-4 tests |

## Proposed hierarchy and shared presentation

One local category sidebar becomes wrapping section buttons at narrow widths.
Stable existing keys remain; General is labeled Launcher, Home is labeled Home &
Widgets. Appearance, Home & Widgets, Launcher, Minecraft & Java, Updates,
Discord & privacy, Accessibility and About each have implemented content.
Minecraft & Java links to the selected instance's Settings and Overview (or the
Library when none selected). Accessibility reports supported system preferences
and links to existing appearance choices. About reuses native version/platform.
Updates stays the canonical separate category for existing deep links.

Compact shared rows and status feedback; native checkboxes/radios/selects/ranges
remain. No global Save. Appearance tiles reuse current tokens/renderer, static
miniatures, native catalog descriptions; motion shows a numeric value. Custom
picker commits on completed color change, preserving direct picker semantics.
Widget checklist reuses widgetCatalog and HomeWidgetsStore; reset is confirmed.
No extra store reads, preview subscriptions or widget renderer are needed.

## Risks and accessibility weaknesses

Hidden color inputs currently have zero-size geometry/pointer suppression; make
the picker a real focusable target. Descriptions are mostly visual without
aria-describedby. Loading can show application setup wording before Discord
actually responds. Save feedback is inconsistent; native checkboxes may visually
toggle ahead of a failed write. Reconcile controls from authoritative state after
completion, announce failure and retain a retry path. Slider/custom-color drafts
must retain failed input for retry, separately from the saved value.

Preserve defaults, category deep links, all config/registry schemas and serialization.
Do not conflate launcher Appearance with account skin/cape Appearance. Do not
claim restart required for live preferences or imply global memory/Java settings.
Default native window is 1120×760; configured minimum remains 720×520. UI-4 has
owner visual approval but exact native 1024×768/minimum acceptance is **PARTIAL**.

## Test and native acceptance plan

Capture matched before/after at identical dimensions/fixtures. Pilot shell/Launcher
and Appearance before remaining categories. Check navigation/arrow keys/scroll
restoration, authoritative current values, toggle/radio/range/color writes and
failure recovery, widget hidden/unknown order/sizing preservation, reset cancel,
Discord opt-ins/dependency/unavailable states, scope links and native About values.
Use 1440×900, 1280×800, 1024×768, 720×520 and measured native minimum at 100/200%
text. Cover long labels, focus, reduced motion/transparency, alternative accent,
forced colors, dialogs, and horizontal containment. Run npm check/test/build plus
UI-1/2/2B/3/3.1/4 suites without overwriting their evidence. Reuse native legacy
config tests with deterministic fixtures. No Rust product change is planned.

Build debug executable with compiled production frontend (no installer). Fresh
marked direct-temp managed/WebView roots, no copied owner state/accounts. Verify
native Settings categories and immediate preference persistence across clean exit
and restart. No sign-in, live Discord, update application or shortcut mutation.
Use actual CSS client/DPR observations, not requested window sizes. Separately
inspect UI-4 only if safely possible; missing exact sizes stays PARTIAL. Preserve
every worktree and protected-file hash before each focused local commit.
