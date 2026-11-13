# UI polish review, round 14

Frozen native GPUI launcher captures using disposable sample data. Default images
are 1402 x 951 physical pixels at 125% Windows scaling. The main job is launching
a saved instance, with stable navigation for configuration, content and accounts.
Production core/ and Slint native/ remain outside this presentation pass.

Discover uses consistent title, description and metadata lines with more breathing
room and quieter Install actions. Content uses a visible Remove action that opens
the existing confirmation, labels its action column, and improves filename size.
Input text uses the shared body scale. Selected navigation has a restrained edge
distinct from hover. Account warnings are more legible and Selected is plain status
text with a checkmark. Paused Sync puts saved values beside their names; enabled
Sync retains editable switches. Instance panels align and the list's existing
Memory/Java summary stays at its bottom. Empty skin preview and import heights
match. Playtime tick labels use the body scale and a wider gutter. No decorative
effects, frameworks, dependencies, persistence or backend operations were added.

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

Additional captures cover Instance, Sync and Settings sections, Home/Skins account
navigation, Sync draft/revert, Content removal confirmation and smaller Discover
windows. Small and minimum captures use requested outer window sizes 1184 x 842
and 1084 x 752 pixels; the captured client dimensions can differ slightly because
of Windows borders. Successive native frames were compared for settled content.
PNG files are unedited application captures; SHA-256 values are recorded in
screenshots-sha256.json.

Validation: native debug build and Windows boot; all 382 unit tests passed; Skins
compile harness, Rust formatting and whitespace checks passed. Semantic native
automation covered all pages and sections, both account routes, removal review
and cancellation, and Sync draft/revert. No content was removed and Sync was not
applied. The unchanged Client review route's backend identity rejection is
documented in round13. Alternate-theme Settings evidence is in round12.

Full keyboard/text-editing accessibility, semantic list scrolling and imported-skin
rendering remain unverified. The UIA tree does not expose all text editors or
scroll containers. Performance was not measured. No tracked files were deleted;
the pre-existing untracked prototype remains untouched.

The council requirement remains open until all participating fresh blind reviewers
score this same snapshot overall >=9/10. Failed, missing or superseded reviews do
not satisfy the requirement. Reviewers receive only the nine main screenshots and
the rubric, without earlier ratings, findings or coordinator opinions.
