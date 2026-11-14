# UI polish review, round 10

Frozen native captures of the experimental GPUI launcher at 1402 x 951 physical
pixels, 125% Windows scaling, using disposable sample data. The primary task is
launching a saved instance, with content, configuration and account recovery
available through stable navigation. Production `core/` and Slint `native/` are
outside this presentation pass.

This revision removes the detached account recovery panel, moves recovery beside
each account, replaces the empty mannequin with an explicit skin-preview empty
state, and centers the Downloads empty state. Disabled controls use neutral
surfaces. Discover retains project details beside installation, while Home
reflects the expired account state and offers a direct route to Accounts.
No new decorative effects or dependencies were added.

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

Additional captures cover Instance configuration sections, Sync sections,
Settings sections, [Midnight](settings-midnight.png), [OLED](settings-oled.png),
[Home sign-in navigation](home-sign-in-navigation.png),
[Skins account navigation](skins-account-navigation.png), and
[loaded project details](discover-project-details-loaded.png).

Validation: native debug build and real Windows boot; 381 unit tests passed;
Rust format and whitespace checks passed. Semantic UI Automation exercised all
nine pages, every grouped section, Home's sign-in route, Skins' account route,
and the contextual Fabric API Details action. The latter loaded real remote
project details without installation. Existing operation handlers, draft fields,
confirmations and core APIs remain in use. Session refresh remains available
for both valid and expired saved accounts.

Text editors and the custom modal are not fully represented in the observed
UIA tree. Full keyboard/text-editing accessibility and imported skin rendering
were not exercised in this round. Performance was not measured; no performance
improvement is claimed. There are no tracked-file deletions, and the pre-existing
untracked native prototype remains untouched.

The screenshot council requirement remains open: fresh blind reviews must all
reach an overall rating of at least 9/10. Failed or missing reviews do not count
as approval. Reviewers receive screenshots and the rubric without prior ratings,
other reviewers' feedback or coordinator opinions.
