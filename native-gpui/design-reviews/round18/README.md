# UI polish review, round 18

Frozen, unedited native GPUI captures using disposable sample data. Nine main
screens are 1402 x 951 physical pixels at 125% Windows scaling. The user job is
to launch a saved instance and manage configuration, content and accounts.
This work stays in native-gpui/; production core/ and Slint native/ are preserved.

Discover gives its four default rows more height, separates metadata with the
shared caption scale, uses the primary Install style, and shows one result range
against the catalog total. Disabled controls now have visibly reduced emphasis.
All twenty loaded results and the existing further-loading operation remain
reachable; no catalog entries or descriptions were removed.

Content keeps enable switches in the normal view. Manage content exposes folder,
refresh, updates and per-item removal, preserving the existing confirmation flow.
This removes repeated destructive actions from ordinary inventory scanning.
Account removal is quieter than Reconnect while retaining text, icon, focus and
confirmation. Accent choices align below their label instead of wrapping in a
narrow column. Empty Skins hides its unused library and duplicate appearance well;
import, account access, loading/error states and populated library stay available.

Sync file categories now use native semantic checkboxes, distinct from the master
on/off switch. The new wrapper uses the existing gpui-base behavior, palette and
focus treatment, with a 36px target. Selection still edits the same typed policy
draft; saving categories while Sync is off keeps it off. No new dependency,
framework, decoration, persisted setting or backend API was introduced.

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

Additional captures cover Discover groups/details, Content management, pagination,
updates/import disclosures and removal review/Cancel, account navigation,
Sync category draft/Revert while off and master draft/Revert, Instance/Settings/
Sync sections, custom-accent preview/Cancel, and smaller windows. Requested smaller
outer sizes are 1184 x 842 and 1084 x 752. SHA-256 manifests record every PNG;
previous Round 17 hashes were verified unchanged.

Validation: native debug build and Windows boot; 383 tests passed; Skins compile
harness, Rust formatting and whitespace checks passed. Semantic UIA verifies
management-mode visibility, the listed navigation/actions, and that category
editing leaves the master switch off. No content was removed, no Sync changes
applied and no install attempted. The exact disposable process was stopped.

Full keyboard/text-editing accessibility, imported-skin rendering and performance
remain unverified. Some editors and scrolling regions lack UIA operations. Smaller
windows use fewer complete rows; Settings needs ordinary page scrolling. The
unchanged Client review backend identity rejection remains recorded in Round 13.
No performance improvement is claimed. No tracked files were deleted; the
pre-existing untracked prototype remains untouched.

The council criterion remains open until every participating blind reviewer
scores this exact nine-screen set overall >=9/10. Missing, failed and superseded
results do not count. Reviewers receive only main PNG paths and the rubric,
without source, other ratings, previous feedback or coordinator opinions.
