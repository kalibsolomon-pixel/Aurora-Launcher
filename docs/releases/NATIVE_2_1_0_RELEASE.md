# Aurora Launcher Native 2.1.0 release record

## Source and compatibility

- UI-1 approval: `bc355b5e9617d22df96c587e3d46c272d7606aa0`.
- UI-2 completion: `f80af44f5fe04237e157395a643e28cdb078b9d7`, a descendant of UI-1 and the published 2.0.1 source `82620e57b5c8b933dc970fae02200635df8cfa7c`.
- Windows x64 NSIS per-user package, application identity `com.aurora.launcher.native-preview`, update channel `native-preview`, and minimum installed version `1.4.2`.
- Native authority before release: `0146fba27066b10166c2444e78d9d44798e4cb05` (2.0.1).
- Legacy WebView authority before release: `df7e824b2e49f8a5b9d2447ca2314bcafa21d119`.
- Established production updater public-key fingerprint: `bb2998279b09a39daeb44cd405a2f0f6d664fa751ce3d02e002463f89f8c50bc`. Updater signing and Windows Authenticode are separate.
- Publication, artifact hashes and final authority verification are pending.

Development acceptance is recorded in the [UI-1 report](../native-ui/NATIVE_UI_REFINEMENT_ACCEPTANCE.md) and [UI-2 report](../native-ui/NATIVE_UI_2_ACCEPTANCE.md). Extremely rapid gameplay transitions can occasionally overwrite Minecraft's built-in activity record before Aurora observes it.
