# Calibration and measurement-quality protocol

Three distinct evidence levels: parser fixtures (performed), controlled native
presentation capture (NOT TESTED), actual Minecraft capture (BLOCKED). A browser
animation or another game's capture cannot close the Minecraft gate.

After installation approval, record source/release/binary SHA-256, permissions,
command options, GPU/driver/display mode and unique ETW session. Preserve complete
CSV plus producer diagnostics, including lost-event/buffer warnings. Do not stop
another trace session. Choose the exact authorized child PID plus creation time;
record intended swap-chain identity and reject unexpected processes or streams.

Use a separately reviewed bounded native OpenGL presentation test (not delivered
or run here): 20 seconds each at fixed 30/60/120 Hz, then an explicit 60/30 Hz
alternation and predeclared 100/250 ms stalls. Retain application-side monotonic
swap markers as a separate internal metric. Record vsync/compositor/VRR/refresh.
Expected pacing is a test stimulus, not proof every frame reached the physical
display. Reconcile emitted swaps, ETW rows, undisplayed rows, display estimates,
startup delay, shutdown tail and producer loss counters. Unexpected losses or
unexplained duplicate/non-monotonic estimates block calibration. Do not erase long
gaps or treat dropped presentations as missing CSV rows automatically.

Check a pre-roll and post-roll of at least 2 seconds around the bounded window.
Record before/after QPC frequency/origin and monotonic/UTC pairs; preserve integer
ticks. Two GC-uptime/run-clock anchors establish offset and drift; include acquisition
uncertainty. Provisional <=100 ppm drift and <=5 ms uncertainty are operational
screening limits, subject to pilot revision before comparing treatments. If larger,
reject alignment or label proximity coarse; never infer GC causation from proximity.
UTC identifies runs, not fine frame/GC alignment. Round-off cannot establish a
sub-millisecond causal relation.

Then, in an authorized isolated Minecraft session, validate lightweight Fabric
first, Aurora/Sodium second, demanding scene third. Obtain actual raw presentation
samples in each relevant mode and compare producer warnings/streams/timestamps
against observed game state. Test stable/variable caps and normal capture shutdown.
Retain main menu/loading separately. Prove the stream follows the exact child and
not DWM, another javaw or an overlay. Lock actual native client resolution/DPR and
render target dimensions independently of browser viewport assumptions.

Quality screens implemented in `quality.py`: missing/empty files, digests, portable
containment, columns, identity, units, positive finite intervals, duplicate/order,
boundary coverage, warm-up/sample depth, CPU counters, sampling gaps, clocks and
cohort locks. Long gaps remain in metrics with warnings; they can be real stutters.
Without producer loss evidence the offline tool cannot detect a missing frame that
looks exactly like a genuine long interval. Add that uncertainty to the run review.
The provisional 250 ms maximum boundary tolerance is an incompleteness screen,
not permission to remove 250 ms of undesirable gameplay from every run.

Instrument-on/off A/A pilots quantify overhead and noise using equal independent
repetitions, not browser FPS. Any numeric effect/resource threshold must be frozen
after those pilots and before testing heap alternatives. Capture calibration output
and Minecraft samples are absent in this P3-A; both gates remain open.
