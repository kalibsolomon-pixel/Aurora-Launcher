# Aurora Launcher 1.5.0 — release notes

Draft for the selected release candidate. Publication requires the remaining
acceptance and owner gates in LAUNCHER_1_5_0_RELEASE_ACCEPTANCE.md.

## New features

- Instance tiles and configurable Material Symbols icons, with Settings as the
  main instance workspace and a simpler Home launch control.
- Add compatible Fabric Modrinth modpack content to an existing ordinary
  instance after reviewing conflicts. Existing files are preserved; merged
  content uses ordinary content management rather than modpack updates.
- Exact reviewed Forge configurations through the managed installation pipeline.
- Explicit recovery-backup storage maintenance for eligible launcher-owned
  installation history.
- Optional instance configuration modes with conservative memory advice. Applying
  a recommendation and saving are explicit actions; existing settings and the
  2048 MiB default remain unchanged.

## Improvements

- Refined Discover, Library, Home, Appearance and Settings layouts, keyboard
  controls, responsive presentation and shared controls.
- Durable game replacement and interrupted-operation recovery, with clearer
  readiness and recovery status.
- Bounded installer processor execution, output capture and cancellation.

## Bug fixes

- Preserve the updater configuration needed for ordinary native startup.
- Wait for the winning startup state reload before checking the selected instance.
- Resolve validated custom window dimensions in Minecraft's conditional arguments.
- Allow concurrent read-only game validation without reporting false damage.
- Use the same executed managed-Java eligibility check for status, readiness and
  final launch preparation; explain reproduced runtime path failures.
- Honor Fabric `provides` aliases and bundled/loader-supplied module alternatives,
  while retaining strict protection for launcher-managed mod identities.

## Compatibility changes

New compatible Aurora instances continue to select Aurora Client 3.0.0 with
Minecraft 1.21.11, Fabric Loader 0.19.5, Fabric API 0.141.6+1.21.11 and managed
Java 21. Existing pins change only through explicit supported actions.

The established production installer/update platform is Windows x64 NSIS.
Linux production packages and secure account persistence are not implemented by
the current release pipeline. No new platform is added by this release.

## Known limitations

- Not every Modrinth modpack or loader generation is supported. Conflicting,
  unsafe or unsupported content is refused with its reason.
- Certain long or non-ASCII paths cannot execute the published managed Java
  build; readiness blocks launch and provides path advice.
- Memory advice is not a validated performance preset. No FPS, CPU-use or
  1536-versus-2048 MiB performance improvement is claimed.
- Production updates require Aurora's existing updater signature. Windows
  Authenticode publisher signing remains unconfigured.
- Registry schema 6 preserves supported older instance settings during migration.
  Older launchers cannot read newer schemas; back up metadata before reverting.
- Iris and Vulkan compatibility are not established by this release.

Final signed-installer, authenticated gameplay, supported-mod-stack and upgrade
acceptance are still pending. This draft is not a publication announcement.
