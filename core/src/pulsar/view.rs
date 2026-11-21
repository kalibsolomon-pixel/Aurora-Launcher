//! Stable presentation contract. Numbers are never formatted or silently filled with zero.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricDef {
    pub key: String,
    pub label: String,
    pub unit: String,
    pub help: String,
    pub confidence: String,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    Available,
    Estimated,
    Incomplete,
    #[default]
    Unavailable,
    RuntimeOnly,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricValue {
    pub definition: MetricDef,
    pub value: Option<f64>,
    pub availability: Availability,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FightKey {
    pub instance_id: String,
    pub fight_id: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryQuery {
    pub instance_id: Option<String>,
    pub search: String,
    pub outcome: OutcomeFilter,
    pub hits_only: bool,
    pub bookmarked_only: bool,
    pub tag: Option<String>,
    pub server: Option<String>,
    pub date_from_ms: Option<i64>,
    pub date_to_ms: Option<i64>,
    pub sort: HistorySort,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutcomeFilter {
    #[default]
    All,
    #[serde(alias = "UNKNOWN", alias = "INTERRUPTED")]
    NoResult,
    Loss,
    Win,
    Draw,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistorySort {
    #[default]
    Newest,
    Oldest,
    Hits,
    Accuracy,
    Reach,
    Duration,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisWindow {
    Last10,
    #[default]
    Last25,
    All,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnalysisRequest {
    pub query: LibraryQuery,
    pub window: AnalysisWindow,
    pub weapon: Option<String>,
    pub timing_result: Option<String>,
    pub focus: Option<FightKey>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DetailRequest {
    pub key: FightKey,
    pub participant: Option<String>,
    pub exchange_id: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DetailAnalysisRequest {
    pub detail: DetailRequest,
    pub weapon: Option<String>,
    /// ALL, HITS or NO_HIT. Only timing samples are affected by this filter.
    pub timing_result: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybackRequest {
    pub detail: DetailRequest,
    pub playhead_seconds: f64,
    pub event_filter: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareRequest {
    pub left: DetailRequest,
    pub right: DetailRequest,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRequest {
    pub key: FightKey,
    pub confirmed: bool,
    pub also_delete_from_instance: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub key: FightKey,
    pub destination: String,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderRequest {
    pub instance_id: String,
    pub kind: FolderKind,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FolderKind {
    #[default]
    Recordings,
    Exports,
    Library,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRoot {
    pub path: String,
    pub legacy: bool,
    pub exists: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIssue {
    pub path: String,
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceScope {
    pub instance_id: String,
    pub instance_name: String,
    pub registered: bool,
    pub source_roots: Vec<SourceRoot>,
    pub recordings: usize,
    pub status: String,
    pub stale: bool,
    pub errors: Vec<FileIssue>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    pub revision: u64,
    pub instances: Vec<InstanceScope>,
    pub rows: Vec<FightSummary>,
    pub total: usize,
    pub filtered_total: usize,
    pub has_more: bool,
    pub errors: Vec<FileIssue>,
    pub runtime_status: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub instance_name: String,
    pub source_path: String,
    pub source_root: String,
    pub modified_epoch_millis: u64,
    pub digest: String,
    pub imported_epoch_millis: u64,
    pub conflicts: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FightSummary {
    pub key: FightKey,
    pub start_epoch_millis: i64,
    pub duration_nanos: i64,
    /// UTC calendar day; the host may regroup in the user's local timezone.
    pub date_group: String,
    pub opponent: String,
    pub opponent_id: Option<String>,
    pub server: Option<String>,
    pub outcome: String,
    pub result_label: String,
    pub resolution: ResolutionView,
    pub metrics: BTreeMap<String, MetricValue>,
    pub manual: bool,
    pub participants: usize,
    pub exchanges: usize,
    pub timeline_loaded: bool,
    pub report_available: bool,
    pub stale: bool,
    pub provenance: Provenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation: Option<super::annotations::FightAnnotation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFightsWidgetSummary {
    pub total_fights: usize,
    pub wins: usize,
    pub losses: usize,
    pub draws: usize,
    pub recent: Vec<FightSummary>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationRequest {
    pub key: FightKey,
    pub bookmarked: bool,
    pub tags: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsExportRequest {
    pub query: LibraryQuery,
    pub format: String,
    pub destination: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub classification: String,
    pub label: String,
    pub source: String,
    pub confidence: String,
    pub scope: String,
    pub derived: bool,
    pub evidence_index: Option<i32>,
    pub event_sequence: Option<i64>,
    pub notes: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub recordings: usize,
    pub timeline_recordings: usize,
    pub context_recordings: usize,
    pub measurement_recordings: usize,
    pub mixed_reach_policy: bool,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisView {
    #[serde(default)]
    pub previous_period: Option<TabView>,
    #[serde(default)]
    pub previous_recordings: usize,
    pub coverage: Coverage,
    pub latest: Option<FightSummary>,
    pub focus: Option<FightKey>,
    pub overview: TabView,
    pub offense: TabView,
    pub defense: TabView,
    pub flow: TabView,
}
/// All tab sections use the same accessible, unit-bearing display primitives.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabView {
    pub metrics: BTreeMap<String, MetricValue>,
    pub charts: Vec<ChartView>,
    pub tables: Vec<TableView>,
    pub interpretations: Vec<String>,
    pub notices: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartView {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub x_unit: String,
    pub y_definition: MetricDef,
    pub series: Vec<ChartSeries>,
    pub bands: Vec<ChartBand>,
    pub description: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSeries {
    pub label: String,
    pub points: Vec<ChartPoint>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartPoint {
    pub x: f64,
    pub y: Option<f64>,
    pub sequence: Option<i64>,
    pub key: Option<FightKey>,
    pub category: String,
    pub label: String,
    pub availability: Availability,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartBand {
    pub start: f64,
    pub end: f64,
    pub label: String,
    pub lane: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableView {
    pub id: String,
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<TableRow>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRow {
    pub id: String,
    pub cells: BTreeMap<String, String>,
    pub metrics: BTreeMap<String, MetricValue>,
    pub sequence: Option<i64>,
    pub key: Option<FightKey>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantView {
    pub id: String,
    pub label: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeScope {
    pub id: String,
    pub participant_id: Option<String>,
    pub opponent: String,
    pub duration_nanos: i64,
    pub resolution: ResolutionView,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FightDetailView {
    pub summary: FightSummary,
    pub coverage: Coverage,
    pub participants: Vec<ParticipantView>,
    pub exchanges: Vec<ExchangeScope>,
    pub selected_exchange: Option<String>,
    pub termination: Option<String>,
    pub summary_tab: TabView,
    pub offense: TabView,
    pub defense: TabView,
    pub flow: TabView,
    pub evidence: TableView,
    pub timeline: Vec<EventView>,
    /// Lossless normalized format-6 analytical payload for rich drill-down views.
    /// This is calculated from raw recordings, never read from an exported report.
    pub derived: serde_json::Value,
    /// Frozen contact geometry by canonical sequence, including measured non-hit contacts.
    /// Values contain idealRegion, box, face and confidence without changing format-6 Derived.
    #[serde(default)]
    pub aim_regions: BTreeMap<i64, serde_json::Value>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    pub sequence: i64,
    pub time: f64,
    pub tick: Option<i64>,
    pub kind: String,
    pub confidence: String,
    pub detail: String,
    pub outcome: Option<String>,
    pub lane: String,
    #[serde(default)]
    pub attributed_incoming: bool,
    #[serde(default)]
    pub eligible_attempt: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackCheckpoint {
    pub time: f64,
    pub hits: usize,
    pub attempts: usize,
    pub incoming: usize,
    pub local_streak: usize,
    pub dominance: Option<f64>,
    pub dominance_state: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackView {
    pub duration_seconds: f64,
    pub playhead_seconds: f64,
    pub speeds: Vec<f64>,
    pub lanes: Vec<String>,
    pub events: Vec<EventView>,
    pub visible_events: Vec<EventView>,
    pub boundaries: Vec<ChartBand>,
    pub cumulative: BTreeMap<String, MetricValue>,
    pub notices: Vec<String>,
    #[serde(default)]
    pub checkpoints: Vec<PlaybackCheckpoint>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingView {
    pub key: String,
    pub group: String,
    pub label: String,
    pub value: serde_json::Value,
    pub persisted: bool,
    pub runtime_only: bool,
    pub help: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub instance_id: String,
    pub source_path: Option<String>,
    pub schema_version: Option<i32>,
    pub settings: Vec<SettingView>,
    pub warnings: Vec<String>,
    pub read_only: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpponentView {
    #[serde(default)]
    pub latest: Option<FightSummary>,
    pub id: String,
    pub label: String,
    pub fights: Vec<FightKey>,
    pub wins: usize,
    pub losses: usize,
    pub draws: usize,
    #[serde(alias = "unknown")]
    pub no_result: usize,
    /// Reasons grouped in the player's No result column.
    #[serde(default)]
    pub other_results: BTreeMap<String, usize>,
    pub last_fought_epoch_millis: i64,
    pub metrics: BTreeMap<String, MetricValue>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordView {
    #[serde(default)]
    pub source: Option<FightSummary>,
    pub id: String,
    pub label: String,
    pub key: Option<FightKey>,
    pub metric: MetricValue,
    pub minimum_attempts: Option<usize>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareMetric {
    pub key: String,
    pub left: MetricValue,
    pub right: MetricValue,
    pub delta: Option<f64>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareView {
    pub left: FightSummary,
    pub right: FightSummary,
    pub metrics: Vec<CompareMetric>,
    pub notices: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionReceipt {
    pub path: Option<String>,
    pub revision: u64,
    pub message: String,
}
