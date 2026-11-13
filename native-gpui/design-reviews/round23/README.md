# UI polish review, round 23

Frozen, unedited native GPUI captures using isolated disposable sample data,
taken with the same procedure, window size (1402 x 951 physical pixels at 125%
Windows scaling) and imported skin fixture as round 22.

Changes since round 22:

- Sync: while Sync is off, saved file selections render as neutral (dormant)
  checkboxes instead of accent-filled ones, so the page no longer reads as active.
  The checkboxes stay editable for the next time Sync is turned on.
- Accounts: Select and Selected share one fixed action slot so rows align;
  Selected uses full-strength text; Remove… is danger-tinted so the destructive
  action is distinct from Reconnect.
- Discover: the toolbar has more padding and separates the content-type tabs
  from the search field; project title links no longer clip descenders.
- Instances and shared list rows: explicit line heights so names such as
  "Skyblock" and "Forge" render their descenders.
- Skins: the signed-out Minecraft account row sits on a card surface instead of
  floating below the preview and import cards.

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
