use super::{
    Error,
    analytics::{self, format, metrics},
    parse, projection, reports, settings,
    store::{self, Entry, Index, Source},
    view::*,
    wire::FightRecord,
};
use crate::{
    instances::{InstanceId, InstanceRegistry},
    paths::ManagedPaths,
};
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

pub(crate) fn sources(paths: &ManagedPaths) -> Result<Vec<Source>, Error> {
    let registry=InstanceRegistry::load(&paths.instance_registry_file()).map_err(|_|Error::new("pulsar_registry_unavailable","The instance registry could not be read; retained library snapshots are still available."))?;
    if registry.instances().len() > 1000 {
        return Err(Error::new(
            "pulsar_scope_limit",
            "Too many registered instances for one refresh.",
        ));
    }
    Ok(registry
        .instances()
        .iter()
        .map(|i| Source {
            id: i.id().to_string(),
            name: i.display_name().into(),
            root: paths.instance_paths(i.id()).root().to_path_buf(),
        })
        .collect())
}
fn validate_query(q: &LibraryQuery) -> Result<(), Error> {
    if let Some(id) = &q.instance_id {
        InstanceId::new(id)
            .map_err(|_| Error::new("pulsar_invalid_instance", "Invalid instance identity."))?;
    }
    if q.search.len() > 256 || q.limit > 1000 || q.offset > 20_000 {
        return Err(Error::new(
            "pulsar_query_limit",
            "Search, offset or page size exceeds the bounded history query.",
        ));
    }
    Ok(())
}
fn validate_filters(weapon: &Option<String>, timing: &Option<String>) -> Result<(), Error> {
    if weapon.as_ref().is_some_and(|w| w.len() > 256) {
        return Err(Error::new(
            "pulsar_query_limit",
            "Weapon filter is too long.",
        ));
    }
    if timing
        .as_ref()
        .is_some_and(|t| !matches!(t.as_str(), "ALL" | "HITS" | "NO_HIT"))
    {
        return Err(Error::new(
            "pulsar_invalid_filter",
            "Timing result must be ALL, HITS or NO_HIT.",
        ));
    }
    Ok(())
}
fn metric_sort(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.total_cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}
pub(crate) fn filtered<'a>(index: &'a Index, q: &LibraryQuery) -> Result<Vec<&'a Entry>, Error> {
    validate_query(q)?;
    let search = q.search.trim().to_lowercase();
    let mut entries: Vec<_> = index
        .entries
        .iter()
        .filter(|e| {
            q.instance_id
                .as_ref()
                .is_none_or(|id| id == &e.key.instance_id)
        })
        .filter(|e| {
            let f = &e.record;
            let outcome = f.outcome.as_deref().unwrap_or("UNKNOWN");
            let matches = match q.outcome {
                OutcomeFilter::All => true,
                OutcomeFilter::Win => outcome == "WIN",
                OutcomeFilter::Loss => outcome == "LOSS",
                OutcomeFilter::Draw => outcome == "DRAW",
                OutcomeFilter::Unknown => outcome == "UNKNOWN",
                OutcomeFilter::Interrupted => outcome == "INTERRUPTED",
            };
            matches
                && (!q.hits_only || f.statistics.hits > 0)
                && (search.is_empty()
                    || format!(
                        "{} {} {}",
                        f.opponent_label.as_deref().unwrap_or(""),
                        f.opponent.as_deref().unwrap_or(""),
                        f.evidence().server.as_deref().unwrap_or("")
                    )
                    .to_lowercase()
                    .contains(&search)
                    || f.session.as_ref().is_some_and(|s| {
                        s.participants.iter().any(|p| {
                            format!(
                                "{} {}",
                                p.id.as_deref().unwrap_or(""),
                                p.label.as_deref().unwrap_or("")
                            )
                            .to_lowercase()
                            .contains(&search)
                        })
                    }))
        })
        .collect();
    entries.sort_by(|a, b| {
        let (a, b) = (&a.record, &b.record);
        match q.sort {
            HistorySort::Newest => b.start_epoch_millis.cmp(&a.start_epoch_millis),
            HistorySort::Oldest => a.start_epoch_millis.cmp(&b.start_epoch_millis),
            HistorySort::Hits => b.statistics.hits.cmp(&a.statistics.hits),
            HistorySort::Accuracy => metric_sort(
                analytics::ratio(
                    a.statistics.hits as f64,
                    (a.statistics.attempts - a.statistics.shield_blocks).max(0) as f64,
                ),
                analytics::ratio(
                    b.statistics.hits as f64,
                    (b.statistics.attempts - b.statistics.shield_blocks).max(0) as f64,
                ),
            ),
            HistorySort::Reach => metric_sort(
                analytics::ratio(a.statistics.reach_sum, a.statistics.reach_count as f64),
                analytics::ratio(b.statistics.reach_sum, b.statistics.reach_count as f64),
            ),
            HistorySort::Duration => b.duration_nanos.cmp(&a.duration_nanos),
        }
        .then(b.start_epoch_millis.cmp(&a.start_epoch_millis))
    });
    Ok(entries)
}
pub(crate) fn summary(paths: &ManagedPaths, e: &Entry) -> FightSummary {
    let f = &e.record;
    let mut values = metrics::summary_values(&analytics::summary(f));
    values.extend(super::cards::summary(f, None));
    for (key, label, unit, help, value) in [
        (
            "duration",
            "Duration",
            "s",
            "Recorded elapsed duration.",
            Some(f.duration()),
        ),
        (
            "shieldBlocks",
            "Confirmed shield blocks",
            "",
            "Frozen count; confirmed blocks are excluded from the accuracy denominator.",
            (f.schema_version >= 3).then_some(f.statistics.shield_blocks as f64),
        ),
        (
            "attacksIntoShield",
            "Attacks into a raised shield",
            "",
            "Frozen observed raised-shield count.",
            (f.schema_version >= 3).then_some(f.statistics.attacks_into_shield as f64),
        ),
    ] {
        let metric = if metrics::definitions().iter().any(|d| d.key == key) {
            metrics::metric(key, value)
        } else {
            metrics::extra(key, label, unit, help, value)
        };
        values.insert(key.into(), metric);
    }
    FightSummary {
        key: e.key.clone(),
        start_epoch_millis: f.start_epoch_millis,
        duration_nanos: f.duration_nanos,
        date_group: format::date_group(f.start_epoch_millis),
        opponent: if f.session.is_some() {
            "Manual session".into()
        } else {
            format::opponent(f.opponent_label.as_deref(), f.opponent.as_deref())
        },
        opponent_id: f.opponent.clone(),
        server: f.evidence().server.clone(),
        outcome: f.outcome.clone().unwrap_or("UNKNOWN".into()),
        result_label: format::result(f),
        resolution: analytics::resolution(f),
        metrics: values,
        manual: f.session.is_some(),
        participants: f
            .session
            .as_ref()
            .map(|s| s.participants.len())
            .unwrap_or(0),
        exchanges: f.session.as_ref().map(|s| s.exchanges.len()).unwrap_or(0),
        timeline_loaded: false,
        report_available: reports::available(paths, e),
        stale: e.stale,
        provenance: e.provenance.clone(),
    }
}
fn library(paths: &ManagedPaths, index: &Index, q: &LibraryQuery) -> Result<LibraryView, Error> {
    let entries = filtered(index, q)?;
    let limit = if q.limit == 0 { 200 } else { q.limit };
    let sources = sources(paths);
    let mut scopes: BTreeMap<String, InstanceScope> = BTreeMap::new();
    let mut errors = Vec::new();
    match sources {
        Ok(sources) => {
            for source in sources {
                let roots = [("phenomena", false), ("kwamster", true)]
                    .into_iter()
                    .map(|(name, legacy)| SourceRoot {
                        path: source.root.join(name).to_string_lossy().into_owned(),
                        legacy,
                        exists: source.root.join(name).is_dir(),
                    })
                    .collect();
                let issues = index.issues.get(&source.id).cloned().unwrap_or_default();
                scopes.insert(
                    source.id.clone(),
                    InstanceScope {
                        instance_id: source.id,
                        instance_name: source.name,
                        registered: true,
                        source_roots: roots,
                        recordings: 0,
                        status: if issues.is_empty() {
                            "ready"
                        } else {
                            "partial"
                        }
                        .into(),
                        stale: !issues.is_empty(),
                        errors: issues,
                    },
                );
            }
        }
        Err(error) => errors.push(FileIssue {
            path: paths
                .instance_registry_file()
                .to_string_lossy()
                .into_owned(),
            code: error.code.into(),
            message: error.message,
        }),
    }
    for e in &index.entries {
        let scope = scopes
            .entry(e.key.instance_id.clone())
            .or_insert_with(|| InstanceScope {
                instance_id: e.key.instance_id.clone(),
                instance_name: e.provenance.instance_name.clone(),
                registered: false,
                source_roots: vec![SourceRoot {
                    path: e.provenance.source_root.clone(),
                    legacy: e.legacy,
                    exists: false,
                }],
                status: "retained".into(),
                ..Default::default()
            });
        scope.recordings += 1;
        scope.stale |= e.stale;
    }
    Ok(LibraryView {
        revision: index.revision,
        instances: scopes.into_values().collect(),
        rows: entries
            .iter()
            .skip(q.offset)
            .take(limit)
            .map(|e| summary(paths, e))
            .collect(),
        total: index.entries.len(),
        filtered_total: entries.len(),
        has_more: q.offset.saturating_add(limit) < entries.len(),
        errors,
        runtime_status: "Live capture state is available only in game.".into(),
    })
}
pub fn list_library(paths: &ManagedPaths, q: LibraryQuery) -> Result<LibraryView, Error> {
    library(paths, &store::read_index(paths)?, &q)
}
pub fn refresh_library(paths: &ManagedPaths, q: LibraryQuery) -> Result<LibraryView, Error> {
    validate_query(&q)?;
    let sources: Vec<_> = sources(paths)?
        .into_iter()
        .filter(|s| q.instance_id.as_ref().is_none_or(|id| id == &s.id))
        .collect();
    let index = store::refresh(paths, &sources)?;
    library(paths, &index, &q)
}

fn analysis_budget(f: &FightRecord) -> Result<(), Error> {
    let damages = f.events.iter().filter(|e| e.kind == "DAMAGE_IN").count();
    let health = f
        .events
        .iter()
        .filter(|e| e.kind == "HEALTH" || e.kind == "JUMP")
        .count();
    if f.events.len() > parse::MAX_TIMELINE_EVENTS || damages.saturating_mul(health) > 25_000_000 {
        return Err(Error::new(
            "pulsar_analysis_limit",
            "This timeline exceeds the bounded analysis budget; its stored summary and original recording remain available.",
        ));
    }
    Ok(())
}
pub(crate) fn selected_record(
    paths: &ManagedPaths,
    request: &DetailRequest,
) -> Result<(Entry, FightRecord, Option<String>), Error> {
    let index = store::read_index(paths)?;
    let entry = store::entry(&index, &request.key)?.clone();
    let parent = store::load_record(paths, &entry)?;
    if let Some(session) = &parent.session {
        let selected = if let Some(id) = &request.exchange_id {
            parse::canonical_id(id)?;
            Some(
                session
                    .exchanges
                    .iter()
                    .find(|ex| {
                        &ex.id == id
                            && request
                                .participant
                                .as_ref()
                                .is_none_or(|p| ex.opponent.as_ref() == Some(p))
                    })
                    .ok_or_else(|| {
                        Error::new(
                            "pulsar_scope_not_found",
                            "The exchange does not belong to this participant/session.",
                        )
                    })?,
            )
        } else if let Some(participant) = &request.participant {
            Some(
                session
                    .exchanges
                    .iter()
                    .find(|ex| ex.opponent.as_ref() == Some(participant))
                    .ok_or_else(|| {
                        Error::new(
                            "pulsar_scope_not_found",
                            "No exchange belongs to this participant.",
                        )
                    })?,
            )
        } else {
            None
        };
        if let Some(ex) = selected {
            analysis_budget(ex)?;
            return Ok((entry, ex.clone(), Some(ex.id.clone())));
        }
    } else if request.participant.is_some() || request.exchange_id.is_some() {
        return Err(Error::new(
            "pulsar_scope_not_found",
            "This recording is not a manual session.",
        ));
    }
    analysis_budget(&parent)?;
    Ok((entry, parent, None))
}
pub fn detail(paths: &ManagedPaths, request: DetailRequest) -> Result<FightDetailView, Error> {
    detail_analysis(
        paths,
        DetailAnalysisRequest {
            detail: request,
            ..Default::default()
        },
    )
}
pub fn detail_analysis(
    paths: &ManagedPaths,
    filters: DetailAnalysisRequest,
) -> Result<FightDetailView, Error> {
    validate_filters(&filters.weapon, &filters.timing_result)?;
    let request = filters.detail;
    let (entry, record, selected) = selected_record(paths, &request)?;
    let analysis = analytics::analyze(&record);
    let parent = store::load_record(paths, &entry)?;
    let mut summary = summary(paths, &entry);
    summary.timeline_loaded = true;
    summary.metrics = metrics::values(&analysis.derived);
    summary
        .metrics
        .extend(super::cards::summary(&record, Some(&analysis)));
    if selected.is_some() {
        summary.start_epoch_millis = record.start_epoch_millis;
        summary.date_group = format::date_group(record.start_epoch_millis);
        summary.opponent =
            format::opponent(record.opponent_label.as_deref(), record.opponent.as_deref());
        summary.opponent_id = record.opponent.clone();
        summary.duration_nanos = record.duration_nanos;
        summary.resolution = analytics::resolution(&record);
        summary.result_label = format::result(&record);
        summary.outcome = record.outcome.clone().unwrap_or("UNKNOWN".into());
    }
    let mut tabs = projection::tabs(&record, &analysis, &filters.weapon, &filters.timing_result);
    tabs.0.tables.extend(super::cards::detail_tables(&record));
    let participants = parent
        .session
        .as_ref()
        .map(|s| {
            s.participants
                .iter()
                .map(|p| ParticipantView {
                    id: p.id.clone().unwrap_or_default(),
                    label: p.label.clone().unwrap_or("Unknown".into()),
                })
                .collect()
        })
        .unwrap_or_default();
    let exchanges = parent
        .session
        .as_ref()
        .map(|s| {
            s.exchanges
                .iter()
                .map(|ex| ExchangeScope {
                    id: ex.id.clone(),
                    participant_id: ex.opponent.clone(),
                    opponent: format::opponent(
                        ex.opponent_label.as_deref(),
                        ex.opponent.as_deref(),
                    ),
                    duration_nanos: ex.duration_nanos,
                    resolution: analytics::resolution(ex),
                })
                .collect()
        })
        .unwrap_or_default();
    let timeline = projection::events(&record, &analysis);
    let mut evidence = projection::table(
        "evidence",
        "Result evidence",
        &serde_json::to_value(&record.evidence().entries).unwrap(),
        None,
    );
    let evidence_base = record
        .evidence()
        .timeline_start_nanos
        .or_else(|| record.events.first().map(|e| e.nano_time));
    if !evidence.columns.contains(&"time".to_owned()) {
        evidence.columns.insert(0, "time".into());
    }
    for (i, row) in evidence.rows.iter_mut().enumerate() {
        let seconds = evidence_base
            .zip(record.evidence().entries.get(i))
            .map(|(base, e)| (e.nano_time - base) as f64 / 1e9);
        row.metrics.insert(
            "time".into(),
            metrics::extra(
                "time",
                "Recording time",
                "s",
                "Elapsed time from the frozen recording timeline origin.",
                seconds,
            ),
        );

        row.cells.insert(
            "Establishing".into(),
            (summary.resolution.evidence_index == Some(i as i32)).to_string(),
        );
    }
    Ok(FightDetailView {
        summary,
        coverage: Coverage {
            recordings: 1,
            timeline_recordings: usize::from(!record.events.is_empty()),
            context_recordings: usize::from(
                record.schema_version >= 3 && !record.events.is_empty(),
            ),
            measurement_recordings: usize::from(record.statistics.measurement_version > 0),
            ..Default::default()
        },
        participants,
        exchanges,
        selected_exchange: selected,
        termination: parent.session.as_ref().and_then(|s| s.termination.clone()),
        summary_tab: tabs.0,
        offense: tabs.1,
        defense: tabs.2,
        flow: tabs.3,
        evidence,
        timeline,
        aim_regions: super::cards::aim_regions(&analysis),
        derived: analysis.derived,
    })
}
pub fn playback(paths: &ManagedPaths, request: PlaybackRequest) -> Result<PlaybackView, Error> {
    if !request.playhead_seconds.is_finite() {
        return Err(Error::new(
            "pulsar_invalid_playhead",
            "Playback time must be finite.",
        ));
    }
    let (_, record, _) = selected_record(paths, &request.detail)?;
    let a = analytics::analyze(&record);
    projection::playback(
        &record,
        &a,
        request.playhead_seconds,
        request.event_filter.as_deref(),
    )
}

pub fn analyze_library(
    paths: &ManagedPaths,
    request: AnalysisRequest,
) -> Result<AnalysisView, Error> {
    validate_query(&request.query)?;
    validate_filters(&request.weapon, &request.timing_result)?;
    let index = store::read_index(paths)?;
    let mut q = request.query.clone();
    q.sort = HistorySort::Newest;
    let entries = filtered(&index, &q)?;
    let count = match request.window {
        AnalysisWindow::Last10 => 10,
        AnalysisWindow::Last25 => 25,
        AnalysisWindow::All => entries.len(),
    };
    let previous: Vec<_> = entries.iter().skip(count).take(count).copied().collect();
    let chosen: Vec<_> = entries.into_iter().take(count).collect();
    // Previous recordings use the same pooled definitions and bounded timeline budget.
    let mut previous_timelines = Vec::new();
    let (mut prior_bytes, mut prior_events) = (0, 0);
    for e in &previous {
        if e.event_count == 0
            || prior_bytes + e.bytes as usize > parse::MAX_TIMELINE_BYTES
            || prior_events + e.event_count > parse::MAX_TIMELINE_EVENTS
        {
            continue;
        }
        prior_bytes += e.bytes as usize;
        prior_events += e.event_count;
        if let Ok(record) = store::load_record(paths, e).and_then(|r| {
            analysis_budget(&r)?;
            Ok(r)
        }) {
            let a = analytics::analyze(&record);
            previous_timelines.push((e.key.clone(), record, a));
        }
    }
    let previous_period = (!previous.is_empty()).then(|| {
        projection::pool(
            &previous,
            &previous_timelines,
            &request.weapon,
            &request.timing_result,
        )
        .0
    });
    let mut coverage = Coverage {
        recordings: chosen.len(),
        measurement_recordings: chosen
            .iter()
            .filter(|e| e.record.statistics.measurement_version > 0)
            .count(),
        ..Default::default()
    };
    let (mut bytes, mut events) = (0, 0);
    let mut timelines = Vec::new();
    for e in &chosen {
        if e.event_count == 0
            || bytes + e.bytes as usize > parse::MAX_TIMELINE_BYTES
            || events + e.event_count > parse::MAX_TIMELINE_EVENTS
        {
            continue;
        }
        bytes += e.bytes as usize;
        events += e.event_count;
        match store::load_record(paths, e).and_then(|r| {
            analysis_budget(&r)?;
            Ok(r)
        }) {
            Ok(record) => {
                coverage.timeline_recordings += 1;
                coverage.context_recordings += usize::from(record.schema_version >= 3);
                let a = analytics::analyze(&record);
                timelines.push((e.key.clone(), record, a));
            }
            Err(error) => {
                if coverage.warnings.len() < 128 {
                    coverage.warnings.push(error.message);
                }
            }
        }
    }
    if coverage.timeline_recordings < coverage.recordings {
        coverage.warnings.push("Detailed distributions cover loaded timelines only; stored totals include every selected recording.".into());
    }
    if let Some((_, newest, _)) = timelines.first() {
        coverage.mixed_reach_policy = timelines.iter().any(|(_, f, _)| {
            f.policy().perfect_reach_target != newest.policy().perfect_reach_target
                || f.policy().perfect_reach_tolerance != newest.policy().perfect_reach_tolerance
        });
    }
    let pooled = projection::pool(&chosen, &timelines, &request.weapon, &request.timing_result);
    let mut overview = pooled.0;
    let offense = pooled.1;
    let defense = pooled.2;
    let mut flow = pooled.3;
    let focus = if let Some(focus) = &request.focus {
        timelines.iter().find(|(key, _, _)| key == focus)
    } else {
        timelines.first()
    };
    let pooled_combo_chart = flow
        .charts
        .iter()
        .find(|c| c.id == "pooled_combo_distribution")
        .cloned();
    if let Some((_, record, a)) = focus {
        flow = projection::tabs(record, a, &request.weapon, &request.timing_result).3;
        flow.metrics
            .extend(projection::pooled_flow_metrics(&chosen, &timelines));
    }
    if let Some(chart) = pooled_combo_chart {
        flow.charts.push(chart);
    }
    if let Some((_, _, latest)) = timelines.first() {
        overview.interpretations = latest.derived["interpretations"]
            .as_array()
            .unwrap()
            .iter()
            .skip(1)
            .take(4)
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
    }
    let trend_window = request
        .query
        .instance_id
        .as_ref()
        .and_then(|id| settings::inspect(paths, id).ok())
        .and_then(|v| v.settings.into_iter().find(|s| s.key == "trendWindow"))
        .and_then(|s| s.value.as_u64())
        .unwrap_or(20)
        .clamp(5, 100) as usize;
    overview.charts.extend(projection::trends(
        &chosen
            .iter()
            .take(trend_window)
            .copied()
            .collect::<Vec<_>>(),
    ));
    let trend = projection::trends(&chosen);
    let mut offense = offense;
    offense.charts.extend(trend.into_iter().filter(|c| {
        matches!(
            c.id.as_str(),
            "trend_accuracy" | "trend_criticalRate" | "trend_sprintRate"
        )
    }));
    for tab in [&mut overview, &mut offense, &mut flow] {
        tab.notices.extend(coverage.warnings.clone());
    }
    if coverage.mixed_reach_policy {
        overview.notices.push("Mixed frozen reach policies; pooled target-window share uses each recording's own policy.".into());
        offense.notices.push("Mixed frozen reach policies; filtered distribution uses the newest loaded target/tolerance.".into());
    }
    let latest = chosen.first().map(|e| {
        let mut view = summary(paths, e);
        if let Some((_, f, a)) = timelines.iter().find(|(key, _, _)| key == &e.key) {
            view.metrics.extend(super::cards::summary(f, Some(a)));
        }
        view
    });
    Ok(AnalysisView {
        previous_period,
        previous_recordings: previous.len(),
        coverage,
        latest,
        focus: focus.map(|(key, _, _)| key.clone()),
        overview,
        offense,
        defense,
        flow,
    })
}
pub fn inspect_settings(
    paths: &ManagedPaths,
    request: FolderRequest,
) -> Result<SettingsView, Error> {
    settings::inspect(paths, &request.instance_id)
}
pub fn open_folder(paths: &ManagedPaths, request: FolderRequest) -> Result<ActionReceipt, Error> {
    reports::open_folder(paths, request)
}
pub fn view_report(paths: &ManagedPaths, key: FightKey) -> Result<ActionReceipt, Error> {
    reports::view(paths, &key)
}
pub fn export_report(paths: &ManagedPaths, request: ExportRequest) -> Result<ActionReceipt, Error> {
    reports::export(paths, &request)
}
pub fn delete_fight(paths: &ManagedPaths, request: DeleteRequest) -> Result<ActionReceipt, Error> {
    store::delete(paths, &request)
}

pub fn opponents(paths: &ManagedPaths, q: LibraryQuery) -> Result<Vec<OpponentView>, Error> {
    let index = store::read_index(paths)?;
    let entries = filtered(&index, &q)?;
    let mut groups: BTreeMap<String, (OpponentView, f64, f64, f64, f64)> = BTreeMap::new();
    let mut add = |f: &FightRecord, entry: &Entry| {
        let key = &entry.key;
        let Some(id) = f.opponent.as_ref() else {
            return;
        };
        let group = groups.entry(id.clone()).or_insert_with(|| {
            (
                OpponentView {
                    id: id.clone(),
                    label: format::opponent(f.opponent_label.as_deref(), Some(id)),
                    ..Default::default()
                },
                0.0,
                0.0,
                0.0,
                0.0,
            )
        });
        if !group.0.fights.contains(key) {
            group.0.fights.push(key.clone());
        }
        match f.outcome.as_deref() {
            Some("WIN") => group.0.wins += 1,
            Some("LOSS") => group.0.losses += 1,
            Some("DRAW") => group.0.draws += 1,
            other => {
                group.0.unknown += 1;
                let label = match other {
                    Some("INTERRUPTED") => "Interrupted",
                    Some("UNKNOWN") => "Unknown",
                    None => "No result",
                    _ => "Unclassified result",
                };
                *group.0.other_results.entry(label.into()).or_default() += 1;
            }
        };
        let recorded_at = entry.record.start_epoch_millis;
        if group
            .0
            .latest
            .as_ref()
            .is_none_or(|old| recorded_at > old.start_epoch_millis)
        {
            group.0.latest = Some(summary(paths, entry));
        }
        group.0.last_fought_epoch_millis = group.0.last_fought_epoch_millis.max(recorded_at);
        group.1 += f.statistics.hits as f64;
        group.2 += (f.statistics.attempts - f.statistics.shield_blocks).max(0) as f64;
        group.3 += f.statistics.reach_sum;
        group.4 += f.statistics.reach_count as f64;
    };
    for e in entries {
        if let Some(session) = &e.record.session {
            for ex in &session.exchanges {
                add(ex, e);
            }
        } else {
            add(&e.record, e);
        }
    }
    let mut result: Vec<_> = groups
        .into_values()
        .map(|(mut view, hits, denom, reach, n)| {
            view.metrics.insert(
                "accuracy".into(),
                metrics::metric("accuracy", analytics::ratio(100.0 * hits, denom)),
            );
            view.metrics.insert(
                "averageReach".into(),
                metrics::metric("averageReach", analytics::ratio(reach, n)),
            );
            view
        })
        .collect();
    result.sort_by(|a, b| {
        b.last_fought_epoch_millis
            .cmp(&a.last_fought_epoch_millis)
            .then(a.id.cmp(&b.id))
    });
    result.truncate(1000);
    Ok(result)
}
pub fn records(paths: &ManagedPaths, q: LibraryQuery) -> Result<Vec<RecordView>, Error> {
    let index = store::read_index(paths)?;
    let entries = filtered(&index, &q)?;
    let mut result = Vec::new();
    for (id, label, key, minimum) in [
        ("longestCombo", "Longest combo", "longestCombo", None),
        ("bestAccuracy", "Best accuracy", "accuracy", Some(10)),
        ("mostHits", "Most confirmed hits", "hits", None),
        ("longestFight", "Longest recording", "duration", None),
        (
            "bestDominanceShare",
            "Best advantage share",
            "dominanceShare",
            None,
        ),
    ] {
        let (mut bytes, mut events) = (0usize, 0usize);
        let mut partial = false;
        let mut best: Option<(&Entry, f64)> = None;
        for e in &entries {
            if minimum.is_some_and(|min| {
                (e.record.statistics.attempts - e.record.statistics.shield_blocks) < min as i32
            }) {
                continue;
            }
            let value = match key {
                "accuracy" => analytics::summary(&e.record)["accuracy"].as_f64(),
                "duration" => Some(e.record.duration()),
                "hits" => Some(e.record.statistics.hits as f64),
                "longestCombo" => Some(e.record.statistics.longest_combo as f64),
                _ => {
                    if e.record.session.is_some() {
                        continue;
                    }
                    if bytes.saturating_add(e.bytes as usize) > parse::MAX_TIMELINE_BYTES
                        || events.saturating_add(e.event_count) > parse::MAX_TIMELINE_EVENTS
                    {
                        partial = true;
                        continue;
                    }
                    bytes += e.bytes as usize;
                    events += e.event_count;
                    let f = match store::load_record(paths, e).and_then(|f| {
                        analysis_budget(&f)?;
                        Ok(f)
                    }) {
                        Ok(f) => f,
                        Err(_) => {
                            partial = true;
                            continue;
                        }
                    };
                    let a = analytics::analyze(&f);
                    if a.dominance.availability != "AVAILABLE" {
                        continue;
                    }
                    analytics::ratio(100.0 * a.dominance.local, a.dominance.active)
                }
            };
            if let Some(value) = value {
                if best.is_none_or(|(old, v)| {
                    value > v
                        || (value == v
                            && e.record.start_epoch_millis > old.record.start_epoch_millis)
                }) {
                    best = Some((e, value));
                }
            }
        }
        let mut metric = if key == "duration" {
            metrics::extra(
                key,
                "Duration",
                "s",
                "Recorded duration; longer is descriptive.",
                best.map(|(_, v)| v),
            )
        } else {
            metrics::metric(key, best.map(|(_, v)| v))
        };
        if partial {
            metric.availability = Availability::Incomplete;
            metric.reason = Some("Best among readable timelines within the 64 MiB / 50,000 event budget; other recordings were skipped.".into());
        }
        result.push(RecordView {
            source: best.map(|(e, _)| summary(paths, e)),
            id: id.into(),
            label: label.into(),
            key: best.map(|(e, _)| e.key.clone()),
            metric,
            minimum_attempts: minimum,
        });
    }
    Ok(result)
}
pub fn compare(paths: &ManagedPaths, request: CompareRequest) -> Result<CompareView, Error> {
    let left = detail(paths, request.left)?;
    let right = detail(paths, request.right)?;
    let keys: BTreeSet<_> = left
        .summary
        .metrics
        .keys()
        .chain(right.summary.metrics.keys())
        .cloned()
        .collect();
    let metrics = keys
        .into_iter()
        .map(|key| {
            let l = left
                .summary
                .metrics
                .get(&key)
                .cloned()
                .unwrap_or_else(|| metrics::metric(&key, None));
            let r = right
                .summary
                .metrics
                .get(&key)
                .cloned()
                .unwrap_or_else(|| metrics::metric(&key, None));
            CompareMetric {
                key,
                left: l.clone(),
                right: r.clone(),
                delta: l.value.zip(r.value).map(|(l, r)| r - l),
            }
        })
        .collect();
    Ok(CompareView{left:left.summary,right:right.summary,metrics,notices:vec!["Differences are right minus left. Frozen policies and evidence coverage can differ; descriptive metrics are not skill scores.".into()]})
}
