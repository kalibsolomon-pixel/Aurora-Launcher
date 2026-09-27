# Discord Rich Presence setup

Aurora implements optional local desktop IPC presence, without Discord account linking or OAuth tokens. No production Discord application ID is configured. **Live presence/logo activation is blocked until Aurora's owner supplies one.**

1. Create/select an Aurora-owned application in the [Discord Developer Portal](https://discord.com/developers/applications), named **Aurora Client**. Never borrow another application's ID.
2. Copy its public Application ID. Activity-only IPC needs no bot token, client secret or redirect registration.
3. Under **Rich Presence → Art Assets**, upload the existing canonical 1024×1024 `static/aurora-app-icon.png` or owner-approved canonical mark composition. Register its lowercase key as **aurora-logo** and wait for availability. See [Discord's asset guide](https://discord.com/developers/discord-social-sdk/development-guides/setting-rich-presence#uploading-assets).
4. Set `AURORA_DISCORD_APPLICATION_ID` to that legitimate public ID in the build environment, then run `npm run tauri build`. Users do not supply arbitrary application identities or IPC paths through Settings. A public ID may later be checked in after ownership review; no secret belongs in source/config.
5. Open desktop Discord and the resulting production build. In **Settings → Discord**, choose **Connect to Discord**. Connected requires actual READY/command acknowledgement. Reconnect handles ordinary drops without restarting Aurora.
6. Connection alone publishes nothing. Explicitly enable **Discord Rich Presence** and verify Aurora Client/In Launcher/logo in Discord. Exercise Starting/Running/normal exit with the supervised game. Extra details default off and opt in separately. Turn master off and verify clear. Discord's own activity visibility settings also apply.
7. Complete real Discord acceptance before treating this feature as publication-ready. IPC acknowledgement alone does not prove the remote profile/logo rendered.

World/server are unavailable pending a separately reviewed Aurora Client activity bridge. They are never inferred from arguments/logs; world paths and raw server addresses are never published. There is no Unlink control because there is no account link.

Sources: [RPC over IPC](https://discord.com/developers/topics/rpc), [Social SDK presence/assets](https://discord.com/developers/discord-social-sdk/development-guides/setting-rich-presence), [current client reference](https://discord.com/developers/docs/social-sdk/classdiscordpp_1_1Client.html), [official activity-only RPC source](https://github.com/discord/discord-rpc). Discord recommends Social SDK for broad integrations; Aurora limits itself to bounded local activity, without SDK binaries, downloaded helpers or account/voice APIs.
