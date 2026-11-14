# UI polish review, round 22

Frozen, unedited native GPUI captures using isolated disposable sample data.
The nine main screens are 1402 x 951 physical pixels at 125% Windows scaling.
The user job is to launch a saved instance and manage configuration, content,
accounts and local skins. Changes remain in native-gpui/; production core/ and
Slint native/ are preserved.

Home places Customize in the page header, removing the redundant Widgets heading
and caption. Its default dashboard fits without a page scrollbar at this size.
Content uses explicit line heights and 50px minimum rows, with the existing
near-label enable switches, management disclosure and confirmed removal intact.
Discover uses accent-colored project-title links for details and one Install action.
On wide windows, author and download metadata sit beside titles; shorter rows show
five complete results at the default size. Compact windows retain stacked metadata. Wide Content rows place filenames in an
aligned second column with Enabled, Name and File name headers; narrower rows keep
the stacked layout. Artwork, when present, remains inside the fixed name column. Chart labels are
larger, with more room between the vertical scale, plot and weekday labels.

The populated skin preview remains beside Import and the local Library. Preview,
Favorite/Unfavorite, Rename and Remove now have visible text labels on a wrapping
action row; the same existing handlers and removal confirmation are retained. Import
controls wrap at narrower widths; the page stacks and scrolls at the minimum size.
The empty preview still shares one surface with import, without an extra empty
library card. Signed-out account guidance is a quiet utility row; a repeated
selected-skin/cape status box is removed. Signed-in account controls are retained. The account guidance explains that local importing works signed out.
Checked file options use the existing accent token and a contrasting checkmark;
the master Sync switch retains its separate on/off role. Existing native renderer,
controls and typed core operations are retained.

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
updates/import disclosures and removal review/Cancel, Home customization open/Done,
account navigation, Sync category draft/Revert while off and master draft/Revert,
Instance/Settings/Sync sections, custom-accent preview/Cancel and smaller windows.
Requested smaller outer sizes are 1184 x 842 and 1084 x 752.

## Imported skin fixture

[Workshop sample](fixtures/workshop-sample.png) is an original 64 x 64 Classic
Minecraft UV texture: teal jacket, slate trousers, opaque base faces and transparent
outer layers. It is a local diagnostic asset, not a bundled production preset.
SHA-256: `fcc34682fcc290880d646610195746ba87677d8cd643abf588d181ad83ee6a23`.

It was imported through the existing authoritative core import operation, using
`cargo run --manifest-path native-gpui/Cargo.toml --example seed_council_skin --
<disposable-root> <sample.png>`. The helper requires an existing root containing
`AURORA_DISPOSABLE_DIAGNOSTIC` whose contents begin with `Disposable`; it does not
create or select production data. Launch the native app with `--showcase` and that
root, open Skins, then choose Preview for Workshop sample.

The main Skins image shows that actual imported local texture selected through UIA.
Supplementary captures retain the unselected/empty preview, Front/Back, zoom/reset,
layer toggle, Favorites-only empty result and confirmed-removal review/Cancel.
Native semantic readback verifies layer/filter state, and pixel comparison verifies
that Back and zoom visibly change the rendered player. No skin was worn or removed.

## Validation and limits

Native debug build and Windows boot; 383 tests passed; Skins compile harness, Rust
formatting and whitespace checks passed. Semantic UIA verified the listed routes,
management visibility, complete paged groups, off-state Sync draft and Revert,
customization Done, and preview controls. No content was removed, no Sync changes
applied and no installation attempted. The exact disposable process was stopped.
SHA-256 manifests record all screenshots; Round 21 hashes were verified unchanged.
No tracked-file deletion, new dependency, framework, backend API or persisted setting.

Full keyboard/text-editing accessibility, native file-picker import and performance
remain unverified. The helper exercises core import, not the complete native picker
workflow. Some editors and scrolling regions lack UIA operations. Minimum-size Skins
and Settings need ordinary page scrolling. The unchanged Client-review backend
identity rejection remains recorded in Round 13. No performance improvement is claimed.
The pre-existing untracked prototype remains untouched.

The council criterion stays open until every participating blind reviewer scores
this exact nine-screen set overall >=9/10. Missing, failed and superseded reviews
do not count. Reviewers receive only the nine main PNG paths and the rubric.
