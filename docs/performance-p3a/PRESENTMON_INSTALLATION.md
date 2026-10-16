# PresentMon 2.3.1 acquisition — PASS

Observed 2026-10-16, P3-A.1. Only the portable console executable was acquired.
No MSI, service, driver, registry/PATH change, elevation or production installation.

| Identity | Actual evidence |
| --- | --- |
| Release | Official Intel/GameTechDev `v2.3.1`, published 2025-06-10 |
| Asset | `PresentMon-2.3.1-x64.exe`, GitHub asset ID 262569611, 421,312 bytes |
| Expected SHA-256 | `364e5d98d4d134bd54dd25c22ed2ca2f4883f8bc3ed6502bee0c151e3436d30c` |
| Observed SHA-256 | Same; verified before execution |
| Authenticode | Windows `Valid`, signer Intel Corporation |
| Signer thumbprint | `9532F93661E3AAE2F6EFB34F1F926AE50530B5CF` |
| Timestamp signer | Sectigo Public Time Stamping Signer R36 |
| Version identity | Actual help banner `PresentMon 2.3.1`; PE FileVersion/ProductVersion absent |
| Execution | `--help` only, exit 1; help text retained, no ETW trace attempted |

[Official release](https://github.com/GameTechDev/PresentMon/releases/tag/v2.3.1),
[exact executable](https://github.com/GameTechDev/PresentMon/releases/download/v2.3.1/PresentMon-2.3.1-x64.exe),
[release API with asset digest](https://api.github.com/repos/GameTechDev/PresentMon/releases/tags/v2.3.1).
The expected digest came from GitHub's official asset metadata, not a mirror or a
locally invented expectation. It is a host-published asset digest; Authenticode
independently supplies the observed Intel signature status. Neither is a promise
that a capture will be complete or accurate.

Staged exclusively in a fresh direct-temp `aurora-p3a1-benchmark-tools-<uuid>`
directory; exact local path is in `.git/performance-p3a1-presentmon-staged.json`.
The executable, help and license are retained there. No executable is committed.
Rehash and recheck the signature when handing it to the test user; do not grant
that user access to the owner's profile or change ACLs to reuse this directory.

The pinned upstream [license](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/LICENSE.txt)
was reviewed and retained beside the executable. It is MIT-style, including notice
retention and warranty disclaimer. This phase does not redistribute the binary.
The release separately offers a 148,594,688-byte MSI; it was not downloaded.

The current process is unelevated and its existing token includes Performance Log
Users (SID `S-1-5-32-559`). This says nothing about a future test user's membership
or successful trace access. The pinned [permissions documentation](https://github.com/GameTechDev/PresentMon/blob/v2.3.1/README.md#user-access-denied)
requires tracing privileges. If the test user receives access denied, stop; request
separate approval for a narrowly specified membership/privilege change. Never use
`--restart_as_admin`, alter security protections or install the MSI as a workaround.

Bounded discovery before staging found no candidate in PATH or the named Intel/
PresentMon Program Files and LocalAppData directories, and no PresentMon process.
This is not an exhaustive disk search. Acquisition/identity PASS; actual tracing,
capture shutdown and process-selection operation remain NOT TESTED. Machine record:
[p3a1-status.json](evidence/p3a1-status.json).
