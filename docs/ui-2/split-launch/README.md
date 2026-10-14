# Home split Launch control

Owner-requested follow-up to `df5d9da7d2ec4fa75fb9a1978d11d020b322607b`,
branch `codex/client-3-production-authority`. Starting tree had 1131 tracked
paths, seven worktrees and only the four expected local artifacts untracked.

The instance card is now only one split Launch control, with a literal 4:3
width/height footprint: 360×270px normally, shrinking to its column width when
necessary. Equal Home columns and the same 24px gap as the widget grid center it
over the first Small widget slot. At single-column widths, both center in the
available content width. No saved widget positions, sizes or format changed.

Its larger left action contains bold 24px Launch and regular 18px instance name
to its right. The prior green/red readiness dot stays beside the name. A thin
accent edge separates the right-hand dropdown arrow. Launch and selection remain
separate semantic buttons, with the existing command, busy guards and keyboard
listbox behavior. Long names ellipsize before the dot and arrow. Starting/Running
labels and accessible status descriptions remain. Detailed blockers/errors and
last-exit information appear below the fixed-ratio control only when needed.

Aurora's existing accent fill/hover/pressed/contrast, glass, edge, shadow, focus and
radius tokens supply the entire surface; the former outer card is removed. One
existing filtered surface is replaced by one filtered control, with no rendering
dependency or idle animation. OLED/Simple and alternate accents remain supported.

Validation: 53 affected frontend tests, zero type-check errors/warnings, static
production frontend build, 17 Home browser cases and 11 UI-1 browser cases.
Browser checks include measured 4:3 ratio, alignment to the first Small widget,
adjacent independent action targets, 700/400 text weights, one-pixel divider,
readiness, guarded repeated clicks, selection, long names, 200% text, dropdown
containment, resizing, widgets and persistence. Outputs and exit codes accompany
this note. Captures are local synthetic browser fixtures at DPR 1, not native
Tauri screenshots; no new native boot is claimed for this presentation follow-up.

Reviewed renders: [1280×800](home-1280x800.png), [1440×900](home-1440x900.png),
[1024×768](home-1024x768.png), [720×520](home-720x520.png),
[blocked](blocked-1280x800.png), and long-name/text-scale states under `states/`.

No Rust, native command/readiness/authentication, application configuration,
release/version or production installation change. No owner data, protected
local file, existing evidence or other worktree was modified. No deletion,
push, merge, release tag or publication.
