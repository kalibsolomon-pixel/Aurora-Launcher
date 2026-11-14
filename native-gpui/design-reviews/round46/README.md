# UI polish review, round 46

Frozen, unedited native GPUI captures using isolated disposable sample data,
taken with the same procedure, window size (1402 x 951 physical pixels at 125%
Windows scaling) and imported skin fixture as round 22.

Changes since round 45:

- Home lists only other instances beside the selected instance; native selection
  and return to the original instance were verified.
- Discover separates the title, concise description and author/download metadata.
  Wide descriptions end with an ellipsis and the project title opens full details.
- Skins groups Preview, Rename, Favorite and Remove into one wrapping action row.
- Sync groups saved choices into a responsive grid and labels their deferred use.
- Accounts summarizes all accounts needing sign-in on an opaque readable surface.

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

Native debug build; 385 tests passed; Rust formatting and whitespace checks passed. Captures were driven through UI
Automation against the disposable showcase root only; no content was removed,
no Sync change applied and no sign-in attempted. Before capture, Entity Culling,
Cloth Config API and FerriteCore were installed from Discover into the disposable Survival
Plus instance so Downloads and Content show real results, and the
disposable root carries a saved (off) Sync policy with two participating
instances; and the exact
disposable process was stopped afterwards. `screenshots-sha256.json` records
every PNG. Earlier frozen screenshot hashes were verified unchanged. Full keyboard/text-editing accessibility, the native file picker and
performance remain unverified, as in round 22.

The council criterion stays open until every participating blind reviewer
scores the nine main screens overall >= 9/10. Reviewers receive only the nine
PNG paths and the rubric.
