//! PvP combat breakdown mirroring Aurora Client's `CombatBreakdown.java`.
//! Computes exchange openers & trades, 1.9+ sword sprint-reset & cooldown discipline,
//! shield/axe disable punish & swap latency, health & absorption pacing, turning points,
//! and concise fact-backed takeaways from existing schema events without mutating the
//! frozen oracle `derived` payload.
use super::{Analytics, Combos, Dominance, ratio};
use crate::pulsar::{
    analytics::metrics,
    view::{ChartBand, ChartPoint, ChartSeries, ChartView, FightKey, TableView},
    wire::{CombatEvent, FightRecord},
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(crate) const TRADE_WINDOW_SECONDS: f64 = 0.10;
pub(crate) const SPRINT_RESET_MAX_GAP_SECONDS: f64 = 2.5;
pub(crate) const FULL_CHARGE_THRESHOLD: f64 = 0.90;
pub(crate) const AXE_SWAP_WINDOW_SECONDS: f64 = 3.0;
pub(crate) const BURST_WINDOW_SECONDS: f64 = 3.0;
pub(crate) const BURST_HEAL_THRESHOLD_HP: f64 = 2.0;
const DISABLE_FOLLOW_UP_SECONDS: f64 = 5.0;

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(crate) struct HealthPoint {
    pub time: f64,
    pub health: f64,
    pub absorption: f64,
    pub total: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(crate) struct HealthPacing {
    pub trace: Vec<HealthPoint>,
    pub start_hp: Option<f64>,
    pub min_hp: Option<f64>,
    pub min_hp_time: Option<f64>,
    pub end_hp: Option<f64>,
    pub total_damage_taken: f64,
    pub total_healing: f64,
    pub burst_heals: usize,
    pub absorption_gained: f64,
    pub damage_taken_per_sec: Option<f64>,
    pub hits_per_sec: Option<f64>,
    pub worst_burst_damage: f64,
    pub worst_burst_time: Option<f64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(crate) struct SwordMechanics {
    pub sprint_reset_opportunities: usize,
    pub sprint_resets: usize,
    pub sprint_reset_rate: Option<f64>,
    pub combo_sprint_reset_opportunities: usize,
    pub combo_sprint_resets: usize,
    pub combo_sprint_reset_rate: Option<f64>,
    pub full_charge_hits: usize,
    pub early_hits: usize,
    pub early_swing_rate: Option<f64>,
    pub mean_hit_cooldown: Option<f64>,
    pub crit_hits: usize,
    pub crit_rate: Option<f64>,
    pub crit_chains: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(crate) struct ShieldPunish {
    pub disables_inflicted: usize,
    pub disables_converted: usize,
    pub disable_conversion_rate: Option<f64>,
    pub mean_disable_punish_delay: Option<f64>,
    pub mean_punish_hits_per_disable: Option<f64>,
    pub axe_to_sword_swaps: usize,
    pub mean_axe_to_sword_seconds: Option<f64>,
    pub best_axe_to_sword_seconds: Option<f64>,
    pub local_disables: usize,
    pub hits_taken_after_local_disable: usize,
    pub damage_taken_after_local_disable: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(crate) struct ExchangeRow {
    pub index: usize,
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    pub opener: String,
    pub opener_converted: bool,
    pub local_hits: usize,
    pub opponent_hits: usize,
    pub hit_diff: i64,
    pub damage_taken: f64,
    pub healing_gained: f64,
    pub net_hp_delta: f64,
    pub dps_taken: Option<f64>,
    pub hits_per_sec: Option<f64>,
    pub crits: usize,
    pub sprint_resets: usize,
    pub opponent_disables: usize,
    pub local_disables: usize,
    pub winner: String,
    pub ended_by: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(crate) struct ExchangeSummary {
    pub total: usize,
    pub contested: usize,
    pub opened_by_you: usize,
    pub opened_by_opponent: usize,
    pub opened_by_trade: usize,
    pub first_hit_rate: Option<f64>,
    pub openers_converted: usize,
    pub first_hit_conversion_rate: Option<f64>,
    pub won: usize,
    pub lost: usize,
    pub even: usize,
    pub trade_ratio: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TurningPoint {
    pub time: f64,
    pub kind: String,
    pub tone: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Takeaway {
    pub tone: String,
    pub category: String,
    pub headline: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Breakdown {
    pub health: HealthPacing,
    pub sword: SwordMechanics,
    pub shields: ShieldPunish,
    pub exchange_summary: ExchangeSummary,
    pub exchanges: Vec<ExchangeRow>,
    pub turning_points: Vec<TurningPoint>,
    pub takeaways: Vec<Takeaway>,
    pub active_span: f64,
}

pub(crate) fn weapon_category(weapon: &str) -> &'static str {
    let base = weapon.split(';').next().unwrap_or("").to_ascii_lowercase();
    if base.contains("_axe") || base.ends_with(":axe") {
        "AXE"
    } else if base.contains("_sword") || base.ends_with(":sword") {
        "SWORD"
    } else if base.contains("mace") {
        "MACE"
    } else {
        "OTHER"
    }
}

pub(crate) fn analyze(
    f: &FightRecord,
    events: &[CombatEvent],
    derived: &Value,
    combos: &Combos,
    dominance: &Dominance,
) -> Breakdown {
    let mut trace = Vec::new();
    let mut sprint_stops = Vec::new();
    for e in events {
        if e.kind == "HEALTH" {
            if let Some(ctx) = &e.context {
                if let Some(hp) = ctx.health {
                    let abs = ctx.absorption.unwrap_or(0.0);
                    trace.push(HealthPoint {
                        time: f.seconds(e.nano_time),
                        health: hp,
                        absorption: abs,
                        total: hp + abs,
                    });
                }
            }
        } else if e.kind == "SPRINT" && e.detail.as_deref() == Some("false") {
            sprint_stops.push(f.seconds(e.nano_time));
        }
    }

    let hits = derived["hits"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let incoming = derived["incomingDetail"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let shield_events = derived["shields"]["events"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let context_available = derived["summary"]["contextAvailable"]
        .as_bool()
        .unwrap_or(false);
    let active_seconds = derived["summary"]["combatActiveSeconds"]
        .as_f64()
        .unwrap_or(0.0);
    let duration = f.duration();
    let active_span = if active_seconds > 0.0 {
        active_seconds
    } else {
        duration
    };

    let health = build_health(trace, active_span, hits.len());
    let (sword, reset_hit_indices) = build_sword(hits, &sprint_stops, context_available);
    let shields = build_shields(hits, incoming, shield_events);
    let (exchange_summary, exchanges) = build_exchanges(
        f,
        derived,
        hits,
        incoming,
        shield_events,
        &health.trace,
        &reset_hit_indices,
        context_available,
    );
    let turning_points = build_turning_points(
        f,
        derived,
        hits,
        shield_events,
        combos,
        dominance,
        &health,
        &shields,
    );
    let takeaways = build_takeaways(derived, &health, &sword, &shields, &exchange_summary);

    Breakdown {
        health,
        sword,
        shields,
        exchange_summary,
        exchanges,
        turning_points,
        takeaways,
        active_span,
    }
}

fn is_crit(context_available: bool, h: &Value) -> bool {
    if context_available {
        h["serverCrit"].as_bool().unwrap_or(false)
    } else {
        h["critical"].as_bool().unwrap_or(false)
    }
}

fn build_health(trace: Vec<HealthPoint>, active_span: f64, hit_count: usize) -> HealthPacing {
    let mut start_hp = None;
    let mut end_hp = None;
    let mut min_hp: Option<f64> = None;
    let mut min_hp_time = None;
    let mut damage_taken = 0.0;
    let mut healing = 0.0;
    let mut absorption_gained = 0.0;
    let mut burst_heals = 0usize;
    let mut drops: Vec<(f64, f64)> = Vec::new();

    for (i, p) in trace.iter().enumerate() {
        if i == 0 {
            start_hp = Some(p.total);
            min_hp = Some(p.total);
            min_hp_time = Some(p.time);
        } else {
            let prev = &trace[i - 1];
            let delta = p.total - prev.total;
            if delta < -1e-6 {
                let lost = -delta;
                damage_taken += lost;
                drops.push((p.time, lost));
            } else if delta > 1e-6 {
                healing += delta;
                if delta >= BURST_HEAL_THRESHOLD_HP {
                    burst_heals += 1;
                }
            }
            if p.absorption > prev.absorption + 1e-6 {
                absorption_gained += p.absorption - prev.absorption;
            }
            if min_hp.is_none_or(|m| p.total < m - 1e-6) {
                min_hp = Some(p.total);
                min_hp_time = Some(p.time);
            }
        }
        end_hp = Some(p.total);
    }

    let mut worst_burst = 0.0;
    let mut worst_burst_time = None;
    let mut left = 0usize;
    let mut window_sum = 0.0;
    for right in 0..drops.len() {
        window_sum += drops[right].1;
        while left < right && drops[right].0 - drops[left].0 > BURST_WINDOW_SECONDS + 1e-9 {
            window_sum -= drops[left].1;
            left += 1;
        }
        if window_sum > worst_burst + 1e-6 {
            worst_burst = window_sum;
            worst_burst_time = Some(drops[left].0);
        }
    }

    let damage_taken_per_sec = if active_span > 0.0 && !trace.is_empty() {
        Some(damage_taken / active_span)
    } else {
        None
    };
    let hits_per_sec = if active_span > 0.0 && hit_count > 0 {
        Some(hit_count as f64 / active_span)
    } else {
        None
    };

    HealthPacing {
        trace,
        start_hp,
        min_hp,
        min_hp_time,
        end_hp,
        total_damage_taken: damage_taken,
        total_healing: healing,
        burst_heals,
        absorption_gained,
        damage_taken_per_sec,
        hits_per_sec,
        worst_burst_damage: worst_burst,
        worst_burst_time,
    }
}

fn build_sword(
    hits: &[Value],
    sprint_stops: &[f64],
    context_available: bool,
) -> (SwordMechanics, BTreeSet<usize>) {
    let (mut opp, mut resets, mut combo_opp, mut combo_resets) = (0, 0, 0, 0);
    let mut reset_indices = BTreeSet::new();
    let mut stop_ptr = 0usize;

    for i in 1..hits.len() {
        let prev = &hits[i - 1];
        let curr = &hits[i];
        let pt = prev["t"].as_f64().unwrap_or(0.0);
        let ct = curr["t"].as_f64().unwrap_or(0.0);
        let dt = ct - pt;
        if prev["sprinting"].as_bool().unwrap_or(false)
            && dt > 0.0
            && dt <= SPRINT_RESET_MAX_GAP_SECONDS
        {
            opp += 1;
            let in_combo = curr["combo"].as_i64().unwrap_or(0) >= 2;
            if in_combo {
                combo_opp += 1;
            }
            while stop_ptr < sprint_stops.len() && sprint_stops[stop_ptr] <= pt {
                stop_ptr += 1;
            }
            let stopped_between = stop_ptr < sprint_stops.len() && sprint_stops[stop_ptr] < ct;
            if curr["sprinting"].as_bool().unwrap_or(false) && stopped_between {
                resets += 1;
                reset_indices.insert(i);
                if in_combo {
                    combo_resets += 1;
                }
            }
        }
    }

    let (mut full_charge, mut early, mut crits, mut crit_chains) = (0, 0, 0, 0);
    let mut cd_sum = 0.0;
    let mut prev_crit = false;
    let mut prev_t: Option<f64> = None;

    for h in hits {
        if let Some(cd) = h["cooldown"].as_f64().filter(|v| v.is_finite()) {
            cd_sum += cd;
            if cd >= FULL_CHARGE_THRESHOLD {
                full_charge += 1;
            } else {
                early += 1;
            }
        }
        let crit = is_crit(context_available, h);
        let t = h["t"].as_f64();
        if crit {
            crits += 1;
            if prev_crit {
                if let (Some(pt), Some(ct)) = (prev_t, t) {
                    if ct - pt <= SPRINT_RESET_MAX_GAP_SECONDS {
                        crit_chains += 1;
                    }
                }
            }
        }
        prev_crit = crit;
        prev_t = t;
    }

    let total_cd = full_charge + early;
    let sword = SwordMechanics {
        sprint_reset_opportunities: opp,
        sprint_resets: resets,
        sprint_reset_rate: ratio(100.0 * resets as f64, opp as f64),
        combo_sprint_reset_opportunities: combo_opp,
        combo_sprint_resets: combo_resets,
        combo_sprint_reset_rate: ratio(100.0 * combo_resets as f64, combo_opp as f64),
        full_charge_hits: full_charge,
        early_hits: early,
        early_swing_rate: ratio(100.0 * early as f64, total_cd as f64),
        mean_hit_cooldown: ratio(cd_sum, total_cd as f64),
        crit_hits: crits,
        crit_rate: ratio(100.0 * crits as f64, hits.len() as f64),
        crit_chains,
    };
    (sword, reset_indices)
}

fn build_shields(hits: &[Value], incoming: &[Value], shield_events: &[Value]) -> ShieldPunish {
    let mut opp_disables = Vec::new();
    let mut local_disables = Vec::new();
    for s in shield_events {
        if s["type"].as_str() == Some("DISABLE") {
            match s["subject"].as_str() {
                Some("opponent") => opp_disables.push(s),
                Some("local") => local_disables.push(s),
                _ => {}
            }
        }
    }

    let mut converted = 0usize;
    let mut punish_hits_sum = 0usize;
    let mut punish_delay_sum = 0.0;
    let mut axe_swaps = Vec::new();
    let mut counted_swap_seqs = BTreeSet::new();

    for d in &opp_disables {
        let dt = d["time"].as_f64().unwrap_or(0.0);
        let d_weapon = d["weapon"].as_str();
        let mut follow_hits = 0usize;
        let mut first_delay = 0.0;
        let mut found_sword = false;
        for h in hits {
            let et = h["evidenceT"].as_f64().unwrap_or(0.0);
            if et <= dt {
                continue;
            }
            let delta = et - dt;
            if delta > DISABLE_FOLLOW_UP_SECONDS {
                break;
            }
            if follow_hits == 0 {
                first_delay = delta;
            }
            follow_hits += 1;
            let hw = h["weapon"].as_str().unwrap_or("");
            if !found_sword
                && delta <= AXE_SWAP_WINDOW_SECONDS
                && weapon_category(hw) == "SWORD"
                && d_weapon.is_none_or(|w| weapon_category(w) == "AXE")
            {
                found_sword = true;
                let swap_delta = h["t"].as_f64().unwrap_or(et) - dt;
                let seq = h["seq"].as_i64().unwrap_or(0);
                if swap_delta > 0.0
                    && swap_delta <= AXE_SWAP_WINDOW_SECONDS
                    && counted_swap_seqs.insert(seq)
                {
                    axe_swaps.push(swap_delta);
                }
            }
        }
        if follow_hits > 0 {
            converted += 1;
            punish_delay_sum += first_delay;
            punish_hits_sum += follow_hits;
        }
    }

    for i in 1..hits.len() {
        let pw = hits[i - 1]["weapon"].as_str().unwrap_or("");
        let cw = hits[i]["weapon"].as_str().unwrap_or("");
        if weapon_category(pw) == "AXE" && weapon_category(cw) == "SWORD" {
            let delta =
                hits[i]["t"].as_f64().unwrap_or(0.0) - hits[i - 1]["t"].as_f64().unwrap_or(0.0);
            let seq = hits[i]["seq"].as_i64().unwrap_or(0);
            if delta > 0.0 && delta <= AXE_SWAP_WINDOW_SECONDS && counted_swap_seqs.insert(seq) {
                axe_swaps.push(delta);
            }
        }
    }

    let swap_sum: f64 = axe_swaps.iter().sum();
    let best_swap = axe_swaps.iter().copied().reduce(f64::min);

    let mut hits_taken_after_local = 0usize;
    let mut dmg_taken_after_local = 0.0;
    let mut counted_in = BTreeSet::new();
    for ld in &local_disables {
        let dt = ld["time"].as_f64().unwrap_or(0.0);
        for inc in incoming {
            if inc["attributed"].as_bool() != Some(true) {
                continue;
            }
            let it = inc["time"].as_f64().unwrap_or(0.0);
            if it <= dt {
                continue;
            }
            if it - dt > DISABLE_FOLLOW_UP_SECONDS {
                break;
            }
            let seq = inc["sequence"].as_i64().unwrap_or(0);
            if counted_in.insert(seq) {
                hits_taken_after_local += 1;
                if let Some(dmg) = inc["damage"].as_f64().filter(|v| v.is_finite()) {
                    dmg_taken_after_local += dmg;
                }
            }
        }
    }

    let inflicted = opp_disables.len();
    ShieldPunish {
        disables_inflicted: inflicted,
        disables_converted: converted,
        disable_conversion_rate: ratio(100.0 * converted as f64, inflicted as f64),
        mean_disable_punish_delay: ratio(punish_delay_sum, converted as f64),
        mean_punish_hits_per_disable: ratio(punish_hits_sum as f64, inflicted as f64),
        axe_to_sword_swaps: axe_swaps.len(),
        mean_axe_to_sword_seconds: ratio(swap_sum, axe_swaps.len() as f64),
        best_axe_to_sword_seconds: best_swap,
        local_disables: local_disables.len(),
        hits_taken_after_local_disable: hits_taken_after_local,
        damage_taken_after_local_disable: dmg_taken_after_local,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_exchanges(
    f: &FightRecord,
    derived: &Value,
    hits: &[Value],
    incoming: &[Value],
    shield_events: &[Value],
    trace: &[HealthPoint],
    reset_indices: &BTreeSet<usize>,
    context_available: bool,
) -> (ExchangeSummary, Vec<ExchangeRow>) {
    let mut spans: Vec<(f64, f64, String)> = Vec::new();
    let active = derived["combatExchanges"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let combo_ex = derived["combos"]["exchanges"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if !active.is_empty() {
        for e in active {
            spans.push((
                e["start"].as_f64().unwrap_or(0.0),
                e["end"].as_f64().unwrap_or(0.0),
                e["endedBy"].as_str().unwrap_or("INACTIVITY").to_owned(),
            ));
        }
    } else if !combo_ex.is_empty() {
        for e in combo_ex {
            spans.push((
                e["start"].as_f64().unwrap_or(0.0),
                e["end"].as_f64().unwrap_or(0.0),
                "GAP".to_owned(),
            ));
        }
    } else if !hits.is_empty() || incoming.iter().any(|i| i["attributed"] == true) {
        spans.push((0.0, f.duration(), "FIGHT".to_owned()));
    }

    let mut rows = Vec::with_capacity(spans.len());
    let (mut hit_c, mut inc_c, mut hp_c, mut sh_c) = (0usize, 0usize, 1usize, 0usize);
    let (mut contested, mut opened_you, mut opened_opp, mut opened_trade, mut converted) =
        (0, 0, 0, 0, 0);
    let (mut won, mut lost, mut even) = (0, 0, 0);

    for (i, (start, end, ended_by)) in spans.iter().enumerate() {
        let include_end = i + 1 == spans.len() || spans[i + 1].0 > *end + 1e-9;
        let end_bound = *end + if include_end { 1e-6 } else { -1e-9 };

        while hit_c < hits.len() && hits[hit_c]["evidenceT"].as_f64().unwrap_or(0.0) < *start - 1e-6
        {
            hit_c += 1;
        }
        let mut ex_hits = Vec::new();
        while hit_c < hits.len() && hits[hit_c]["evidenceT"].as_f64().unwrap_or(0.0) <= end_bound {
            ex_hits.push(hit_c);
            hit_c += 1;
        }

        while inc_c < incoming.len()
            && incoming[inc_c]["time"].as_f64().unwrap_or(0.0) < *start - 1e-6
        {
            inc_c += 1;
        }
        let mut ex_inc = Vec::new();
        while inc_c < incoming.len() && incoming[inc_c]["time"].as_f64().unwrap_or(0.0) <= end_bound
        {
            if incoming[inc_c]["attributed"].as_bool() == Some(true) {
                ex_inc.push(&incoming[inc_c]);
            }
            inc_c += 1;
        }

        let (mut dmg, mut heal) = (0.0, 0.0);
        while hp_c < trace.len() && trace[hp_c].time < *start - 1e-6 {
            hp_c += 1;
        }
        while hp_c < trace.len() && trace[hp_c].time <= end_bound {
            let delta = trace[hp_c].total - trace[hp_c - 1].total;
            if delta < -1e-6 {
                dmg += -delta;
            } else if delta > 1e-6 {
                heal += delta;
            }
            hp_c += 1;
        }

        let (mut opp_dis, mut loc_dis) = (0, 0);
        while sh_c < shield_events.len()
            && shield_events[sh_c]["time"].as_f64().unwrap_or(0.0) < *start - 1e-6
        {
            sh_c += 1;
        }
        while sh_c < shield_events.len()
            && shield_events[sh_c]["time"].as_f64().unwrap_or(0.0) <= end_bound
        {
            if shield_events[sh_c]["type"].as_str() == Some("DISABLE") {
                match shield_events[sh_c]["subject"].as_str() {
                    Some("opponent") => opp_dis += 1,
                    Some("local") => loc_dis += 1,
                    _ => {}
                }
            }
            sh_c += 1;
        }

        let mut ex_crits = 0;
        let mut ex_resets = 0;
        for &hi in &ex_hits {
            if is_crit(context_available, &hits[hi]) {
                ex_crits += 1;
            }
            if reset_indices.contains(&hi) {
                ex_resets += 1;
            }
        }

        let first_local = ex_hits
            .first()
            .and_then(|&hi| hits[hi]["evidenceT"].as_f64());
        let first_opp = ex_inc.first().and_then(|inc| inc["time"].as_f64());
        let opener = match (first_local, first_opp) {
            (None, None) => "NONE",
            (Some(_), None) => "YOU",
            (None, Some(_)) => "OPPONENT",
            (Some(fl), Some(fo)) => {
                if (fl - fo).abs() <= TRADE_WINDOW_SECONDS + 1e-9 {
                    "TRADE"
                } else if fl < fo {
                    "YOU"
                } else {
                    "OPPONENT"
                }
            }
        };

        let mut opener_converted = false;
        if opener == "YOU" && ex_hits.len() >= 2 {
            let second_t = hits[ex_hits[1]]["evidenceT"].as_f64().unwrap_or(0.0);
            if first_opp.is_none_or(|fo| second_t < fo - 1e-9) {
                opener_converted = true;
            }
        }

        let local_hits = ex_hits.len();
        let opponent_hits = ex_inc.len();
        let hit_diff = local_hits as i64 - opponent_hits as i64;
        let winner = if hit_diff > 0 {
            "YOU"
        } else if hit_diff < 0 {
            "OPPONENT"
        } else {
            "EVEN"
        };

        if opener != "NONE" {
            contested += 1;
            match opener {
                "YOU" => opened_you += 1,
                "OPPONENT" => opened_opp += 1,
                "TRADE" => opened_trade += 1,
                _ => {}
            }
            if opener_converted {
                converted += 1;
            }
            match winner {
                "YOU" => won += 1,
                "OPPONENT" => lost += 1,
                _ => even += 1,
            }
        }

        let dur = (*end - *start).max(0.0);
        rows.push(ExchangeRow {
            index: i + 1,
            start: *start,
            end: *end,
            duration: dur,
            opener: opener.to_owned(),
            opener_converted,
            local_hits,
            opponent_hits,
            hit_diff,
            damage_taken: dmg,
            healing_gained: heal,
            net_hp_delta: heal - dmg,
            dps_taken: (dur >= 0.5 && dmg > 0.0).then(|| dmg / dur),
            hits_per_sec: (dur >= 0.5 && local_hits > 0).then(|| local_hits as f64 / dur),
            crits: ex_crits,
            sprint_resets: ex_resets,
            opponent_disables: opp_dis,
            local_disables: loc_dis,
            winner: winner.to_owned(),
            ended_by: ended_by.clone(),
        });
    }

    let attributed_in = derived["summary"]["attributedIncoming"]
        .as_f64()
        .unwrap_or_else(|| incoming.iter().filter(|i| i["attributed"] == true).count() as f64);
    let summary = ExchangeSummary {
        total: rows.len(),
        contested,
        opened_by_you: opened_you,
        opened_by_opponent: opened_opp,
        opened_by_trade: opened_trade,
        first_hit_rate: ratio(100.0 * opened_you as f64, contested as f64),
        openers_converted: converted,
        first_hit_conversion_rate: ratio(100.0 * converted as f64, opened_you as f64),
        won,
        lost,
        even,
        trade_ratio: ratio(hits.len() as f64, attributed_in),
    };
    (summary, rows)
}

#[allow(clippy::too_many_arguments)]
fn build_turning_points(
    f: &FightRecord,
    derived: &Value,
    hits: &[Value],
    shield_events: &[Value],
    combos: &Combos,
    dominance: &Dominance,
    health: &HealthPacing,
    shields: &ShieldPunish,
) -> Vec<TurningPoint> {
    let mut pts = Vec::new();
    let dom_payload = dominance.payload();
    if let Some(sw) = dom_payload["swing"].as_object() {
        if let (Some(from), Some(to), Some(fv), Some(tv)) = (
            sw.get("from").and_then(Value::as_f64),
            sw.get("to").and_then(Value::as_f64),
            sw.get("fromValue").and_then(Value::as_f64),
            sw.get("toValue").and_then(Value::as_f64),
        ) {
            let delta = tv - fv;
            if delta.abs() >= 0.35 {
                let positive = delta > 0.0;
                pts.push(TurningPoint {
                    time: from,
                    kind: "SWING".into(),
                    tone: if positive { "POSITIVE" } else { "NEGATIVE" }.into(),
                    title: if positive {
                        "Momentum swing in your favor"
                    } else {
                        "Momentum swing to opponent"
                    }
                    .into(),
                    detail: format!("Dominance moved {fv:+.2} -> {tv:+.2} ({from:.1}s-{to:.1}s)"),
                });
            }
        }
    }

    let opp_avail = derived["summary"]["opponentComboAvailability"].as_str();
    let best_local = combos
        .streaks
        .iter()
        .filter(|s| s.local && s.seqs.len() >= 3)
        .max_by_key(|s| s.seqs.len());
    if let Some(s) = best_local {
        let start = f.seconds(s.start);
        let span = (f.seconds(s.end) - start).max(0.0);
        pts.push(TurningPoint {
            time: start,
            kind: "LOCAL_COMBO".into(),
            tone: "POSITIVE".into(),
            title: format!("{}-hit combo", s.seqs.len()),
            detail: format!("{} unanswered hits over {span:.1}s", s.seqs.len()),
        });
    }
    if opp_avail == Some("Observed") {
        let best_opp = combos
            .streaks
            .iter()
            .filter(|s| !s.local && s.seqs.len() >= 3)
            .max_by_key(|s| s.seqs.len());
        if let Some(s) = best_opp {
            let start = f.seconds(s.start);
            let span = (f.seconds(s.end) - start).max(0.0);
            pts.push(TurningPoint {
                time: start,
                kind: "OPPONENT_COMBO".into(),
                tone: "NEGATIVE".into(),
                title: format!("Opponent {}-hit combo", s.seqs.len()),
                detail: format!("{} consecutive incoming hits over {span:.1}s", s.seqs.len()),
            });
        }
    }

    if health.worst_burst_damage >= 6.0 {
        if let Some(bt) = health.worst_burst_time {
            pts.push(TurningPoint {
                time: bt,
                kind: "DAMAGE_BURST".into(),
                tone: "NEGATIVE".into(),
                title: format!("Heavy burst (-{:.1} HP)", health.worst_burst_damage),
                detail: format!(
                    "Lost {:.1} HP inside a {:.0}s window",
                    health.worst_burst_damage, BURST_WINDOW_SECONDS
                ),
            });
        }
    }

    let mut best_disable: Option<(f64, usize)> = None;
    for s in shield_events {
        if s["type"].as_str() == Some("DISABLE") && s["subject"].as_str() == Some("opponent") {
            let st = s["time"].as_f64().unwrap_or(0.0);
            let follow = hits
                .iter()
                .filter(|h| {
                    let et = h["evidenceT"].as_f64().unwrap_or(0.0);
                    et > st && et - st <= DISABLE_FOLLOW_UP_SECONDS
                })
                .count();
            if best_disable.is_none_or(|(_, best)| follow > best) {
                best_disable = Some((st, follow));
            }
        }
    }
    if let Some((dt, follow)) = best_disable.filter(|(_, f)| *f >= 2) {
        pts.push(TurningPoint {
            time: dt,
            kind: "SHIELD_PUNISH".into(),
            tone: "POSITIVE".into(),
            title: format!("Shield disable punish ({follow} hits)"),
            detail: format!(
                "Landed {follow} confirmed hits in the 5s window after breaking shield"
            ),
        });
    } else if shields.local_disables > 0 && shields.hits_taken_after_local_disable >= 2 {
        if let Some(s) = shield_events.iter().find(|s| {
            s["type"].as_str() == Some("DISABLE") && s["subject"].as_str() == Some("local")
        }) {
            pts.push(TurningPoint {
                time: s["time"].as_f64().unwrap_or(0.0),
                kind: "LOCAL_SHIELD_BREAK".into(),
                tone: "NEGATIVE".into(),
                title: "Shield broken".into(),
                detail: format!(
                    "Took {} hits (-{:.1} HP) during local disable windows",
                    shields.hits_taken_after_local_disable,
                    shields.damage_taken_after_local_disable
                ),
            });
        }
    }

    let mut best_heal = 0.0;
    let mut best_heal_t = None;
    for i in 1..health.trace.len() {
        let gain = health.trace[i].total - health.trace[i - 1].total;
        if gain > best_heal + 1e-6 {
            best_heal = gain;
            best_heal_t = Some(health.trace[i].time);
        }
    }
    if best_heal >= 4.0 {
        if let Some(ht) = best_heal_t {
            pts.push(TurningPoint {
                time: ht,
                kind: "BURST_HEAL".into(),
                tone: "NEUTRAL".into(),
                title: format!("Burst recovery (+{best_heal:.1} HP)"),
                detail: format!("Health + absorption rose by {best_heal:.1} HP"),
            });
        }
    }

    pts.sort_by(|a, b| a.time.total_cmp(&b.time));
    pts.truncate(6);
    pts
}

fn build_takeaways(
    derived: &Value,
    health: &HealthPacing,
    sword: &SwordMechanics,
    shields: &ShieldPunish,
    ex: &ExchangeSummary,
) -> Vec<Takeaway> {
    let mut list = Vec::new();
    if ex.contested >= 2 {
        if let Some(fhr) = ex.first_hit_rate {
            let tone = if fhr >= 60.0 {
                "POSITIVE"
            } else if fhr <= 40.0 {
                "NEGATIVE"
            } else {
                "NEUTRAL"
            };
            let headline = if fhr >= 60.0 {
                format!("Won {fhr:.0}% of exchange openers")
            } else if fhr <= 40.0 {
                format!(
                    "Opponent opened {}/{} exchanges",
                    ex.opened_by_opponent, ex.contested
                )
            } else {
                format!("Split openers ({fhr:.0}% first-hit)")
            };
            let trade = ex
                .trade_ratio
                .map(|r| format!(" | {r:.2}x hit trade"))
                .unwrap_or_default();
            let conv = if ex.opened_by_you > 0 {
                format!(
                    ", converted {}/{} into combos",
                    ex.openers_converted, ex.opened_by_you
                )
            } else {
                String::new()
            };
            list.push(Takeaway {
                tone: tone.into(),
                category: "Openers".into(),
                headline,
                detail: format!(
                    "Opened {}/{} contested exchanges{conv} ({}W-{}L-{}E{trade}).",
                    ex.opened_by_you, ex.contested, ex.won, ex.lost, ex.even
                ),
            });
        }
    }

    if sword.sprint_reset_opportunities >= 4 {
        if let Some(srr) = sword.sprint_reset_rate {
            let tone = if srr >= 65.0 {
                "POSITIVE"
            } else if srr < 40.0 {
                "NEGATIVE"
            } else {
                "NEUTRAL"
            };
            let headline = if srr >= 65.0 {
                format!("Consistent sprint resets ({srr:.0}%)")
            } else if srr < 40.0 {
                format!("Low sprint-reset rate ({srr:.0}%)")
            } else {
                format!("Moderate sprint resets ({srr:.0}%)")
            };
            let combo_part = if sword.combo_sprint_reset_opportunities >= 2 {
                sword
                    .combo_sprint_reset_rate
                    .map(|r| format!(" ({r:.0}% inside combos)"))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let cd_part = if sword.early_hits > 0 {
                format!(" | {} early hits (<90% charge)", sword.early_hits)
            } else {
                String::new()
            };
            list.push(Takeaway {
                tone: tone.into(),
                category: "Sword & Movement".into(),
                headline,
                detail: format!(
                    "Reset sprint before {}/{} sprinting follow-ups{combo_part}{cd_part}.",
                    sword.sprint_resets, sword.sprint_reset_opportunities
                ),
            });
        }
    } else if sword.full_charge_hits + sword.early_hits >= 6 {
        if let Some(esr) = sword.early_swing_rate.filter(|r| *r >= 25.0) {
            list.push(Takeaway {
                tone: "NEGATIVE".into(),
                category: "Cooldown".into(),
                headline: format!("Early swings on {esr:.0}% of hits"),
                detail: format!(
                    "{}/{} confirmed hits landed below 90% charge (mean {:.2}).",
                    sword.early_hits,
                    sword.full_charge_hits + sword.early_hits,
                    sword.mean_hit_cooldown.unwrap_or(0.0)
                ),
            });
        }
    }

    if shields.disables_inflicted >= 1 {
        let rate = shields.disable_conversion_rate.unwrap_or(0.0);
        let tone = if rate >= 60.0 {
            "POSITIVE"
        } else if rate < 50.0 {
            "NEGATIVE"
        } else {
            "NEUTRAL"
        };
        let mut detail = if let Some(delay) = shields.mean_disable_punish_delay {
            format!(
                "Avg {delay:.2}s to first punish hit ({:.1} hits/disable)",
                shields.mean_punish_hits_per_disable.unwrap_or(0.0)
            )
        } else {
            "No confirmed follow-up hits within 5s of a disable".into()
        };
        if shields.axe_to_sword_swaps > 0 {
            if let Some(swap) = shields.mean_axe_to_sword_seconds {
                detail.push_str(&format!(
                    " | {swap:.2}s avg axe-to-sword swap ({})",
                    shields.axe_to_sword_swaps
                ));
            }
        }
        detail.push('.');
        list.push(Takeaway {
            tone: tone.into(),
            category: "Shield & Axe".into(),
            headline: format!(
                "Converted {}/{} shield disables",
                shields.disables_converted, shields.disables_inflicted
            ),
            detail,
        });
    } else if shields.local_disables >= 1 && shields.hits_taken_after_local_disable >= 2 {
        list.push(Takeaway {
            tone: "NEGATIVE".into(),
            category: "Shield Defense".into(),
            headline: "Punished after shield disable".into(),
            detail: format!(
                "Took {} hits (-{:.1} HP) in 5s windows after {} local shield disable(s).",
                shields.hits_taken_after_local_disable,
                shields.damage_taken_after_local_disable,
                shields.local_disables
            ),
        });
    }

    if !health.trace.is_empty() && health.total_damage_taken >= 4.0 {
        let heavy = health.worst_burst_damage >= 8.0;
        let headline = if heavy {
            format!("Sharpest burst: -{:.1} HP in 3s", health.worst_burst_damage)
        } else if health.total_healing >= 4.0 {
            format!("Recovered +{:.1} HP across fight", health.total_healing)
        } else {
            format!(
                "Health lost: {:.1} HP (min {:.1})",
                health.total_damage_taken,
                health.min_hp.unwrap_or(0.0)
            )
        };
        let dps = health
            .damage_taken_per_sec
            .map(|d| format!(" ({d:.2} HP/s)"))
            .unwrap_or_default();
        let heal = if health.total_healing > 0.5 {
            format!(
                ", recovered +{:.1} HP ({} burst heal{})",
                health.total_healing,
                health.burst_heals,
                if health.burst_heals == 1 { "" } else { "s" }
            )
        } else {
            String::new()
        };
        list.push(Takeaway {
            tone: if heavy { "NEGATIVE" } else { "NEUTRAL" }.into(),
            category: "Health Pacing".into(),
            headline,
            detail: format!(
                "Lost {:.1} HP{dps}{heal}; lowest point {:.1} HP at {:.1}s.",
                health.total_damage_taken,
                health.min_hp.unwrap_or(0.0),
                health.min_hp_time.unwrap_or(0.0)
            ),
        });
    }

    if list.len() < 3 {
        let s = &derived["summary"];
        if let (Some(acc), Some(hits), Some(denom)) = (
            s["accuracy"].as_f64(),
            s["hits"].as_i64(),
            s["accuracyDenominator"].as_i64(),
        ) {
            if denom >= 6 {
                let tone = if acc >= 65.0 {
                    "POSITIVE"
                } else if acc < 45.0 {
                    "NEGATIVE"
                } else {
                    "NEUTRAL"
                };
                list.push(Takeaway {
                    tone: tone.into(),
                    category: "Accuracy".into(),
                    headline: format!("{acc:.0}% accuracy ({hits}/{denom})"),
                    detail: format!(
                        "Confirmed {hits} hits from {denom} eligible attempts (longest combo {}).",
                        s["longestCombo"].as_i64().unwrap_or(0)
                    ),
                });
            }
        }
    }

    list.truncate(4);
    list
}

/// Emits single-fight breakdown metrics, health chart, and exchange/turning-point tables into tabs.
pub(crate) fn apply_to_tabs(
    b: &Breakdown,
    overview: &mut crate::pulsar::view::TabView,
    offense: &mut crate::pulsar::view::TabView,
    defense: &mut crate::pulsar::view::TabView,
    flow: &mut crate::pulsar::view::TabView,
    table_fn: fn(&str, &str, &Value, Option<&FightKey>) -> TableView,
) {
    // Concise fact-backed takeaways at the top of Overview interpretations.
    if !b.takeaways.is_empty() {
        let mut lines: Vec<String> = b
            .takeaways
            .iter()
            .map(|tk| format!("[{}] {} — {}", tk.category, tk.headline, tk.detail))
            .collect();
        lines.extend(overview.interpretations.drain(..));
        overview.interpretations = lines;
    }

    overview.metrics.insert(
        "firstHitRate".into(),
        metrics::extra(
            "firstHitRate",
            "First-hit rate",
            "%",
            "Share of contested exchanges where your confirmed hit landed first.",
            b.exchange_summary.first_hit_rate,
        ),
    );
    overview.metrics.insert(
        "sprintResetRate".into(),
        metrics::extra(
            "sprintResetRate",
            "Sprint-reset rate",
            "%",
            "Sprinting follow-up hits (within 2.5 s) preceded by a recorded sprint release (W-tap / S-tap).",
            b.sword.sprint_reset_rate,
        ),
    );
    overview.metrics.insert(
        "tradeRatio".into(),
        metrics::extra(
            "tradeRatio",
            "Hit trade ratio",
            "x",
            "Confirmed hits dealt divided by attributed incoming opponent hits.",
            b.exchange_summary.trade_ratio,
        ),
    );

    // Offense: sprint-reset cadence, charge discipline, axe-to-sword swap.
    for (key, label, unit, help, val) in [
        (
            "sprintResetRate",
            "Sprint-reset rate",
            "%",
            "Sprinting follow-up hits (within 2.5 s) preceded by a recorded sprint release (W-tap / S-tap).",
            b.sword.sprint_reset_rate,
        ),
        (
            "comboSprintResetRate",
            "Combo sprint-reset rate",
            "%",
            "Sprint-reset rate specifically inside 2+ hit combos.",
            b.sword.combo_sprint_reset_rate,
        ),
        (
            "earlySwingRate",
            "Early swing rate",
            "%",
            "Confirmed hits landed below 90% melee attack charge.",
            b.sword.early_swing_rate,
        ),
        (
            "axeToSwordSwap",
            "Axe-to-sword swap",
            "s",
            "Mean seconds from an axe hit or shield disable to the immediately following sword hit within 3 s.",
            b.shields.mean_axe_to_sword_seconds,
        ),
    ] {
        offense
            .metrics
            .insert(key.into(), metrics::extra(key, label, unit, help, val));
    }

    // Defense: shield/axe punish & health/damage pacing + health timeline chart.
    for (key, label, unit, help, val) in [
        (
            "disableConversionRate",
            "Disable conversion",
            "%",
            "Share of opponent shield disables followed by at least one confirmed hit within 5 s.",
            b.shields.disable_conversion_rate,
        ),
        (
            "disablePunishDelay",
            "Disable punish delay",
            "s",
            "Mean seconds from an opponent shield disable to your first confirmed follow-up hit.",
            b.shields.mean_disable_punish_delay,
        ),
        (
            "axeToSwordSwap",
            "Axe-to-sword swap",
            "s",
            "Mean seconds from an axe hit or shield disable to the immediately following sword hit within 3 s.",
            b.shields.mean_axe_to_sword_seconds,
        ),
        (
            "localDisableHitsTaken",
            "Hits taken after local disable",
            "",
            "Attributed opponent hits taken during 5 s windows after your own shield was disabled.",
            (b.shields.local_disables > 0)
                .then_some(b.shields.hits_taken_after_local_disable as f64),
        ),
        (
            "totalHealing",
            "Health recovered",
            "HP",
            "Sum of observed increases in your local health plus absorption.",
            (!b.health.trace.is_empty()).then_some(b.health.total_healing),
        ),
        (
            "burstHeals",
            "Burst heals",
            "",
            "Observed health + absorption jumps of 2.0 HP or more (potions, gapples, totems).",
            (!b.health.trace.is_empty()).then_some(b.health.burst_heals as f64),
        ),
        (
            "damageTakenPerSec",
            "Damage taken / s",
            "HP/s",
            "Health lost divided by active combat time.",
            b.health.damage_taken_per_sec,
        ),
        (
            "worstBurstDamage",
            "Worst 3 s burst",
            "HP",
            "Largest cumulative local HP + absorption drop within any 3-second window.",
            (b.health.worst_burst_damage > 0.0).then_some(b.health.worst_burst_damage),
        ),
    ] {
        defense
            .metrics
            .insert(key.into(), metrics::extra(key, label, unit, help, val));
    }

    if !b.health.trace.is_empty() {
        let hp_chart = health_chart(&b.health);
        defense.charts.push(hp_chart);
    }

    // Flow: exchange openers, trade efficiency, enriched exchange table & turning points.
    for (key, label, unit, help, val) in [
        (
            "firstHitRate",
            "First-hit rate",
            "%",
            "Share of contested exchanges where your confirmed hit landed first.",
            b.exchange_summary.first_hit_rate,
        ),
        (
            "firstHitConversionRate",
            "Opener conversion",
            "%",
            "Share of exchanges you opened that converted into a 2+ hit combo before retaliation.",
            b.exchange_summary.first_hit_conversion_rate,
        ),
        (
            "tradeRatio",
            "Hit trade ratio",
            "x",
            "Confirmed hits dealt divided by attributed incoming opponent hits.",
            b.exchange_summary.trade_ratio,
        ),
    ] {
        flow.metrics
            .insert(key.into(), metrics::extra(key, label, unit, help, val));
    }

    if !b.exchanges.is_empty() {
        let rows: Vec<Value> = b
            .exchanges
            .iter()
            .map(|e| {
                json!({
                    "exchange": format!("#{}", e.index),
                    "start": e.start,
                    "end": e.end,
                    "opener": e.opener,
                    "converted": if e.opener_converted { "Yes" } else { "-" },
                    "hits": format!("{} / {}", e.local_hits, e.opponent_hits),
                    "hitDiff": e.hit_diff,
                    "damageTaken": (e.damage_taken > 0.0).then_some(e.damage_taken),
                    "healing": (e.healing_gained > 0.0).then_some(e.healing_gained),
                    "winner": e.winner,
                })
            })
            .collect();
        flow.tables.push(table_fn(
            "pvp_exchanges",
            "Exchange openers & trades",
            &json!(rows),
            None,
        ));
    }

    if !b.turning_points.is_empty() {
        let rows: Vec<Value> = b
            .turning_points
            .iter()
            .map(|tp| {
                json!({
                    "time": tp.time,
                    "kind": tp.kind,
                    "title": tp.title,
                    "detail": tp.detail,
                })
            })
            .collect();
        flow.tables.push(table_fn(
            "pvp_turning_points",
            "Key PvP turning points",
            &json!(rows),
            None,
        ));
    }
}

/// Pools PvP breakdown metrics across all loaded timelines for multi-fight scope tabs.
pub(crate) fn apply_pooled(
    timelines: &[(FightKey, FightRecord, Analytics)],
    overview: &mut crate::pulsar::view::TabView,
    offense: &mut crate::pulsar::view::TabView,
    defense: &mut crate::pulsar::view::TabView,
    flow: &mut crate::pulsar::view::TabView,
) {
    let (mut contested, mut opened_you, mut openers_conv) = (0usize, 0usize, 0usize);
    let (mut sr_opp, mut sr_hit, mut csr_opp, mut csr_hit) = (0usize, 0usize, 0usize, 0usize);
    let (mut fc_hits, mut early_hits) = (0usize, 0usize);
    let (mut dis_inf, mut dis_conv, mut swaps, mut local_dis, mut local_dis_hits) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut dis_delay_sum, mut swap_delay_sum, mut heal_sum, mut dmg_sum, mut span_sum) =
        (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut burst_heals, mut has_health) = (0usize, false);

    for (_, _, a) in timelines {
        let b = &a.breakdown;
        contested += b.exchange_summary.contested;
        opened_you += b.exchange_summary.opened_by_you;
        openers_conv += b.exchange_summary.openers_converted;
        sr_opp += b.sword.sprint_reset_opportunities;
        sr_hit += b.sword.sprint_resets;
        csr_opp += b.sword.combo_sprint_reset_opportunities;
        csr_hit += b.sword.combo_sprint_resets;
        fc_hits += b.sword.full_charge_hits;
        early_hits += b.sword.early_hits;
        dis_inf += b.shields.disables_inflicted;
        dis_conv += b.shields.disables_converted;
        if let Some(d) = b.shields.mean_disable_punish_delay {
            dis_delay_sum += d * b.shields.disables_converted as f64;
        }
        swaps += b.shields.axe_to_sword_swaps;
        if let Some(s) = b.shields.mean_axe_to_sword_seconds {
            swap_delay_sum += s * b.shields.axe_to_sword_swaps as f64;
        }
        local_dis += b.shields.local_disables;
        local_dis_hits += b.shields.hits_taken_after_local_disable;
        if !b.health.trace.is_empty() {
            has_health = true;
            heal_sum += b.health.total_healing;
            dmg_sum += b.health.total_damage_taken;
            burst_heals += b.health.burst_heals;
            span_sum += b.active_span;
        }
    }

    let first_hit = ratio(100.0 * opened_you as f64, contested as f64);
    let opener_conv = ratio(100.0 * openers_conv as f64, opened_you as f64);
    let sprint_reset = ratio(100.0 * sr_hit as f64, sr_opp as f64);
    let combo_sr = ratio(100.0 * csr_hit as f64, csr_opp as f64);
    let early_rate = ratio(100.0 * early_hits as f64, (fc_hits + early_hits) as f64);
    let dis_conv_rate = ratio(100.0 * dis_conv as f64, dis_inf as f64);
    let dis_delay = ratio(dis_delay_sum, dis_conv as f64);
    let swap_avg = ratio(swap_delay_sum, swaps as f64);
    let dps = if has_health {
        ratio(dmg_sum, span_sum)
    } else {
        None
    };

    overview.metrics.insert(
        "firstHitRate".into(),
        metrics::extra(
            "firstHitRate",
            "First-hit rate",
            "%",
            "Pooled share of contested exchanges you opened across loaded timelines.",
            first_hit,
        ),
    );
    overview.metrics.insert(
        "sprintResetRate".into(),
        metrics::extra(
            "sprintResetRate",
            "Sprint-reset rate",
            "%",
            "Pooled sprint-reset rate across loaded timelines.",
            sprint_reset,
        ),
    );

    for (key, label, unit, help, val) in [
        (
            "sprintResetRate",
            "Sprint-reset rate",
            "%",
            "Pooled sprint-reset rate across loaded timelines.",
            sprint_reset,
        ),
        (
            "comboSprintResetRate",
            "Combo sprint-reset rate",
            "%",
            "Pooled sprint-reset rate inside 2+ hit combos.",
            combo_sr,
        ),
        (
            "earlySwingRate",
            "Early swing rate",
            "%",
            "Confirmed hits landed below 90% melee attack charge.",
            early_rate,
        ),
        (
            "axeToSwordSwap",
            "Axe-to-sword swap",
            "s",
            "Mean seconds from an axe hit or shield disable to the following sword hit.",
            swap_avg,
        ),
    ] {
        offense
            .metrics
            .insert(key.into(), metrics::extra(key, label, unit, help, val));
    }

    for (key, label, unit, help, val) in [
        (
            "disableConversionRate",
            "Disable conversion",
            "%",
            "Pooled share of opponent shield disables punished within 5 s.",
            dis_conv_rate,
        ),
        (
            "disablePunishDelay",
            "Disable punish delay",
            "s",
            "Mean seconds from an opponent shield disable to first follow-up hit.",
            dis_delay,
        ),
        (
            "axeToSwordSwap",
            "Axe-to-sword swap",
            "s",
            "Mean seconds from an axe hit or shield disable to the following sword hit.",
            swap_avg,
        ),
        (
            "localDisableHitsTaken",
            "Hits taken after local disable",
            "",
            "Attributed opponent hits taken within 5 s after your shield was disabled.",
            (local_dis > 0).then_some(local_dis_hits as f64),
        ),
        (
            "totalHealing",
            "Health recovered",
            "HP",
            "Pooled observed local health + absorption increases.",
            has_health.then_some(heal_sum),
        ),
        (
            "burstHeals",
            "Burst heals",
            "",
            "Pooled burst heal jumps (>= 2.0 HP).",
            has_health.then_some(burst_heals as f64),
        ),
        (
            "damageTakenPerSec",
            "Damage taken / s",
            "HP/s",
            "Pooled health lost divided by active combat time.",
            dps,
        ),
    ] {
        defense
            .metrics
            .insert(key.into(), metrics::extra(key, label, unit, help, val));
    }

    for (key, label, unit, help, val) in [
        (
            "firstHitRate",
            "First-hit rate",
            "%",
            "Pooled share of contested exchanges you opened.",
            first_hit,
        ),
        (
            "firstHitConversionRate",
            "Opener conversion",
            "%",
            "Pooled share of your openers converted into 2+ hit combos.",
            opener_conv,
        ),
    ] {
        flow.metrics
            .insert(key.into(), metrics::extra(key, label, unit, help, val));
    }
}

fn health_chart(hp: &HealthPacing) -> ChartView {
    let mut bands = Vec::new();
    if hp.worst_burst_damage >= 6.0 {
        if let Some(bt) = hp.worst_burst_time {
            bands.push(ChartBand {
                start: bt,
                end: bt + BURST_WINDOW_SECONDS,
                label: format!("Worst 3 s burst (-{:.1} HP)", hp.worst_burst_damage),
                lane: "burst".into(),
            });
        }
    }
    ChartView {
        id: "health_timeline".into(),
        title: "Health & absorption timeline".into(),
        kind: "line".into(),
        x_unit: "s".into(),
        y_definition: metrics::extra(
            "healthTotal",
            "Health + absorption",
            "HP",
            "Observed client health plus absorption over time.",
            None,
        )
        .definition,
        series: vec![ChartSeries {
            label: "Health + absorption".into(),
            points: hp
                .trace
                .iter()
                .map(|p| ChartPoint {
                    x: p.time,
                    y: Some(p.total),
                    sequence: None,
                    key: None,
                    category: "health".into(),
                    label: if p.absorption > 0.01 {
                        format!("{:.1} HP + {:.1} abs", p.health, p.absorption)
                    } else {
                        format!("{:.1} HP", p.health)
                    },
                    availability: crate::pulsar::view::Availability::Available,
                })
                .collect(),
        }],
        bands,
        description: "Local client health + absorption samples over recording time.".into(),
    }
}
