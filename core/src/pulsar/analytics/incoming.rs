use super::super::wire::*;
use super::{attributed, mean, num, text};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Incoming {
    pub rows: Vec<Value>,
    pub unexplained: Vec<f64>,
}
fn difference(a: i64, b: i64) -> i128 {
    a as i128 - b as i128
}
fn motion(detail: Option<&str>) -> Option<f64> {
    let (mut vx, mut vz) = (None, None);
    for part in detail?.split(';') {
        if let Some(v) = part.strip_prefix("vx=") {
            vx = Some(v.parse::<f64>().ok()?);
        } else if let Some(v) = part.strip_prefix("vz=") {
            vz = Some(v.parse::<f64>().ok()?);
        }
    }
    let (x, z) = (vx?, vz?);
    (x.is_finite() && z.is_finite()).then_some(x.hypot(z))
}
pub(crate) fn build(f: &FightRecord, events: &[CombatEvent]) -> Incoming {
    let damage: Vec<_> = events.iter().filter(|e| e.kind == "DAMAGE_IN").collect();
    let jumps: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "JUMP" && e.client_tick.is_some())
        .collect();
    let health: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "HEALTH" && e.context.as_ref().is_some_and(|c| c.health.is_some()))
        .collect();
    let mut health_by_tick: BTreeMap<i64, Vec<&CombatEvent>> = BTreeMap::new();
    let mut velocities: BTreeMap<i64, Vec<&CombatEvent>> = BTreeMap::new();
    let mut motions: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
    let crits: BTreeSet<i64> = events
        .iter()
        .filter(|e| {
            e.kind == "CRIT"
                && e.context.as_ref().and_then(|c| c.subject.as_deref()) == Some("local")
        })
        .filter_map(|e| e.client_tick)
        .collect();
    let damage_ticks: BTreeSet<i64> = damage.iter().filter_map(|e| e.client_tick).collect();
    for e in &health {
        if let Some(t) = e.client_tick {
            health_by_tick.entry(t).or_default().push(e);
        }
    }
    for e in events {
        if let Some(t) = e.client_tick {
            if e.kind == "VELOCITY" && e.context.as_ref().is_some_and(|c| c.vx.is_some()) {
                velocities.entry(t).or_default().push(e);
            } else if e.kind == "MOTION" {
                if let Some(speed) = motion(e.detail.as_deref()) {
                    motions.entry(t).or_default().push(speed);
                }
            }
        }
    }
    let mut damage_counts: BTreeMap<i64, usize> = BTreeMap::new();
    for d in &damage {
        if let Some(t) = d.client_tick {
            *damage_counts.entry(t).or_default() += 1;
        }
    }
    let mut rows = Vec::new();
    for d in &damage {
        let ctx = d.context.as_ref();
        let att = attributed(d, f.opponent.as_deref());
        let attribution = if att { "OPPONENT_MELEE" } else { "NON_MELEE" };
        let (mut drop, mut kb, mut after, mut offset) = (None, None, None, None);
        let mut jump_class = if !att {
            "NOT_APPLICABLE"
        } else if d.client_tick.is_none() {
            "UNAVAILABLE"
        } else if ctx.and_then(|c| c.movement_valid) == Some(false) {
            "EXCLUDED"
        } else {
            "NO_JUMP"
        };
        if let Some(t) = d.client_tick {
            let value = |h: &CombatEvent| {
                let c = h.context.as_ref().unwrap();
                c.health.unwrap() + c.absorption.unwrap_or(0.0)
            };
            let before = health
                .iter()
                .rev()
                .find(|h| h.sequence < d.sequence && h.client_tick.is_some_and(|ht| ht <= t))
                .map(|h| value(h));
            let minimum = health_by_tick
                .range(t..=t.saturating_add(2))
                .flat_map(|(_, v)| v)
                .filter(|h| !(h.sequence < d.sequence && h.client_tick.is_some_and(|ht| ht <= t)))
                .map(|h| value(h))
                .reduce(f64::min);
            if let Some((before, minimum)) = before.zip(minimum).filter(|_| {
                damage_counts
                    .range(t.saturating_sub(2)..=t.saturating_add(2))
                    .map(|(_, n)| *n)
                    .sum::<usize>()
                    == 1
            }) {
                if before - minimum > 1e-6 {
                    drop = Some(before - minimum);
                }
            }
            // The source chooses the first velocity by sequence, not lowest tick.
            kb = velocities
                .range(t..=t.saturating_add(1))
                .flat_map(|(_, v)| v)
                .min_by_key(|e| e.sequence)
                .and_then(|e| {
                    let c = e.context.as_ref()?;
                    Some(c.vx?.hypot(c.vz?))
                });
            if damage_counts
                .range(t.saturating_sub(1)..=t.saturating_add(1))
                .map(|(_, n)| *n)
                .sum::<usize>()
                != 1
            {
                kb = None;
            }
            let speeds: Vec<_> = motions
                .range(t.saturating_add(2)..=t.saturating_add(8))
                .flat_map(|(_, v)| v)
                .copied()
                .collect();
            after = mean(&speeds);
            if jump_class == "NO_JUMP" {
                if let Some(j) = jumps
                    .iter()
                    .min_by_key(|j| (difference(j.client_tick.unwrap(), t).abs(), j.sequence))
                {
                    let delta = difference(j.client_tick.unwrap(), t);
                    if delta.abs() <= 20 {
                        offset = Some(delta as i64);
                        jump_class = if delta == 0 {
                            "IDEAL_CANDIDATE"
                        } else if delta.abs() <= f.policy().jump_window_ticks as i128 {
                            "ATTEMPT_CANDIDATE"
                        } else {
                            "OUTSIDE_WINDOW"
                        };
                    }
                }
            }
        }
        let crit = d.client_tick.is_some_and(|t| {
            (-1..=1).any(|v| t.checked_add(v).is_some_and(|tick| crits.contains(&tick)))
        });
        rows.push(json!({"sequence":d.sequence,"time":f.seconds(d.nano_time),"tick":d.client_tick,"attribution":attribution,"attributed":att,"sourceName":ctx.and_then(|c|c.extra.get("sourceName")),"cause":damage_cause(d,f.opponent.as_deref()),"damageType":ctx.and_then(|c|c.damage_type.as_deref()),"damage":drop,"knockback":kb,"ground":ctx.and_then(|c|c.ground),"sprinting":ctx.and_then(|c|c.sprinting),"movementValid":ctx.and_then(|c|c.movement_valid),"jumpClass":jump_class,"jumpOffsetTicks":offset,"crit":crit,"speedAfter":after}));
    }
    let mut unexplained = Vec::new();
    let mut previous = None;
    for h in health {
        let c = h.context.as_ref().unwrap();
        let value = c.health.unwrap() + c.absorption.unwrap_or(0.0);
        if previous.is_some_and(|p| value < p - 1e-6) {
            if let Some(t) = h.client_tick {
                if damage_ticks.range(t.saturating_sub(2)..=t).next().is_none() {
                    unexplained.push(f.seconds(h.nano_time));
                }
            }
        }
        previous = Some(value);
    }
    Incoming { rows, unexplained }
}
pub(crate) fn jumps(f: &FightRecord, events: &[CombatEvent]) -> (Vec<Value>, usize) {
    let damage: Vec<_> = events
        .iter()
        .filter(|e| attributed(e, f.opponent.as_deref()))
        .collect();
    // Null opponent in JumpResets is the legacy overload and considers all incoming rows.
    let tick_damage: Vec<_> = events
        .iter()
        .filter(|e| {
            e.kind == "DAMAGE_IN" && (f.opponent.is_none() || attributed(e, f.opponent.as_deref()))
        })
        .collect();
    let mut out = Vec::new();
    let mut without = 0;
    for jump in events.iter().filter(|e| e.kind == "JUMP") {
        let nearest_tick = tick_damage.iter().min_by_key(|d| {
            let ticks = match (jump.client_tick, d.client_tick) {
                (Some(a), Some(b)) => difference(a, b).abs(),
                _ => i128::MAX,
            };
            (ticks, difference(jump.nano_time, d.nano_time).abs())
        });
        let offset = nearest_tick
            .and_then(|d| Some(difference(jump.client_tick?, d.client_tick?)))
            .filter(|delta| delta.abs() <= 20)
            .map(|v| v as i64);
        let class = match offset {
            None => "UNAVAILABLE",
            Some(0) => "IDEAL_CANDIDATE",
            Some(delta) if delta.abs() <= f.policy().jump_window_ticks as i64 => {
                "ATTEMPT_CANDIDATE"
            }
            _ => "OUTSIDE_WINDOW",
        };
        let nearest = damage
            .iter()
            .min_by_key(|d| difference(jump.nano_time, d.nano_time).abs());
        if nearest.is_none()
            || (offset.is_none()
                && nearest
                    .is_some_and(|d| difference(jump.nano_time, d.nano_time).abs() > 1_000_000_000))
        {
            without += 1;
            continue;
        }
        let nearest = nearest.unwrap();
        out.push(json!({"t":f.seconds(jump.nano_time),"offsetMs":difference(jump.nano_time,nearest.nano_time)as f64/1e6,"candidate":matches!(class,"IDEAL_CANDIDATE"|"ATTEMPT_CANDIDATE"),"offsetTicks":offset,"classification":class}));
    }
    (out, without)
}
pub(crate) fn jump_effect(rows: &[Value]) -> Value {
    let (mut candidates, mut others) = (Vec::new(), Vec::new());
    for r in rows {
        if r["attributed"] != true
            || r["speedAfter"].as_f64().is_none()
            || r["knockback"].as_f64().is_none()
        {
            continue;
        }
        match text(r, "jumpClass") {
            "IDEAL_CANDIDATE" | "ATTEMPT_CANDIDATE" => candidates.push(num(r, "speedAfter")),
            "NO_JUMP" | "OUTSIDE_WINDOW" => others.push(num(r, "speedAfter")),
            _ => {}
        }
    }
    if candidates.len() < 3 || others.len() < 3 {
        Value::Null
    } else {
        json!({"candidates":candidates.len(),"candidateSpeed":mean(&candidates),"others":others.len(),"otherSpeed":mean(&others)})
    }
}

pub(crate) fn damage_cause(e: &CombatEvent, opponent: Option<&str>) -> String {
    let c = e.context.as_ref();
    let name = c
        .and_then(|c| c.extra.get("sourceName"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .or(e.actor.as_deref());
    if c.and_then(|c| c.attribution.as_deref())
        .is_some_and(|a| matches!(a, "UNATTRIBUTED" | "UNKNOWN_CAUSE"))
    {
        return "Unknown cause".into();
    }
    let Some(t) = c.and_then(|c| c.damage_type.as_deref()) else {
        return if attributed(e, opponent) {
            format!("Melee from {}", name.unwrap_or("null"))
        } else {
            "Unknown cause".into()
        };
    };
    let id = t.rsplit(':').next().unwrap_or(t);
    let cause = match id {
        "player_attack" | "mob_attack" | "mob_attack_no_aggro" => "Melee".into(),
        "mace_smash" => "Mace smash".into(),
        "arrow" => "Arrow".into(),
        "trident" => "Trident".into(),
        "fall" => "Fall".into(),
        "on_fire" | "in_fire" => "Fire".into(),
        "lava" => "Lava".into(),
        "out_of_world" | "fell_out_of_world" => "Void".into(),
        "drown" => "Drowning".into(),
        "explosion" | "player_explosion" => "Explosion".into(),
        _ => {
            let mut chars = id.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + &chars.as_str().replace('_', " "))
                .unwrap_or_else(|| "Unknown cause".into())
        }
    };
    name.map(|n| format!("{cause} from {n}")).unwrap_or(cause)
}
