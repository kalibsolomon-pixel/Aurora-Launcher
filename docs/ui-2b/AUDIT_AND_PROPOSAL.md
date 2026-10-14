# UI-2B audit and bounded proposal

Starting branch `codex/client-3-production-authority`, HEAD `a3154bd`. This is
three legitimate owner-directed follow-ups beyond the request's `bfa44d5`:
`df5d9da` removed card details and added readiness dots; `e56d64f` composed a
split control; `a3154bd` matched the 4:1 reference and moved the brand to the rail.
The Note screenshot in this conversation and current Aurora captures are
available; Lunar artwork is not, so only the supplied Lunar principles inform
this pass. No third-party assets are acquired.

## Architecture and materials

HomePage owns layout and displays the existing homeLaunchState projection of
instance/account-keyed native readiness and process snapshots. InstanceSwitcher
owns the two independent Launch/select buttons and keyboard listbox. PlayerPreview
owns account-keyed skin input and bounded manual software redraws. HomeWidgets,
homeLayout.svelte.ts and widgets/* own registry rendering and the existing typed
native persistence commands. AppShell/phase-f.css own the 96px rail and bottom
account dock; Background/borealis.ts own the bundled image/video lifecycle.
No state owner, native command or persistence schema needs to change.

Existing materials: --f-panel, --f-edge, --f-shadow, --f-radius-panel (22px),
--f-radius-control (14px), configured --color-accent tokens, 4px spacing scale,
120/180ms interaction motion and locally available system typography. Simple/OLED
and reduced transparency resolve opaque theme surfaces. Avoid new filtered
ancestors around an already filtered Launch button; use static CSS gradients.

## Diagnosis

1. Background dominance: bright curtains sit directly behind the action and
   headings. This is a contrast/hierarchy judgment, not an overflow defect.
2. Disconnected hero: independent 320×80 action and approximately 400px player
   stage have no shared edge or local lighting. Centered action floats in blank
   space; the player has no floor. This is an aesthetic grouping problem.
3. Isolated Launch: the compact selector identifies the instance, but no readable
   full-name context or version line supports it. Long names ellipsize. No launch
   authority or status computation defect was observed.
4. Widget transition: a quiet heading follows a large open stage, then much
   darker, separately filtered cards. The abrupt material change is aesthetic.
5. Rail separation: transparent rail shares all background brightness with the
   stage, making its active outline the only boundary. Existing targets and
   account placement are contained and must remain so.

Matched measurements are in before/geometry.json: four client CSS sizes,
DPR 1, browser scaling 100%, no native titlebar. No horizontal overflow was
observed. Native minimum remains configured 720×520; actual client geometry and
OS scale will be recorded during the separate isolated native pass.

## One design direction

Use one two-column hero with a translucent dark foundation, a quiet edge and
accent-derived lighting toward the player. Give its left region a full instance
heading, truthful installed-version line, existing compact split Launch control,
and quiet existing Settings access. The heading exposes the full identity while
the compact name retains the established selector affordance. Keep readiness as
the inline dot with accessible name/tooltip; blockers stay explicit. Align hero
and widget outer edges, and align left content to their shared padding rather
than retaining the earlier floating centered action.

Reduce the player stage to a deliberate bounded scale, add a static soft floor
and illumination, and retain independent rendering, controls and absent-skin
fallback. No generic skin, idle animation or new subscription. Use a localized
Home backing gradient to reduce bright curtains behind text; keep the image and
video, motion policy and other pages intact. Give widgets the same restrained
edge and dark material, without a surrounding dashboard card. Add only a faint
rail backing/boundary; preserve destinations, logo and account dock.

Implement and visually inspect hero first, then background, then widgets/rail.
Capture each stage. At <=800px the player still yields; text and control may
grow vertically, all saved widget spans still clamp to one column. Use existing
responsive tokens and ordinary vertical scrolling. Risks: listbox clipping,
200% name wrapping, text contrast, doubled blur, altered Home widget alignment,
and shared rail impact on Mods/Settings. Verify each with real geometry, browser
interactions and isolated native screenshots before concluding.
