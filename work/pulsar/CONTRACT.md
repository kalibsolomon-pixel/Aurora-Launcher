# Pulsar view contract

Core owns `core/src/pulsar/view.rs`; native re-exports these same serde camelCase types. Application operations are async and own `spawn_blocking`; callers use `api::pulsar_*` and discard responses whose query/instance/selection revision changed.

- `pulsar_library` lists launcher-owned snapshots; `pulsar_refresh` imports finalized instance-root `phenomena/fights` and legacy `kwamster/fights`, then returns the same bounded view. `LibraryQuery` includes instance/all scope, opponent/server search, reference outcome/hits/sort filters, offset and limit (0 means default).
- `pulsar_analysis(AnalysisRequest)` returns Overview/Offense/Defense/Flow for Last10/Last25/All. `Coverage` distinguishes totals from loaded timelines and structured context. `TabView` uses metrics, accessible chart series/bands and tables. IDs are stable semantic names. Missing chart values are gaps.
- `pulsar_detail(DetailRequest)` selects a fight by `(instanceId,fightId)`. Manual participant selection chooses its first exchange unless an explicit matching exchange is selected. Entire parent scope has no tracked opponent. All tabs, evidence and timeline use the selected scope's origin. `derived` is the complete format-6 analytical dictionary computed from raw data; it supplies geometry, distributions, frozen measurements and advanced selectors without parsing reports.
- `pulsar_playback` provides canonical lanes, bounded event table at the playhead and cumulative metrics. Playback timing is UI state; core does not start timers.
- Every `MetricValue` carries its exact MetricDefs definition and availability. `value:null` means unavailable; numeric zero is observed zero. Extra supported measurements use explicit definitions. Do not assign skill grades to descriptive data.
- `pulsar_settings` is a read-only persisted/default inspector. Runtime fields/actions are labelled Change in game. `pulsar_opponents`, `pulsar_records`, `pulsar_compare` supply the beyond-parity views.
- `pulsar_open_folder`, `pulsar_view_report`, `pulsar_export`, `pulsar_delete` are explicit actions. Export/delete require `confirmed:true`. Delete writes a tombstone. Optional instance deletion is refused while launching/running. Existing reports are copied unchanged; no report generation or config writes.

Rust request defaults: Last25, newest history, all outcomes, no hits filter, all instances. String enum names follow the annotations in view.rs (history uses SCREAMING_SNAKE_CASE; analysis window camelCase). Results and errors are bounded. Structured error codes use `pulsar_*`. Source roots, retained/deleted-instance identity, conflict notices, stale snapshots and per-file errors remain visible.

Contract milestone: operation bodies may initially return `pulsar_not_ready`; later implementation is additive. Breaking names/shapes must be recorded in CONTRACT_CHANGES.md before use.
