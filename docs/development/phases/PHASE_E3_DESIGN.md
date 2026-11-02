# Phase E3 design and service research

Research date: 2026-09-28. No account credential was used for this investigation.

## Service evidence

Minecraft's [official skin help](https://help.minecraft.net/hc/en-us/articles/4408894664461-Make-a-Custom-Skin-in-Minecraft-Java-Edition) confirms Classic/Slim selection, custom skin uploads, launcher skin libraries, and choosing an owned cape. Its [official skin statement](https://www.minecraft.net/en-us/article/minecraft-java-edition-skins-issue-update) confirms Java players can upload custom PNG skins through Minecraft's web service. I found no public official Minecraft Services reference specifying the profile and mutation HTTP contracts. The details below are corroborated by the [Minecraft Wiki API documentation](https://minecraft.wiki/w/Mojang_API), [CmlLib's Minecraft Services client](https://github.com/CmlLib/MojangAPI/blob/master/MojangAPI/Mojang.cs), and the [Voxelum launcher implementation](https://github.com/voxelum/minecraft-launcher-core-node/blob/master/packages/user/mojang.ts). They are ecosystem-verified, not an official published contract or a live authorized mutation test.

| Operation | Endpoint and method | Request | Expected result |
|---|---|---|---|
| Read own profile, current skin/model, owned capes | `GET https://api.minecraftservices.com/minecraft/profile` | `Authorization: Bearer <Minecraft access token>` | JSON profile with `id`, `name`, `skins` (`state`, `url`, `variant`) and `capes` (`id`, `state`, `url`, `alias`). `ACTIVE` identifies selected skin/cape; skin variants are `CLASSIC` and `SLIM`. |
| Upload skin | `POST https://api.minecraftservices.com/minecraft/profile/skins` | Bearer token; multipart form fields `variant=classic|slim` and `file` as PNG | Success status/profile. Re-fetch profile to establish authoritative current state. |
| Select owned cape | `PUT https://api.minecraftservices.com/minecraft/profile/capes/active` | Bearer token; JSON `{ "capeId": "<owned profile cape ID>" }` | Success status/profile; 400 can mean the profile does not own that cape. Re-fetch profile. |
| Disable cape | `DELETE https://api.minecraftservices.com/minecraft/profile/capes/active` | Bearer token, no body | Success status; re-fetch profile for no `ACTIVE` cape. |

All operations use the Rust-owned Minecraft access token, renewed through the existing Microsoft refresh chain when its session expires. 401 requires reauthentication/refresh handling; 429 requires a user-visible rate-limit result and no rapid retry. 5xx and transport failures are service-unavailable states. Raw provider error bodies are never shown or logged. Exact rate-limit quotas and `Retry-After` policy are not publicly specified here; the launcher must avoid polling loops and automatic mutation retries. Response bodies on successful mutations vary across implementations, so a successful status followed by a fresh profile read is the stable confirmation path.

Java skin upload reports and current launcher practice support 64×64 and legacy 64×32 PNG. No verified evidence supports larger dimensions on the Java Minecraft Services upload route, so import accepts those two shapes only and returns an explicit unsupported-dimensions error otherwise. Model choice is explicit and maps `Classic` to `classic` and `Slim` to `slim`; it is never inferred from pixels.

## Boundaries

The browser's system file picker supplies the selected PNG bytes to a narrow native import command. No frontend path is accepted. Rust owns validation, credentials, Minecraft Services calls, preset persistence, ownership checks and post-mutation profile refresh. Svelte receives only typed cosmetic state and opaque preset/cape IDs. Presets are launcher-global local assets; current skin and owned capes are account-specific remote state. The existing 3D preview consumes native-decoded current skin pixels. Successful skin mutation refreshes that path without a restart. The renderer currently draws skin only, so cape state is represented in the selector rather than added to the renderer.
