# Isolated-session authorization gate

No Microsoft login, credential read/copy, entitlement bypass or Minecraft launch
was attempted. Installation readiness is distinct from authenticated playability.
The isolated root has zero accounts and no owner configuration copied into it.

Critical boundary: `src-tauri/src/auth/credentials.rs` derives the OS credential
target from account UUID only. A separate data/WebView root under the same Windows
user is **not** a separate secret store. Signing into the owner's same account
could rotate the refresh credential used by production. Removing that account could
delete that shared credential. Do not do either as a benchmark cleanup operation.

Safe proposed procedure, requiring explicit owner authorization:

1. Use a separate Windows test user/session, with its own Credential Manager and
   WebView profile. Creating/configuring that user is an owner action, not performed
   here. Alternatively review a dedicated test account proven unused in production;
   do not discover this by reading owner account databases. Changing credential
   namespace is separate compatibility/security work, not a P3-A workaround.
2. Recreate the disposable launcher root through the normal native preparation
   workflow in that session. Re-resolve/revalidate the locked official runtime and
   fail on identity drift. Do not copy accounts, tokens or owner worlds. The prepared
   root in this phase is useful unauthenticated evidence, not an authenticated handoff.
3. Owner explicitly authorizes a legitimate Minecraft-owned account and initiates
   system-browser Microsoft sign-in. Owner handles passwords/MFA/consent. Never embed
   login, intercept cookies, scrape pages or put credentials in scripts/manifests.
4. Aurora performs its normal OAuth/entitlement/profile/session checks. No fake
   identity, offline token or account-file injection. Cancel/error leaves the gate
   pending. Application-registration problems are blockers, not reasons to borrow IDs.
5. Owner authorizes the specific short capture-validation session and, separately,
   P3-B repeated gameplay after reviewing readiness. Specify permitted workloads,
   tools, resource/time window, retention and exact processes. No P3-B authority is
   implied by P3-A or by successful installation.

Separate approvals still needed: pinned frame-tool acquisition/installation and
ETW privileges if required; an available quiet resource window; disposable world
creation/material locks; any future automation mod. No external upload is needed.
Keep raw recordings local. Export only reviewed sanitized artifacts; no account
UUID/name/email, server destination, access/refresh token or full child command line.

Future duration: 300 s warm-up plus 600 s measurement, at least five usable runs
per effective configuration = >=75 minutes per workload/configuration, plus setup,
cooldown and failures. Expect sustained CPU/GPU work, instance heap plus native/GPU
memory, and multiple GiB of asset/world/raw-data storage. First calibrate a short
authorized capture. Do not close owner applications, change power settings or begin
the sustained series merely to make the system quiet.
