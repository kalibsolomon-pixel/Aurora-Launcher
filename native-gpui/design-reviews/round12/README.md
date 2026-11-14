# UI polish review, round 12

Frozen native screenshots of the experimental GPUI launcher. Default captures are
1402 x 951 physical pixels at 125% Windows scaling, using disposable sample data.
The primary job is launching a saved instance; configuration, library management
and account recovery remain available through stable navigation. Production
`core/` and Slint `native/` are outside this presentation pass.

This revision puts Home activity below the launch controls, removes a duplicate
sign-in warning, and aligns chart labels with gridlines. Content gains a local
name/file-name filter, one Enabled column label, and compact, item-named removal
buttons retaining the existing confirmation. Discover uses available height for
one result scroller at larger sizes, and ordinary page scrolling at smaller sizes.
Skins puts imports beside the preview and account appearance below. Account
Reconnect explains saved-credential recovery; Sync omits the redundant off/idle
badge and uses a bounded Refresh button. Downloads uses an unframed empty state;
Motion's value aligns with its slider. No new effects or dependencies were added.

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

Additional screenshots cover Instance, Sync and Settings sections, Midnight and
OLED themes, account navigation from Home and Skins, loaded remote project details,
and Sync draft/revert. [Small](discover-small.png) and
[minimum](discover-minimum.png) Discover captures are 1184 x 842 and 1084 x 752
physical pixels. Capture verification compares successive frames for settled
content while allowing background movement. These are native captures, not edited
composites. SHA-256 checksums are in screenshots-sha256.json.

Validation: native debug build and real Windows boot; 382 unit tests passed,
including case-insensitive local filtering by name and file name. The Skins
compile harness, Rust format and whitespace checks passed. Semantic UI Automation
exercised all nine pages and grouped sections, both account-navigation paths,
Fabric API Details (real remote data loaded without installing), and Sync's
native Toggle action. Enabling the draft exposed Apply/Revert; Revert restored
the saved off state and removed Apply. Existing operation handlers and core APIs
remain. No tracked-file deletions; the pre-existing untracked prototype is untouched.

Full keyboard/text-editing accessibility, custom modal accessibility, semantic
list scrolling, and imported-skin rendering remain unverified. Text editors are
not fully represented in the observed UIA tree; the filter was verified through
its behavior test and visual capture, not physical typing. Performance was not
measured, and no performance improvement is claimed.

The council requirement remains open until every fresh blind screenshot reviewer
returns overall >=9/10. Failed or missing reviews do not count. Reviewers receive
only the nine main screenshots and the rubric, without ratings, prior feedback
or coordinator opinions.
