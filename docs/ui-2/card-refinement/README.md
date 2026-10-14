# Owner-requested Home card refinement

Baseline: `bfa44d526f0780c94eca1896410491afe1c67332` on
`codex/client-3-production-authority`, 1096 tracked paths, seven worktrees,
no tracked modifications. Existing protected untracked files remain unchanged.

Removed the installed-version metadata and Manage instance footer from the Home
instance card. Existing Instance Details widgets, saved widget layouts, selector
dropdown details and management through Instances remain available.

The primary button now reads **Launch**. Starting/Running labels, disabled guards,
native command/targeting and detailed errors remain unchanged. The Home-only
readiness dot is inside the selector, immediately right of its name and before
the chevron. Green follows the existing Ready/Running presentation; all other
states use red. Status has no visible word/badge but retains its tooltip,
accessible label and selector description. Long names shrink before the dot.
Other screens' status presentation is unchanged.

Verification outputs and new browser screenshots are in this directory; previous
UI-1/UI-2 evidence was preserved. The updated Home suite exercises dot position,
actual green/red token colors, all native states, long names at 200% text, removed
footer, selection and unchanged guarded launch. The UI-1 regression suite uses
the new Launch label while retaining its existing interactions and checks.

The initial full frontend run had 271 passing tests and one test-only failure:
its new dot assertion assumed adjacent CSS classes, overlooking Svelte's generated
class. The corrected assertion preserves the readiness/accessibility check;
affected frontend suites were rerun. The first browser attempt hit Vite's
"Outdated Optimize Dep" response while frontend suites shared its development
cache. The owned review server was restarted after those checks/build completed.
No application behavior was altered for either harness issue.

No Rust, authentication, launch pipeline, readiness authority, persistence schema,
Tauri configuration, version, release or production installation changes. This
follow-up's screenshots are browser fixtures; the preceding UI-2 native smoke
evidence remains historical, and no new native boot is claimed here.

Final checks: 53/53 affected frontend tests; type check with zero errors/warnings;
production frontend build; 17/17 Home browser cases and 11/11 UI-1 browser cases.
Commands/results and exit codes are retained alongside this note. Rendered Ready,
blocked, desktop/minimum and 200% long-name screenshots were inspected.

Review: [Home 1280×800](home-1280x800.png),
[blocked/red dot](blocked-1280x800.png), [minimum](home-720x520.png).
