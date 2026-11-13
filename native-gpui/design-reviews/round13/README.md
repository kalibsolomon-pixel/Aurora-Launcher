# UI polish review, round 13

Frozen native captures of the experimental GPUI launcher, using disposable sample
data. Default screenshots are 1402 x 951 physical pixels at 125% Windows scaling.
The primary job is launching a saved instance; configuration, library management
and account recovery use stable navigation. Production `core/` and Slint `native/`
remain outside this presentation pass.

This revision gives Home's primary action the available width and labels the
instance total. Accounts has one primary Microsoft sign-in action, equally styled
Reconnect actions, and a shorter security note. Skins separates import from the
saved library and reduces/unframes the empty preview. Disabled Content entries
have an explicit badge. Paused Sync uses legible plain saved values. Downloads
centers the empty state in the available window height. Discover shows six complete
rows at the default size and adds a scroll cue. Aurora Client uses explicit
Install/Remove actions with the existing review handler and guards. No new effects,
dependencies, persistence or installation logic were introduced.

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

Additional screenshots cover Instance, Sync and Settings sections, Home/Skins
account navigation, Sync draft/revert, and smaller Discover windows.
[Small](discover-small.png) and [minimum](discover-minimum.png) captures are
1184 x 842 and 1084 x 752 physical pixels. Successive frames are compared for
settled content while allowing background motion. These are native captures,
not edited composites. SHA-256 values are in screenshots-sha256.json.

Validation: native debug build and Windows boot; 382 unit tests passed; the
Skins compile harness, Rust formatting and whitespace checks passed. Semantic
UI Automation exercised navigation, grouped sections and both account routes.
Enabling Sync exposed Apply/Revert; Revert restored the saved off state.
[Client review](instances-client-review.png) reached the existing backend,
which rejected published artifact identity with `update_metadata_invalid`.
Apply stayed disabled and Cancel closed the review; nothing was installed.
This verifies the button's review route and rejection state, not a successful
Client install. Alternate-theme Settings evidence remains in round12; this
revision does not change theme tokens or Settings.

Full keyboard/text-editing accessibility, semantic list scrolling, and imported
skin rendering remain unverified. The observed UIA tree does not fully expose
text editors. Performance was not measured; no performance improvement is
claimed. No tracked files were deleted; the pre-existing untracked prototype
remains untouched.

The council requirement remains open until all six fresh blind reviewers score
the same current screenshot set overall >=9/10. Prior, failed, missing or
superseded reviews do not satisfy that condition. Reviewers receive only the nine
main screenshots and rubric, with no ratings, previous findings or coordinator
opinions.
