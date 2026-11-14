# UI polish: owner visual review

Council review concluded after Round 50 at the owner's direction. No further
council round is required; the PR remains open and unmerged for owner review.
The earlier unanimous overall >=9 criterion is superseded, not claimed achieved.

| Reviewer | Overall /10 | Would approve |
| --- | --- | --- |
| GPT-6.1 Terra | 8 | Yes |
| Kimi K3.1 | 8 | Yes |
| GLM-5.3 Flash (coding plan) | 9 | Yes |
| Inkling 2 Small | 9 | Yes |
| LongCat 2.5 | 9 | Yes |
| Claude Haiku 5.5 | 8 | Yes |

Full scores and advice are retained in [council-results.json](council-results.json).
These scores concern the frozen [Round 50 screens](../round50/README.md), before
the final fixes below. They are not ratings of these owner-review captures.

## Final changes

- Skin thumbnails expand their validated 8x8 pixels to 64x64 with nearest-neighbor
  sampling once on arrival, then retain the cached GPU image. This preserves
  pixel-art identity instead of stretching the raw eight-pixel image directly.
- Saved Sync inclusions use a constrained two-column list instead of a wrapping
  inline run. Explicit exclusions and Edit choices remain visible while Sync is off.
- Manage skin library can close while Favorites hides all rows. Busy-state
  protection and the disabled empty-library entry state are preserved.

## Advice retained for owner judgment

Content pagination stays bounded by vertical space. Showing all eleven items
at the reference window would require shorter rows or scrolling; additional
horizontal space does not increase the available row height. Typography and
contrast concerns remain visible for the owner's final assessment, using the
existing shared text tokens rather than making all metadata equally prominent.

Sparse Downloads and Instances regions are left without invented history or
filler. Saved runtime summarizes values from the separate editing tabs and is
not an additional control. Zero-activity chart days retain weekday positions and
a baseline, without fabricating activity bars. Account selection and authentication
remain separate states, with Reconnect as the recovery action. Existing action
weights, required-mod lock explanations, preview framing and accent controls are
retained for the owner's visual review.

## Evidence and verification

Fresh screenshots use an isolated disposable showcase data root. Nine main pages,
section tabs, disclosures, pagination, skin preview controls, filters, removal
review/cancel, Sync draft/revert and smaller windows are exercised through native
UI Automation. No skin/content removal, sign-in or Sync Apply is performed.
Earlier frozen screenshot manifests are verified unchanged.

Native debug build and Windows boot; all 385 tests pass. Rust formatting and
whitespace checks pass. No dependencies, core launcher operations or supported
Slint layouts changed in this final pass. Full keyboard/text-editing accessibility,
the complete native file-picker workflow and performance remain unverified.

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
