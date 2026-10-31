# Spxctra UI refinement — 2026-10-31

Implemented for Aurora Launcher, a Tauri 2 / Rust application with a Svelte 5,
TypeScript and static SvelteKit frontend rendered by the platform WebView
(WebView2 on this Windows host). This is a presentation refinement, not a native
renderer migration or an assertion of owner visual acceptance.

## Scope and decisions

Home's job is to select an instance and launch it. Instances manages isolated
installations; each workspace manages that instance's configuration and content.
Launcher Settings changes launcher preferences. Skins and capes remain in their
existing global destination. These workflows and the Rust authority for all
operations are retained.

The audit prioritized these changes:

1. Retain navigation destinations, managed state, account handling, content
   operations, themes, accent choices and optional Borealis preferences.
2. Remove repeated glass/filter, gradient, shadow and rounded-container treatments.
3. Flatten the outer workspace so its individual groups carry the information.
4. Reduce Home's oversized launch treatment and neutralize instance icon grounds.
5. Keep navigation labels visible, add a skip link, and improve muted text contrast.
6. Reuse existing components and shape/spacing/type tokens; add no product dependency.

## Skills installed and read

Installed in the existing personal Codex `skills` directory, without changing
other agent settings. They are available for automatic discovery on the next
turn; their instructions were read and applied directly during this task.

| Skill | Source / pinned identity | Verification |
| --- | --- | --- |
| `spxctra-ui` | Supplied Downloads/SKILL.md, `1.0.0-draft`; SHA-256 `d7cfaace5e2102ef7398a823177d3d97de22ae060d305af826b1a18febdd197f` | Exact copy, readable |
| `frontend-design` | [Anthropic's current official skills repository](https://github.com/anthropics/skills/tree/ca45fc496a3436251f191354091d557da31c15b5/skills/frontend-design), commit `ca45fc496a3436251f191354091d557da31c15b5` | SKILL.md and LICENSE.txt installed with the bundled skill installer |
| `web-design-guidelines` | [Vercel agent-skills](https://github.com/vercel-labs/agent-skills/tree/1a2c79d4e6c4a2283dc2c614b6538cbe1ed5ca62/skills/web-design-guidelines), commit `1a2c79d4e6c4a2283dc2c614b6538cbe1ed5ca62` | SKILL.md installed; current upstream web-interface-guidelines read |

The requested `anthropics/claude-code/plugins/frontend-design` path was absent
from the current repository tree; the current official Anthropic skill above
was used and its origin recorded. The supplied Spxctra file references
`references/REVIEW_CHECKLIST.md`, which was not supplied. No supporting checklist
was invented or claimed installed. The in-file workflow and the owner's detailed
request supplied the usable requirements. Spxctra simplicity, existing product
tokens and platform behavior take precedence over supporting skills' aesthetic
or website-specific suggestions. Computer Use was used for Windows inspection.

## Implemented subtraction and component consistency

- Shared panels are opaque tonal surfaces, with no backdrop filters, glass accent
  stacks or group shadows. The existing optional background remains visible in
  page whitespace under a static dark scrim. Simple was already the native default
  and remains so; saved preferences were not migrated or reset.
- Shared panel and control radii reuse the existing 14 px and 10 px tokens instead
  of 22 px / 14 px pilot values. Dense appearance/cosmetics controls use those
  tokens too. Status pills and switches retain their functional shapes.
- Workspace loses its enclosing outlined panel and filtered pseudo-element.
  Active tabs retain a clear underline; inactive content is quiet.
- Home loses its decorative accent wash and player halo. Its primary control is
  56 px high rather than 80 px, with a 320 px hero at standard sizes. The same
  instance picker, Rust-owned readiness and Play action remain connected.
- Instance glyphs have smaller, neutral grounds. Tiles retain selected, hover,
  focus and disabled behavior without press scaling or default shadows.
- Navigation retains its existing 96 px rail, with visible labels and an active
  marker. The first keyboard stop is **Skip to content**, targeting the focusable
  main region. No tooltip is required to discover a destination.
- Appearance loses the summary row that repeated the selected radio controls.
  Its background help now describes optional motion and how to hide it. Shared
  modals contain scrolling and have no entrance animation under reduced motion.
- Muted text is brighter in each supported dark theme. For example the previous
  Aurora Dark muted token on a raised surface was about 4.21:1; the revised token
  exceeds 4.5:1 even on the theme's hover surface (about 5.52:1). This token check
  does not constitute a whole-application WCAG certification.

No new decorative asset, background animation, font, network request, preference,
native command or renderer dependency was added. Authentication, artifact trust,
downloads, updater authority, instance state and game launch code were not edited.

## Visual and interaction evidence

- [Before contact sheet](before/contact-sheet.jpg) and [after contact sheet](after/contact-sheet.jpg).
- Full screenshots and dimension results: [before](before/results.json), [after](after/results.json).
- Ten screen configurations at 1120×760 and 720×520: Home, Instances, workspace
  configuration, Mods, Resource Packs, Shaders, Launcher, Appearance and cosmetics.
  The historical `overview` fixture resolves to current instance Settings; it is
  not an additional product tab.
- Midnight, OLED, optional Borealis, account modal, keyboard skip-link focus and
  forced-colors captures are retained. No light theme exists in the application.
- Actual Windows captures: [baseline Home](native/before-home.jpg),
  [refined Home](native/after-home.jpg), [Settings](native/after-settings.jpg),
  [Appearance](native/after-appearance.jpg). These use a disposable diagnostic
  root, not personal accounts or instances. Windows captures include the host's
  cursor highlight and capture artifacts; browser PNGs provide clearer pixel review.
- UI-3's existing content acceptance also exercises Browse, explicit search,
  categories, pagination, install state, error/retry, content switches, details,
  safe removal cancellation, long text and 200% text at four sizes.
- L3 acceptance covers cosmetics at 1600×1030, 1120×760, 860×800 and 720×520,
  keyboard rotation, modal focus return, cape confirmation, account switching and
  widget navigation. These accounts and remote mutations are synthetic fixtures.

The full screenshots/contact sheets and representative native screens were
inspected. Browser tests assert containment and zero page errors. The packaged
native capture used the host's 125% scale. Narrator and additional DPI/multi-monitor
acceptance were not performed. No authenticated
game launch, real purchase/entitlement or real provider installation was attempted.

## Performance observations and rendering decision

Six browser-only samples per implementation (three each for Simple/Borealis)
measure a three-second idle interval on fixture Launcher Settings, then RAF
intervals. Raw observations are in the before/after result files.

| Fixture setting | Before median task time / 3 s | After median task time / 3 s | Before / after JS heap | Backdrop-filter elements before / after |
| --- | ---: | ---: | ---: | ---: |
| Simple | 0.47 ms | 0.44 ms | 6.86 / 6.83 MiB | 0 / 0 |
| Borealis | 8.23 ms | 3.06 ms | 6.88 / 6.86 MiB | 4 / 0 |

RAF p95 was approximately 16.7–16.8 ms in these headless observations. These short
samples share a host with build activity and use development components. They
confirm filter removal, not a statistically established speedup, physical frame
visibility, native GPU savings or production resource acceptance.

Windows process observations include the launcher and eight observed processes
in its WebView2 tree: [before](native/before-resources.json),
[after HMR](native/after-resources.json), and
[fresh after](native/after-fresh-resources.json). Fresh runs observed roughly
478 MiB before and 455 MiB after working set. The HMR sample was roughly 549 MiB.
Startup/cache/foreground conditions were not controlled, so these numbers are
diagnostic observations, not proof of a memory improvement or regression.
Native startup latency, input latency and GPU frame consistency remain unmeasured.

Native rendering remains the preferred direction to evaluate. Both
[Slint's Winit backends](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/)
and [egui/eframe](https://github.com/emilk/egui) are plausible Rust alternatives.
Slint documents software/GPU renderers and OS accessibility integration; egui's
[reactive repaint mode](https://github.com/emilk/egui/blob/main/crates/egui_demo_app/src/backend_panel.rs)
and [AccessKit integration](https://github.com/emilk/egui/blob/main/docs/accessibility.md)
mean it must not be assumed to require continuous idle rendering.

**Decision: retain the working renderer for this refinement.** There are 120
registered native commands plus Tauri-specific AppHandle paths, events, window
activity and updater integration, alongside working content browsers, account
dialogs and player/cosmetics canvases. Domain logic can be reused, but equivalent
native presentation and a host adapter still need implementation and acceptance.
No equivalent candidate or representative comparative benchmark exists here to
justify replacing this functioning frontend. Neither framework was benchmarked
or declared technically unsuitable. No incomplete prototype is presented as a
migration. A native replacement should first demonstrate the actual account,
instance, content and launch workflows, accessibility and measured resource
behavior against the retained implementation, with reversible data/host handling.

Remaining SDL deviations are explicit: WebView rendering remains, and the
pre-existing optional animated Borealis asset (including its original imagery)
is preserved as a user choice. Its established inactive/minimized/reduced-motion
pause behavior remains tested; it is not introduced or made the default here.

## Verification and repository protection

| Check | Result |
| --- | --- |
| `npm run check` | Pass, zero errors and warnings |
| `node --test --test-concurrency=1 'src/lib/**/*.test.ts'` | [301 passed](frontend-tests.txt) |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Pass |
| `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | [Pass](cargo-check.txt), four existing test-code warnings |
| `cargo test --manifest-path src-tauri/Cargo.toml` | [1,003 unit + 6 integration passed](rust-tests.txt), 31 ignored by the existing suite |
| `npm run tauri build -- --no-bundle` | [Pass](production-build.txt), including production frontend build and optimized native executable |
| `node tools/visual/spxctra-review.mjs after` | 20 dimension checks, theme/state captures, keyboard skip link, category keys, modal focus return; zero page errors |
| `node tools/visual/ui3.test.mjs` | [11 content interaction/layout groups passed](browser-content-tests.txt) |
| `node tools/visual/l3.test.mjs` | [Four sizes and cosmetics/account interactions passed](browser-cosmetics-tests.txt) |
| Isolated native package with bundled production frontend | [Pass](native/package-acceptance.json), native persistence, navigation, skip link, signed-out empty Home, no debug-only destination, zero page errors |

The native package used a temporary external Tauri overlay with identifier
`com.aurora.launcher.spxctra-review` and the existing debug-only disposable data-root
override. The production configuration/identifier was not changed. Its actual
bundled URL is `http://tauri.localhost/`, not the development or fixture server.
Midnight was selected through the native UI, saved by Rust, and verified after
restart into the bundled frontend. The same UI then saved Aurora Dark back to the
disposable root. Clearer native captures: [packaged Home](native/packaged-home.png),
[restored Midnight](native/packaged-restored-midnight.png),
[saved Appearance](native/packaged-appearance.png). The optimized executable was
built and hashed, but the runtime inspection used this isolated debug package;
no optimized-runtime performance acceptance is claimed.

The initial concurrent frontend run was interrupted after stalling during other
development-server activity; the same test file set passed sequentially. The first
Rust test build met Windows' lock on the running review executable; stopping that
exact test process allowed the complete retry to pass. An initial temporary native
overlay interpreted an absolute frontend path as a directory URL; retaining the
repository's relative frontend path corrected that validation configuration.
Failed-attempt logs are retained, and none of these attempts changed product code.
The existing static-VC-runtime build deprecation warning also remains.

No owner visual acceptance is implied. Real sign-in, authenticated game launch and
real content transactions were not repeated for this presentation-only change.

Original branch: `codex/instance-tiles-icons`; baseline HEAD:
`8c45ad2fa66b4b8f67c3086b55a10cf4e134893f`. Work branch:
`codex/spxctra-ui-refinement`. Status, history, tracked/untracked inventory,
critical-file presence and hashes of pre-existing changed/untracked files were
recorded before implementation under the retained temporary
`aurora-spxctra-20261031` evidence directory. [Preservation verification](preservation.json)
checks all 2,793 original tracked paths and every protected file. No pre-existing
file was deleted. Existing user changes are excluded from the local commit.

No push, publication, release or remote-branch change is authorized or performed.

**AURORA CLIENT HANDOFF: NOT YET REQUIRED**
