# P3-B.1a bounded camera continuity procedure

Declared before the first validation Play. This supplements P3-B.1 only for
camera/input recovery; it does not authorize another heap comparison. Historical
results and strict research quality gates retain their original classifications.

Use two fresh native-supervised processes, with a third available only for the
one justified correction/follow-up allowed by the owner. Stop after a repeated
failure; preserve every attempt. Each process gets a new world directory copied
byte-for-byte from the frozen `workload-heap-static-24-12-v1` snapshot, equal
options/configuration, 2048 MiB, empty custom JVM text, default G1, exact locked
Java and six-JAR stack. No preset, game-input, system or Client changes.

Archive SHA-256: `e53e838b0136a03dbdfa8848321063b001e44a33c3dc42d76001b85dfacaf287`.
Options SHA-256: `33695bc0df527fbfe49ec2177cc75bed7b46cadfc581ab542eedaa16ca20fe87`.
Camera: `(128.5,96,128.5)`, yaw 135, pitch 15. The older WORKLOAD_SPEC treats
movement >0.1 block or >0.5 degree as provisional warnings requiring review;
P3-B.1's newer fixed scene check requires exact saved orientation. F3 is rounded
to one decimal place and cannot establish exact orientation. Report those display
values as rounded observations. Normal-save metadata must additionally match the
P3-B.1 exact `[135.0,15.0]` orientation, without increasing tolerance. A mismatch
or an absent checkpoint excludes the session; never reset drift and keep capture.

1. Verify source inventory, options/configuration, mods, installed manifests,
   managed Java and executable. Use native deep validation/readiness and Play.
   Verify live managed root and WebView profile first; normal refresh remains
   Rust-owned. Owner handles interactive sign-in if required.
2. At the title screen, select Singleplayer and the copied world with keyboard
   navigation. No pointer clicks, drags, scrolling or repositioning after entry.
   No teleport is planned: loading the exact frozen camera is itself a gate.
3. Observe F3 after world entry, then again immediately before warm-up. Retain
   screenshot times, process identity and rounded camera values. Hide F3 before
   starting the existing 300-second foreground/resource sampler. Do not activate
   another window or issue UI input during warm-up.
4. Observe F3 after warm-up, hide it, then prepare the unchanged privacy-filtered
   JFR. Observe F3 immediately before capture and hide it. Start the existing
   30-second Overlay using F10 and immediately start the existing 0.5-second
   resource sampler; normal named JFR stop follows. Keep all game screens closed,
   F3 hidden, focus stable and UI input absent during the measurement.
5. Immediately after the recording, observe F3. Observe it once more after final
   inspection with no intervening pointer action. Retain images and actual gaps;
   screenshot/read operations are boundary work, not continuous monitoring.
6. Escape to the menu, keyboard-select Save and Quit, then Quit Game. Record
   native-supervised normal exit, read saved player position/orientation, retain
   world/options/config/log/observer evidence. Never overwrite previous captures.

The helper exposes coordinate clicks/drags/scrolls and keyboard chords; its
supported API does not certify raw/relative mouse deltas or promise that window
activation is input-neutral. Keyboard-only plus passive boundary snapshots reduces
known pointer operations, but does not prove the helper or physical input caused
B2. Foreground samples prove foreground ownership only. Endpoint agreement cannot
exclude a transient move-and-return between observations. A failed boundary names
the earliest observed interval, including possible inspection effects.

No new mod, camera lock, native hook or continuous monitor. If drift appears,
inspect the smallest implicated input interval, document at most one evidence-led
procedural correction, and use the remaining bounded follow-up. Otherwise report
PARTIAL with that specific next action. Two passing sessions support this procedure
on this host; unexplained historical root cause still requires a PARTIAL verdict.

These sessions are not a performance comparison. Keep the existing P3-B.1
F10-to-sampler gap gate (<0.5 s) unchanged and report its outcome separately.
A timing failure does not erase camera observations, and those observations do
not promote a failed timing window into eligible performance evidence.
