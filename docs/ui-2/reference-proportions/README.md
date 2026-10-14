# Reference proportions and sidebar branding

PASS — ready for owner visual review. Follow-up to starting HEAD
`e56d64f012d66bc03acb842e893f0bb2912e0f5d` on
`codex/client-3-production-authority`: 1164 tracked files, seven worktrees, and
only the four expected protected local artifacts untracked. See `baseline.json`.

The owner's latest screenshot supersedes the prior literal 4:3 request. Its
Play control measures approximately 229×57 in the displayed reference, so Home's
split Launch control now uses 4:1 proportions, normally 320×80px. It remains
centered over the first Small widget column. Bold Launch, the smaller regular
instance name, inline readiness dot and thin dropdown divider remain intact.
Large text may increase the minimum height to keep the action readable. The
selector menu now opens beneath the compact control.

The existing Aurora asset moves from Home to the top of the shared sidebar,
48×48px above the navigation icons (40×40px in short windows). Navigation moves
down naturally; the account dock remains at the bottom. Aurora's configured
accent, glass, radius, focus, hover and pressed materials remain in use.

Validation: 53 affected frontend tests, zero Svelte/type-check errors or warnings,
successful production frontend build, 17 Home browser cases and 11 UI-1 browser
cases, all exit 0. Browser geometry checks cover the 4:1 ratio, smaller logo above
navigation, account separation, widget alignment, independent launch/selection
targets, readiness, selection, guarded repeated clicks, resizing, persistence,
long names, 200% text, dropdown containment and alternate appearance settings.
No browser page errors occurred. Logs and exit records accompany this note.

Reviewed [1280×800](home-1280x800.png), [720×520](home-720x520.png), and
[200% text at 900px](states/text-200-900.png). Additional captures cover 1440×900,
1024×768, 900×700 and 801×600. All captures use synthetic local browser fixtures
at DPR 1. The small fixture provenance label is outside the production UI.
This presentation follow-up does not claim a new native Tauri boot.

No Rust, command, readiness, authentication, saved widget format, application
configuration, release/version or production installation changes. Owner data,
all protected local files, existing evidence and other worktrees remain intact.
No tracked deletions, push, merge, release tag or publication.
