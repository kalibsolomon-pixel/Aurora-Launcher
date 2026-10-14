# Page-wide selection accent correction

The workspace and content tab highlights shared a fixed cyan token. That token
now references the current appearance accent, so preset and custom colors apply
live to all instance tabs and the Installed/Browse and Discover content switches.
Home's Playtime range now also uses the accent and its soft backing. Main navigation,
Settings categories, Cosmetics tabs, selected instances and recent-target switches
already used the appearance palette; their behavior was verified unchanged.

Production changes are limited to `phase-f.css` and `PlaytimeWidget.svelte` styles.
There are no provider, instance, native, configuration or dependency changes.

Verification:

- `node tools/visual/tab-accent.test.mjs`: 84 combinations passed, no page errors.
  Actual components use browser-only fixtures: custom red, preset violet and cyan;
  Simple and Borealis; Home, Instances, Settings, Cosmetics, all five workspace
  tabs, installed Mods/Packs and browsing Mods/Packs/Modpacks. A standalone modpack
  browser has no nested content tabs, so its selected main navigation is checked.
- `npm run check`: zero errors and warnings; exit 0.
- Focused appearance/material tests: 11 tests passed; exit 0.
- `npm run build`: production static frontend passed; exit 0.

See [measured browser styles](results.json), [command results](checks.json), and
[red selected workspace/content tabs](red-selected-tabs.png). The other red PNGs
show the global pages. Screenshots are browser fixtures, not native acceptance.
The existing UI-3 acceptance evidence is retained unchanged. All pre-existing
tracked files outside the two scoped styles, all four protected local files and
all seven worktrees were verified unchanged. No owner data or preferences were
used. No push or release activity.
