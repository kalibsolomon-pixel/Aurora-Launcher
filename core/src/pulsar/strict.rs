//! Read-time compatibility with client strict outcomes. Never writes source recordings.
use super::{analytics, wire::*};
use std::collections::{BTreeMap, VecDeque};

pub(crate) fn label(old: &str) -> &'static str {
    match old {
        "HIT" => "HIT",
        "BLOCKED" | "SHIELD_BLOCK" | "UNCERTAIN_SHIELD" | "ESTIMATED_BLOCK" => "BLOCKED",
        "DISABLED" | "SHIELD_DISABLE" => "DISABLED",
        "EXCLUDED" => "EXCLUDED",
        _ => "MISS",
    }
}

pub(crate) struct Defense {
    pub sequence: i64,
    pub time: i64,
    pub result: &'static str,
}
pub(crate) fn no_result_reason(source: &str) -> &'static str {
    match source {
        "THIRD_PARTY_KILL" => "Opponent killed by another player",
        "OPPONENT_REMOVED" => "Opponent left",
        "DISCONNECT" => "Disconnected",
        "MANUAL" | "CLIENT" | "EXTERNAL" => "Recording stopped",
        "TARGET_CHANGE" => "Target changed",
        "EVENT_LIMIT" => "Recording limit reached",
        "CONFLICT" => "Results disagreed",
        "ROUND_BOUNDARY" => "Next round started",
        _ => "Ended without a death",
    }
}

pub(crate) fn normalize(f: &mut FightRecord) {
    if let Some(session) = &mut f.session {
        for child in &mut session.exchanges {
            normalize(child);
        }
    }
    if f.schema_version < 3 && f.evaluations.is_empty() {
        let mut pending: VecDeque<&CombatEvent> = VecDeque::new();
        let window = f.policy().correlation_millis * 1e6;
        for e in &f.events {
            while pending
                .front()
                .is_some_and(|a| (e.nano_time as i128 - a.nano_time as i128) as f64 > window)
            {
                pending.pop_front();
            }
            if analytics::outgoing(e)
                && e.attack.is_some()
                && !e
                    .detail
                    .as_deref()
                    .is_some_and(|d| d.starts_with("empty-swing"))
            {
                pending.push_back(e);
            }
            if e.kind != "HURT"
                || e.confidence != "CONFIRMED"
                || e.actor.as_deref() != Some("local")
                || e.detail.as_deref() != Some("local-melee-source")
            {
                continue;
            }
            if let Some(k) = pending.iter().position(|a| a.target == e.target) {
                let a = pending.remove(k).unwrap();
                let attack = a.attack.as_ref().unwrap();
                let grade = if attack.cooldown >= f.policy().perfect_cooldown {
                    "PERFECT"
                } else if attack.cooldown >= f.policy().great_cooldown {
                    "GREAT"
                } else if attack.cooldown >= 0.5 {
                    "NORMAL"
                } else {
                    "POOR"
                };
                f.evaluations.push(Evaluation {
                    attack_sequence: a.sequence,
                    evidence_sequence: e.sequence,
                    readiness: Some(grade.into()),
                    confidence: Some("CONFIRMED".into()),
                    result: Some("HIT".into()),
                    ..Default::default()
                });
            }
        }
    }
    for e in &mut f.evaluations {
        e.result = Some(e.result.as_deref().map(label).unwrap_or("HIT").into());
    }
    let old_resolution = analytics::resolution(f);
    let class = old_resolution.classification.as_str();
    let unproven = matches!(class, "INFERRED_WIN" | "INFERRED_LOSS");
    let strict_class = if unproven
        || !matches!(class, "CONFIRMED_WIN" | "CONFIRMED_LOSS" | "DRAW")
        || class == "DRAW" && old_resolution.confidence != "CONFIRMED"
    {
        "NO_RESULT"
    } else {
        class
    };
    if strict_class == "NO_RESULT" {
        let source = f
            .resolution
            .as_ref()
            .and_then(|r| r.source.as_deref())
            .unwrap_or("LEGACY");
        f.reason = Some(if unproven {
            "Not proven by a death".into()
        } else if class == "NO_RESULT" && f.reason.is_some() {
            f.reason.clone().unwrap()
        } else {
            no_result_reason(source).into()
        });
    }
    let r = f.resolution.get_or_insert_with(|| Resolution {
        source: Some("LEGACY".into()),
        confidence: Some(old_resolution.confidence.clone()),
        scope: Some(old_resolution.scope.clone()),
        evidence_index: old_resolution.evidence_index,
        event_sequence: old_resolution.event_sequence,
        notes: old_resolution.notes.clone(),
        derived: true,
        ..Default::default()
    });
    r.classification = Some(strict_class.into());
    f.outcome = Some(
        match strict_class {
            "CONFIRMED_WIN" => "WIN",
            "CONFIRMED_LOSS" => "LOSS",
            "DRAW" => "DRAW",
            _ => "NO_RESULT",
        }
        .into(),
    );
    for round in &mut f.match_evidence.get_or_insert_with(Default::default).rounds {
        if round.confidence.as_deref() != Some("CONFIRMED")
            || !matches!(round.outcome.as_deref(), Some("WIN" | "LOSS" | "DRAW"))
        {
            let inferred = matches!(round.outcome.as_deref(), Some("WIN" | "LOSS"));
            round.outcome = Some("NO_RESULT".into());
            round.extra.entry("reason".into()).or_insert_with(|| {
                if inferred {
                    "Not proven by a death"
                } else if round.end_nanos.is_none() {
                    "Round still in progress"
                } else {
                    "Round ended without a result"
                }
                .into()
            });
        }
    }
    if super::replay::replayable(f) {
        super::replay::replay(f);
    }
    super::replay::pool(f);
    let defensive = super::replay::defense(
        &f.events,
        f.opponent.as_deref(),
        f.policy().correlation_millis,
        &super::replay::boundaries(f),
    );
    let defense = &defensive.outcomes;
    let s = &mut f.statistics;
    if s.strict_version == 0 {
        s.non_melee_damage = s
            .unattributed_incoming
            .saturating_add(s.third_party_incoming);
    }
    let mut mapped = !s.attack_classifications.is_empty();
    for v in s.attack_classifications.values_mut() {
        *v = label(v).into();
    }
    if !mapped
        && !f.events.is_empty()
        && s.strict_version == 0
        && (!f.evaluations.is_empty() || s.hits == 0)
    {
        let proof: BTreeMap<_, _> = f
            .evaluations
            .iter()
            .map(|e| (e.attack_sequence, e.result.clone().unwrap()))
            .collect();
        for e in f.events.iter().filter(|e| analytics::outgoing(e)) {
            let result = proof.get(&e.sequence).cloned().unwrap_or_else(|| {
                if e.target.is_none() {
                    "EXCLUDED"
                } else if e.context.as_ref().is_some_and(|c| {
                    c.target_blocking == Some(true) && c.target_facing == Some(true)
                }) {
                    "BLOCKED"
                } else {
                    "MISS"
                }
                .into()
            });
            s.attack_classifications.insert(e.sequence, result);
        }
        mapped = !s.attack_classifications.is_empty();
    }
    for ev in &f.evaluations {
        if s.attack_classifications
            .get(&ev.attack_sequence)
            .is_some_and(|c| c == "BLOCKED")
            && ev.result.as_deref() == Some("DISABLED")
        {
            s.attack_classifications
                .insert(ev.attack_sequence, "DISABLED".into());
        }
    }
    if mapped {
        s.attempts = 0;
        s.hits = 0;
        s.empty_misses = 0;
        s.eligible_misses = 0;
        s.shield_blocks = 0;
        s.shield_disables = 0;
        s.excluded_swings = 0;
        for c in s.attack_classifications.values() {
            if c == "EXCLUDED" {
                s.excluded_swings += 1;
                continue;
            }
            s.attempts += 1;
            match c.as_str() {
                "HIT" => s.hits += 1,
                "MISS" => {
                    s.empty_misses += 1;
                    s.eligible_misses += 1;
                }
                "BLOCKED" => s.shield_blocks += 1,
                "DISABLED" => {
                    s.shield_blocks += 1;
                    s.shield_disables += 1;
                }
                _ => unreachable!(),
            }
        }
    } else if s.strict_version == 0 {
        let e = s.estimated_shield_blocks.max(s.uncertain_shield);
        s.shield_blocks = s.shield_blocks.saturating_add(e).max(s.shield_disables);
        s.empty_misses = 0
            .max(s.attempts - s.hits - s.shield_blocks)
            .max(s.empty_misses.saturating_add((s.uncertain - e).max(0)));
        s.attempts = s
            .hits
            .saturating_add(s.empty_misses)
            .saturating_add(s.shield_blocks);
        s.eligible_misses = s.empty_misses;
    }
    s.uncertain = 0;
    s.uncertain_shield = 0;
    s.estimated_shield_blocks = 0;
    if f.events.iter().any(|e| e.kind == "DAMAGE_IN") {
        if f.opponent.is_some() {
            s.incoming = f
                .events
                .iter()
                .filter(|e| analytics::attributed(e, f.opponent.as_deref()))
                .count() as i32;
        }
        s.non_melee_damage = f.events.iter().filter(|e| e.kind == "DAMAGE_IN").count() as i32
            - s.incoming
            - if f.opponent.is_none() {
                s.stun_incoming_damage.len() as i32
            } else {
                0
            };
    }
    if !f.events.is_empty() && f.opponent.is_some() {
        s.not_counted_incoming_sequences = defensive.not_counted;
        s.not_counted_incoming_swings = s.not_counted_incoming_sequences.len() as i32;
        s.local_stuns = defensive.stuns;
        s.stun_incoming_damage = defensive.damage;
        s.stun_incoming_follow_ups = defensive.follow_ups;
    }
    s.incoming -= f
        .events
        .iter()
        .filter(|e| {
            analytics::attributed(e, f.opponent.as_deref())
                && s.stun_incoming_damage.contains(&e.sequence)
        })
        .count() as i32;
    if !defense.is_empty()
        || !s.not_counted_incoming_sequences.is_empty()
        || f.events.iter().any(|e| {
            e.kind == "SHIELD"
                && e.context.as_ref().and_then(|c| c.subject.as_deref()) == Some("local")
        })
    {
        s.local_shield_blocks = defense.iter().filter(|d| d.result == "BLOCKED").count() as i32;
        s.local_shield_disabled = defense.iter().filter(|d| d.result == "DISABLED").count() as i32;
        s.local_shield_not_blocked =
            defense.iter().filter(|d| d.result == "NOT_BLOCKED").count() as i32;
    }
    s.unattributed_incoming = 0;
    s.third_party_incoming = 0;
    s.strict_version = s.strict_version.max(1);
    f.evaluations.retain(|e| {
        s.attack_classifications
            .get(&e.attack_sequence)
            .is_none_or(|c| Some(c) == e.result.as_ref())
    });
    if !f.events.is_empty() && f.opponent.is_some() {
        let events = analytics::canonical_events(f);
        let combos = analytics::strict_combos(f, &events);
        if !f.evaluations.is_empty() || f.statistics.hits == 0 {
            f.statistics.longest_combo = combos.longest(true) as i32;
            f.statistics.combo_count = combos.count() as i32;
            f.statistics.combo_hits = combos.hits() as i32;
            f.statistics.longest_unanswered = combos.longest_unanswered as i32;
        }
        if f.events.iter().any(|e| e.kind == "DAMAGE_IN") {
            f.statistics.longest_opponent_combo = combos.longest(false) as i32;
        }
        f.statistics.jump_candidates = analytics::strict_jump_count(f, &events) as i32;
    }
}
