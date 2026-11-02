# Aurora Launcher 1.2.0 — release notes

New compatible Aurora instances use the verified Aurora Client 2.1.5 release
with Minecraft 1.21.11, Fabric Loader 0.19.5, Fabric API 0.141.6+1.21.11,
and managed Java 21. Existing instances remain pinned.

### Activity & Quick Launch

- Playtime tracking with seven- and thirty-day charts and an all-time total.
- Recent Worlds and Recent Servers Home widgets built from launcher-owned
  gameplay history.
- Quick Launch starts a recent world or server through the normal validated
  launch path, for instances pinned to Minecraft 1.21.11.
- Gameplay history is stored only on this device, bounded in size, and never
  uploaded; there is no telemetry.

### Skins & Capes

- Skin Manager with saved local skin presets and PNG import.
- Classic and Slim skin model support, with authenticated skin application to
  the signed-in account.
- Cape Selector lists the account's owned capes; select one or disable capes,
  with previews that follow the selected account.

### Aurora Client integration

- Aurora Client 2.1.5 with bridge v2 support for verified world and server
  activity reporting.
- Improved world and server activity integration while playing with the
  reviewed client artifact.

### Reliability / Privacy

- Gameplay history is launcher-local; Discord Rich Presence remains optional
  and off by default.
- World, server, and server-address visibility in Discord remains controlled by
  the existing separate privacy settings.
- Aurora Client artifact trust remains cryptographically pinned to the
  reviewed 2.1.5 release; existing instances never silently update.

Compatibility checks cannot guarantee every cross-mod interaction or resolve
every dependency conflict. Installing this launcher does not change the Aurora
release pinned by existing instances.
