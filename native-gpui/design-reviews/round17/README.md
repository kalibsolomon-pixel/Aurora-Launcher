# UI polish review, round 17

Frozen, unedited native GPUI captures using disposable sample data. Nine main
screens are 1402 x 951 physical pixels at 125% Windows scaling. The user job is
to launch a saved instance and manage its configuration, content and accounts.
This change stays within native-gpui/. Production core/ and Slint native/ remain
untouched.

Discover fits four complete results at the default size and separates total
matches from the loaded-result pager. All twenty loaded results remain reachable.
Content shows all eight sample items together, reclaiming the pager row when the
complete inventory fits. More actions groups folder, refresh and updates;
filtering, imports and confirmed removal retain their existing handlers.

Sync category choices can be edited while the master switch is off. The panel
explicitly explains that saving choices leaves Sync off; the typed policy draft
and existing Apply/Revert flow remain authoritative. The new regression test
verifies that preparing this write preserves disabled Sync and its baseline,
and Revert restores the draft. Instance and path controls retain their existing
master-switch dependency. Skin preview copy now describes the preview only;
import instructions stay in the adjacent form.

No new framework, dependency, decoration, backend API or persisted setting.
Existing shared typography, surfaces, semantic colors and controls are retained.

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

Additional captures cover five Discover groups and return, details, Content's
More actions, pagination, import/update disclosures, removal review/Cancel,
Home sign-in navigation, Sync off-category draft/Revert and master draft/Revert,
Instance/Settings/Sync sections, custom-accent preview/Cancel, and smaller windows.
Requested smaller outer windows are 1184 x 842 and 1084 x 752. PNG hashes are
recorded alongside; the prior frozen Round 16 checksums were verified unchanged.

Validation: native debug build and Windows boot; 383 tests passed; Skins compile
harness, formatting and whitespace checks passed. Semantic UIA verified the
listed routes and controls. The master switch was verified off after category
editing. No content was removed, no Sync changes applied, no install attempted.
The exact disposable application process was stopped after verification.

Full keyboard/text editing accessibility, imported-skin rendering and performance
remain unverified. Some native editors/scroll regions lack UIA operations.
At minimum window sizes pagination reduces capacity; Settings requires ordinary
page scrolling. Client review's unchanged backend artifact-identity rejection
remains documented in Round 13. No performance improvement is claimed. No tracked
files were deleted; the pre-existing untracked prototype was left untouched.

The council criterion remains open until every participating blind reviewer
scores this same nine-screen set overall >=9/10. Missing, failed and superseded
reviews do not count. Reviewers receive only exact main PNG paths and the rubric,
without previous ratings, feedback or coordinator opinions.
