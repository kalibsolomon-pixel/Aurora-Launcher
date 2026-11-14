# 2.2.1 acceptance

The Content job is to inspect and manage every installed item for the selected
instance. The Home customization job is to position and resize widgets precisely
without changing geometry on viewport changes.

## Changes and compatibility

- Removed Content pagination and its viewport-derived page size. All filtered
  entries render in the existing page scroll area; no nested scroll area added.
- Removed the global maximum-note row height. Each row uses the existing 100px
  minimum and grows only for its own notes. Artwork, right-side toggles and Delete
  actions retain existing controls, tokens and accessible names.
- Widget cells have four subcells per legacy cell. Optional `grid.subdivisions`
  accepts 1, 2 or 4; absence retains legacy meaning. Maximum fine dimensions are
  24 columns by 12 rows, with anchor row at most 252. Unknown widgets survive.
- Fine click-and-drag snapping is enabled by default, without manual sizing
  fields or snap selectors. Live previews resolve
  collisions with the same placement algorithm as committed edits. Previews and
  cancellations never save; responsive rendering preserves requested dimensions.
- Replaced large empty grid outlines with grid dots during customization.
- Installer uses the existing ownership-checked transactional replacement. The
  default retires the prior executable after activation; optional retention does
  not create a second installed-app entry or change data paths.

Older readers reject the new optional precision field. Reset Home layout in
2.2.1 before downgrade. The retained Slint reference host compiles with the DTO
addition but is not the production fine-grid renderer.

## Verification, 2026-11-14

- Core library: 1,213 passed, 34 intentionally ignored, zero failed.
- GPUI binary tests: 388 passed, zero failed. Includes legacy/fine round-trip,
  narrow viewport preservation, precision bounds and preview-without-save tests.
- Release tooling: 16 passed. Version contract passes for 2.2.1.
- Debug GPUI build passed with the existing production update public key.
- Installer acceptance rerun against the 2.2.1 debug payload uses an isolated
  package identity, registry key and filesystem root. It covers default retirement,
  explicit retention, occupied backup refusal, ownership/in-use/downgrade guards,
  handoff and uninstall, plus unknown-file and managed-data sentinels.

Native screenshots at 125% Windows scale use a disposable copy of 37 installed
mods, never the owner's live content. `C:/Dev/aurora-221-content.png` shows compact
rows and aligned actions; the accessibility tree exposes all 37 Delete controls
and no pager. `aurora-221-widgets.png` and `aurora-221-widgets-narrow.png` capture
an intermediate manual-control implementation, since removed at the owner
request. That implementation persisted a width change from 12 to 13 fine columns
and `subdivisions: 4` in the disposable configuration; narrowing retained the
saved width. Final screenshots capture the simplified drag-only controls.

Evidence logs are in `C:/Dev/aurora-221-*-tests.log`. Baseline screenshots supplied
by the owner show oversized mod rows and two-item pagination. No unexpected
tracked deletions; the pre-existing GPUI prototype remains untouched.

Performance was not measured. Pointer drag, physical keyboard focus, light theme,
screen-reader coverage and a production installed update/restart were not tested
end to end. Semantic controls and geometry unit tests cover the exercised edits;
these checks do not establish full accessibility compliance.
