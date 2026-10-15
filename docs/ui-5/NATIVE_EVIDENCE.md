# Native evidence index

All successful captures use the actual compiled frontend at `http://tauri.localhost/`,
the recorded debug executable and marked temporary managed/WebView roots. No
account, instance or owner preference was copied. Each JSON records actual CSS
client dimensions, DPR, control state and native preferences. Discord remained
off throughout. Windows display scaling / WebView DPR was 125% / 1.25.

## Successful native evidence

- `native-final/overview.*`: default 1120×790 client, Launcher desktop capability.
- `native-final/appearance-default.*`: original Simple/violet preferences.
- `native-final/borealis.*`, `cyan-verified.*`, `motion-verified.*`: actual native
  Borealis, Cyan and motion speed 70 changes; successful saved feedback.
- `native-final/home-widgets.*`, `widget-hidden.*`: the native widget list and
  hidden Instance Details; all order/size/other flags unchanged.
- `native-final/minecraft-java.*`: truthful empty-selection access to Library.
- `native-final/discord-disabled.*`: missing build registration, all flags off.
- `native-final/accessibility.*`, `about.*`, `updates.*`: actual supported content;
  no update installation or Discord connection was attempted.
- `native-final/minimum-proof.json`, `minimum-host.png`, `minimum-712x512.*`,
  `minimum-widgets.*`, `minimum-discord.*`: actual native **712×512** client.
  Corner/top-edge drags reached the minimum; further shrinking remained clamped.
  The configured 720×520 minimum was not changed or bypassed.
- `native-final/medium-proof.json`, `medium-host.png`,
  `medium-host-appearance-verified.*`, `medium-host-discord-verified.*`,
  `medium-widgets-verified.*`: actual native host **1024×768** client, reached
  through OS System > Size keyboard sizing and mouse alignment. Host screenshots
  independently show the complete app, not a resized embedded view.
- `native-final/keyboard.json`: actual End/Home/ArrowDown category and focus results.
- `native-final/reduced-transparency-simulated.*`, `media-simulation.json`: actual
  compiled WebView with **emulated** reduced-motion/transparency media preferences;
  opaque panel, no backdrop filter. Windows preferences were not changed.
- `native-final/before-restart.*`, `native-persistence/restored-home.*`,
  `restored-settings.*`, `restored-appearance.*`: native persistence and restored
  live rendering. Both final and restart process handles report exit code 0.
- `native-verification.json`: native preference equality, schema 7, exact widget
  preservation and separately reported UI-4 status.

## Retained attempts excluded from successful acceptance

`native-final/medium-updates.*` (1024×767) and `medium-1024x768.*` were produced
using Browser.setWindowBounds. On WebView2 that API changed/repositioned the
embedded child view while its host stayed large, producing clipped host output.
They are **not** native host-size acceptance. The view was restored before the
actual OS resize evidence above. No viewport emulation grants native acceptance.

`native-final/cyan-accent.*` follows an offscreen click refusal and still records
violet. `motion-value.*` records 50 after an input landed during scrolling.
Only the `*-verified` captures establish those successful changes. Original
attempt files are retained; none was overwritten to conceal the failed attempts.

The earlier `native/` session belongs to the interrupted first turn, with its own
root and clean exit. It is not the final acceptance executable. Owner Escape
stopped automation in that turn; the human subsequently requested resumption.

## Owner-data observation

The initial owner hashes were read at 03:07:57 UTC on October 15. The earlier
isolated app exited at 04:07:11 UTC. Production config last-write time was
04:07:23 UTC, during the pause and before the final isolated start at 07:27:44 UTC.
Its current appearance differs from the synthetic Cyan/70 state. No operation
targeted that production path, and it was never restored or rewritten by this
task. Cause of the concurrent write was not directly observed. The installed
executable, accounts, registry and skin-library hashes remained unchanged.
`owner-observations.json` records the distinction; blanket configuration-byte
identity is deliberately not claimed.

## UI-4 dependency, separately PARTIAL

UI-4 owner visual approval is retained. UI-5 host-size evidence applies to
Settings only. No native full-body skin or populated library scrolling was
verified in this account-free UI-5 root, so UI-4 remains technically PARTIAL.
Its original report and screenshots are byte-identical to the repository baseline.
