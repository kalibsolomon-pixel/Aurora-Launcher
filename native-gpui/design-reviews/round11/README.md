# UI polish review, round 11

Frozen native captures of the experimental GPUI launcher at 1402 x 951 physical
pixels, 125% Windows scaling, using disposable sample data. The primary task is
launching a saved instance, with content, configuration and account recovery
available through stable navigation. Production `core/` and Slint `native/` are
outside this presentation pass.

This revision groups the Home icon with its instance details, simplifies expired
account recovery to one recommended row action, replaces paused Sync switches
with explicit saved values, and shows commit controls only for drafts. Discover
integrates search submission and presents complete result rows inside a bounded
list with a scrollbar. Skins pairs the preview with appearance and cape status,
then gives local imports a full-width form. General instance settings include the
existing reviewed Client transition. Chart and Motion labels are simplified.
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

Additional captures cover Instance, Sync and Settings sections,
[Midnight](settings-midnight.png), [OLED](settings-oled.png),
[Home sign-in navigation](home-sign-in-navigation.png),
[Skins account navigation](skins-account-navigation.png),
[loaded project details](discover-project-details-loaded.png), and
[Sync draft](sync-enabled-draft.png) / [reverted](sync-reverted.png) states.
Capture verification compares successive frames for settled content while
allowing subtle background movement; captures are not edited composites.

Validation: native debug build and real Windows boot; 381 unit tests passed;
the Skins compile harness, Rust format and whitespace checks passed. Semantic
UI Automation exercised all nine pages, grouped sections, Home and Skins account
navigation, and Fabric API Details, which loaded real remote project information
without installation. The native Toggle action enabled the Sync draft; Apply and
Revert appeared, and Revert restored the saved off state and removed Apply.
Existing operation handlers, draft fields, confirmations and core APIs remain.
Session refresh remains available for both valid and expired saved accounts.

Text editors and the custom modal are not fully represented in the observed
UIA tree. Full keyboard/text-editing accessibility, semantic list scrolling, and
imported-skin rendering were not verified in this round. Performance was not
measured; no performance improvement is claimed. There are no tracked-file
deletions, and the pre-existing untracked native prototype remains untouched.

The screenshot council requirement remains open: fresh blind reviews must all
reach an overall rating of at least 9/10. Failed or missing reviews do not count
as approval. Reviewers receive screenshots and the rubric without prior ratings,
other reviewers' feedback or coordinator opinions.
