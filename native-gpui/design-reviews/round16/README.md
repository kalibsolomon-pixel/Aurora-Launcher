# UI polish review, round 16

Frozen, unedited native GPUI captures using disposable sample data. The nine main
screens are 1402 x 951 physical pixels at 125% Windows scaling. The launcher job is
to start a saved instance, then manage its configuration, content and accounts.
This presentation pass is confined to native-gpui/; core/ and the Slint native/
application are preserved.

Discover replaces nested scrolling with viewport-sized groups of complete rows
and explicit Previous/Next controls. Project titles open details; one trailing
Install action remains. All twenty loaded sample results are reachable, and the
last group retains the existing Load more operation. Search resets the group.
Content uses the same complete-row pattern with filtering and accessible removal
confirmation. Updates and Import local are explicit disclosures; opening either
reserves space by reducing the row count. Narrow toolbars wrap without detached
column labels. Filenames use the foreground text token and removal actions use a
quieter existing button variant.

Saved Memory and Java values now belong to the selected-instance details, rather
than the master list. Account selection uses a quiet Select action beside the
separate Reconnect action. Accent choices include visible names and selected/focus
borders. Paused Sync describes future inclusion with Will include/Will skip. Home
gives Settings a quieter action tier beside Sign in to play. No new framework,
dependency, decoration, persisted setting, authentication or installation behavior
was added.

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

Additional captures cover all seven Discover groups and return to the first,
Content's second group, open/closed import and updates, removal review, project
details, account navigation, Sync draft/revert, Instance/Settings/Sync sections,
custom-accent preview/cancel and smaller windows. The smaller requested outer
sizes are 1184 x 842 and 1084 x 752; client captures exclude Windows borders.
Frames were compared for settling, and all PNG hashes are recorded alongside.
Earlier frozen Round 15 checksums were verified unchanged.

Validation: native debug build and Windows boot; 382 tests passed; Skins compile
harness, Rust formatting and whitespace checks passed. Semantic UIA verified
pagination, details open/close, import and update disclosures, removal review and
Cancel, account routes, custom accent Cancel, and Sync draft/Revert. No content
was removed, no sync changes were applied and no installation was attempted.
Project details was captured while its remote details were loading. The separate
Client review backend rejection remains documented in Round 13.

Full keyboard/text-editing accessibility and imported-skin rendering remain
unverified. Some native editors and scroll regions lack UIA operations. Settings
at the smallest size uses ordinary page scrolling; its lower preferences remain
below the initial viewport. Performance was not measured. No tracked files were
deleted; the pre-existing untracked prototype remains untouched. The disposable
application was stopped after verification.

The council requirement remains open until every participating fresh blind
reviewer scores this same set overall >=9/10. Missing, failed and superseded
reviews do not count. Each review receives only the exact nine main PNGs and the
rubric, without other ratings, previous feedback or coordinator opinions.
