# UI polish review, round 30

Frozen, unedited native GPUI captures using isolated disposable sample data,
taken with the same procedure, window size (1402 x 951 physical pixels at 125%
Windows scaling) and imported skin fixture as round 22.

Changes since round 29:

- Accounts: every Reconnect is a contained button again; the selected
  account's is tinted.
- Sync: while off, saved choices recede (neutral checks, softened labels).
- Content: a library other mods need is locked on; off switches have an
  outlined track.
- Downloads: finished rows show when they finished ("Just now", "1 min ago").
- Home chart: axis values align against the plot.
- Skins: the preview's Zoom in, Zoom out and Reset tools carry visible labels.

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

Native debug build; 384 tests passed. Captures were driven through UI
Automation against the disposable showcase root only; no content was removed,
no Sync change applied and no sign-in attempted. Before capture, Entity Culling,
Cloth Config API and FerriteCore were installed from Discover into the disposable Survival
Plus instance so Downloads and Content show real results; and the exact
disposable process was stopped afterwards. `screenshots-sha256.json` records
every PNG. Full keyboard/text-editing accessibility, the native file picker and
performance remain unverified, as in round 22.

The council criterion stays open until every participating blind reviewer
scores the nine main screens overall >= 9/10. Reviewers receive only the nine
PNG paths and the rubric.
