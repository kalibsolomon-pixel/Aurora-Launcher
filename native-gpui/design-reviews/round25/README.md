# UI polish review, round 25

Frozen, unedited native GPUI captures using isolated disposable sample data,
taken with the same procedure, window size (1402 x 951 physical pixels at 125%
Windows scaling) and imported skin fixture as round 22.

Changes since round 24:

- Home: the playtime axis rounds its peak to readable values (for example
  3 h and 1 h 30 min instead of 2 h 16 min and 1 h 8 min).
- Discover: a tighter toolbar and slightly shorter rows leave a gutter below
  the results and pagination; search submits through a visible button.
- Accounts: Selected occupies a button-shaped chip in the same slot as Select,
  and Remove keeps its danger tint without a heavy outline.
- Sync: saved choices while Sync is off use a quieter neutral checkbox, and the
  status reads "Choices saved / Sync is off" instead of "All changes saved".
- Settings: the Motion description reads "Set to 0 for a still background."

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
