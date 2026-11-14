# UI polish review, round 24

Frozen, unedited native GPUI captures using isolated disposable sample data,
taken with the same procedure, window size (1402 x 951 physical pixels at 125%
Windows scaling) and imported skin fixture as round 22.

Changes since round 23:

- Accounts: Remove… is a full-size, danger-tinted button so the destructive
  action is legible and distinct from Reconnect.
- Discover: per-row Install uses the accent-tinted Soft style instead of a
  column of solid primary buttons; Search remains the page's control focus.
- Content: Manage content is a labelled tool toggle (tune icon) rather than a
  disclosure chevron, and the Name column is wider than the file-name column.
- Skins: the library count is part of the Library caption instead of a
  free-floating badge.

| Page | Screenshot |
| --- | --- |
| Home | [home.png](home.png) |
| Instances | [instances.png](instances.png) |
| Discover | [discover.png](discover.png) |
| Content | [content.png](content.png) |
| Downloads | [downloads.png](downloads.png) |
| Skins | [skins.png](skins.png) |
| Accounts | [accounts.png](accounts.png) |
| Sync | [sync.png](sync.png) |
| Settings | [settings.png](settings.png) |

Supplementary captures cover the same interaction states as round 22 (Discover
paging and details, Content management/import/updates/removal review, Home
customization, Sync draft and Revert while off, Instance/Settings/Sync sections,
custom accent preview, skin preview controls and the smaller window sizes).

## Validation and limits

Native debug build; 383 tests passed. Captures were driven through UI
Automation against the disposable showcase root only; no content was removed,
no Sync change applied, no sign-in or install attempted, and the exact
disposable process was stopped afterwards. `screenshots-sha256.json` records
every PNG. Full keyboard/text-editing accessibility, the native file picker and
performance remain unverified, as in round 22.

The council criterion stays open until every participating blind reviewer
scores the nine main screens overall >= 9/10. Reviewers receive only the nine
PNG paths and the rubric.
