# UI polish review, round 27

Frozen, unedited native GPUI captures using isolated disposable sample data,
taken with the same procedure, window size (1402 x 951 physical pixels at 125%
Windows scaling) and imported skin fixture as round 22.

Changes since round 26:

- Instances: the saved runtime summary moves under the instance list as
  label/value rows, so values never truncate and the detail column stays focused.
- Sync: selected files keep the accent check; an inline notice states that Sync
  is off and checked files start syncing when it is turned on.
- Home: the hero's secondary action reads "Edit instance".
- Accounts: Reconnect is emphasized on the selected account that needs sign-in,
  and a divider separates Remove from the other actions.
- Content: rows show the source ("Modrinth") instead of internal ids, and a
  library other mods depend on gets a quiet "Required" badge instead of a warning.
- Downloads: the capture includes two real completed installs.
- Skins and Settings: Remove sits apart from the other library actions; the
  Custom accent chip has an add icon.

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
no Sync change applied and no sign-in attempted. Before capture, Entity Culling
and Cloth Config API were installed from Discover into the disposable Survival
Plus instance so Downloads and Content show real results; and the exact
disposable process was stopped afterwards. `screenshots-sha256.json` records
every PNG. Full keyboard/text-editing accessibility, the native file picker and
performance remain unverified, as in round 22.

The council criterion stays open until every participating blind reviewer
scores the nine main screens overall >= 9/10. Reviewers receive only the nine
PNG paths and the rubric.
