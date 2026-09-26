# Phase D corrective acceptance — 2026-09-26

This report replaces the rejected Accounts-page/Home design. The user waived further automated window-size checks, performed the game launch themselves, and confirmed that window sizes were fine and the game launched properly. All work remains local.

## Git

Starting HEAD: `3dad55c17228268bd88289c262f8cc95d20977e6`. Fetched origin/main and merge-base: `1bf2e14edea3ba706d2d34b5aa8c72f82987ccdf`; starting ahead/behind **5/0**. Fetch immediately before committing confirmed the same remote. The five existing Phase D commits (e591a38, 5c130d7, 9c967ed, 6cfc8c4, 3dad55c) are preserved without amendments. Corrective implementation commit: `27220fa` — `fix: center Home on instances and move accounts into shell dialog`. Documentation receives a separate corrective commit; exact final hashes are in the final response/external Git audit. Nothing is pushed.

## Account correction

Automatic reuse occurred because the authorization request lacked an interaction prompt. Add account now requests `prompt=select_account`, Microsoft's supported chooser behavior. Refresh grants omit prompt and remain noninteractive; Check account does not start an account chooser. PKCE S256, state, scopes, loopback validation, Xbox/Minecraft exchanges, Credential Manager and token redaction remain unchanged. [Microsoft authorization-code documentation](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow) describes the prompt behavior.

Global navigation is Home, Instances, Settings and About. Bottom-left identity opens a shell-level native dialog over the current surface without navigation. Native modal focus entry/containment, inert background, Escape/Close and focus restoration apply. Inline removal confirmation consumes Escape first. Current identity, multiple accounts, switching, Add, Check, avatar refresh and confirmed local removal remain available. Signed-out identity opens the same dialog with Microsoft sign-in. No real account was removed.

The existing AccountsPage.svelte file is retained as live dialog contents only, with no page route, root-page import or duplicate hidden destination. Rust retains account/profile/auth authority.

## Home correction

The name/chevron picker is inside the primary instance card. Concise options show instance names/selection; the card shows authoritative Minecraft/platform/loader/Aurora configuration, status, Play and Manage Instance. The standalone selector, account row and Manage accounts action are removed. Workspace owns detailed management; sidebar shortcuts retain Workspace access without changing the Play target.

The right side shows a cosmetic full-body canvas from Rust-validated skin pixels. UV fixtures cover head, torso, arms, legs, classic/slim geometry, outer layers/transparency, modern distinct limbs and legacy mirrored limbs. Legacy opaque hat normalization preserves base-arm pixels. Existing PNG/locator/allocation restrictions and bounded native memory reuse remain. No frontend texture URL, disk image cache, dependency or heavyweight renderer was added. Missing/malformed skin uses a local player fallback without blocking Play.

Narrow layouts hide the decorative player before squeezing primary controls; the dialog has bounded sizing/scrolling. Empty-instance states remain coherent. Selection and Play retain the synchronized native C2 path, readiness authority and duplicate prevention.

## Provider review — 9c967ed

Phase D exposed Vanilla local content. The panels previously requested Fabric-only provider lifecycle data despite native Modrinth capability being unavailable, creating a misleading Vanilla error. This commit checks the existing native capability first and skips only the unavailable lifecycle read, clearing stale entries/errors. Local inspection remains available; Fabric keeps its existing lifecycle path.

The correction belongs to the exposed Phase D surface. It changes no Rust mutation/ownership, protected content, manifests, dependency previews or rollback and introduces no future loader/provider behavior. The full native capability/lifecycle/ownership/C2 suite passes; compiled configuration surfaces pass. Actual release Vanilla Mods showed local-only content, no Browse claim and no Fabric-only lifecycle error.

## Verification

| Check | Result |
|---|---|
| Full Rust suite | **477 passed / 16 ignored / 0 failed**: 471 library + 6 integration |
| Frontend suite | **92 passed / 0 failed**, 13 suites |
| Svelte/TypeScript | **0 errors / 0 warnings** |
| Rust formatting / all-target check | Passed |
| Frontend production build | Passed |
| Tauri MSI/NSIS production build | Passed |
| Version contract | **1.0.0**, passed |
| Whitespace / preservation audits | Passed |
| Real production boot | Passed; earlier corrective launcher normal close returned exit 0 |

Exact authorization URL/security coverage and refresh-grant prompt exclusion pass. Native additions cover legacy normalization and full-body cache corruption; all previous tests remain. Five pixel fixtures cover UVs, slim geometry, legacy mirroring, all outer layers and malformed fallback. Compiled Svelte tests cover modal state without navigation, signed-out surfaces, account commands, integrated selection, account-row removal, configuration/fallback and existing Play targeting/races.

## Live release acceptance and limits

The repository release executable was booted directly without installing over the user's older shortcut. Final-build Add account from signed-in state opened the actual Microsoft **Choose an account** interaction with **Sign in with a different account**, rather than silently completing. Cancellation safely preserved the original identity. Browser evidence is cropped to interaction labels, excluding URLs/codes/remembered-account details; no authentication-dialog input was automated.

The user added **WWolfram** during acceptance. Native switching between both real accounts succeeded with distinct heads. Per explicit user preference, **Spxcterr is restored and active**, with both records preserved. Dialog behavior was checked over Home, Workspace and Settings, including Close, Escape, keyboard entry, inert background and focus restoration.

Vanilla/no-Aurora, Fabric/no-Aurora and Fabric+Aurora Home selections all reached Ready with accurate configurations, integrated selection and matching Manage Instance destinations. Selection-only comparison before gameplay and later user content interaction found **19,347 instance files unchanged and registry byte-identical**. Selection did not reinstall or alter content.

The user performed the final game launch and confirmed success and acceptable window sizes. The original 1.21.11 Fabric+Aurora instance's new log independently records:

```text
[19:26:04] [main/INFO]: Loading Minecraft 1.21.11 with Fabric Loader 0.19.5
[19:26:10] [Render thread/INFO]: Aurora Client ready.
[19:26:26] [Render thread/INFO]: Stopping!
```

Afterward no managed game window remained and Workspace displayed Ready. Intermediate Starting/Running, disabled duplicate Play, game menu and supervised game exit code were **not captured during this user-operated corrective run**; Stopping alone does not prove exit 0. They retain deterministic coverage and earlier Phase D live evidence, which is not represented as a new corrective observation. Vanilla/Fabric-without-Aurora received Home inspection but no new live launches. Further sizing checks and narrow screenshots were explicitly waived; no unperformed observations are claimed.

Final read-only checks confirm the original selected instance, full launcher configuration (OLED/custom accent) and registry are byte-identical to starting snapshots. Both accounts remain, Spxcterr selected. User-driven content interactions and game writes are preserved, never rolled back.

## Evidence

All corrective files are outside Git at `C:\Users\kalib\AppData\Local\Temp\aurora-phase-d-corrective-evidence`. User screenshots are before-change references.

| Filename | Purpose |
|---|---|
| home-aurora-player.jpg | Revised Fabric+Aurora Home/full-body player |
| home-vanilla.jpg; home-fabric-no-aurora.jpg | Other accurate Ready configurations |
| integrated-picker-open.jpg | Concise integrated picker/readable options |
| account-dialog-home.jpg | Bottom-left modal over Home |
| account-dialog-workspace-two-accounts.jpg; account-dialog-final-two-accounts.jpg | Workspace modal/both identities/restored Spxcterr |
| account-dialog-settings.jpg; account-keyboard-focus.jpg | Settings overlay/keyboard focus |
| browser-choose-account-heading.jpg; browser-different-account-option.jpg | Actual final-build chooser, privacy-cropped |
| account-awaiting-browser.jpg; account-after-cancel.jpg | Native progress/safe cancellation |
| workspace-aurora.jpg; workspace-fabric.jpg; vanilla-local-content.jpg | Management entry/provider capability |
| selection-invariance.txt; user-launch-aurora-log.txt | Content invariance/new ready-stop log |
| rust-tests.txt; rust-check.txt; frontend-tests.txt; frontend-check.txt; production-build.txt | Automated/build results |
| production-exit.txt | Earlier corrective launcher normal close, exit 0 |

## Repository safety and deferred work

Baseline: **148 tracked files**, **606 protected untracked diagnostics**. Precommit audits compare every original tracked path and all 606 SHA-256 values. No tracked deletion or missing baseline file occurred; diagnostics remain unchanged/untracked. Root docs, manifests, lockfiles, frontend metadata and native configuration remain present. No generated build/game/runtime data or screenshots are staged. No broad cleanup, git clean, credential-file restore, .minecraft access or user-content rollback occurred. The second account was added by the user.

No Forge, NeoForge, Quilt, CurseForge, cross-process locking, durable crash recovery, automatic dependency conflict resolution, per-instance account binding, instance deletion or unrelated future work began. Stop after corrective Phase D for review. Do not publish.
