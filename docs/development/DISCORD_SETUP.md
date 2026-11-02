# Discord Rich Presence setup

Aurora implements optional local desktop IPC presence, without Discord account linking or OAuth tokens. The **Aurora Client** Discord application uses the registered **aurora-logo** Rich Presence asset. Its owner-approved public Application ID, **1553653987545317396**, is built in for native and WebView builds. Activity sharing remains off until enabled in Settings.

1. Create/select an Aurora-owned application in the [Discord Developer Portal](https://discord.com/developers/applications), named **Aurora Client**. Never borrow another application's ID.
2. Copy its public Application ID. Activity-only IPC needs no bot token, client secret or redirect registration.
3. Under **Rich Presence → Art Assets**, confirm the registered **aurora-logo** key and wait for availability. The launcher requests that exact key. See [Discord's asset guide](https://discord.com/developers/discord-social-sdk/development-guides/setting-rich-presence#uploading-assets).
4. Build the launcher normally. `AURORA_DISCORD_APPLICATION_ID` is an optional compile-time override for owner-managed builds; an explicitly invalid override fails closed. Users do not supply arbitrary application identities or IPC paths through Settings. No secret belongs in source/config.
5. In **Settings → Discord & privacy**, enable **Discord Rich Presence**. Aurora connects automatically through local desktop IPC. Connected requires actual READY/command acknowledgement. Discord being absent is an ordinary status; retries wait 15, 30, 60 and then at most 120 seconds. State changes do not bypass that backoff. Successful publication resets it; an enabled connected actor checks IPC every 15 seconds. Startup is detached and never waits for Discord.
6. Verify Aurora Client/In Launcher/logo in Discord. Exercise Starting/Running/normal exit with the supervised game. Extra details default off and opt in separately. Turning the master off clears activity once, disconnects, and stops connection/publication timers. Re-enabling tries immediately. Discord's own activity visibility settings also apply.
7. Complete real Discord acceptance before treating this feature as publication-ready. IPC acknowledgement alone does not prove the remote profile/logo rendered.

Ordinary developer and release builds now use the built-in ID, including the
Slint native launcher. An existing executable must be rebuilt and restarted to
receive this change; setting an environment variable only when launching it has
no effect. Existing release workflows may continue supplying the same public ID.

The supplied Discord Public Key is not required or stored: activity-only desktop
IPC uses the Application ID, not incoming-interaction signature verification.
No bot token, client secret, OAuth account link or new permissions are added.

Settings now exposes **Show World**, **Show Server** and **Show Server Address**, all defaulting off. Address sharing additionally requires Show Server. They use only authenticated native snapshots from the reviewed client protocol v1; nothing is inferred from arguments or logs. Missing bridge data leaves generic presence. World paths are never published. Raw addresses require explicit address consent, and display names containing the address/host are suppressed while that consent is off. There is no Unlink control because there is no account link.

The originally integrated Aurora 2.1.2 JAR predates the bridge. Current production Client metadata and the reviewed bridge gate are documented in `docs/compatibility/AURORA_CLIENT_2_1_5_ACTIVATION.md`; exact verified artifact identity, active bootstrap state and Minecraft compatibility govern gameplay snapshots. A version display alone never grants bridge capability.

After owner setup, verify the rendered Discord profile through Main Menu → Singleplayer → Main Menu → Multiplayer → Main Menu → normal Quit. Check world off/on; server off/on; server on/address off (friendly name only); server plus address on; master off (clear); and Discord disconnect/reconnect while the game remains healthy. Use disposable fixture identities for screenshots. Native projection/IPC tests do not prove Discord's remote profile or logo rendered.

Sources: [RPC over IPC](https://discord.com/developers/topics/rpc), [Social SDK presence/assets](https://discord.com/developers/discord-social-sdk/development-guides/setting-rich-presence), [current client reference](https://discord.com/developers/docs/social-sdk/classdiscordpp_1_1Client.html), [official activity-only RPC source](https://github.com/discord/discord-rpc). Discord recommends Social SDK for broad integrations; Aurora limits itself to bounded local activity, without SDK binaries, downloaded helpers or account/voice APIs.
