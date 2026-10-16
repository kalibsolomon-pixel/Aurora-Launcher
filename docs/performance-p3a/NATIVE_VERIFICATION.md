# Native verification and screenshots

The compiled development host used production frontend assets, not a browser
fixture or installed production executable. Exact binary SHA-256:
`05b97f5be5517972863e2460738e36c1a92257853ec9f72809c610c81114e09a`.
Separate marked direct-temp managed and WebView roots; no copied owner file or
account. No authentication or Play command. No dashboard or UI changes.

Normal native create_instance installed Minecraft 1.21.11/Fabric 0.19.5, Aurora
disabled. Normal ensure_instance_runtime resolved and verified Java. The native
configuration update persisted Balanced with unchanged 2048 MiB/empty custom text;
actual exit/restart restored it. Restart native runtime validation verified 402
files/99,041,788 bytes and executed the normal bounded version diagnostic. A later
UI Check Java populated the existing Overview row. Content Ready is not world/
authentication/playability readiness; account remains Not signed in and game Not running.

Six **actual native WebView screenshots**, all 1120x790 CSS, DPR 1.25, no browser
viewport override. They substantiate states rather than native sizing acceptance
(P2 owns minimum-window geometry tests). No OS/native minimum-sizing claim is made.

| Image | Evidence |
| --- | --- |
| [isolated-startup.png](evidence/native/isolated-startup.png) | Empty fresh launcher, no account |
| [benchmark-instance.png](evidence/native/benchmark-instance.png) | Actually installed lightweight base instance |
| [performance-settings.png](evidence/native/performance-settings.png) | Existing P2 Balanced/2048 settings in isolated host |
| [runtime-and-auth-required.png](evidence/native/runtime-and-auth-required.png) | Content present, UI Java not yet checked, authentication absent; not a Java-Ready screenshot |
| [restart-settings.png](evidence/native/restart-settings.png) | Persisted settings after real process restart |
| [validated-runtime.png](evidence/native/validated-runtime.png) | UI Check Java shows Ready / Java 21 / 21.0.7; account absent and game not running |

[Initial checks](evidence/native/verification.json), [restart](evidence/native/restart.json),
[runtime UI](evidence/native/runtime-ui.json). All three retained exact processes
exited 0. Private exit/session paths remain in .git. The initial automation driver
reported a connection-closed error after successfully asking the window to close;
the process exited normally. The helper now handles only that expected close
disconnect; subsequent restart/UI drivers return 0. No assertion was weakened.

Additional safe preparation installed B/C through native creation with Aurora 3.0.0
and Fabric API, then fingerprint-reviewed native Modrinth installation of the exact
Sodium/Lithium versions. All artifact hashes were rechecked. Both returned complete
content Ready with no problems, while worlds/authentication/execution remain absent.
[Materials](evidence/native/materials.json), [digest proof](evidence/mod-environment.json).
That fourth exact native session also exited 0. The shared Java lock was not changed.

No real LaunchSpec/session arguments were exported. Effective order/equality are
proved by the existing native resolver test (included in the full Rust suite) and
documented source trace, not misrepresented as a real game command capture.
