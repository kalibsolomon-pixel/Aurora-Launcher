# Character preview polish

Owner follow-up: remove Home's Instance settings button and enlarge the skin
slightly while reducing jagged edges. The control is removed; management remains
through the existing Instances destination. No launch or navigation logic changed.

The displayed raster height changes from 480–560px to 512–592px (512 to 544px at
an 800px viewport), roughly 6% larger. Browser smoothing replaces forced nearest-
neighbor pixelated scaling, softening the silhouette while retaining Minecraft
texture detail. The existing 560×800 raster, fixed angle, hero-only crop, skin
ownership, missing-skin explanation and narrow/error-state priorities remain.
No engine change, continuous rendering, new dependency or native command.

Validation: 62 affected frontend tests, type check with zero errors/warnings,
production frontend build, 17 Home browser cases, six Borealis cases and six
pixel/geometry crop cases; all exit 0. Inspected the 1280×800 render. Screenshots
and logs here are local synthetic browser fixtures; no new native acceptance is
claimed. The prior native Ready-character visual limitation remains documented
in the phase report. Existing native minimum/medium measurements remain historical.

All starting tracked paths, critical/protected hashes and seven worktrees are
preserved. No deletion, owner-data mutation, production change, push or release.
