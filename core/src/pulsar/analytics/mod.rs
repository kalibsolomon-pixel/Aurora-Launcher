//! Deterministic port of the frozen recording analytics. Reports are never read here.
mod combos;
mod dominance;
pub(crate) mod format;
mod incoming;
mod interpretation;
pub(crate) mod metrics;

use super::{view::ResolutionView, wire::*};
pub(crate) use combos::Combos;
pub(crate) use dominance::Dominance;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn ratio(n: f64, d: f64) -> Option<f64> {
    if d == 0.0 {
        None
    } else {
        let v = n / d;
        v.is_finite().then_some(v)
    }
}
pub(crate) fn mean(v: &[f64]) -> Option<f64> {
    ratio(v.iter().sum(), v.len() as f64)
}
pub(crate) fn sd(v: &[f64]) -> Option<f64> {
    mean(v).map(|m| (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64).sqrt())
}
pub(crate) fn num(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or(0.0)
}
pub(crate) fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
pub(crate) fn hit_evaluation(e: &Evaluation) -> bool {
    e.result.as_deref().is_none_or(|r| r == "HIT")
}
pub(crate) fn outgoing(e: &CombatEvent) -> bool {
    e.kind == "ATTACK" && e.actor.as_deref() == Some("local")
}
pub(crate) fn attributed(e: &CombatEvent, opponent: Option<&str>) -> bool {
    if e.kind != "DAMAGE_IN" || opponent.is_none() || e.actor.as_deref() != opponent {
        return false;
    }
    let Some(c) = &e.context else {
        return true;
    };
    let att = c.attribution.as_deref();
    if matches!(att, Some("UNATTRIBUTED" | "UNKNOWN_CAUSE")) {
        return false;
    }
    match c.damage_type.as_deref() {
        None => att.is_none_or(|a| matches!(a, "OPPONENT_DIRECT" | "OPPONENT_INDIRECT")),
        Some(t) => {
            att == Some("OPPONENT_DIRECT")
                && c.direct == Some(true)
                && matches!(t.rsplit(':').next(), Some("player_attack" | "mace_smash"))
        }
    }
}
pub(crate) fn strict_combos(f: &FightRecord, events: &[CombatEvent]) -> Combos {
    combos::analyze(f, events)
}
pub(crate) fn strict_jump_count(f: &FightRecord, events: &[CombatEvent]) -> usize {
    incoming::jumps(f, events)
        .0
        .iter()
        .filter(|j| j["candidate"] == true)
        .count()
}

pub(crate) fn item(m: Option<&Measurement>) -> &str {
    m.and_then(|m| m.weapon.as_deref())
        .map(|w| w.split(';').next().unwrap_or("unknown"))
        .unwrap_or("unknown")
}
pub(crate) fn comparable(a: &Attack) -> Option<f64> {
    if a.measurement.as_ref().and_then(|m| m.mode.as_deref()) == Some("MELEE_ATTEMPT_CREATIVE") {
        None
    } else {
        (a.reach >= 0.0 && a.reach.is_finite()).then_some(a.reach)
    }
}
pub(crate) fn face_horizontal(a: &Attack) -> f64 {
    if a.aim_x.min(1.0 - a.aim_x) < a.aim_z.min(1.0 - a.aim_z) {
        a.aim_z
    } else {
        a.aim_x
    }
}
pub(crate) fn face(a: &Attack) -> &'static str {
    let coords = [a.aim_x, a.aim_y, a.aim_z];
    let mut axis = 0;
    let mut best = 2.0;
    for (k, v) in coords.iter().enumerate() {
        let edge = v.min(1.0 - v);
        if edge < best {
            best = edge;
            axis = k;
        }
    }
    match (axis, coords[axis] > 0.5) {
        (0, true) => "EAST",
        (0, false) => "WEST",
        (1, true) => "TOP",
        (1, false) => "BOTTOM",
        (_, true) => "SOUTH",
        _ => "NORTH",
    }
}
pub(crate) fn aim_distance(a: &Attack) -> Option<f64> {
    let m = a.measurement.as_ref()?;
    let r = m.ideal_region.as_ref()?;
    let b = m.r#box.as_ref()?;
    let ideal = r.ideal.as_ref()?;
    Some(
        (((a.aim_x - ideal.x) * (b.max_x - b.min_x)).powi(2)
            + ((a.aim_y - ideal.y) * (b.max_y - b.min_y)).powi(2)
            + ((a.aim_z - ideal.z) * (b.max_z - b.min_z)).powi(2))
        .sqrt(),
    )
}
fn reach_bin(v: f64) -> usize {
    ((v / 0.5) as usize).min(12)
}
pub(crate) fn canonical_events(f: &FightRecord) -> Vec<CombatEvent> {
    if f.statistics.measurement_version == 0 {
        return f.events.clone();
    }
    let by_seq: BTreeMap<i64, &CombatEvent> = f.events.iter().map(|e| (e.sequence, e)).collect();
    f.events
        .iter()
        .filter_map(|e| {
            if !outgoing(e) {
                return Some(e.clone());
            }
            let id = *f.statistics.attack_canonical_ids.get(&e.sequence)?;
            if id != e.sequence {
                return None;
            }
            let mut canonical = e.clone();
            if let Some(sample) = f
                .statistics
                .attack_samples
                .get(&id)
                .and_then(|s| by_seq.get(s))
            {
                canonical.target = sample.target.clone();
                canonical.attack = sample.attack.clone();
                canonical.detail = sample.detail.clone();
                canonical.context = sample.context.clone();
            }
            if let (Some(m), Some(t)) = (
                canonical
                    .attack
                    .as_mut()
                    .and_then(|a| a.measurement.as_mut()),
                f.statistics.attack_timings.get(&id),
            ) {
                m.timing = Some(t.clone());
            }
            Some(canonical)
        })
        .collect()
}

pub(crate) fn resolution(f: &FightRecord) -> ResolutionView {
    let stored = f.resolution.as_ref();
    let outcome = f.outcome.as_deref().unwrap_or("UNKNOWN");
    let confirmed = f.evidence().confidence.as_deref() == Some("CONFIRMED")
        && matches!(outcome, "WIN" | "LOSS");
    let class = stored
        .and_then(|r| r.classification.as_deref())
        .unwrap_or(match outcome {
            "WIN" => {
                if confirmed {
                    "CONFIRMED_WIN"
                } else {
                    "INFERRED_WIN"
                }
            }
            "LOSS" => {
                if confirmed {
                    "CONFIRMED_LOSS"
                } else {
                    "INFERRED_LOSS"
                }
            }
            "DRAW" => "DRAW",
            "INTERRUPTED" => "INTERRUPTED",
            _ => "UNKNOWN",
        });
    let source = stored
        .and_then(|r| r.source.as_deref())
        .unwrap_or(if stored.is_some() {
            "EXTERNAL"
        } else {
            "LEGACY"
        });
    let confidence = stored
        .and_then(|r| r.confidence.as_deref())
        .unwrap_or(if stored.is_some() {
            "OBSERVED"
        } else if confirmed {
            "CONFIRMED"
        } else if matches!(outcome, "WIN" | "LOSS") {
            "INFERRED"
        } else {
            "OBSERVED"
        });
    ResolutionView {
        reason: (class == "NO_RESULT").then(|| f.reason.clone()).flatten(),
        classification: class.into(),
        label: match class {
            "CONFIRMED_WIN" => "Win",
            "CONFIRMED_LOSS" => "Loss",
            "INFERRED_WIN" => "Inferred win",
            "INFERRED_LOSS" => "Inferred loss",
            "DRAW" => "Draw",
            "INTERRUPTED" => "Interrupted",
            _ => "No result",
        }
        .into(),
        source: source_label(source).into(),
        confidence: confidence.into(),
        scope: stored
            .and_then(|r| r.scope.as_deref())
            .unwrap_or(if stored.is_none() && f.evidence().round_id.is_some() {
                "MATCH"
            } else {
                "ENGAGEMENT"
            })
            .into(),
        derived: stored.is_none_or(|r| r.derived),
        evidence_index: if let Some(r) = stored {
            r.evidence_index
        } else {
            f.evidence()
                .entries
                .iter()
                .rposition(|e| {
                    e.accepted && !matches!(e.kind.as_deref(), Some("START" | "ROUND_START"))
                })
                .map(|i| i as i32)
        },
        event_sequence: stored.and_then(|r| r.event_sequence),
        notes: stored.map(|r| r.notes.clone()).unwrap_or_else(|| {
            f.reason
                .as_ref()
                .map(|r| vec![format!("Stored reason: {r}")])
                .unwrap_or_default()
        }),
    }
}
fn source_label(source: &str) -> &str {
    match source {
        "SERVER_MATCH_RESULT" => "Server match result",
        "SERVER_ROUND_RESULT" => "Server round result",
        "SERVER_KILL_MESSAGE" => "Server kill message",
        "GENERIC_RESULT" => "Generic result text with endpoint",
        "LOCAL_DEATH" => "Local death observed",
        "OPPONENT_DEATH" => "Opponent death observed",
        "OPPONENT_REMOVED" => "Opponent left the client world",
        "THIRD_PARTY_KILL" => "Opponent's killer",
        "CONFLICT" => "Contradictory evidence",
        "INACTIVITY" => "Combat inactivity",
        "DISCONNECT" => "Disconnect or world change",
        "TARGET_CHANGE" => "New target",
        "ROUND_BOUNDARY" => "New round announced",
        "EVENT_LIMIT" => "Event limit",
        "CLIENT" => "Client stopped or reset",
        "MANUAL" => "Ended by you",
        "LEGACY" => "Older recording",
        _ => "External finalization",
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Analytics {
    pub events: Vec<CombatEvent>,
    pub derived: Value,
    pub combos: Combos,
    pub dominance: Dominance,
}
pub(crate) fn analyze(original: &FightRecord) -> Analytics {
    let events = canonical_events(original);
    let adjusted = if original.evidence().timeline_start_nanos.is_none() {
        let mut adjusted = original.clone();
        adjusted
            .match_evidence
            .as_mut()
            .unwrap()
            .timeline_start_nanos = Some(events.first().map(|e| e.nano_time).unwrap_or(0));
        Some(adjusted)
    } else {
        None
    };
    let f = adjusted.as_ref().unwrap_or(original);
    let context = f.schema_version >= 3;
    let by_seq: BTreeMap<i64, &CombatEvent> = events.iter().map(|e| (e.sequence, e)).collect();
    let crit_ticks: BTreeSet<i64> = events
        .iter()
        .filter(|e| {
            e.kind == "CRIT"
                && e.context.as_ref().and_then(|c| c.subject.as_deref()) == Some("opponent")
        })
        .filter_map(|e| e.client_tick)
        .collect();
    let (mut hits, mut misses, mut shields, mut disables) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut hit_ids = BTreeSet::new();
    let mut outcomes = BTreeMap::new();
    let mut readiness = BTreeMap::from([("PERFECT", 0), ("GREAT", 0), ("NORMAL", 0), ("POOR", 0)]);
    let (mut hit_bins, mut miss_bins, mut cooldown_bins) = (vec![0; 13], vec![0; 13], vec![0; 10]);
    let (mut center, mut edge, mut high, mut low, mut blocks, mut disable_count, mut server_crits) =
        (0, 0, 0, 0, 0, 0, 0);
    for ev in &f.evaluations {
        let Some(event) = by_seq.get(&ev.attack_sequence) else {
            continue;
        };
        let Some(a) = event.attack.as_ref() else {
            continue;
        };
        let evidence = by_seq.get(&ev.evidence_sequence);
        let evidence_t = f.seconds(evidence.map(|e| e.nano_time).unwrap_or(event.nano_time));
        let weapon = item(a.measurement.as_ref());
        if matches!(ev.result.as_deref(), Some("BLOCKED" | "DISABLED")) {
            let disable = ev.result.as_deref() == Some("DISABLED");
            blocks += 1;
            if disable {
                disable_count += 1;
                disables.push(evidence_t);
            }
            outcomes.insert(ev.attack_sequence, ev.result.clone().unwrap());
            shields.push(json!({"time":evidence_t,"type":if disable{"DISABLE"}else{"BLOCK"},"subject":"opponent","attackSequence":ev.attack_sequence,"weapon":weapon}));
            continue;
        }
        if !hit_evaluation(ev) {
            continue;
        }
        hit_ids.insert(ev.attack_sequence);
        outcomes.insert(ev.attack_sequence, "HIT".into());
        let tick = evidence.map(|e| e.client_tick).unwrap_or(event.client_tick);
        let crit = if context {
            Some(tick.is_some_and(|t| {
                (-1..=1).any(|delta| {
                    t.checked_add(delta)
                        .is_some_and(|v| crit_ticks.contains(&v))
                })
            }))
        } else {
            None
        };
        if crit == Some(true) {
            server_crits += 1;
        }
        let reach = comparable(a);
        let perfect = a.ray_contact
            && reach.is_some_and(|r| {
                (r - f.policy().perfect_reach_target).abs()
                    <= f.policy().perfect_reach_tolerance + 1e-9
            });
        hits.push(json!({"seq":ev.attack_sequence,"t":f.seconds(event.nano_time),"evidenceT":evidence_t,"reach":reach,"contact":a.ray_contact,"aimH":a.ray_contact.then(||face_horizontal(a)),"aimV":a.ray_contact.then_some(a.aim_y),"cooldown":a.cooldown,"readiness":ev.readiness,"sprinting":a.sprinting,"critical":a.critical_conditions,"combo":ev.combo,"momentum":ev.momentum,"measurement":a.measurement,"perfectReach":(f.schema_version>=2).then_some(perfect),"weapon":weapon,"serverCrit":crit,"face":a.ray_contact.then(||face(a)),"aimDistance":if a.ray_contact{aim_distance(a)}else{None},"tick":tick}));
        if let Some(n) = ev.readiness.as_deref().and_then(|g| readiness.get_mut(g)) {
            *n += 1;
        }
        if let Some(r) = reach {
            hit_bins[reach_bin(r)] += 1;
        }
        cooldown_bins[((a.cooldown * 10.0) as i32).clamp(0, 9) as usize] += 1;
        if a.ray_contact {
            if a.aim_y > 0.75 {
                high += 1;
            } else if a.aim_y < 0.25 {
                low += 1;
            } else if (face_horizontal(a) - 0.5).abs() > 0.35 {
                edge += 1;
            } else {
                center += 1;
            }
        }
    }
    let intervals: Vec<f64> = hits
        .windows(2)
        .map(|pair| num(&pair[1], "t") - num(&pair[0], "t"))
        .collect();
    let combos = combos::analyze(f, &events);
    let incoming_times: Vec<f64> = events
        .iter()
        .filter(|e| attributed(e, f.opponent.as_deref()))
        .map(|e| f.seconds(e.nano_time))
        .collect();
    let (mut into_shield, mut bypassed) = (0, 0);
    let mut shield_ids: BTreeSet<i64> = shields
        .iter()
        .filter_map(|v| v["attackSequence"].as_i64())
        .collect();
    for e in events.iter().filter(|e| outgoing(e)) {
        let t = f.seconds(e.nano_time);
        let class = f
            .statistics
            .attack_classifications
            .get(&e.sequence)
            .map(String::as_str);
        if let Some(c) = class {
            outcomes.insert(e.sequence, c.into());
        }
        let raised = class != Some("EXCLUDED")
            && e.context.as_ref().and_then(|c| c.target_blocking) == Some(true);
        if raised {
            into_shield += 1;
        }
        if hit_ids.contains(&e.sequence) {
            if raised {
                bypassed += 1;
            }
            continue;
        }
        if class == Some("HIT")
            || class.is_none()
                && f.statistics.attack_classifications.is_empty()
                && f.statistics.hits > 0
                && f.evaluations.is_empty()
        {
            continue;
        }
        if matches!(class, Some("BLOCKED" | "DISABLED")) {
            if shield_ids.insert(e.sequence) {
                blocks += 1;
                let disable = class == Some("DISABLED");
                if disable {
                    disable_count += 1;
                    disables.push(t);
                }
                shields.push(json!({"time":t,"type":if disable{"DISABLE"}else{"BLOCK"},"subject":"opponent","attackSequence":e.sequence,"weapon":item(e.attack.as_ref().and_then(|a|a.measurement.as_ref()))}));
            }
            continue;
        }
        if class == Some("EXCLUDED") || class.is_none() && outcomes.contains_key(&e.sequence) {
            continue;
        }
        if class.is_none() {
            outcomes.insert(e.sequence, "MISS".into());
        }
        let empty = e.attack.is_none()
            || e.detail
                .as_deref()
                .is_some_and(|d| d.starts_with("empty-swing"));
        let a = e.attack.as_ref();
        let reach = a.and_then(comparable);
        let contact = a.is_some_and(|a| a.ray_contact);
        misses.push(json!({"seq":e.sequence,"t":t,"empty":empty,"estimatedReach":reach,"contact":contact,"aimH":if contact{a.map(face_horizontal)}else{None},"aimV":if contact{a.map(|a|a.aim_y)}else{None}}));
        if let Some(r) = reach {
            miss_bins[reach_bin(r)] += 1;
        }
    }
    let (mut local_blocks, mut local_disabled, mut local_not_blocked) = (0, 0, 0);
    for d in super::strict::defense(
        &events,
        f.opponent.as_deref(),
        f.policy().correlation_millis,
    ) {
        let kind = match d.result {
            "BLOCKED" => {
                local_blocks += 1;
                "BLOCK"
            }
            "DISABLED" => {
                local_disabled += 1;
                "DISABLE"
            }
            _ => {
                local_not_blocked += 1;
                "NOT_BLOCKED"
            }
        };
        shields.push(json!({"time":f.seconds(d.time),"type":kind,"subject":"local","attackSequence":d.sequence,"weapon":null}));
    }
    if local_blocks + local_disabled + local_not_blocked == 0 {
        local_blocks = f.statistics.local_shield_blocks;
        local_disabled = f.statistics.local_shield_disabled;
        local_not_blocked = f.statistics.local_shield_not_blocked;
    }
    if f.evaluations.is_empty() && f.statistics.attack_classifications.is_empty() {
        blocks = f.statistics.shield_blocks;
        disable_count = f.statistics.shield_disables;
    }
    shields.sort_by(|a, b| num(a, "time").total_cmp(&num(b, "time")));
    let after_disable = hits
        .iter()
        .filter(|h| {
            disables
                .iter()
                .any(|d| num(h, "evidenceT") > *d && num(h, "evidenceT") - d <= 5.0)
        })
        .count();
    let shield_payload = json!({"available":context,"attacksIntoShield":into_shield,"confirmedBlocks":blocks,"disables":disable_count,"hitsAfterDisable":after_disable,"bypassed":bypassed,"localBlocks":local_blocks,"localDisabled":local_disabled,"localNotBlocked":local_not_blocked,"events":shields});
    let incoming = incoming::build(f, &events);
    let mut damage_causes: BTreeMap<String, usize> = BTreeMap::new();
    for row in &incoming.rows {
        *damage_causes.entry(text(row, "cause").into()).or_default() += 1;
    }
    if incoming.rows.is_empty() && f.statistics.non_melee_damage > 0 {
        damage_causes.insert(
            "Unknown cause".into(),
            f.statistics.non_melee_damage as usize,
        );
    }
    let dominance = dominance::analyze(f, &combos, &shields);
    let eligible: Vec<&CombatEvent> = events
        .iter()
        .filter(|e| {
            e.kind != "ATTACK"
                || (outgoing(e)
                    && f.statistics
                        .attack_classifications
                        .get(&e.sequence)
                        .is_none_or(|c| c != "EXCLUDED"))
        })
        .collect();
    let timings: Vec<Value> = events
        .iter()
        .filter(|e| {
            outgoing(e)
                && f.statistics
                    .attack_classifications
                    .get(&e.sequence)
                    .is_none_or(|c| c != "EXCLUDED")
        })
        .filter_map(|e| {
            e.attack.as_ref().and_then(|a| a.measurement.as_ref()).map(
                |m| json!({"sequence":e.sequence,"time":f.seconds(e.nano_time),"measurement":m}),
            )
        })
        .collect();
    let timing_samples:Vec<Value>=eligible.iter().filter(|e|e.kind=="ATTACK").filter_map(|e|{let m=e.attack.as_ref()?.measurement.as_ref()?;let timing=m.timing.as_ref()?;let opener=timing.grade.as_deref()==Some("OPENER");Some(json!({"seq":e.sequence,"t":f.seconds(e.nano_time),"weapon":item(Some(m)),"attackSpeed":m.attack_speed,"expected":(!opener).then_some(timing.expected_ticks),"interval":(!opener).then_some(timing.interval_ticks),"delta":(!opener).then_some(timing.interval_ticks-timing.expected_ticks),"grade":timing.grade,"outcome":outcomes.get(&e.sequence)}))}).collect();
    let reach = reach_summary(
        &hits,
        f.policy().perfect_reach_target,
        f.policy().perfect_reach_tolerance,
    );
    let aim = aim_summary(f, &eligible, &hit_ids);
    let (jumps, jumps_without) = incoming::jumps(f, &events);
    let active = active_exchanges(f, &events, &hit_ids, &combos, &dominance);
    let active_seconds: f64 = f
        .statistics
        .combat_exchanges
        .iter()
        .map(|e| (f.seconds(e.end_nanos) - f.seconds(e.start_nanos)).max(0.0))
        .sum();
    let availability = if f.session.is_some() {
        "Unavailable"
    } else {
        "Observed"
    };
    let mut summary = summary(f);
    let average = ratio(
        f.statistics.combo_hits as f64,
        f.statistics.combo_count as f64,
    );
    let changes = json!({"averageCombo":average,"averageLocalCombo":average,"attributedIncoming":f.statistics.incoming,"unexplainedHealthDrops":incoming.unexplained.len(),"serverCrits":context.then_some(server_crits),"opponentComboAvailability":availability,"combatActiveSeconds":(f.statistics.measurement_version>0).then_some(active_seconds)});
    summary
        .as_object_mut()
        .unwrap()
        .extend(changes.as_object().unwrap().clone());
    let boundaries: Value = if let Some(session) = &f.session {
        serde_json::to_value(&session.announcements).unwrap()
    } else {
        json!(
            f.evidence()
                .entries
                .iter()
                .map(|e| json!({"time":f.seconds(e.nano_time),"kind":e.kind,"accepted":e.accepted}))
                .collect::<Vec<_>>()
        )
    };
    let interpretations = interpretation::of(
        f,
        &hits,
        &misses,
        &combos,
        &dominance,
        &reach,
        &shield_payload,
        &incoming.rows,
        &timing_samples,
        availability,
    );
    if f.session.is_some() {
        let s = &f.statistics;
        for (k, v) in [
            ("comboCount", s.combo_count),
            ("comboHits", s.combo_hits),
            ("longestOpponentCombo", s.longest_opponent_combo),
            ("longestUnanswered", s.longest_unanswered),
            ("attributedIncoming", s.incoming),
        ] {
            summary[k] = v.into();
        }
        summary["opponentComboAvailability"] = "Unavailable".into();
    }
    let derived = json!({"summary":summary,"hits":hits,"misses":misses,"readiness":readiness,"timings":timings,"reachBinSize":0.5,"hitReachBins":hit_bins,"missReachBins":miss_bins,"cooldownBins":cooldown_bins,"intervals":intervals,"comboLengths":combos.streaks.iter().filter(|s|s.local).map(|s|s.seqs.len()).collect::<Vec<_>>(),"placement":{"center":center,"edge":edge,"high":high,"low":low},"momentum":momentum(f,&hits,&incoming_times),"incoming":incoming_times,"jumps":jumps,"jumpsWithoutDamage":jumps_without,"motionSamples":events.iter().filter(|e|e.kind=="MOTION").count(),"sprintTransitions":events.iter().filter(|e|e.kind=="SPRINT").count(),"schema":f.schema_version,"resolution":resolution(f),"interpretations":interpretations,"combos":combos.payload(original),"combatExchanges":active,"exchangeTimeoutSeconds":f.statistics.exchange_timeout_seconds,"dominance":dominance.payload(),"shields":shield_payload,"incomingDetail":incoming.rows,"damageCauses":damage_causes,"timingSamples":timing_samples,"attemptOutcomes":outcomes.iter().map(|(k,v)|(k.to_string(),v)).collect::<BTreeMap<_,_>>(),"reach":reach,"aim":aim,"jumpEffect":incoming::jump_effect(&incoming.rows),"jumpWindowTicks":f.policy().jump_window_ticks,"boundaries":boundaries});
    Analytics {
        events,
        derived,
        combos,
        dominance,
    }
}

pub(crate) fn summary(f: &FightRecord) -> Value {
    let s = &f.statistics;
    let measurement = s.measurement_version > 0;
    let interval = ratio(s.interval_sum, s.interval_count as f64);
    let mut summary = json!({"attempts":s.attempts,"measurementVersion":s.measurement_version,"totalInputs":measurement.then_some(s.total_inputs),"uniqueAttacks":measurement.then_some(s.unique_attacks),"excludedSwings":measurement.then_some(s.excluded_swings),"deduplicatedAttacks":measurement.then_some(s.deduplicated_attacks),"eligibleMisses":measurement.then_some(s.eligible_misses),"combatActiveSeconds":null,"hits":s.hits,"emptyMisses":s.empty_misses,"misses":s.empty_misses,"blocked":s.shield_blocks-s.shield_disables,"disabled":s.shield_disables,"blockedAttacks":s.shield_blocks,"nonMeleeDamage":s.non_melee_damage,"localShieldNotBlocked":s.local_shield_not_blocked,"contacts":s.contacts,"accuracy":ratio(100.0*s.hits as f64,(s.hits+s.empty_misses)as f64),"averageReach":ratio(s.reach_sum,s.reach_count as f64),"maxReach":(s.reach_count>0).then_some(s.max_reach),"reachSamples":s.reach_count,"criticalConditions":s.critical_conditions,"criticalRate":ratio(100.0*s.critical_conditions as f64,s.hits as f64),"sprintHits":s.sprint_hits,"sprintRate":ratio(100.0*s.sprint_hits as f64,s.hits as f64)});
    summary.as_object_mut().unwrap().extend(json!({"averageInterval":interval,"intervalDeviation":interval.map(|m|(s.interval_squared/s.interval_count as f64-m*m).max(0.0).sqrt()),"jumpCandidates":s.jump_candidates,"perfectReachHits":(f.schema_version>=2).then_some(s.perfect_reach_hits),"incoming":s.incoming,"longestCombo":s.longest_combo,"averageCombo":null,"averageLocalCombo":null,"duration":f.duration(),"accuracyDenominator":(s.hits+s.empty_misses),"serverCrits":(f.schema_version>=3).then_some(s.server_crits),"damageTaken":(f.schema_version>=3).then_some(s.damage_taken),"comboCount":s.combo_count,"comboHits":s.combo_hits,"longestOpponentCombo":s.longest_opponent_combo,"longestUnanswered":s.longest_unanswered,"attributedIncoming":s.incoming,"unexplainedHealthDrops":0,"opponentComboAvailability":"Unavailable","contextAvailable":f.schema_version>=3}).as_object().unwrap().clone());
    summary
}

pub(crate) fn reach_summary(hits: &[Value], target: f64, tolerance: f64) -> Value {
    let mut values = Vec::new();
    let mut weapons = Vec::new();
    let mut by: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for h in hits {
        if h["contact"] != true {
            continue;
        }
        let Some(v) = h["reach"].as_f64().filter(|v| *v >= 0.0 && v.is_finite()) else {
            continue;
        };
        let weapon = text(h, "weapon");
        values.push(v);
        weapons.push(weapon.to_owned());
        by.entry(weapon.into()).or_default().push(v);
    }
    let within = |v: &[f64]| {
        v.iter()
            .filter(|v| (**v - target).abs() <= tolerance + 1e-9)
            .count()
    };
    let by_weapon:BTreeMap<_,_>=by.iter().map(|(k,v)|(k.clone(),json!({"eligible":v.len(),"within":within(v),"mean":mean(v),"deviation":sd(v),"max":v.iter().copied().reduce(f64::max)}))).collect();
    json!({"target":target,"tolerance":tolerance,"eligible":values.len(),"within":within(&values),"withinPercent":ratio(100.0*within(&values)as f64,values.len()as f64),"mean":mean(&values),"deviation":sd(&values),"max":values.iter().copied().reduce(f64::max),"meanAbsoluteError":ratio(values.iter().map(|v|(v-target).abs()).sum(),values.len()as f64),"values":values,"weapons":weapons,"byWeapon":by_weapon})
}
fn aim_summary(f: &FightRecord, events: &[&CombatEvent], hit_ids: &BTreeSet<i64>) -> Value {
    let (mut samples, mut distances) = (Vec::new(), Vec::new());
    let mut grades = [0; 4];
    let (mut by_weapon, mut by_reach): (BTreeMap<String, [i32; 4]>, BTreeMap<String, [i32; 4]>) =
        (BTreeMap::new(), BTreeMap::new());
    for e in events {
        if e.kind != "ATTACK" {
            continue;
        }
        let Some(a) = e.attack.as_ref().filter(|a| a.ray_contact) else {
            continue;
        };
        let m = a.measurement.as_ref();
        let hit = hit_ids.contains(&e.sequence);
        let grade = m
            .and_then(|m| m.aim_quality.as_deref())
            .unwrap_or("UNAVAILABLE");
        let distance = aim_distance(a);
        let weapon = item(m);
        let band = match comparable(a) {
            None => "unknown",
            Some(r) if r < 2.5 => "< 2.5 m",
            Some(r) if r < 3.0 => "2.5-3.0 m",
            _ => ">= 3.0 m",
        };
        samples.push(json!({"seq":e.sequence,"t":f.seconds(e.nano_time),"weapon":weapon,"face":face(a),"x":a.aim_x,"y":a.aim_y,"z":a.aim_z,"grade":grade,"distance":distance,"band":band,"hit":hit}));
        if !hit {
            continue;
        }
        if let Some(g) = ["IDEAL", "GOOD", "PERIPHERAL", "POOR"]
            .iter()
            .position(|g| *g == grade)
        {
            grades[g] += 1;
            by_weapon.entry(weapon.into()).or_default()[g] += 1;
            by_reach.entry(band.into()).or_default()[g] += 1;
            if let Some(distance) = distance {
                distances.push(distance);
            }
        }
    }
    json!({"samples":samples,"grades":grades,"meanDistance":mean(&distances),"distanceDeviation":sd(&distances),"byWeapon":by_weapon,"byReach":by_reach})
}
fn momentum(f: &FightRecord, hits: &[Value], incoming: &[f64]) -> Vec<[f64; 2]> {
    let mut changes: Vec<(f64, bool, bool)> = hits
        .iter()
        .map(|h| (num(h, "evidenceT"), true, false))
        .chain(incoming.iter().map(|t| (*t, false, false)))
        .collect();
    for ex in &f.statistics.combat_exchanges {
        changes.push((f.seconds(ex.start_nanos), false, true));
        changes.push((f.seconds(ex.end_nanos), false, true));
    }
    changes.sort_by(|a, b| a.0.total_cmp(&b.0).then((!a.2).cmp(&!b.2)));
    let (mut value, mut at) = (0.0_f64, 0.0_f64);
    let mut points = Vec::new();
    for (t, hit, reset) in changes {
        let decayed = value * (-(t - at).max(0.0) / 3.0).exp();
        value = if reset {
            0.0
        } else if hit {
            (decayed + 0.2).min(1.0)
        } else {
            (decayed - 0.25).max(-1.0)
        };
        at = t;
        points.push((at, value));
    }
    (0..200)
        .map(|i| {
            let t = f.duration() * i as f64 / 199.0;
            let (at, value) = points
                .iter()
                .rev()
                .find(|p| p.0 <= t)
                .copied()
                .unwrap_or((0.0, 0.0));
            [t, value * (-(t - at).max(0.0) / 3.0).exp()]
        })
        .collect()
}
fn active_exchanges(
    f: &FightRecord,
    events: &[CombatEvent],
    hits: &BTreeSet<i64>,
    combos: &Combos,
    dominance: &Dominance,
) -> Vec<Value> {
    let starts: BTreeSet<i64> = combos
        .streaks
        .iter()
        .filter(|s| s.local && s.seqs.len() >= 2)
        .map(|s| s.seqs[0])
        .collect();
    let (mut event_index, mut turning_index) = (0, 0);
    let mut result = Vec::new();
    for (i, interval) in f.statistics.combat_exchanges.iter().enumerate() {
        let include_end = f
            .statistics
            .combat_exchanges
            .get(i + 1)
            .is_none_or(|n| n.start_nanos > interval.end_nanos);
        let (mut hit, mut miss, mut opp, mut combo, mut changes) = (0, 0, 0, 0, 0);
        while event_index < events.len() && events[event_index].nano_time < interval.start_nanos {
            event_index += 1;
        }
        while event_index < events.len()
            && (events[event_index].nano_time < interval.end_nanos
                || (include_end && events[event_index].nano_time == interval.end_nanos))
        {
            let e = &events[event_index];
            event_index += 1;
            if hits.contains(&e.sequence) {
                hit += 1;
            }
            if f.statistics
                .attack_classifications
                .get(&e.sequence)
                .is_some_and(|c| c == "MISS")
            {
                miss += 1;
            }
            if attributed(e, f.opponent.as_deref()) {
                opp += 1;
            }
            if starts.contains(&e.sequence) {
                combo += 1;
            }
        }
        let (from, to) = (
            f.seconds(interval.start_nanos),
            f.seconds(interval.end_nanos),
        );
        while turning_index < dominance.turning.len() && dominance.turning[turning_index][0] < from
        {
            turning_index += 1;
        }
        while turning_index < dominance.turning.len() && dominance.turning[turning_index][0] <= to {
            changes += 1;
            turning_index += 1;
        }
        result.push(json!({"start":from,"end":to,"hits":hit,"misses":miss,"opponentHits":opp,"combos":combo,"momentumChanges":changes,"endedBy":interval.ended_by}));
    }
    result
}
