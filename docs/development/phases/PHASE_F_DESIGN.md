# Phase F — visual system and shell pilot

Status: design proposal, followed by a bounded implementation pilot. Full rollout requires owner visual approval.

## Direction

Aurora is a place to start playing. A quiet, atmospheric stage gives its existing white mark visual authority, followed by one wide Play action. Readiness and installed configuration remain truthful and subordinate. No new launcher lifecycle or authentication architecture is proposed.

## Research and reference scope

Reviewed 28 September 2026: [Lunar's official launcher design presentation](https://www.lunarclient.com/news/the-new-launcher-is-here) (published August 2023, historical design reference rather than a claim about the latest binary), [Lunar's launcher guide](https://www.lunarclient.com/news/how-to-use-the-new-lunar-client-launcher), and [Dawn's current official launcher presentation](https://dawn.gg/). Lunar gives launch/brand a strong central identity and moves utility controls to the edges. Dawn separates profile organization from a large player-led entertainment surface, with an economical icon rail. Aurora adopts the hierarchy principles, not their artwork, card geometry, social panels, or composition.

The supplied references are current Home, Settings, Windows window controls, and Aurora artwork. No Home sketch is present; the written instructions supply the composition. The repository's canonical transparent `static/aurora-icon.png` remains the logo, unchanged. OS icons remain unchanged.

## Pilot composition

1. **Home:** centered two-part composition, logo above a roughly 400–540 px launch panel; 78 px high Play at desktop, instance-name selector directly below, installed versions and native status tertiary. At smaller widths Play remains dominant. Remove Home explanatory header and Manage Instance.
2. **Navigation:** slim icon rail with familiar house, monitor, gear and information symbols; a readable avatar/name account dock remains at its foot. Full labels appear in focus/hover tooltips. Instances opens the existing registry/workspace; no duplicate list occupies the rail.
3. **Window chrome:** integrated 40 px titlebar, native Tauri window operations and drag region, conventional right-aligned minimize/maximize/close. Preserve native decorations on non-Windows initially; the Windows pilot must be boot-tested. Do not claim Windows 11 hover snap layouts without proof.
4. **Player:** separate right-side showcase. Keep existing bounded software rasterizer, redraw only on skin or explicit rotation, with keyboard controls. No autonomous animation. Hide decorative preview first at minimum widths and suspend rendering while hidden.
5. **Widgets:** retain all eight registrations and native persistence. Pencil opens a clear edit mode with Done, an add-widget tray, per-card hide/size/order, and keyboard alternatives to dragging. All-off still exposes the pencil. Unknown persisted widgets remain preserved. Normal mode has no rearrangement affordances.
6. **Settings:** secondary categories Appearance, Home, Discord & privacy. Appearance presents Theme, Background, Accent in that order. Home links to direct editing and retains reset. Discord master control and all separate detail/address consents stay visible in their category. Desktop integration leaves this presentation; native commands and installer ownership remain intact.

## System

7. **Theme / background / accent:** independent native persisted choices. Existing three themes and seven presets/custom accent remain. Default background is Simple, preserving current users' appearance. Borealis is explicit opt-in. OLED + Simple is true black.
8. **Background:** original generated aurora photograph, bundled locally and optimized. The pilot uses a slow, seamless transform of this asset rather than shipping unlicensed footage or a runtime shader. This is atmospheric movement of a still image, not filmed evolving curtains; replacement by a licensed natural-motion loop is an owner review decision. The 45-second drift uses 450 small steps (10 updates/second), after measuring the cost of continuous glass recomposition. No video decoder or JavaScript animation loop. Reduced motion/hidden/unfocused windows pause it. Plain mode does not mount the image.
9. **Glass:** one dark glass surface, 16 px backdrop blur, thin light edge, soft shadow. Applied to pilot panels only over Borealis; opaque theme surfaces otherwise. Menus remain opaque for readability. No blur sliders.
10. **Motion:** 120 ms hover, 180 ms state changes, 1 px press displacement for prominent buttons. No pulsing, page choreography, or perpetual player animation. Reduced motion removes decorative motion and transforms.
11. **Typography:** retain locally available system UI fonts. Strong 28–32 px Play, 26 px page headings, 14–15 px controls, 12–13 px metadata. Avoid downloaded fonts and excessive uppercase copy.
12. **Spacing:** extend existing 4 px scale; 24–40 px section gutters, 16–24 px panel padding, 12 px control gaps. Shell and pilot primitives live in a dedicated stylesheet, scoped to the pilot where needed.
13. **Responsive:** default 1120×760, existing minimum 720×520. Home caps its width instead of stretching Play indefinitely. Rail stays fixed, content alone scrolls. At narrow widths player collapses, widget columns become one, settings category nav becomes horizontal. Account name remains visible.
14. **Accessibility:** native buttons/radios/selects, visible focus, named icon controls, tooltips on hover and focus, non-color selected markers, keyboard widget controls, readable errors, dialog focus containment preserved. No motion required to understand state.
15. **Performance:** measure native/browser process CPU and working set with Simple, Borealis, player, minimized; record sample duration and limitations. Measure renderer draw duration and counts. Bound texture size; no network assets or fonts at runtime. Background and player lifetimes follow page/window visibility.
16. **Components:** shared Icon, TitleBar, Background, scoped surface/control/tooltip primitives; existing typed navigation, appearance, HomeLayout, readiness and account stores remain authoritative. Narrow native appearance DTO extension only; no trusted paths, launch arguments, tokens or arbitrary URLs added.
17. **Rollout:** proposal → shell/primitives → Home/editor → Settings/background → tests and actual visual inspection → owner review. Instances, content, Modrinth, accounts dialog, About and setup keep their current internal layouts.
18. **Acceptance:** inspect normal/maximized/minimum Home, OLED, edit/all-off/populated widgets, both Settings treatments, focus/tooltips, account dock and titlebar states. Screenshots and exact checks accompany the review; unverified native behavior is explicitly identified.
