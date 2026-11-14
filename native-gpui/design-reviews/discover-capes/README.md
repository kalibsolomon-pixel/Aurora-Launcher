# Discover pagination, install state and cape clarity

Discover renders the full 20-result API batch on each page, using the existing outer page scrollbar. Next requests the next batch when necessary; Previous reuses loaded results. Page changes ask the parent to reset scroll during rendering. The former viewport-dependent 1-8 item pagination and separate Load more action are removed. Existing row size, API page size, result cap, filters, core operations and supported Slint behavior remain intact.

Busy projects replace the Install button with one fixed-width Installing indicator. The metadata status that previously collided with the disabled Install button is removed. A real Not Enough Animations install verified both the busy row and successful completion using disposable showcase data.

Capes show the outward 10x16 artwork through the existing tested player::cape_artwork UV extraction, enlarged with nearest-neighbor scaling once per cached texture. The collection displays a 60x96 portrait in a 72x112 well instead of shrinking the entire texture sheet into 44x22. View/Wear sit beneath the name, can wrap in narrow cards, and have unique accessible names. The Skins wrapper uses its content height so the outer page can scroll to the enlarged cape controls at compact sizes. Cape selection/wearing and the 3D renderer are unchanged.

## Evidence and checks

The three before images are owner-supplied. After images cover 1418x959 and 1084x752 physical windows at 125% scaling; the complete wide cape page uses 1418x1600. Bottom-of-page captures use the existing AURORA_SHOWCASE_SCROLL diagnostic override. Actual UI Automation snapshots assert 20 mod rows on each of pages 1 and 2, original identities on returning, and 20 resource-pack rows. Resource-page navigation uses the same handler, but only the mod Next/Previous sequence was verified. See verification.txt. Cape View is read-only; no account skin/cape change was submitted.

Native debug build and Windows boot, 385 tests passing, cargo fmt check and git diff whitespace check. The real pagination run caught a keyed-window-state call outside render; the final implementation stores a reset request and applies it during render. The final Next/Previous run completed without a panic.

Full keyboard traversal and semantic scroll automation remain unverified: the native page does not expose UIA ScrollItemPattern. Diagnostic scroll captures establish the list/footer and compact cape presentation without physical input. Automatic return-to-top is implemented but the diagnostic override prevents measuring its effect in these captures. Performance and light theme were not measured. No new framework, dependency, backend API or storage format. Council reviews remain concluded.
