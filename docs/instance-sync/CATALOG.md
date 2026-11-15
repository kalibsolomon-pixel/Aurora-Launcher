# Instance sync format catalog

Verified 2026-11-15, before phase 1. DECISIONS.md governs classification.

This is a format/portability decision record, not permission for the legacy opaque config writer to copy these files. Runtime catalogs must be explicit and release-bound. Unlisted keys, values outside a verified codec, unknown versions, unknown Client releases and unknown nested members remain per-instance with a visible reason. No Minecraft or launcher was run to collect this evidence; no owner instance/data was opened.

## Evidence and method

Minecraft client JARs came from the read-only Gradle cache `C:/Users/kalib/.gradle/caches/fabric-loom/<version>/minecraft-client.jar`. Each SHA-1 matches its official Mojang version document. The version documents were fetched over HTTPS from their manifest URLs, checked against manifest SHA-1 and compared semantically with cached metadata. These are Mojang SHA-1 identities, not invented SHA-256 publisher checks.

| Version | Official client SHA-1 | Options data version |
|---|---|---|
| 26.1.2 | `4e618f09a0c649dde3fdf829df443ce0b8831e65` | 4790 |
| 26.2 | `2dc72797acbc1b63fc16a11c4ac393605f453754` | 4903 |
| 26.3 | `e877b6a07acd633fb3bb475002175cec036e7b87` | 5023 |

`tools/instance-sync/inspect_formats.py` reads JVM class files in Python without starting Java. It records serialized field names and FieldAccess overloads directly from `Options.processOptions` / `processDumpedOptions`, initializer instructions, generic signatures and codec/enum/input support classes. Decompiled 26.2/26.3 source JARs in the Client Loom cache were inspected as a second representation. 26.1.2 decisions use its actual class bytecode, not an assumed copy of 26.2. Evidence and repeatable tooling are checked in under `docs/instance-sync/` and `tools/instance-sync/`.

Aurora source: `C:/Dev/Aurora-Client`, read-only. Published tag `v3.5.0` resolves to `14b8690454ccd84e538ee292c6685293d42c1912`; development HEAD is `5d7e9f8a90ed243fd93036361f8579dfa02e5c99`. `gradle/minecraft-versions.json` supports all three requested lines. A release tag verifies the source format; installed artifact identity must still match the release adapter before writes. The development-only Pulsar additions are not evidence that the published release supports them.

## Vanilla per-key decisions

Each version cell gives its independently inspected codec; `absent` means do not inject an alias. Effective range checks must be strict; the game's broader lenient reader is not permission to silently clamp or wrap an unsupported preference. No graphics opt-in: verified graphics/tuning fields sync by default under the owner decision.

| Serialized key | 26.1.2 | 26.2 | 26.3 | Decision / reason |
|---|---|---|---|---|
| `advancedItemTooltips` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Sync preferences; skip absent versions |
| `allowCursorChanges` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `allowServerListing` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Local: Privacy choice |
| `ao` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync graphics/performance; skip absent versions |
| `attackIndicator` | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | Sync preferences; skip absent versions |
| `autoJump` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `autoSuggestions` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `backgroundForChatOnly` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `biomeBlendRadius` | JSON integer [0,7]; validate effective range, not codec's extra endpoint | JSON integer [0,7]; validate effective range, not codec's extra endpoint | JSON integer [0,7]; validate effective range, not codec's extra endpoint | Sync graphics/performance; skip absent versions |
| `bobView` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `chatColors` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `chatDelay` | JSON double [0,6] | JSON double [0,6] | JSON double [0,6] | Sync preferences; skip absent versions |
| `chatHeightFocused` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `chatHeightUnfocused` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `chatLineSpacing` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `chatLinks` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `chatLinksPrompt` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `chatOpacity` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `chatScale` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `chatVisibility` | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | Sync preferences; skip absent versions |
| `chatWidth` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `chunkSectionFadeInTime` | JSON double [0,2] | JSON double [0,2] | JSON double [0,2] | Sync graphics/performance; skip absent versions |
| `cloudRange` | JSON integer [2,128]; validate effective range, not codec's extra endpoint | JSON integer [2,128]; validate effective range, not codec's extra endpoint | JSON integer [2,128]; validate effective range, not codec's extra endpoint | Sync graphics/performance; skip absent versions |
| `ctrlClickEmulatesRightClick` | absent | absent | JSON boolean true/false | Local: Platform-specific behavior |
| `cutoutLeaves` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync graphics/performance; skip absent versions |
| `damageTiltStrength` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `darkMojangStudiosBackground` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `darknessEffectScale` | JSON double [0,1]; legacy boolean accepted as 0/1; saved=sqrt(effective), effective=saved^2 | JSON double [0,1]; legacy boolean accepted as 0/1; saved=sqrt(effective), effective=saved^2 | JSON double [0,1]; legacy boolean accepted as 0/1; saved=sqrt(effective), effective=saved^2 | Sync preferences; skip absent versions |
| `debugGuiScale` | absent | absent | JSON integer [-1,2147483646]; target clamps to window maximum, 0=auto | Local: Diagnostic display |
| `directionalAudio` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `discrete_mouse_scroll` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `enableVsync` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync graphics/performance; skip absent versions |
| `entityDistanceScaling` | JSON double [0.5,5] | JSON double [0.5,5] | JSON double [0.5,5] | Sync graphics/performance; skip absent versions |
| `entityShadows` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync graphics/performance; skip absent versions |
| `exclusiveFullscreen` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync graphics/performance; skip absent versions |
| `forceUnicodeFont` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `fov` | JSON double [-1,1]; degrees=int(saved*40+70), saved=(degrees-70)/40 | JSON double [-1,1]; degrees=int(saved*40+70), saved=(degrees-70)/40 | JSON double [-1,1]; degrees=int(saved*40+70), saved=(degrees-70)/40 | Sync preferences; skip absent versions |
| `fovEffectScale` | JSON double [0,1] | JSON double [0,1] | JSON double [0,1] | Sync preferences; skip absent versions |
| `fullscreen` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync graphics/performance; skip absent versions |
| `gamma` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync graphics/performance; skip absent versions |
| `glDebugVerbosity` | signed int32 (raw) | signed int32 (raw) | signed int32 (raw) | Local: Diagnostics |
| `glintSpeed` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `glintStrength` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `graphicsPreset` | JSON string {fast, fancy, fabulous, custom} | JSON string {fast, fancy, fabulous, custom} | JSON string {fast, fancy, fabulous, custom} | Sync graphics/performance; skip absent versions |
| `guiScale` | JSON integer [0,2147483646]; target clamps to window maximum, 0=auto | JSON integer [0,2147483646]; target clamps to window maximum, 0=auto | JSON integer [0,2147483646]; target clamps to window maximum, 0=auto | Sync preferences; skip absent versions |
| `hideLightningFlashes` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `hideMatchedNames` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Local: Trust/privacy choice |
| `hideServerAddress` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Local: Privacy choice |
| `hideSplashTexts` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `highContrast` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `highContrastBlockOutline` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `improvedTransparency` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync graphics/performance; skip absent versions |
| `inGameNotification` | absent | JSON boolean true/false | JSON boolean true/false | Local: Account service notification preference; keep local initially |
| `inactivityFpsLimit` | JSON string {minimized, afk} | JSON string {minimized, afk} | JSON string {minimized, afk} | Sync graphics/performance; skip absent versions |
| `incompatibleResourcePacks` | JSON string array (FieldAccess reader/writer; activation planner only) | JSON string array (FieldAccess reader/writer; activation planner only) | JSON string array (FieldAccess reader/writer; activation planner only) | Local: Target compatibility acknowledgement; never grant acceptance |
| `invertXMouse` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `invertYMouse` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `japaneseGlyphVariants` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `joinedFirstServer` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Local: Local progress |
| `lang` | UTF-8 string (raw) | UTF-8 string (raw) | UTF-8 string (raw) | Sync preferences; skip absent versions |
| `lastServer` | UTF-8 string (raw) | UTF-8 string (raw) | UTF-8 string (raw) | Local: Connection history |
| `macFullscreenMenuVisibility` | absent | absent | JSON boolean true/false | Local: Platform-specific behavior |
| `mainHand` | unverified codec; local | unverified codec; local | unverified codec; local | Local on unverified version(s); verified codecs only elsewhere |
| `maxAnisotropyBit` | JSON integer [1,3]; validate effective range, not codec's extra endpoint | JSON integer [1,3]; validate effective range, not codec's extra endpoint | JSON integer [1,3]; validate effective range, not codec's extra endpoint | Sync graphics/performance; skip absent versions |
| `maxFps` | JSON integer [10,260] | JSON integer [10,260] | JSON integer [10,260] | Sync graphics/performance; skip absent versions |
| `menuBackgroundBlurriness` | JSON integer [0,10]; validate effective range, not codec's extra endpoint | JSON integer [0,10]; validate effective range, not codec's extra endpoint | JSON integer [0,10]; validate effective range, not codec's extra endpoint | Sync graphics/performance; skip absent versions |
| `mipmapLevels` | JSON integer [0,4]; validate effective range, not codec's extra endpoint | JSON integer [0,4]; validate effective range, not codec's extra endpoint | JSON integer [0,4]; validate effective range, not codec's extra endpoint | Sync graphics/performance; skip absent versions |
| `mouseSensitivity` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync controls; skip absent versions |
| `mouseWheelSensitivity` | JSON double [0.01,10]; logMouse(n)=10^(n/100) | JSON double [0.01,10]; logMouse(n)=10^(n/100) | JSON double [0.01,10]; logMouse(n)=10^(n/100) | Sync controls; skip absent versions |
| `musicFrequency` | JSON string {DEFAULT, FREQUENT, CONSTANT} | JSON string {DEFAULT, FREQUENT, CONSTANT} | JSON string {DEFAULT, FREQUENT, CONSTANT} | Sync preferences; skip absent versions |
| `musicToast` | JSON string {never, pause, pause_and_toast} | JSON string {never, pause, pause_and_toast} | JSON string {never, pause, pause_and_toast} | Sync preferences; skip absent versions |
| `narrator` | JSON integer {0, 1, 2, 3} | JSON integer {0, 1, 2, 3} | JSON integer {0, 1, 2, 3} | Sync preferences; skip absent versions |
| `narratorHotkey` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `notificationDisplayTime` | JSON double [0.5,10] | JSON double [0.5,10] | JSON double [0.5,10] | Sync preferences; skip absent versions |
| `onboardAccessibility` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Local: Local onboarding state |
| `onlyShowSecureChat` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Local: Trust/privacy choice |
| `operatorItemsTab` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `overrideHeight` | signed int32 (raw) | signed int32 (raw) | signed int32 (raw) | Local: Machine window override |
| `overrideWidth` | signed int32 (raw) | signed int32 (raw) | signed int32 (raw) | Local: Machine window override |
| `panoramaScrollSpeed` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `particles` | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | Sync graphics/performance; skip absent versions |
| `pauseOnLostFocus` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Sync preferences; skip absent versions |
| `preferredGraphicsBackend` | absent | JSON string {default, opengl, vulkan} | JSON string {default, opengl, vulkan} | Local: Renderer capability differs; no cross-version backend adapter |
| `prioritizeChunkUpdates` | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | Sync graphics/performance; skip absent versions |
| `quitShortcuts` | absent | absent | JSON boolean true/false | Local: Platform-specific behavior |
| `rawMouseInput` | JSON boolean true/false | JSON boolean true/false | absent | Sync controls; skip absent versions |
| `realmsNotifications` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Local: Account service notification preference; keep local initially |
| `reducedDebugInfo` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `renderClouds` | JSON string {false, fast, true}; legacy boolean false=off,true=fancy | JSON string {false, fast, true}; legacy boolean false=off,true=fancy | JSON string {false, fast, true}; legacy boolean false=off,true=fancy | Sync graphics/performance; skip absent versions |
| `renderDistance` | JSON integer [2,16 or 32]; heap/debug-dependent target limit, out-of-range skip | JSON integer [2,16 or 32]; heap/debug-dependent target limit, out-of-range skip | JSON integer [2,16 or 32]; heap/debug-dependent target limit, out-of-range skip | Sync graphics/performance; skip absent versions |
| `resourcePacks` | JSON string array (FieldAccess reader/writer; activation planner only) | JSON string array (FieldAccess reader/writer; activation planner only) | JSON string array (FieldAccess reader/writer; activation planner only) | Sync enablement/order by content identity; one coordinated options output |
| `rotateWithMinecart` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `saveChatDrafts` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `screenEffectScale` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `sharePresence` | absent | JSON string {none, limited, all} | JSON string {none, limited, all} | Local: Privacy choice |
| `showAutosaveIndicator` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `showSubtitles` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync preferences; skip absent versions |
| `simulationDistance` | JSON integer [5,16 or 32]; heap/debug-dependent target limit, out-of-range skip | JSON integer [5,16 or 32]; heap/debug-dependent target limit, out-of-range skip | JSON integer [5,16 or 32]; heap/debug-dependent target limit, out-of-range skip | Sync graphics/performance; skip absent versions |
| `skipMultiplayerWarning` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Local: Warning acknowledgement |
| `soundDevice` | unverified codec; local | unverified codec; local | unverified codec; local | Local: Device identity |
| `sprintWindow` | JSON integer [0,10]; validate effective range, not codec's extra endpoint | JSON integer [0,10]; validate effective range, not codec's extra endpoint | JSON integer [0,10]; validate effective range, not codec's extra endpoint | Sync controls; skip absent versions |
| `startedCleanly` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Local: Startup bookkeeping |
| `syncChunkWrites` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Sync graphics/performance; skip absent versions |
| `telemetryOptInExtra` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Local: Consent |
| `textBackgroundOpacity` | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | JSON double [0,1]; legacy boolean accepted as 0/1 | Sync preferences; skip absent versions |
| `textureFiltering` | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | JSON integer {0, 1, 2} | Sync graphics/performance; skip absent versions |
| `toggleAttack` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `toggleCrouch` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `toggleSprint` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `toggleUse` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync controls; skip absent versions |
| `touchscreen` | JSON boolean true/false | absent | absent | Sync controls; skip absent versions |
| `tutorialStep` | unverified codec; local | unverified codec; local | unverified codec; local | Local: Local tutorial progress |
| `useNativeTransport` | boolean true/false (raw) | boolean true/false (raw) | boolean true/false (raw) | Local: Native transport selection |
| `vignette` | JSON boolean true/false | JSON boolean true/false | JSON boolean true/false | Sync graphics/performance; skip absent versions |
| `weatherRadius` | JSON integer [3,10]; validate effective range, not codec's extra endpoint | JSON integer [3,10]; validate effective range, not codec's extra endpoint | JSON integer [3,10]; validate effective range, not codec's extra endpoint | Sync graphics/performance; skip absent versions |
| `version` | raw integer 4790 | raw integer 4903 | raw integer 5023 | Always local; preserve target data version. For missing options omit until target format is established. |
| `fullscreenResolution` | raw display-mode string | raw display-mode string | raw display-mode string | Always local machine display mode. |

`graphicsMode`, `subtitles` and arbitrary old aliases are not emitted under those names in these versions. Do not translate them speculatively. `touchscreen` exists only in 26.1.2; `rawMouseInput` only in 26.1.2/26.2. Never equate an absent key with false. FPS cap 260 represents the game's unlimited cutoff, not an arbitrary unbounded integer. Render/simulation distances must be validated against the target heap/capabilities. Fullscreen and VSync are preferences; physical resolution overrides remain local.

## Enumerated audio, model parts and vanilla controls

Dynamic prefixes are not open-ended allowlists. Emit only the exact names verified below. `soundCategory_*` uses the Options sound slider UnitDouble codec (finite [0,1], legacy bool decode); model parts use raw booleans. Unknown mod sound channels remain local.

| Family | 26.1.2 | 26.2 | 26.3 | Decision |
|---|---|---|---|---|
| `soundCategory_ambient` | present | present | present | Sync audio |
| `soundCategory_block` | present | present | present | Sync audio |
| `soundCategory_hostile` | present | present | present | Sync audio |
| `soundCategory_master` | present | present | present | Sync audio |
| `soundCategory_music` | present | present | present | Sync audio |
| `soundCategory_neutral` | present | present | present | Sync audio |
| `soundCategory_player` | present | present | present | Sync audio |
| `soundCategory_record` | present | present | present | Sync audio |
| `soundCategory_ui` | present | present | present | Sync audio |
| `soundCategory_voice` | present | present | present | Sync audio |
| `soundCategory_weather` | present | present | present | Sync audio |
| `modelPart_cape` | present | present | present | Sync appearance |
| `modelPart_hat` | present | present | present | Sync appearance |
| `modelPart_jacket` | present | present | present | Sync appearance |
| `modelPart_left_pants_leg` | present | present | present | Sync appearance |
| `modelPart_left_sleeve` | present | present | present | Sync appearance |
| `modelPart_right_pants_leg` | present | present | present | Sync appearance |
| `modelPart_right_sleeve` | present | present | present | Sync appearance |

Vanilla binding values are raw named strings resolved by InputConstants.getKey and saved by KeyMapping.saveString. GLFW 26.1.2/26.2 and SDL 26.3 use different numeric codes. Sync only named keyboard/mouse values present in both verified input tables, including the explicit unknown/unbound name. Numeric fallback names, arbitrary scancodes and missing named keys stay local. Mouse left/right/middle map to different numeric IDs in SDL; preserve their verified names. An action name alone does not establish a supported mod or binding codec.

| Serialized binding | 26.1.2 | 26.2 | 26.3 | Decision |
|---|---|---|---|---|
| `key_key.advancements` | present | present | present | Sync controls with verified named-key codec |
| `key_key.attack` | present | present | present | Sync controls with verified named-key codec |
| `key_key.back` | present | present | present | Sync controls with verified named-key codec |
| `key_key.chat` | present | present | present | Sync controls with verified named-key codec |
| `key_key.command` | present | present | present | Sync controls with verified named-key codec |
| `key_key.debug.clearChat` | present | present | present | Local diagnostic binding |
| `key_key.debug.copyLocation` | present | present | present | Local diagnostic binding |
| `key_key.debug.copyRecreateCommand` | present | present | present | Local diagnostic binding |
| `key_key.debug.crash` | present | present | present | Local diagnostic binding |
| `key_key.debug.debugOptions` | present | present | present | Local diagnostic binding |
| `key_key.debug.dumpDynamicTextures` | present | present | present | Local diagnostic binding |
| `key_key.debug.dumpVersion` | present | present | present | Local diagnostic binding |
| `key_key.debug.focusPause` | present | present | present | Local diagnostic binding |
| `key_key.debug.fpsCharts` | present | present | present | Local diagnostic binding |
| `key_key.debug.improvedTransparency` | absent | absent | present | Local diagnostic binding |
| `key_key.debug.lightmapTexture` | present | present | present | Local diagnostic binding |
| `key_key.debug.modifier` | present | present | present | Local diagnostic binding |
| `key_key.debug.networkCharts` | present | present | present | Local diagnostic binding |
| `key_key.debug.overlay` | present | present | present | Local diagnostic binding |
| `key_key.debug.profiling` | present | present | present | Local diagnostic binding |
| `key_key.debug.profilingChart` | present | present | present | Local diagnostic binding |
| `key_key.debug.reloadChunk` | present | present | present | Local diagnostic binding |
| `key_key.debug.reloadResourcePacks` | present | present | present | Local diagnostic binding |
| `key_key.debug.showAdvancedTooltips` | present | present | present | Local diagnostic binding |
| `key_key.debug.showChunkBorders` | present | present | present | Local diagnostic binding |
| `key_key.debug.showHitboxes` | present | present | present | Local diagnostic binding |
| `key_key.debug.spectate` | present | present | present | Local diagnostic binding |
| `key_key.debug.switchGameMode` | present | present | present | Local diagnostic binding |
| `key_key.drop` | present | present | present | Sync controls with verified named-key codec |
| `key_key.forward` | present | present | present | Sync controls with verified named-key codec |
| `key_key.friends` | absent | present | present | Sync controls with verified named-key codec |
| `key_key.fullscreen` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.1` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.2` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.3` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.4` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.5` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.6` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.7` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.8` | present | present | present | Sync controls with verified named-key codec |
| `key_key.hotbar.9` | present | present | present | Sync controls with verified named-key codec |
| `key_key.inventory` | present | present | present | Sync controls with verified named-key codec |
| `key_key.jump` | present | present | present | Sync controls with verified named-key codec |
| `key_key.left` | present | present | present | Sync controls with verified named-key codec |
| `key_key.loadToolbarActivator` | present | present | present | Sync controls with verified named-key codec |
| `key_key.pickItem` | present | present | present | Sync controls with verified named-key codec |
| `key_key.playerlist` | present | present | present | Sync controls with verified named-key codec |
| `key_key.quickActions` | present | present | present | Sync controls with verified named-key codec |
| `key_key.right` | present | present | present | Sync controls with verified named-key codec |
| `key_key.saveToolbarActivator` | present | present | present | Sync controls with verified named-key codec |
| `key_key.screenshot` | present | present | present | Sync controls with verified named-key codec |
| `key_key.smoothCamera` | present | present | present | Sync controls with verified named-key codec |
| `key_key.sneak` | present | present | present | Sync controls with verified named-key codec |
| `key_key.socialInteractions` | present | present | present | Sync controls with verified named-key codec |
| `key_key.spectatorHotbar` | present | present | present | Sync controls with verified named-key codec |
| `key_key.spectatorOutlines` | present | present | present | Sync controls with verified named-key codec |
| `key_key.sprint` | present | present | present | Sync controls with verified named-key codec |
| `key_key.swapOffhand` | present | present | present | Sync controls with verified named-key codec |
| `key_key.toggleGui` | present | present | present | Sync controls with verified named-key codec |
| `key_key.togglePerspective` | present | present | present | Sync controls with verified named-key codec |
| `key_key.toggleSpectatorShaderEffects` | present | present | present | Sync controls with verified named-key codec |
| `key_key.use` | present | present | present | Sync controls with verified named-key codec |

## Aurora JSON projection

`config/aurora.json` is a pretty-printed Gson object of public mutable AuroraConfig fields, no top-level schema version. `config/profiles/<sanitized name>.json` is a Gson Profile object. AuroraConfig.load runs migration pre-passes; ProfileManager.load subsequently applies activeProfile and can override config values. Capture and apply must validate the coherent config/profile/controls set, with profile-scoped active values taking precedence. Match exact release adapters, preserve unknown members, and reject malformed/ambiguous JSON rather than round-tripping it through a lossy DTO.

Gson codecs: boolean is JSON boolean; int/long are exact bounded JSON integers; float/double are finite JSON numbers; String is a JSON string; enums use exact Java constant names; arrays/lists/sets are JSON arrays; maps keyed by strings are JSON objects whose values use their declared codec. ThemeDefinition, ModuleLayout, Waypoint and custom nested types require their release's field schema, not arbitrary JSON blob copying. Any unverified nested member stays local. Empty and absent/null values must not be conflated.

The following table enumerates every top-level field of both reviewed AuroraConfig revisions. `Profile` means included by the Client's actual ProfileFieldSet, not a separate portability policy. All sync classifications apply both to config and matching profile fields. Counters, world data and guards are protected even if a Client profile includes them. perf fields sync in config although Client intentionally excludes them from profiles; do not invent profile entries the Client ignores.

| Field | Gson Java value codec | v3.5.0 / dev HEAD line | Client scope | Projection decision |
|---|---|---|---|---|
| `activeFont` | `FontType` | 219 / 225 | Profile | Sync preferences; validate nested members by release |
| `activeProfile` | `String` | 132 / 138 | Global | Sync selection with coherent imported definitions |
| `adaptiveRenderSleeping` | `boolean` | 468 / 474 | Profile | Sync preferences; validate nested members by release |
| `animationCurve` | `AnimationCurve` | 420 / 426 | Profile | Sync preferences; validate nested members by release |
| `armorAlertEnabled` | `boolean` | 568 / 574 | Profile | Sync preferences; validate nested members by release |
| `armorAlertSound` | `boolean` | 570 / 576 | Profile | Sync preferences; validate nested members by release |
| `armorAlertSoundChoice` | `ArmorAlertSoundChoice` | 571 / 577 | Profile | Sync preferences; validate nested members by release |
| `armorAlertThresholdPct` | `int` | 569 / 575 | Profile | Sync preferences; validate nested members by release |
| `armorDurabilityFormat` | `ArmorDurabilityFormat` | 563 / 569 | Profile | Sync preferences; validate nested members by release |
| `armorHorizontal` | `boolean` | 574 / 580 | Profile | Sync preferences; validate nested members by release |
| `armorHudBgColor` | `int` | 793 / 799 | Profile | Sync preferences; validate nested members by release |
| `armorHudBgMode` | `HudBackground` | 792 / 798 | Profile | Sync preferences; validate nested members by release |
| `armorHudEnabled` | `boolean` | 561 / 567 | Profile | Sync preferences; validate nested members by release |
| `armorHudTextColor` | `Integer` | 502 / 508 | Profile | Sync preferences; validate nested members by release |
| `armorHudThemeCorners` | `boolean` | 794 / 800 | Profile | Sync preferences; validate nested members by release |
| `armorShowDurabilityBars` | `boolean` | 572 / 578 | Profile | Sync preferences; validate nested members by release |
| `blockOverlayEnabled` | `boolean` | 651 / 657 | Profile | Sync preferences; validate nested members by release |
| `blockOverlayFillColor` | `int` | 655 / 661 | Profile | Sync preferences; validate nested members by release |
| `blockOverlayFillEnabled` | `boolean` | 654 / 660 | Profile | Sync preferences; validate nested members by release |
| `blockOverlayLineWidth` | `double` | 658 / 664 | Profile | Sync preferences; validate nested members by release |
| `blockOverlayMode` | `BlockOverlayMode` | 656 / 662 | Profile | Sync preferences; validate nested members by release |
| `blockOverlayOutlineColor` | `int` | 653 / 659 | Profile | Sync preferences; validate nested members by release |
| `blockOverlayOutlineEnabled` | `boolean` | 652 / 658 | Profile | Sync preferences; validate nested members by release |
| `blockOverlayRainbowSpeed` | `float` | 657 / 663 | Profile | Sync preferences; validate nested members by release |
| `blurTestKey` | `int` | 1235 / 1241 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `containerPreviewEnabled` | `boolean` | 1042 / 1048 | Profile | Sync preferences; validate nested members by release |
| `containerPreviewEnderChest` | `boolean` | 1046 / 1052 | Profile | Sync preferences; validate nested members by release |
| `containerPreviewOtherBlocks` | `boolean` | 1050 / 1056 | Profile | Sync preferences; validate nested members by release |
| `containerPreviewShowCounts` | `boolean` | 1052 / 1058 | Profile | Sync preferences; validate nested members by release |
| `containerPreviewShulker` | `boolean` | 1044 / 1050 | Profile | Sync preferences; validate nested members by release |
| `cpsBgColor` | `int` | 789 / 795 | Profile | Sync preferences; validate nested members by release |
| `cpsBgMode` | `HudBackground` | 788 / 794 | Profile | Sync preferences; validate nested members by release |
| `cpsEnabled` | `boolean` | 560 / 566 | Profile | Sync preferences; validate nested members by release |
| `cpsTextColor` | `Integer` | 501 / 507 | Profile | Sync preferences; validate nested members by release |
| `cpsThemeCorners` | `boolean` | 790 / 796 | Profile | Sync preferences; validate nested members by release |
| `crosshairAttackIndicatorEnabled` | `boolean` | 952 / 958 | Profile | Sync preferences; validate nested members by release |
| `crosshairAttackIndicatorSize` | `int` | 953 / 959 | Profile | Sync preferences; validate nested members by release |
| `crosshairColor` | `int` | 948 / 954 | Profile | Sync preferences; validate nested members by release |
| `crosshairCustomHeight` | `int` | 962 / 968 | Profile | Sync preferences; validate nested members by release |
| `crosshairCustomPixels` | `boolean[]` | 960 / 966 | Profile | Sync preferences; validate nested members by release |
| `crosshairCustomWidth` | `int` | 961 / 967 | Profile | Sync preferences; validate nested members by release |
| `crosshairEnabled` | `boolean` | 946 / 952 | Profile | Sync preferences; validate nested members by release |
| `crosshairGap` | `int` | 951 / 957 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorColor` | `int` | 959 / 965 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorCustomHeight` | `int` | 965 / 971 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorCustomPixels` | `boolean[]` | 963 / 969 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorCustomWidth` | `int` | 964 / 970 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorEnabled` | `boolean` | 954 / 960 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorGap` | `int` | 958 / 964 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorSize` | `int` | 956 / 962 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorStyle` | `CrosshairStyle` | 955 / 961 | Profile | Sync preferences; validate nested members by release |
| `crosshairIndicatorThickness` | `int` | 957 / 963 | Profile | Sync preferences; validate nested members by release |
| `crosshairSize` | `int` | 949 / 955 | Profile | Sync preferences; validate nested members by release |
| `crosshairStyle` | `CrosshairStyle` | 947 / 953 | Profile | Sync preferences; validate nested members by release |
| `crosshairThickness` | `int` | 950 / 956 | Profile | Sync preferences; validate nested members by release |
| `customTitleScreen` | `boolean` | 155 / 161 | Profile | Sync preferences; validate nested members by release |
| `damageTiltEnabled` | `boolean` | 1029 / 1035 | Profile | Sync preferences; validate nested members by release |
| `damageTiltStrength` | `double` | 1030 / 1036 | Profile | Sync preferences; validate nested members by release |
| `deathWaypointColor` | `int` | 931 / 937 | Profile | Sync preferences; validate nested members by release |
| `deathWaypointEnabled` | `boolean` | 921 / 927 | Profile | Sync preferences; validate nested members by release |
| `deathWaypointMaxCount` | `int` | 929 / 935 | Profile | Sync preferences; validate nested members by release |
| `deathWaypointReplacesPrevious` | `boolean` | 923 / 929 | Profile | Sync preferences; validate nested members by release |
| `disableVSync` | `boolean` | 436 / 442 | Profile | Sync preferences; validate nested members by release |
| `effectExpiryAlertEnabled` | `boolean` | 584 / 590 | Profile | Sync preferences; validate nested members by release |
| `effectExpiryExcludedEffects` | `Set<String>` | 606 / 612 | Profile | Sync preferences; validate nested members by release |
| `effectExpiryIncludedEffects` | `List<String>` | 597 / 603 | Profile | Sync preferences; validate nested members by release |
| `effectExpiryThresholdSeconds` | `int` | 586 / 592 | Profile | Sync preferences; validate nested members by release |
| `entityMovementSmoothingEnabled` | `boolean` | 395 / 401 | Profile | Sync preferences; validate nested members by release |
| `entityMovementSmoothingStrength` | `double` | 397 / 403 | Profile | Sync preferences; validate nested members by release |
| `fightStatsPlaytimeSeconds` | `long` | 1219 / 1225 | Global | Local: lifetime counters |
| `fightStatsTotalFights` | `int` | 1217 / 1223 | Global | Local: lifetime counters |
| `fixHeldItemSeams` | `boolean` | 469 / 475 | Profile | Sync preferences; validate nested members by release |
| `fogDistanceMultiplier` | `double` | 762 / 768 | Profile | Sync preferences; validate nested members by release |
| `frameCapFps` | `int` | 349 / 355 | Profile | Sync tuning |
| `framePacerLowCpuMode` | `boolean` | 335 / 341 | Profile | Sync tuning |
| `framePacerParkThresholdMicros` | `int` | 334 / 340 | Profile | Sync tuning |
| `framePacerSpinThresholdMicros` | `int` | 333 / 339 | Profile | Sync tuning |
| `framePacingStrategy` | `PacingStrategy` | 332 / 338 | Profile | Sync tuning |
| `freeLookEnabled` | `boolean` | 620 / 626 | Profile | Sync preferences; validate nested members by release |
| `freeLookKey` | `int` | 830 / 836 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `freeLookSensitivity` | `double` | 621 / 627 | Profile | Sync preferences; validate nested members by release |
| `fullBright` | `boolean` | 324 / 330 | Profile | Sync preferences; validate nested members by release |
| `fullBrightGamma` | `int` | 325 / 331 | Profile | Sync preferences; validate nested members by release |
| `glintColor` | `int` | 483 / 489 | Profile | Sync preferences; validate nested members by release |
| `glintColorAlternative` | `boolean` | 479 / 485 | Profile | Sync preferences; validate nested members by release |
| `glintColorEnabled` | `boolean` | 477 / 483 | Profile | Sync preferences; validate nested members by release |
| `glintColorIntensity` | `int` | 481 / 487 | Profile | Sync preferences; validate nested members by release |
| `glintColorTintArmor` | `boolean` | 484 / 490 | Profile | Sync preferences; validate nested members by release |
| `guiFpsLimit` | `int` | 284 / 290 | Profile | Sync preferences; validate nested members by release |
| `heldItemInflation` | `double` | 470 / 476 | Profile | Sync preferences; validate nested members by release |
| `hitColor` | `int` | 472 / 478 | Profile | Sync preferences; validate nested members by release |
| `hitColorEnabled` | `boolean` | 471 / 477 | Profile | Sync preferences; validate nested members by release |
| `hitColorTintArmor` | `boolean` | 473 / 479 | Profile | Sync preferences; validate nested members by release |
| `hitColorTintArmorTrims` | `boolean` | 475 / 481 | Profile | Sync preferences; validate nested members by release |
| `hitboxColor` | `int` | 979 / 985 | Profile | Sync preferences; validate nested members by release |
| `hitboxEyeLine` | `boolean` | 976 / 982 | Profile | Sync preferences; validate nested members by release |
| `hitboxLineWidth` | `double` | 980 / 986 | Profile | Sync preferences; validate nested members by release |
| `hitboxLookDirection` | `boolean` | 977 / 983 | Profile | Sync preferences; validate nested members by release |
| `hitboxLookLength` | `double` | 978 / 984 | Profile | Sync preferences; validate nested members by release |
| `hitboxProjectileColor` | `int` | 982 / 988 | Profile | Sync preferences; validate nested members by release |
| `hitboxProjectileColorEnabled` | `boolean` | 981 / 987 | Profile | Sync preferences; validate nested members by release |
| `hitboxProjectileLineWidth` | `double` | 983 / 989 | Profile | Sync preferences; validate nested members by release |
| `hitboxTargetColor` | `int` | 987 / 993 | Profile | Sync preferences; validate nested members by release |
| `hitboxTargetEnabled` | `boolean` | 984 / 990 | Profile | Sync preferences; validate nested members by release |
| `hitboxTargetEyeLine` | `boolean` | 985 / 991 | Profile | Sync preferences; validate nested members by release |
| `hitboxTargetLookDirection` | `boolean` | 986 / 992 | Profile | Sync preferences; validate nested members by release |
| `hitboxToggleKey` | `int` | 836 / 842 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `hitregAlertDelays` | `boolean` | 1152 / 1158 | Profile | Sync preferences; validate nested members by release |
| `hitregAlertGhosts` | `boolean` | 1154 / 1160 | Profile | Sync preferences; validate nested members by release |
| `hitregAlertInconsistencies` | `boolean` | 1156 / 1162 | Profile | Sync preferences; validate nested members by release |
| `hitregColorCrossFar` | `int` | 1191 / 1197 | Profile | Sync preferences; validate nested members by release |
| `hitregColorCrossFarWithHitbox` | `int` | 1193 / 1199 | Profile | Sync preferences; validate nested members by release |
| `hitregColorCrossNear` | `int` | 1192 / 1198 | Profile | Sync preferences; validate nested members by release |
| `hitregColorCrossNearWithHitbox` | `int` | 1194 / 1200 | Profile | Sync preferences; validate nested members by release |
| `hitregColorFloor` | `int` | 1209 / 1215 | Profile | Sync preferences; validate nested members by release |
| `hitregColorGrid` | `int` | 1208 / 1214 | Profile | Sync preferences; validate nested members by release |
| `hitregColorHitboxFar` | `int` | 1195 / 1201 | Profile | Sync preferences; validate nested members by release |
| `hitregColorHitboxNear` | `int` | 1196 / 1202 | Profile | Sync preferences; validate nested members by release |
| `hitregColorJumpReset` | `int` | 1206 / 1212 | Profile | Sync preferences; validate nested members by release |
| `hitregColorPerfectHit` | `int` | 1207 / 1213 | Profile | Sync preferences; validate nested members by release |
| `hitregColorServerHitbox` | `int` | 1197 / 1203 | Profile | Sync preferences; validate nested members by release |
| `hitregColorTheirJumpFar` | `int` | 1204 / 1210 | Profile | Sync preferences; validate nested members by release |
| `hitregColorTheirJumpNear` | `int` | 1205 / 1211 | Profile | Sync preferences; validate nested members by release |
| `hitregColorTheirReachFar` | `int` | 1200 / 1206 | Profile | Sync preferences; validate nested members by release |
| `hitregColorTheirReachNear` | `int` | 1201 / 1207 | Profile | Sync preferences; validate nested members by release |
| `hitregColorYourJumpFar` | `int` | 1202 / 1208 | Profile | Sync preferences; validate nested members by release |
| `hitregColorYourJumpNear` | `int` | 1203 / 1209 | Profile | Sync preferences; validate nested members by release |
| `hitregColorYourReachFar` | `int` | 1198 / 1204 | Profile | Sync preferences; validate nested members by release |
| `hitregColorYourReachNear` | `int` | 1199 / 1205 | Profile | Sync preferences; validate nested members by release |
| `hitregCustomHitreg` | `boolean` | 1137 / 1143 | Profile | Sync preferences; validate nested members by release |
| `hitregDelayMs` | `int` | 1139 / 1145 | Profile | Sync preferences; validate nested members by release |
| `hitregEnabled` | `boolean` | 1116 / 1122 | Profile | Sync preferences; validate nested members by release |
| `hitregFloorGridSize` | `int` | 1147 / 1153 | Profile | Sync preferences; validate nested members by release |
| `hitregMetronome` | `int` | 1145 / 1151 | Profile | Sync preferences; validate nested members by release |
| `hitregMuffleAmount` | `double` | 1141 / 1147 | Profile | Sync preferences; validate nested members by release |
| `hitregScoreLeftKey` | `int` | 1122 / 1128 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `hitregScoreResetKey` | `int` | 1128 / 1134 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `hitregScoreRightKey` | `int` | 1124 / 1130 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `hitregScoreSendKey` | `int` | 1126 / 1132 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `hitregSettingsKey` | `int` | 1118 / 1124 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `hitregSharpenAmount` | `double` | 1143 / 1149 | Profile | Sync preferences; validate nested members by release |
| `hitregSwitchHandKey` | `int` | 1120 / 1126 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `hitregTrackFights` | `boolean` | 1164 / 1170 | Profile | Sync preferences; validate nested members by release |
| `hitregVoidWorld` | `boolean` | 1186 / 1192 | Profile | Sync preferences; validate nested members by release |
| `hotbarBounceEnabled` | `boolean` | 1038 / 1044 | Profile | Sync preferences; validate nested members by release |
| `hudColor` | `int` | 497 / 503 | Profile | Sync preferences; validate nested members by release |
| `hudEditorKey` | `int` | 833 / 839 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `idleSwayCurve` | `IdleSwayCurve` | 386 / 392 | Profile | Sync preferences; validate nested members by release |
| `idleSwayEnabled` | `boolean` | 382 / 388 | Profile | Sync preferences; validate nested members by release |
| `idleSwayStrength` | `double` | 384 / 390 | Profile | Sync preferences; validate nested members by release |
| `infoBgColor` | `int` | 780 / 786 | Profile | Sync preferences; validate nested members by release |
| `infoBgMode` | `HudBackground` | 779 / 785 | Profile | Sync preferences; validate nested members by release |
| `infoEnabled` | `boolean` | 557 / 563 | Profile | Sync preferences; validate nested members by release |
| `infoModuleTextColor` | `Integer` | 500 / 506 | Profile | Sync preferences; validate nested members by release |
| `infoModuleThemeCorners` | `boolean` | 781 / 787 | Profile | Sync preferences; validate nested members by release |
| `infoPlaytimePerWorld` | `boolean` | 545 / 551 | Profile | Sync preferences; validate nested members by release |
| `infoShowBiome` | `boolean` | 521 / 527 | Profile | Sync preferences; validate nested members by release |
| `infoShowChunkCoords` | `boolean` | 517 / 523 | Profile | Sync preferences; validate nested members by release |
| `infoShowChunkLocal` | `boolean` | 515 / 521 | Profile | Sync preferences; validate nested members by release |
| `infoShowCps` | `boolean` | 535 / 541 | Profile | Sync preferences; validate nested members by release |
| `infoShowDimension` | `boolean` | 523 / 529 | Profile | Sync preferences; validate nested members by release |
| `infoShowFacing` | `boolean` | 513 / 519 | Profile | Sync preferences; validate nested members by release |
| `infoShowFrameTime` | `boolean` | 529 / 535 | Profile | Sync preferences; validate nested members by release |
| `infoShowLightAtFeet` | `boolean` | 525 / 531 | Profile | Sync preferences; validate nested members by release |
| `infoShowMemory` | `boolean` | 531 / 537 | Profile | Sync preferences; validate nested members by release |
| `infoShowPing` | `boolean` | 533 / 539 | Profile | Sync preferences; validate nested members by release |
| `infoShowPlaytime` | `boolean` | 539 / 545 | Profile | Sync preferences; validate nested members by release |
| `infoShowYRelSea` | `boolean` | 519 / 525 | Profile | Sync preferences; validate nested members by release |
| `inputSamplingDecoupled` | `boolean` | 434 / 440 | Profile | Sync preferences; validate nested members by release |
| `itemPhysicsEnabled` | `boolean` | 1056 / 1062 | Profile | Sync preferences; validate nested members by release |
| `itemPhysicsFlatOnGround` | `boolean` | 1058 / 1064 | Profile | Sync preferences; validate nested members by release |
| `itemPhysicsTumbleStrength` | `double` | 1060 / 1066 | Profile | Sync preferences; validate nested members by release |
| `itemScaleEnabled` | `boolean` | 704 / 710 | Profile | Sync preferences; validate nested members by release |
| `itemScales` | `Map<String, ItemScaleData>` | 707 / 713 | Profile | Sync preferences; validate nested members by release |
| `keybindVersion` | `int` | 827 / 833 | Profile | Target codec marker; never copy as a preference |
| `keystrokesAccentColor` | `int` | 1090 / 1096 | Profile | Sync preferences; validate nested members by release |
| `keystrokesBgColor` | `int` | 784 / 790 | Profile | Sync preferences; validate nested members by release |
| `keystrokesBgMode` | `HudBackground` | 783 / 789 | Profile | Sync preferences; validate nested members by release |
| `keystrokesBgOpacity` | `double` | 1092 / 1098 | Profile | Sync preferences; validate nested members by release |
| `keystrokesEnabled` | `boolean` | 1071 / 1077 | Profile | Sync preferences; validate nested members by release |
| `keystrokesExtraKeys` | `java.util.List<Integer>` | 1098 / 1104 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `keystrokesShowCps` | `boolean` | 1079 / 1085 | Profile | Sync preferences; validate nested members by release |
| `keystrokesShowExtra` | `boolean` | 1083 / 1089 | Profile | Sync preferences; validate nested members by release |
| `keystrokesShowMiddleMouse` | `boolean` | 1077 / 1083 | Profile | Sync preferences; validate nested members by release |
| `keystrokesShowMouse` | `boolean` | 1075 / 1081 | Profile | Sync preferences; validate nested members by release |
| `keystrokesShowMovement` | `boolean` | 1073 / 1079 | Profile | Sync preferences; validate nested members by release |
| `keystrokesShowSpacebar` | `boolean` | 1081 / 1087 | Profile | Sync preferences; validate nested members by release |
| `keystrokesTextColor` | `Integer` | 507 / 513 | Profile | Sync preferences; validate nested members by release |
| `keystrokesThemeCorners` | `boolean` | 786 / 792 | Profile | Sync preferences; validate nested members by release |
| `lavaClarityEnabled` | `boolean` | 767 / 773 | Profile | Sync preferences; validate nested members by release |
| `lavaFogMultiplier` | `double` | 768 / 774 | Profile | Sync preferences; validate nested members by release |
| `lavaHideOverlay` | `boolean` | 769 / 775 | Profile | Sync preferences; validate nested members by release |
| `lowLatencyRender` | `boolean` | 435 / 441 | Profile | Sync preferences; validate nested members by release |
| `mainHandDefaultScale` | `float` | 705 / 711 | Profile | Sync preferences; validate nested members by release |
| `migratedEffectExpiryExclusions` | `boolean` | 614 / 620 | Global | Local: one-shot migration guard |
| `migratedHitregProperties` | `boolean` | 1226 / 1232 | Global | Local: one-shot migration guard |
| `minimapBgMode` | `HudBackground` | 1286 / 1292 | Profile | Sync preferences; validate nested members by release |
| `minimapBorderColor` | `int` | 1293 / 1299 | Profile | Sync preferences; validate nested members by release |
| `minimapCircular` | `boolean` | 1243 / 1249 | Profile | Sync preferences; validate nested members by release |
| `minimapCoordColor` | `int` | 1295 / 1301 | Profile | Sync preferences; validate nested members by release |
| `minimapEnabled` | `boolean` | 1102 / 1108 | Profile | Sync preferences; validate nested members by release |
| `minimapRotateWithPlayer` | `boolean` | 1249 / 1255 | Profile | Sync preferences; validate nested members by release |
| `minimapShowBiome` | `boolean` | 894 / 900 | Profile | Sync preferences; validate nested members by release |
| `minimapShowCompass` | `boolean` | 1284 / 1290 | Profile | Sync preferences; validate nested members by release |
| `minimapShowCoords` | `boolean` | 1282 / 1288 | Profile | Sync preferences; validate nested members by release |
| `minimapShowEntities` | `boolean` | 1272 / 1278 | Profile | Sync preferences; validate nested members by release |
| `minimapShowHostiles` | `boolean` | 1274 / 1280 | Profile | Sync preferences; validate nested members by release |
| `minimapShowPassives` | `boolean` | 1276 / 1282 | Profile | Sync preferences; validate nested members by release |
| `minimapShowPlayers` | `boolean` | 1278 / 1284 | Profile | Sync preferences; validate nested members by release |
| `minimapShowSelfArrow` | `boolean` | 1280 / 1286 | Profile | Sync preferences; validate nested members by release |
| `minimapShowTerrain` | `boolean` | 1251 / 1257 | Profile | Sync preferences; validate nested members by release |
| `minimapShowWaypointDistance` | `boolean` | 1270 / 1276 | Profile | Sync preferences; validate nested members by release |
| `minimapShowWaypoints` | `boolean` | 1265 / 1271 | Profile | Sync preferences; validate nested members by release |
| `minimapSize` | `int` | 1107 / 1113 | Profile | Sync preferences; validate nested members by release |
| `minimapTerrainAlpha` | `double` | 1253 / 1259 | Profile | Sync preferences; validate nested members by release |
| `minimapThemeCorners` | `boolean` | 1297 / 1303 | Profile | Sync preferences; validate nested members by release |
| `minimapToggleKey` | `int` | 1104 / 1110 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `minimapUseWorldMapCache` | `boolean` | 1263 / 1269 | Profile | Sync preferences; validate nested members by release |
| `minimapZoom` | `int` | 1241 / 1247 | Profile | Sync preferences; validate nested members by release |
| `moduleGridView` | `boolean` | 157 / 163 | Global | Sync preferences; validate nested members by release |
| `moduleLayouts` | `Map<String, ModuleLayout>` | 1007 / 1013 | Profile | Sync preferences; validate nested members by release |
| `nametagPingEnabled` | `boolean` | 559 / 565 | Profile | Sync preferences; validate nested members by release |
| `nametagTotemPopsEnabled` | `boolean` | 820 / 826 | Profile | Sync preferences; validate nested members by release |
| `noFogEnabled` | `boolean` | 761 / 767 | Profile | Sync preferences; validate nested members by release |
| `offHandDefaultScale` | `float` | 706 / 712 | Profile | Sync preferences; validate nested members by release |
| `particleColor` | `Map<String, Integer>` | 1004 / 1010 | Profile | Sync preferences; validate nested members by release |
| `particleControlsEnabled` | `boolean` | 992 / 998 | Profile | Sync preferences; validate nested members by release |
| `particleScale` | `Map<String, Float>` | 997 / 1003 | Profile | Sync preferences; validate nested members by release |
| `particleVisibility` | `Map<String, Boolean>` | 995 / 1001 | Profile | Sync preferences; validate nested members by release |
| `perfDebugEntryCache` | `boolean` | 457 / 463 | Global | Sync tuning |
| `perfFreeSpriteMipmaps` | `boolean` | 463 / 469 | Global | Sync tuning |
| `perfGuiItemBaking` | `boolean` | 461 / 467 | Global | Sync tuning |
| `perfHideReloadOverlay` | `boolean` | 447 / 453 | Global | Sync tuning |
| `perfHideTitleFade` | `boolean` | 449 / 455 | Global | Sync tuning |
| `perfMergeTextDecorations` | `boolean` | 459 / 465 | Global | Sync tuning |
| `perfMinimapRefreshCap` | `boolean` | 465 / 471 | Global | Sync tuning |
| `perfNoProfileWait` | `boolean` | 453 / 459 | Global | Sync tuning |
| `perfSkipLevelLoadDelay` | `boolean` | 451 / 457 | Global | Sync tuning |
| `perfSoundListenerThrottle` | `boolean` | 455 / 461 | Global | Sync tuning |
| `pingHudEnabled` | `boolean` | 558 / 564 | Profile | Sync preferences; validate nested members by release |
| `pingHudTextColor` | `Integer` | 505 / 511 | Profile | Sync preferences; validate nested members by release |
| `playerHealthDisplayMode` | `HealthDisplayMode` | 744 / 750 | Profile | Sync preferences; validate nested members by release |
| `playerHealthIndicators` | `boolean` | 722 / 728 | Profile | Sync preferences; validate nested members by release |
| `playerHealthOnlyAttacked` | `boolean` | 723 / 729 | Profile | Sync preferences; validate nested members by release |
| `playerHealthShowPlayers` | `boolean` | 729 / 735 | Profile | Sync preferences; validate nested members by release |
| `playtimePerWorld` | `java.util.Map<String, Long>` | 554 / 560 | Global | Local: lifetime counters |
| `playtimeTotalMs` | `long` | 547 / 553 | Global | Local: lifetime counters |
| `potionBgColor` | `int` | 805 / 811 | Profile | Sync preferences; validate nested members by release |
| `potionBgMode` | `HudBackground` | 804 / 810 | Profile | Sync preferences; validate nested members by release |
| `potionHudEnabled` | `boolean` | 660 / 666 | Profile | Sync preferences; validate nested members by release |
| `potionHudTextColor` | `Integer` | 504 / 510 | Profile | Sync preferences; validate nested members by release |
| `potionHudThemeCorners` | `boolean` | 806 / 812 | Profile | Sync preferences; validate nested members by release |
| `profilesOwnControls` | `boolean` | 142 / 148 | Global | Sync preferences; validate nested members by release |
| `pulsarEnabled` | `boolean` | absent / 32 | Profile | Local: development-only; no published release adapter verified |
| `pulsarKey` | `int` | absent / 33 | Profile | Local: development-only; no published release adapter verified |
| `pulsarSettings` | `dev.phenomena.config.Config` | absent / 34 | Profile | Local: development-only; no published release adapter verified |
| `reachBgColor` | `int` | 797 / 803 | Profile | Sync preferences; validate nested members by release |
| `reachBgMode` | `HudBackground` | 796 / 802 | Profile | Sync preferences; validate nested members by release |
| `reachDisplayTextColor` | `Integer` | 503 / 509 | Profile | Sync preferences; validate nested members by release |
| `reachDisplayThemeCorners` | `boolean` | 798 / 804 | Profile | Sync preferences; validate nested members by release |
| `reachEnabled` | `boolean` | 562 / 568 | Profile | Sync preferences; validate nested members by release |
| `resourcepackBrowserEnabled` | `boolean` | 1331 / 1337 | Profile | Sync preferences; validate nested members by release |
| `saturationBarEnabled` | `boolean` | 622 / 628 | Profile | Sync preferences; validate nested members by release |
| `searchInstant` | `boolean` | 281 / 287 | Global | Sync preferences; validate nested members by release |
| `searchTypoTolerance` | `int` | 283 / 289 | Global | Sync preferences; validate nested members by release |
| `serverListDragReorder` | `boolean` | 1067 / 1073 | Profile | Sync preferences; validate nested members by release |
| `shieldStatusAvailableColor` | `int` | 694 / 700 | Profile | Sync preferences; validate nested members by release |
| `shieldStatusDisabledColor` | `int` | 701 / 707 | Profile | Sync preferences; validate nested members by release |
| `shieldStatusEnabled` | `boolean` | 687 / 693 | Profile | Sync preferences; validate nested members by release |
| `shieldStyle` | `ShieldStyle` | 670 / 676 | Profile | Sync preferences; validate nested members by release |
| `shieldTweaksEnabled` | `boolean` | 669 / 675 | Profile | Sync preferences; validate nested members by release |
| `showCoords` | `boolean` | 487 / 493 | Profile | Sync preferences; validate nested members by release |
| `showFps` | `boolean` | 486 / 492 | Profile | Sync preferences; validate nested members by release |
| `showTime` | `boolean` | 488 / 494 | Profile | Sync preferences; validate nested members by release |
| `smoothAnimationsEnabled` | `boolean` | 366 / 372 | Profile | Sync preferences; validate nested members by release |
| `smoothCamera` | `boolean` | 364 / 370 | Profile | Sync preferences; validate nested members by release |
| `smoothCameraStrength` | `double` | 365 / 371 | Profile | Sync preferences; validate nested members by release |
| `smoothFovEnabled` | `boolean` | 1035 / 1041 | Profile | Sync preferences; validate nested members by release |
| `smoothFovSpeed` | `double` | 1036 / 1042 | Profile | Sync preferences; validate nested members by release |
| `smoothFramePacer` | `boolean` | 326 / 332 | Profile | Sync preferences; validate nested members by release |
| `sprintHudBgColor` | `int` | 801 / 807 | Profile | Sync preferences; validate nested members by release |
| `sprintHudBgMode` | `HudBackground` | 800 / 806 | Profile | Sync preferences; validate nested members by release |
| `swingAnimationCurve` | `AnimationCurve` | 367 / 373 | Profile | Sync preferences; validate nested members by release |
| `swingStyle` | `SwingStyle` | 372 / 378 | Profile | Sync preferences; validate nested members by release |
| `tabPingEnabled` | `boolean` | 623 / 629 | Profile | Sync preferences; validate nested members by release |
| `textCase` | `TextCase` | 276 / 282 | Profile | Sync preferences; validate nested members by release |
| `textHudShadow` | `TextShadow` | 277 / 283 | Profile | Sync preferences; validate nested members by release |
| `textItalic` | `boolean` | 274 / 280 | Profile | Sync preferences; validate nested members by release |
| `textRendererMode` | `TextRendererMode` | 218 / 224 | Profile | Sync preferences; validate nested members by release |
| `textUnderline` | `boolean` | 275 / 281 | Profile | Sync preferences; validate nested members by release |
| `textWeight` | `TextWeight` | 273 / 279 | Profile | Sync preferences; validate nested members by release |
| `theme` | `ThemeDefinition` | 308 / 314 | Profile | Sync preferences; validate nested members by release |
| `themeEnabled` | `boolean` | 300 / 306 | Profile | Sync preferences; validate nested members by release |
| `toggleSneakKey` | `int` | 832 / 838 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `toggleSprintKey` | `int` | 831 / 837 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `toggleSprintSneakDisplayMode` | `ToggleSprintDisplayMode` | 642 / 648 | Profile | Sync preferences; validate nested members by release |
| `toggleSprintSneakEnabled` | `boolean` | 624 / 630 | Profile | Sync preferences; validate nested members by release |
| `toggleSprintSneakHudEnabled` | `boolean` | 625 / 631 | Profile | Sync preferences; validate nested members by release |
| `toggleSprintSneakTextColor` | `Integer` | 508 / 514 | Profile | Sync preferences; validate nested members by release |
| `toggleSprintSneakThemeCorners` | `boolean` | 802 / 808 | Profile | Sync preferences; validate nested members by release |
| `toolDurabilityAlertEnabled` | `boolean` | 616 / 622 | Profile | Sync preferences; validate nested members by release |
| `totemParticleScale` | `int` | 666 / 672 | Profile | Sync preferences; validate nested members by release |
| `totemPopBgColor` | `int` | 817 / 823 | Profile | Sync preferences; validate nested members by release |
| `totemPopBgMode` | `HudBackground` | 816 / 822 | Profile | Sync preferences; validate nested members by release |
| `totemPopEnabled` | `boolean` | 809 / 815 | Profile | Sync preferences; validate nested members by release |
| `totemPopTextColor` | `Integer` | 506 / 512 | Profile | Sync preferences; validate nested members by release |
| `totemPopThemeCorners` | `boolean` | 818 / 824 | Profile | Sync preferences; validate nested members by release |
| `totemResetKey` | `int` | 838 / 844 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `totemResetOnDeath` | `boolean` | 815 / 821 | Profile | Sync preferences; validate nested members by release |
| `totemShowOthers` | `boolean` | 811 / 817 | Profile | Sync preferences; validate nested members by release |
| `totemTopOthers` | `int` | 813 / 819 | Profile | Sync preferences; validate nested members by release |
| `totemTweaksEnabled` | `boolean` | 665 / 671 | Profile | Sync preferences; validate nested members by release |
| `underwaterClarityEnabled` | `boolean` | 763 / 769 | Profile | Sync preferences; validate nested members by release |
| `underwaterFogMultiplier` | `double` | 764 / 770 | Profile | Sync preferences; validate nested members by release |
| `vanillaKeyModifiers` | `Map<String, Integer>` | 152 / 158 | Global | Sync controls: verified action name -> modifier mask 0..7 |
| `viewBobAmplitude` | `double` | 380 / 386 | Profile | Sync preferences; validate nested members by release |
| `viewBobCurve` | `AnimationCurve` | 368 / 374 | Profile | Sync preferences; validate nested members by release |
| `waypointBeamWidth` | `double` | 900 / 906 | Profile | Sync preferences; validate nested members by release |
| `waypointBlockOutlineColorOverride` | `int` | 917 / 923 | Profile | Sync preferences; validate nested members by release |
| `waypointBlockRadius` | `int` | 911 / 917 | Profile | Sync preferences; validate nested members by release |
| `waypointDisplay` | `WaypointDisplay` | 898 / 904 | Profile | Sync preferences; validate nested members by release |
| `waypointDropKey` | `int` | 840 / 846 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `waypointHiddenGroups` | `java.util.Set<String>` | 892 / 898 | Profile | Sync preferences; validate nested members by release |
| `waypointLabelScale` | `double` | 902 / 908 | Profile | Sync preferences; validate nested members by release |
| `waypointManagerKey` | `int` | 842 / 848 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `waypointMaxDistance` | `int` | 891 / 897 | Profile | Sync preferences; validate nested members by release |
| `waypointMaxVisible` | `int` | 889 / 895 | Profile | Sync preferences; validate nested members by release |
| `waypointNavigationByWorld` | `java.util.Map<String, String>` | 893 / 899 | Profile | Local: world data |
| `waypointShowLabel` | `boolean` | 919 / 925 | Profile | Sync preferences; validate nested members by release |
| `waypointWorldMarkerLimit` | `int` | 890 / 896 | Profile | Sync preferences; validate nested members by release |
| `waypointsByWorld` | `java.util.Map<String, java.util.List<Waypoint>>` | 937 / 943 | Profile | Local: world data |
| `waypointsEnabled` | `boolean` | 897 / 903 | Profile | Sync preferences; validate nested members by release |
| `worldMapCacheRegions` | `int` | 1322 / 1328 | Profile | Sync tuning |
| `worldMapCaptureBudget` | `int` | 1316 / 1322 | Profile | Sync tuning |
| `worldMapEnabled` | `boolean` | 1303 / 1309 | Profile | Sync preferences; validate nested members by release |
| `worldMapKey` | `int` | 1308 / 1314 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `worldMapShowWaypointNames` | `boolean` | 895 / 901 | Profile | Sync preferences; validate nested members by release |
| `zoomEnabled` | `boolean` | 321 / 327 | Profile | Sync preferences; validate nested members by release |
| `zoomKey` | `int` | 829 / 835 | Profile | Sync controls: release-specific SDL/GLFW + KeyCombo codec |
| `zoomLevel` | `double` | 322 / 328 | Profile | Sync preferences; validate nested members by release |
| `zoomSmoothness` | `double` | 323 / 329 | Profile | Sync preferences; validate nested members by release |

## Profile envelope, controls and protected fields

| JSON member | Verified codec | Projection |
|---|---|---|
| `name` | String, sanitized filename identity | Sync named definition; collision requires preview and provenance. No overwrite of unrelated target-only profile. |
| `createdAtMs` | long integer | Preserve definition provenance; not edit time or conflict authority. |
| `fields` | LinkedHashMap<String,JsonElement> of config field names | Project only release-verified fields above. Retain target protected/world fields in every matching profile. |
| `icon` | String | Sync metadata with definition. |
| `hotkey` | int KeyCombo; -1 unbound | Sync only when controls enabled and format verified. |
| `controls` | nullable Map<String,String> action -> vanilla saved key name | Sync only verified actions/key names when controls enabled. Preserve target-only/unsupported mappings. |

KeyCombo uses low 16 bits for SDL scancode, bits 16..18 for Ctrl=1, Shift=2, Alt=4; -1 is unbound. KeybindMigrator.VERSION=1 and `keybindVersion` mark migrated SDL fields, and config/profile markers are independent. Unmarked legacy GLFW values require the exact mapping table in KeybindMigrator; unsupported keys (e.g. F25) stay local with a reason instead of silently becoming unbound. Do not copy a marker without converting its values. `vanillaKeyModifiers` uses action-name keys and low modifier mask 0..7. Aurora mapping registration must establish the mapping-name/config-field pair for the installed release; never synchronize all `key_key.aurora.*` solely by prefix.

Controls off means retain target Aurora key fields, profile hotkeys/controls and vanilla modifiers together. Profiles off still requires a coherent target active profile projection so the next Client startup cannot undo changed settings. Preserve local-only definitions and never delete target-only profiles automatically. Active profile change/removal and name collisions must appear in preview; deleted or unknown-profile names are not permission to load factory defaults.

Always excluded: `saves/`, world directories, level.dat/regions, `config/aurora-worldmap/`, both world maps in config and profiles, `phenomena/` fight/session data, standalone phenomena.json/kwamster.json imports, Hitreg import history, credentials, logs, backups and temporary files. No advanced selection can widen this authority.

## Uncertainty and implementation gates

- Unverified enum/number codec cells remain local on that version. Do not infer support from matching field names or a source initializer hash alone.
- No shader-loader/version adapter has been verified. Pool shader files only; selection remains local with an explanation until a real loader format is reviewed.
- Saved servers remain optional/off and additions-only; do not treat any NBT edit as scalar settings propagation.
- Unknown Client releases and the development-only Pulsar fields remain local. The published v3.5.0 source is the initial candidate adapter, requiring artifact identity checks during implementation.
- Scalar conflict clocks are capture/fully-terminated-exit times, never file mtimes. Newest capture wins; exact ties prefer main, then a stable instance-ID ordering. Join seeding is main-authoritative before edit-anywhere capture begins.
- No runtime sync authority is enabled by this catalog. Durable stopped ownership, journaled backups/atomic commit/recovery and Restore are prerequisites for enabling the new writer.
