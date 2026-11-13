# UI polish review, round 26

Frozen, unedited native GPUI captures using isolated disposable sample data,
taken with the same procedure, window size (1402 x 951 physical pixels at 125%
Windows scaling) and imported skin fixture as round 22.

Changes since round 25:

- Instances: the instance list card stretches with the detail column, so the
  master column reads as one panel instead of ending mid-page.
- Discover: project titles use full-strength text; the accent marks hover and
  focus.
- Content: file names use near-primary text instead of caption gray.
- Skins: Remove uses the same danger tint as account removal.
- Accounts: Select and Selected fill one fixed slot so rows align exactly.
- Sync: saved choices while off use a clear neutral check rather than a
  disabled-looking one.
- Home and navigation: chart gridlines and group labels have more contrast.

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
