# UI-4 visual review

Implementation is ready to inspect; overall acceptance is **PARTIAL** until actual
native 1024×768 and enforced-minimum sizing is witnessed. Browser fixtures cover
both dimensions; do not substitute them for native screenshots.

## Actual Windows application

These PNGs capture the compiled `http://tauri.localhost/` frontend with real Rust
local storage, an isolated marked application-data root and separate WebView profile.
The skin is synthetic; no account or credentials were copied or authenticated.

- [Borealis / cyan full-body workspace](native/appearance-default.png).
- [Imported classic skin](native/imported-classic.png).
- [Rotated and tilted character](native/rotated.png).
- [Reset](native/reset-view.png), [zoom](native/zoomed.png).
- [Slim preview; saved geometry stays classic](native/slim-preview.png).
- [Outer layers off](native/outer-layers-off.png).
- [Cape unavailable without account](native/cape-no-account.png).
- [Maximized, 2048×1152 CSS](native/maximized-cape-state.png).
- [No account / empty library](native/no-account.png).
- [Library survives restart](native-restart/persisted-library.png).
- [Persisted skin preview](native-restart/persisted-preview.png).
- [Removal confirmation](native-restart/removal-confirmation.png).
- [Entry removed](native-restart/removed-entry.png).

Default native client is 1120×790 CSS, DPR 1.25. `*-attempt` screenshots retain
unsuccessful resizing observations at that same size; they are **not** minimum
or medium acceptance. Start/exit records, executable hashes, matching viewport
JSON and isolated storage verification accompany these screenshots. The final
restart uses the final executable; earlier native screenshots use the same final
layout before the dependency-free camera-constant import correction (identical
rendering defaults). Every application run exited with code 0.

## Deterministic browser fixtures

These exercise actual Svelte components with synthetic accounts/service outcomes,
not a native account or real service mutation:

- [1440×900](browser/appearance-1440x900.png), [1280×800](browser/appearance-1280x800.png).
- [1024×768](browser/appearance-1024x768.png), [720×520](browser/appearance-720x520.png), [706×512](browser/appearance-706x512.png).
- [Cape artwork and local back preview](browser/cape-preview.png).
- [No capes](browser/no-capes.png), [offline](browser/offline.png), [expired](browser/expired.png), [no account with local preview](browser/accounts-0.png).
- [Reduced transparency](browser/reduced-transparency.png).
- [200% minimum top](browser/text-200-706x512.png), [scrolled stage](browser/text-200-minimum-stage.png), [scrolled library](browser/text-200-minimum-library.png).

At narrow sizes the stage precedes controls/library in one vertical page; scroll
to inspect the complete character and then the library. At 200% text more scrolling
is expected. Import remains at the top, dialogs scroll within the viewport, and
every tested width has zero horizontal overflow. The character stays full-body
inside its canvas; ordinary page scrolling is distinct from Home's deliberate crop.

## Comparison and remaining gate

Use the approved [Home](../ui-3-1/regression/home-1440x900.png) and
[Mods](../ui-3-1/after/mods-1440x900.png) references. UI-4 retains Aurora's rail,
background, accent, typography and glass; it puts character inspection ahead of
the compact collection. All four matched Home screenshot files are byte-identical
to the prior evidence (`home-comparison.json`). No Note artwork/reference image
was attached to this request; its written organization principles were used only.

Next: witness exact native medium/minimum sizes and visually approve this workspace.
Real account skin/cape display remains covered by deterministic fixtures here;
optional live service mutation is deliberately unperformed. Do not start UI-5 or
release preparation.
