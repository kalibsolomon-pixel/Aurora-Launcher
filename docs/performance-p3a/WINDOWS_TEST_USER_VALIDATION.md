> P3-A.2 supersedes this mandatory separate-user gate with the owner-approved
> same-user risk acceptance in [AUTHORIZATION.md](AUTHORIZATION.md). The procedure
> below records P3-A.1 and remains an optional future isolation approach.

# Windows test-user isolation — BLOCKED

The owner confirmed: **Not provisioned; document owner setup.** No Windows user was
created or modified. The active process belongs to the owner's existing security
principal, not a test user. Its private name/SID/session/token evidence is retained
in `.git/performance-p3a1-environment.json`; personal identifiers are not exported.
Built-in and Codex sandbox accounts are not authorized substitutes. A session-list
utility was unavailable; no assumption about an isolated session was made.

Source: [AUTHORIZATION.md](AUTHORIZATION.md), `auth/credentials.rs` target
`com.aurora.launcher/account/<uuid>`, and `paths.rs`. A different data root changes
filesystem state only. Windows [CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw)
uses the credential set associated with the current token's logon session. A verified
different Windows user/profile is required here; opening a second terminal, choosing
a different instance, or an elevation prompt is insufficient. No credential was
read, enumerated, exported, copied or rotated during this phase.

## Owner setup and principal verification

1. The owner creates or selects a dedicated **standard local Windows user**, through
   Settings > Accounts > Other users > Add account; for a local user choose the
   no-sign-in-information / add-without-Microsoft-account options. Follow Microsoft's
   [account setup guide](https://support.microsoft.com/en-au/windows/security/identity-signin/manage-user-accounts-in-windows).
   The owner handles any administrator prompt and passwords locally. This document
   is a procedure, not authorization for the agent to create accounts or elevate.
2. Switch user and interactively sign in to that new profile. Keep the owner's
   session intact. Do not use Run as administrator or repurpose Codex service users.
   Do not associate the test Windows profile with the owner's Microsoft account or
   import browser profiles, credential backups or Windows settings.
3. In an unelevated PowerShell under the test user, inspect the actual token:

   ```powershell
   $p3aIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
   $p3aPrincipal = [Security.Principal.WindowsPrincipal]::new($p3aIdentity)
   $p3aIdentity.Name
   $p3aIdentity.User.Value
   $env:USERPROFILE
   [Diagnostics.Process]::GetCurrentProcess().SessionId
   $p3aPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
   ```

   Record these locally and compare the SID against the privately recorded owner
   SID and the owner-selected test SID. Require a different SID/profile, expected
   interactive session and `False` for elevation. The pure `isolation_guard` in
   `readiness_checks.py` rejects equal/wrong SIDs and unknown/elevated contexts;
   passing supplied strings does not independently prove the token or vault.
4. Open Credential Manager **only in the test user's session** and visually confirm
   no imported Aurora entries before sign-in. Do not reveal secret values or export
   credential listings. Different verified user tokens plus a fresh, non-imported
   profile establish the namespace argument; an empty launcher account list alone
   does not. Do not inspect the owner's vault as a comparison experiment.

## Stage and start safely, after the checklist passes

Use a fresh test-owned direct-temp root with distinct `managed`, `webview`,
`benchmark-tools` and `benchmark-data` children. Retain its marker and local launch
record. Reject links/junctions/reparse ancestors and any path into the owner profile,
production application directory, repository, another worktree or `.minecraft`.
All instance paths must derive from the test managed root and registry IDs.

Hand off only a hash-locked development executable and reviewed benchmark source/
documents, plus the verified PresentMon executable/license. The existing P3-A
unauthenticated installed materials may be copied only through an explicit reviewed
allowlist: recorded instance `game` trees, exact locked mod JARs/ownership documents,
runtime/cache artifacts and unauthenticated registry identities. Never copy account
databases, configuration, WebView/browser data, credential material, owner instances,
worlds or whole profiles. Do not move or modify the original P3-A root. If a safe
allowlist cannot be established, recreate the same exact workload through native
creation/provider installation and revalidate all locks; report that deviation.

Recheck the development executable against P3-A native evidence (observed SHA-256
`05b97f5be5517972863e2460738e36c1a92257853ece9f72809c610c81114e09a`).
An altered/rebuilt binary needs a new recorded identity and review. Revalidate game,
mods and exact Java through native commands before Play. Do not inject Ready state.

`AURORA_DIAGNOSTIC_DATA_ROOT` is **debug-only**. A production executable ignores it;
never use installed Launcher 1.4.2 for this procedure. Before starting the approved
debug executable in the test session, set that variable to the test `managed` child
and `WEBVIEW2_USER_DATA_FOLDER` to its `webview` child. Launch via `Start-Process`
with `-WindowStyle Hidden -PassThru`, explicit working directory and private bounded
logs, retaining its exact process handle and executable hash. Omit remote-debugging
browser arguments. Verify the native-reported managed root before any account action.

The old `native-prepare.ps1` has a fixed existing session record, and the restart
helper deliberately refuses nonempty accounts. Do not overwrite/reuse their P3-A
records or weaken their guards for authenticated work. Preparation/start under the
test user remains pending; no runnable authenticated wrapper is asserted here.

Owner initiates normal Aurora system-browser Microsoft login and handles passwords,
MFA and consent locally. A legitimate Minecraft-entitled account is required; never
send secrets in chat. Authentication/entitlement failures stop the short session.
Stop game through its menu, wait for the exact supervised child's exit, close the
test launcher normally, retain logs and switch back to the owner's user. Do not
terminate other Java/launcher processes or sign out/remove the owner's account.

## Isolation checklist

| Check | Result this phase |
| --- | --- |
| Owner-selected separate Windows user exists | BLOCKED — owner says not provisioned |
| Live test SID differs from owner and matches approved SID | NOT TESTED |
| Fresh test profile, distinct credential namespace, no imported secrets | NOT TESTED |
| Unelevated test token and interactive session verified | NOT TESTED |
| Separate managed/WebView/registry/game/artifact paths proven | NOT TESTED |
| Native paths and locked development binary verified in test session | NOT TESTED |
| Normal isolated authentication and entitlement | BLOCKED |
| Production not accessed by test authentication | PASS — no authentication attempted |

Every isolation prerequisite must PASS before authentication/gameplay. Next owner
action: provision the standard test user and make its interactive session available.
Any tracing group change, software installation or elevation remains a separate
explicit decision; existing owner membership does not carry over.
