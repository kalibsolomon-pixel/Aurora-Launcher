# Instance and Settings refinement

2026-10-20. Launcher-only work on `codex/instance-tiles-icons`, starting at
`db3b241`. Premium Design and Emil Design Eng guided the presentation changes.

| Before | After | Why |
| --- | --- | --- |
| Small instance icon, Selected/Ready pills, loader line and Validate button | Larger icon, name, secondary Minecraft version; selected accent edge | Give instance identity a clear hierarchy |
| Explanatory subtitles, footers and setting descriptions | Titles with focused info tooltips or explicit disclosures | Keep guidance available without filling the screen with prose |
| Discord & privacy and Minecraft & Java categories | Discord with five toggles; game settings remain in the instance workspace | Keep global Settings focused |
| Desktop create/remove controls and read-only Start status | Add Desktop Shortcut, Add to Start Menu, Open Path | Expose useful native actions with accurate labels |

Cards still open the instance workspace. Interrupted installation, recovery,
damage and stale-configuration messages remain visible, including Retry install
where supported. Validation remains available in the instance workspace.

Discord exposes Discord Rich Presence, Elapsed Play Time, Show World, Show Server
and Show Server Address. The address toggle retains its Show Server dependency.
Other preference fields still have native activity consumers, so the configuration
schema and existing saved values are preserved. Saving one of the five visible
preferences continues to preserve the hidden fields.

The typed Rust commands create real Windows shell links through the existing COM
implementation and known-folder APIs. Start uses a Programs shortcut, reuses an
existing installer shortcut and refuses a foreign occupant. The button is honestly
named **Add to Start Menu**: pinning to the Start grid remains a Windows user action.
Open Path opens Aurora's managed data folder through the native opener and accepts
no frontend path. Errors appear inline. Unsupported platforms return explicit errors.
The existing restriction to installed production builds for permanent shortcuts
remains enforced and is explained by the info controls.

## Verification

- `npm run check`: zero errors and warnings.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`: passed.
- `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`: passed;
  existing test-only warnings remain.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 1,009 passed, 31 ignored.
  The real Windows COM test covers desktop/Start creation, Start idempotence,
  foreign-file preservation and reuse of an installer shortcut.
- Frontend suite: 301 passed using
  `node --test --test-force-exit --test-concurrency=2 --test-timeout=120000 "src/lib/**/*.test.ts"`.
  Tests cover five Discord controls, command dispatch without frontend paths,
  native error feedback, card identity and retained configuration behavior.
- Production build: MSI and NSIS succeeded; final asset refresh verified before commit.
- Actual Tauri debug boot: passed using a separate temporary managed root and
  WebView profile. Open Path opened that exact root in Explorer. Debug shortcut
  refusal appeared as an inline error. No real account or instance was copied.
- Browser visual fixtures: Instances, Launcher and Discord fit at 1120×760 and
  720×520 without horizontal overflow. Cards open their workspace. Info controls
  open on keyboard focus and dismiss with Escape.

## Screenshots and limits

- [Populated instance cards — synthetic browser fixture](instances-populated-fixture.png)
- [Instances — actual native app, empty isolated registry](instances-native.png)
- [Launcher settings — actual native app](settings-launcher-native.png)
- [Discord settings — actual native app](settings-discord-native.png)

Permanent shortcuts in an installed production application were not exercised
end to end in the user's actual Desktop/Start folders. Real shell-link operations
were tested in a disposable directory; the native debug UI verified the existing
production-build guard. Windows Start-grid pinning and non-Windows desktop
integration are deliberately unsupported. No installer was run.

All 2,788 baseline tracked files remain present. The task retains and wires the
pre-existing, directly relevant Start-menu shortcut implementation. Unrelated
working changes in `docs/development/PRIVATE_CANDIDATE_TESTING.md`, `tools/visual/ui2.test.mjs` and
pre-existing untracked files are kept outside the task commit. No Client source,
installed game data, runtime data or normal Minecraft directory was changed.
