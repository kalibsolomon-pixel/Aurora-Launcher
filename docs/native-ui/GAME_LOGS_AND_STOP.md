# Game output and stopping a launch

Settings → Game contains **Show game log window when launching**, off by
default. **Game log** beside Play opens the selected instance's window at any
time; reopening focuses that window. It remains open after the game exits.
The window supports live following, scroll-to-pause, Jump to latest, text search,
warning/error filters, Copy all, local diagnosis, and opening existing game log
and crash-report folders. Rows use the existing theme and shared controls.

## Output ownership

`launch::process` drains stdout and stderr concurrently as bytes arrive. It
frames complete lines across reads, flushes the final partial line, and replaces
records exceeding 64 KiB with an omission notice. Session output is redacted
before insertion into a per-instance ring capped at 10,000 lines and 2 MiB.
Eviction counts appear in the window. Copy all copies all retained lines,
regardless of the current filter.

The existing bounded initialization/fatal-tail capture, redacted
`logs/aurora-launch-*.log` file, and unsuccessful-exit classifier remain in use.
Explicit Diagnose also examines up to 1 MiB of the newest `.txt` crash report.
It recognizes the existing mod, dependency, Java and mixin failures, plus memory,
native/driver and disk-space failures. It makes no network request.

The shared native API exposes snapshots and a Tokio watch subscription. The
window awaits notifications, coalesces output bursts for 50 ms, and sends through
a bounded channel to GPUI. It has no recurring idle timer. The list virtualizes
rows. Closing the log window cancels its observer; the session survives.

## Redaction and public upload

Launch-spec credentials, both forms of the authenticated UUID, and the activity
bridge capability are exact-value redactions. Label-based redaction additionally
covers access/refresh tokens, session IDs, client tokens, UUID arguments, Bearer
credentials and Authorization headers. Longer exact values are replaced first.

Upload applies another redaction pass: the current home directory becomes `~`
(including slash, escaped-backslash and case variants), Windows user-directory
names become `[user]`, and occurrences of the current username become `[user]`.
Unrecognized personal information emitted by mods can remain, as the public-link
confirmation explains.

Only **Upload public log** dispatches the upload. The core also requires the
confirmation flag. The request is a URL-encoded form POST to
`https://api.mclo.gs/1/log` with one `content` field. Content is capped below
2 MiB, discarding oldest complete lines with a truncation notice. Redirects are
disabled; connect/total timeouts are 10/30 seconds. Responses are capped at
64 KiB, require successful HTTP/API status, and must return an alphanumeric ID
and the exact corresponding HTTPS `mclo.gs/<id>` URL. Errors never echo response
bodies. Copy link and Open use the validated response. Tests substitute the
HTTP transport; they never contact mclo.gs.

## Stop capability and state transitions

Play becomes a danger-toned **Kill** immediately after launch intent. A
per-instance preparation reservation owns a cancellation signal and the existing
cooperative content-job control. Kill during preparation needs no confirmation:
it signals cancellation immediately, emits **Stopped by you**, and lets active
writes settle safely. Play remains disabled during that cleanup. Cancellation
wins over a simultaneous checkpoint error. Spawn checks the stop signal while
holding the control lock, preventing a cancelled preparation from spawning.

Once running, Kill requires confirmation about unsaved world progress and the
displayed PID must match the supervisor's in-memory capability. On Windows the
exact child starts suspended, joins a Job Object, then resumes. Explicit Kill
terminates that job, including descendants. The job has no kill-on-close flag.
If tree supervision cannot be established, the game runs without a Kill action.
Linux uses a dedicated process group; other platforms currently expose no
running Kill capability. No process-name lookup is used for termination.

A requested termination becomes **Stopped**, with no crash diagnosis. It also
finishes the playtime record as Interrupted and closes its activity observer. Normal
exit/failure behavior remains distinct. `kill_on_drop(false)` and the
non-killing Job Object preserve game survival when launcher supervision ends.
After restart, or for an otherwise unsupervised instance, a reported Running
state has no Kill capability. The launcher does not rediscover or adopt Java
processes. Output retained in memory belongs to the current/last session in this
launcher process; existing disk logs remain available after restart.

## Acceptance scope

Process tests launch the test executable as a long-running fake game and fake
descendant. They cover streaming before exit, split credentials, bounded records,
PID mismatch rejection, cancellation before spawn, tree termination, an
unrelated fake tree remaining alive, and survival after launcher-runtime
shutdown. UI tests cover Play/Kill labels, unsupervised states, filters and the
Stopped transition during preparation cleanup.

Showcase states display synthetic output and process states without launching
Minecraft or uploading. Captures are saved outside the repository at
`C:/Users/kalib/Aurora Launcher/work/game-logs/captures/`. Every capture requires
an absent `javaw` process and a window on the portrait secondary monitor.

Limitations: Copy selection is not implemented; use Copy all. Long rows are
clipped to the viewport, with full content available in Copy all. Real Minecraft
and a real public upload are intentionally outside this acceptance run.

Verification on Windows, 2026-11-15:

- Core: 1,228 tests passed, 0 failed, 36 ignored; 0 documentation tests.
- GPUI: 391 tests passed, 0 failed, 0 ignored.
- Core all-target checking, debug/release GPUI builds, touched-file formatting and
  whitespace checks passed.
- With the Simple background and motion disabled, the settled launcher plus log
  window consumed 0 ms CPU over 5,005 ms with no incoming output. This measurement
  does not describe the existing animated Borealis background or active output.
- Native accessibility inspection exposed action buttons, disabled states and
  confirmation dialogs. Snapshots did not expose search/static diagnosis text
  sufficiently to automate a search edit or read diagnosis text; filter actions
  and diagnosis were verified visually. Full screen-reader acceptance is open.

Captures: `log-live.png`, `log-filtered.png`, `log-upload-confirmation.png`,
`log-diagnose-result.png`, `play.png`, `kill-preparing.png`, `kill-running.png`,
and `kill-confirmation.png`. All eight were refreshed from the final debug
binary on the portrait secondary monitor with no `javaw` process running.
