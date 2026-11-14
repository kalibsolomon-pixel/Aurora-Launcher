# Content rows: owner visual refinement

Content now uses Discover's 48px artwork, stacked title and metadata, 14px spacing, quiet dividers, hover surface and 100px minimum row height. Toggles and visible labeled Delete buttons share a fixed trailing action lane. The former table headings and leading switches are removed. Delete is available without expanding Manage content and opens the existing guarded removal confirmation. All filesystem changes still go through the existing backend API and instance/permission checks.

Mods and resource packs retain their real enable state. Shaders show In game because the game/loader owns their activation. Required-mod explanations remain visible beneath filenames; their locked knobs use flex centering. Pagination accounts for row notes, wrapped compact toolbars and the update notice.

## Evidence

- `before-mods.png`: owner's supplied baseline; amber accent.
- `mods.png`, `mods-compact.png`, `delete-confirmation.png`: final build using the existing owner-review fixture; current black/red preferences preserved. Confirmation was canceled, with no owner-fixture deletion.
- Resource/shader screenshots and `*-deleted.png`: disposable seeded content, including generated ZIP packs. Actual deletion verified for all three categories; Cancel preserves the shader file. Resource switching persisted options.txt, and mod switching renamed the JAR to/from .disabled. See `functional-results.txt`.
- Normal window: 1418 x 959 physical pixels; compact: 1084 x 752, at 125% scaling. Compact pagination remains visible both with and without an update notice. Next/Previous verified. Resource/shader baseline shots precede the final pagination adjustment; their row styling is unchanged.

## Validation and limits

Native debug build, 385 passing tests, Rust formatting and whitespace checks. Semantic Windows UI Automation exercised uniquely named per-item switches/Delete buttons and dialog Cancel/Delete. One post-action switch read became stale during rerender; filesystem state and a fresh snapshot verified success before the next action. No physical input fallback used.

No new framework, dependency, persisted state or launcher-core behavior. Full keyboard traversal, light theme, exhaustive long-warning wrapping and performance were not measured. Existing dead-code compiler warnings remain. Council reviews were not restarted.
