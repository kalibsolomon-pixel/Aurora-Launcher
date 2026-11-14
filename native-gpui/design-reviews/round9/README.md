# UI polish review, round 9

This set shows the experimental GPUI launcher at 1402 × 951 physical pixels,
125% Windows scaling, using disposable sample data and the Borealis background.
The supported Slint host and launcher core are outside this presentation pass.

The primary tasks are launching a saved instance, managing its content and
configuration, and restoring an account session. The revision groups long forms
into sections, keeps Home widgets visible, labels content switches, simplifies
empty states, and separates destructive actions. Existing operation handlers,
draft fields, confirmations and core APIs remain in use.

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

Additional instance sections: [game version](instances-game-version.png),
[runtime](instances-runtime.png), [Java](instances-java.png), and
[maintenance](instances-maintenance.png). Sync's [conflicts](sync-conflicts.png)
remain accessible in a dedicated section with a persistent count in navigation.

Validation: native debug build and boot; 380 unit tests passed; Rust format and
whitespace checks passed. Semantic UI Automation navigation exercised all nine
pages, every new section, memory disclosure, and Skins → Manage accounts.
No deletions or production core/Slint changes are included. Full keyboard and
text-editing accessibility, alternative theme screenshots and performance
measurements remain unverified. No performance improvement is claimed.

The screenshot council has not approved this revision yet. Approval requires
fresh independent reviews, with every overall rating at least 9/10. Reviewers
receive screenshots and the same rubric without other ratings or feedback.
