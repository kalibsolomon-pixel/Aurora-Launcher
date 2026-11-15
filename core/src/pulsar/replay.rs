//! Packet-order replay of schema-6+ observations. Only mutates the decoded copy.
//! Mirrors Client OrderedOutcomes version 3 (79c2c9b).
use super::{analytics, wire::*};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const STUN_NANOS: i128 = 150_000_000;
fn delta(a: i64, b: i64) -> i128 {
    a as i128 - b as i128
}
pub(crate) fn empty(e: &CombatEvent) -> bool {
    e.attack.is_none()
        || e.detail
            .as_deref()
            .is_some_and(|s| s.starts_with("empty-swing"))
}
fn eligible(e: &CombatEvent) -> bool {
    e.target.is_some() && e.attack.is_some()
}
fn weapon(e: &CombatEvent) -> Option<&str> {
    e.attack.as_ref()?.measurement.as_ref()?.weapon.as_deref()
}
fn axe(e: &CombatEvent) -> bool {
    weapon(e).is_some_and(|w| {
        let w = w.split(';').next().unwrap();
        w == "axe" || w.ends_with("_axe")
    })
}
fn follow_up(a: &CombatEvent, b: &CombatEvent) -> bool {
    b.sequence > a.sequence
        && (0..=STUN_NANOS).contains(&delta(b.nano_time, a.nano_time))
        && a.actor == b.actor
        && a.target == b.target
        && a.client_tick
            .zip(b.client_tick)
            .is_some_and(|(a, b)| (0..=3).contains(&delta(b, a)))
}
pub(crate) fn spam(a: &CombatEvent, prior: Option<&CombatEvent>) -> bool {
    if empty(a) || !eligible(a) {
        return false;
    }
    if a.attack.as_ref().unwrap().cooldown < 0.5 {
        return true;
    }
    let Some(h) = prior.filter(|h| h.sequence < a.sequence && h.target == a.target) else {
        return false;
    };
    match (a.client_tick, h.client_tick) {
        (Some(a), Some(h)) => (0..=10).contains(&delta(a, h)),
        _ => (0..=500_000_000).contains(&delta(a.nano_time, h.nano_time)),
    }
}
#[derive(Clone)]
struct Attempt {
    attack: CombatEvent,
    evidence: Option<CombatEvent>,
    result: Option<&'static str>,
    parent: Option<usize>,
    child: Option<usize>,
    considered: bool,
}
struct Matcher {
    window: i128,
    incoming: bool,
    pending: VecDeque<usize>,
    disables: VecDeque<usize>,
    last: Option<usize>,
    order: Vec<usize>,
    attempts: Vec<Attempt>,
}
impl Matcher {
    fn new(millis: f64, incoming: bool) -> Self {
        Self {
            window: (millis * 1e6) as i128,
            incoming,
            pending: VecDeque::new(),
            disables: VecDeque::new(),
            last: None,
            order: Vec::new(),
            attempts: Vec::new(),
        }
    }
    fn boundary(&mut self) {
        self.pending.clear();
        self.disables.clear();
        self.last = None;
    }
    fn add(&mut self, event: &CombatEvent) -> usize {
        let k = self.attempts.len();
        self.attempts.push(Attempt {
            attack: event.clone(),
            evidence: None,
            result: None,
            parent: None,
            child: None,
            considered: false,
        });
        let at = self.order.partition_point(|&i| {
            (
                self.attempts[i].attack.nano_time,
                self.attempts[i].attack.sequence,
            ) <= (event.nano_time, event.sequence)
        });
        self.order.insert(at, k);
        let at = self.pending.partition_point(|&i| {
            (
                self.attempts[i].attack.nano_time,
                self.attempts[i].attack.sequence,
            ) <= (event.nano_time, event.sequence)
        });
        self.pending.insert(at, k);
        self.disables
            .retain(|&i| delta(event.nano_time, self.attempts[i].attack.nano_time) <= STUN_NANOS);
        for &i in &self.disables {
            if !self.attempts[i].considered && follow_up(&self.attempts[i].attack, event) {
                self.attempts[i].considered = true;
                self.attempts[i].child = Some(k);
                self.attempts[k].parent = Some(i);
                break;
            }
        }
        k
    }
    fn resolve(&mut self, k: usize, e: &CombatEvent, result: &'static str) {
        self.attempts[k].result = Some(result);
        self.attempts[k].evidence = Some(e.clone());
        self.last = Some(k);
        if result != "HIT" {
            if let Some(p) = self.attempts[k].parent.take() {
                self.attempts[p].child = None;
            }
        }
        if result == "DISABLED" {
            self.disables.push_back(k);
            let at = self.order.iter().position(|&i| i == k).unwrap();
            for &b in &self.order[at + 1..] {
                if self.attempts[k].considered
                    || delta(
                        self.attempts[b].attack.nano_time,
                        self.attempts[k].attack.nano_time,
                    ) > STUN_NANOS
                {
                    break;
                }
                if self.attempts[b].parent.is_some()
                    || !follow_up(&self.attempts[k].attack, &self.attempts[b].attack)
                {
                    continue;
                }
                self.attempts[k].considered = true;
                if self.attempts[b].result.is_none_or(|r| r == "HIT") {
                    self.attempts[k].child = Some(b);
                    self.attempts[b].parent = Some(k);
                }
                break;
            }
        }
    }
    fn matched(&mut self, e: &CombatEvent, result: &'static str) -> Option<usize> {
        self.pending
            .retain(|&i| delta(e.nano_time, self.attempts[i].attack.nano_time) <= self.window);
        if result == "DISABLED" {
            if let Some(k) = self.last {
                let a = &self.attempts[k];
                let prev = a.evidence.as_ref().unwrap();
                if a.result == Some("BLOCKED")
                    && (axe(&a.attack) || self.incoming && weapon(&a.attack).is_none())
                    && (0..=1_000_000).contains(&delta(e.nano_time, prev.nano_time))
                    && delta(e.sequence, prev.sequence) == 1
                    && e.client_tick.is_some()
                    && e.client_tick == prev.client_tick
                    && e.context
                        .as_ref()
                        .and_then(|c| c.distance)
                        .zip(prev.context.as_ref().and_then(|c| c.distance))
                        .is_some_and(|(a, b)| (a - b).abs() <= 0.001)
                {
                    self.resolve(k, e, result);
                    return Some(k);
                }
            }
        }
        let at = self.pending.iter().position(|&i| {
            let a = &self.attempts[i].attack;
            let age = delta(e.nano_time, a.nano_time);
            age >= if self.incoming { -self.window } else { 0 }
                && age <= self.window
                && (self.incoming || e.kind != "HURT" || a.target == e.target)
                && (result != "DISABLED" || axe(a) || self.incoming && weapon(a).is_none())
        })?;
        let k = self.pending.remove(at).unwrap();
        self.resolve(k, e, result);
        Some(k)
    }
}
pub(crate) fn boundaries(f: &FightRecord) -> Vec<i64> {
    let mut b = BTreeSet::new();
    for e in &f.statistics.combat_exchanges {
        b.insert(e.start_nanos);
        if !matches!(e.ended_by.as_deref(), Some("ACTIVE" | "RECORDING_END")) {
            b.insert(e.end_nanos);
        }
    }
    for r in &f.evidence().rounds {
        b.insert(r.start_nanos);
        if let Some(end) = r.end_nanos.filter(|_| r.outcome.is_some()) {
            b.insert(end);
        }
    }
    b.into_iter().collect()
}
pub(crate) struct DefenseResult {
    pub outcomes: Vec<super::strict::Defense>,
    pub stuns: i32,
    pub damage: BTreeSet<i64>,
    pub follow_ups: BTreeMap<i64, i64>,
    pub not_counted: BTreeSet<i64>,
}
pub(crate) fn defense(
    events: &[CombatEvent],
    opponent: Option<&str>,
    millis: f64,
    bounds: &[i64],
) -> DefenseResult {
    let mut d = DefenseResult {
        outcomes: Vec::new(),
        stuns: 0,
        damage: BTreeSet::new(),
        follow_ups: BTreeMap::new(),
        not_counted: BTreeSet::new(),
    };
    let Some(opponent) = opponent else {
        return d;
    };
    let mut swing_ids = BTreeSet::new();
    let swings: Vec<_> = events
        .iter()
        .filter(|e| swing_ids.insert(e.sequence))
        .filter(|e| {
            e.kind == "ATTACK"
                && e.actor.as_deref() == Some(opponent)
                && e.target.as_deref() == Some("local")
        })
        .collect();
    let mut m = Matcher::new(millis, true);
    let (mut next, mut boundary) = (0, 0);
    let mut seen = BTreeSet::new();
    for e in events {
        if !seen.insert(e.sequence) {
            continue;
        }
        let damage = analytics::attributed(e, Some(opponent));
        let sound = e.kind == "SHIELD"
            && e.context.as_ref().and_then(|c| c.subject.as_deref()) == Some("local");
        if !damage && !sound {
            continue;
        }
        while boundary < bounds.len() && e.nano_time >= bounds[boundary] {
            m.boundary();
            boundary += 1;
        }
        let begin = if boundary == 0 {
            i64::MIN
        } else {
            bounds[boundary - 1]
        };
        let end = bounds.get(boundary).copied().unwrap_or(i64::MAX);
        while next < swings.len()
            && swings[next].nano_time as i128 <= e.nano_time as i128 + m.window
            && swings[next].nano_time < end
        {
            let swing = swings[next];
            next += 1;
            m.add(swing);
            if swing.nano_time < begin {
                m.boundary();
            }
        }
        let result = if damage {
            "HIT"
        } else if e.context.as_ref().and_then(|c| c.shield.as_deref()) == Some("DISABLE") {
            "DISABLED"
        } else {
            "BLOCKED"
        };
        if m.matched(e, result).is_none() && damage {
            let mut seed = e.clone();
            seed.kind = "ATTACK".into();
            seed.actor = Some(opponent.into());
            seed.target = Some("local".into());
            seed.attack = None;
            m.add(&seed);
            m.matched(e, result);
        }
    }
    for swing in &swings[next..] {
        while boundary < bounds.len() && swing.nano_time >= bounds[boundary] {
            m.boundary();
            boundary += 1;
        }
        m.add(swing);
    }
    for k in m.order.iter().copied() {
        let a = &m.attempts[k];
        let Some(result) = a.result else {
            d.not_counted.insert(a.attack.sequence);
            continue;
        };
        if let Some(p) = a.parent {
            d.follow_ups
                .insert(a.attack.sequence, m.attempts[p].attack.sequence);
            if let Some(e) = a.evidence.as_ref().filter(|e| e.kind == "DAMAGE_IN") {
                d.damage.insert(e.sequence);
            }
            continue;
        }
        if a.child.is_some_and(|c| m.attempts[c].result == Some("HIT")) {
            d.stuns += 1;
        }
        if a.attack.context.as_ref().and_then(|c| c.target_blocking) != Some(true)
            && !matches!(result, "BLOCKED" | "DISABLED")
        {
            continue;
        }
        d.outcomes.push(super::strict::Defense {
            sequence: a.attack.sequence,
            time: a.attack.nano_time,
            result: if result == "HIT" {
                "NOT_BLOCKED"
            } else {
                result
            },
        });
    }
    d
}

/// Adjacent empty/real observations may share an input, never distinct real packets.
struct Observations {
    anchor: Option<CombatEvent>,
    sample: Option<CombatEvent>,
    sample_eligible: bool,
}
impl Observations {
    fn new() -> Self {
        Self {
            anchor: None,
            sample: None,
            sample_eligible: false,
        }
    }
    fn observe(
        &mut self,
        e: &CombatEvent,
        valid: bool,
        s: &mut Statistics,
    ) -> (CombatEvent, bool, bool) {
        s.total_inputs += 1;
        let duplicate = self.anchor.as_ref().is_some_and(|a| {
            a.sequence == e.sequence
                || empty(a) != empty(e)
                    && a.client_tick.is_some()
                    && a.client_tick == e.client_tick
                    && a.nano_time == e.nano_time
                    && a.target == e.target
                    && weapon(a) == weapon(e)
        });
        if !duplicate {
            self.anchor = Some(e.clone());
            self.sample = Some(e.clone());
            self.sample_eligible = valid && eligible(e);
            s.unique_attacks += 1;
        } else {
            s.deduplicated_attacks += 1;
        }
        let id = self.anchor.as_ref().unwrap().sequence;
        s.attack_canonical_ids.insert(e.sequence, id);
        let was = s
            .attack_classifications
            .get(&id)
            .is_some_and(|c| c != "EXCLUDED");
        let mut upgraded = false;
        if valid
            && eligible(e)
            && (!self.sample_eligible || empty(self.sample.as_ref().unwrap()) && !empty(e))
        {
            upgraded = self.sample_eligible && was;
            self.sample = Some(e.clone());
            self.sample_eligible = true;
        }
        let sample = self.sample.as_ref().unwrap();
        s.attack_samples.insert(id, sample.sequence);
        if !s.attack_classifications.contains_key(&id)
            || !was
            || upgraded
                && !matches!(
                    s.attack_classifications.get(&id).map(String::as_str),
                    Some("HIT" | "DISABLED")
                )
        {
            let result =
                if !self.sample_eligible {
                    "EXCLUDED"
                } else if sample.context.as_ref().is_some_and(|c| {
                    c.target_blocking == Some(true) && c.target_facing == Some(true)
                }) {
                    "BLOCKED"
                } else {
                    "MISS"
                };
            s.attack_classifications.insert(id, result.into());
        }
        let mut a = self.anchor.as_ref().unwrap().clone();
        a.target = sample.target.clone();
        a.attack = sample.attack.clone();
        a.detail = sample.detail.clone();
        a.context = sample.context.clone();
        (a, duplicate, upgraded)
    }
}
pub(crate) fn replayable(f: &FightRecord) -> bool {
    let outgoing: Vec<_> = f.events.iter().filter(|e| analytics::outgoing(e)).collect();
    f.schema_version >= 6
        && f.opponent.is_some()
        && !outgoing.is_empty()
        && f.statistics.total_inputs as usize <= outgoing.len()
        && outgoing
            .iter()
            .all(|e| e.client_tick.is_some() && (empty(e) || weapon(e).is_some()))
}
pub(crate) fn replay(f: &mut FightRecord) {
    let bounds = boundaries(f);
    let policy = f.policy().clone();
    let opponent = f.opponent.as_deref();
    let mut s = Statistics {
        strict_version: 3,
        measurement_version: 1,
        combat_exchanges: f.statistics.combat_exchanges.clone(),
        exchange_timeout_seconds: f.statistics.exchange_timeout_seconds,
        damage_taken: f.statistics.damage_taken,
        ..Default::default()
    };
    let mut obs = Observations::new();
    let mut m = Matcher::new(policy.correlation_millis, false);
    let mut seen = BTreeSet::new();
    let mut nodes = BTreeMap::new();
    let mut boundary = 0;
    for e in &f.events {
        if !seen.insert(e.sequence) {
            continue;
        }
        while boundary < bounds.len() && e.nano_time >= bounds[boundary] {
            m.boundary();
            boundary += 1;
        }
        if analytics::outgoing(e) {
            let (a, duplicate, upgraded) = obs.observe(e, e.target.as_deref() == opponent, &mut s);
            if (!duplicate || upgraded)
                && eligible(&a)
                && a.target.as_deref() == opponent
                && !empty(&a)
            {
                nodes.insert(a.sequence, m.add(&a));
            }
        } else if e.kind == "HURT"
            && e.confidence == "CONFIRMED"
            && e.actor.as_deref() == Some("local")
            && e.detail.as_deref() == Some("local-melee-source")
        {
            m.matched(e, "HIT");
        } else if e.kind == "SHIELD"
            && e.context.as_ref().and_then(|c| c.subject.as_deref()) == Some("opponent")
        {
            match e.context.as_ref().and_then(|c| c.shield.as_deref()) {
                Some("DISABLE") => {
                    m.matched(e, "DISABLED");
                }
                Some("BLOCK") => {
                    m.matched(e, "BLOCKED");
                }
                _ => {}
            }
        }
    }
    for (&id, &k) in &nodes {
        let a = &m.attempts[k];
        if let Some(p) = a.parent {
            s.attack_classifications.remove(&id);
            s.stun_follow_ups.insert(id, m.attempts[p].attack.sequence);
            s.attack_canonical_ids
                .insert(id, m.attempts[p].attack.sequence);
            if let Some(e) = a.evidence.as_ref().filter(|e| e.kind == "HURT") {
                s.stun_outgoing_damage.insert(e.sequence);
            }
        } else if let Some(result) = a.result {
            s.attack_classifications.insert(id, result.into());
        }
        if a.child.is_some() {
            s.stuns += 1;
        }
    }
    let hit_evidence: BTreeSet<_> = nodes
        .values()
        .filter_map(|&k| {
            let a = &m.attempts[k];
            (a.parent.is_none() && a.result == Some("HIT"))
                .then(|| a.evidence.as_ref().unwrap().sequence)
        })
        .collect();
    let mut prior = BTreeMap::new();
    boundary = 0;
    for e in &f.events {
        while boundary < bounds.len() && e.nano_time >= bounds[boundary] {
            prior.clear();
            boundary += 1;
        }
        if let Some(&k) = nodes.get(&e.sequence) {
            let a = &m.attempts[k];
            if a.parent.is_none()
                && a.result.is_none()
                && s.attack_classifications
                    .get(&a.attack.sequence)
                    .is_some_and(|r| r == "MISS")
                && spam(&a.attack, prior.get(&a.attack.target).copied())
            {
                s.attack_classifications.remove(&a.attack.sequence);
                s.spam_click_sequences.insert(a.attack.sequence);
            }
        }
        if hit_evidence.contains(&e.sequence) {
            prior.insert(e.target.clone(), e);
        }
    }
    s.spam_clicks = s.spam_click_sequences.len() as i32;
    // Rebuild per-attempt timing from counted inputs, using complete weapon keys.
    let mut copy = f.clone();
    copy.statistics = s.clone();
    let canonical = analytics::canonical_events(&copy);
    let (mut previous_weapon, mut previous_tick, mut cooldown) = (None::<String>, -1, 0.0);
    boundary = 0;
    for e in canonical.iter().filter(|e| {
        analytics::outgoing(e)
            && eligible(e)
            && s.attack_classifications
                .get(&e.sequence)
                .is_none_or(|c| c != "EXCLUDED")
    }) {
        while boundary < bounds.len() && e.nano_time >= bounds[boundary] {
            previous_weapon = None;
            previous_tick = -1;
            boundary += 1;
        }
        let a = e.attack.as_ref().unwrap();
        if a.ray_contact {
            s.contacts += 1;
        }
        if e.context.as_ref().and_then(|c| c.target_blocking) == Some(true) {
            s.attacks_into_shield += 1;
        }
        if let (Some(measure), Some(tick)) = (&a.measurement, e.client_tick) {
            let key = measure.weapon.clone();
            let next = measure.cooldown_ticks;
            let timing = if key.is_none() || !next.is_finite() || next <= 0. {
                previous_weapon = None;
                previous_tick = -1;
                TimingResult {
                    grade: Some("UNAVAILABLE".into()),
                    ..Default::default()
                }
            } else {
                let interval = if previous_tick < 0 {
                    0.
                } else {
                    delta(tick, previous_tick) as f64
                };
                let mut grade = "OPENER";
                if previous_weapon == key
                    && previous_tick >= 0
                    && interval >= 0.
                    && interval <= 2. * cooldown
                {
                    grade = if cooldown <= policy.timing_tolerance_ticks.max(1.) {
                        "BELOW_RESOLUTION"
                    } else if (interval - cooldown).abs() <= policy.timing_tolerance_ticks {
                        "PERFECT"
                    } else if interval < cooldown {
                        "TOO_FAST"
                    } else {
                        "TOO_SLOW"
                    };
                }
                let expected = if grade == "OPENER" { next } else { cooldown };
                previous_weapon = key;
                previous_tick = tick;
                cooldown = next;
                TimingResult {
                    grade: Some(grade.into()),
                    interval_ticks: interval,
                    expected_ticks: expected,
                    ..Default::default()
                }
            };
            s.attack_timings.insert(e.sequence, timing);
        }
    }
    let by_evidence: BTreeMap<_, _> = nodes
        .values()
        .filter_map(|&k| {
            let a = &m.attempts[k];
            if a.parent.is_none() {
                a.evidence.as_ref().map(|e| (e.sequence, k))
            } else {
                None
            }
        })
        .collect();
    let defensive = defense(&f.events, opponent, policy.correlation_millis, &bounds);
    let crits: BTreeSet<_> = f
        .events
        .iter()
        .filter(|e| {
            e.kind == "CRIT"
                && e.context.as_ref().and_then(|c| c.subject.as_deref()) == Some("opponent")
        })
        .filter_map(|e| e.client_tick)
        .collect();
    let mut evaluations = Vec::new();
    boundary = 0;
    for e in &f.events {
        while boundary < bounds.len() && e.nano_time >= bounds[boundary] {
            s.current_combo = 0;
            s.momentum = 0.;
            boundary += 1;
        }
        if analytics::attributed(e, opponent) && !defensive.damage.contains(&e.sequence) {
            s.incoming += 1;
            s.current_combo = 0;
            s.momentum = (momentum(&s, e.nano_time) - 0.25).max(-1.);
            s.momentum_at = e.nano_time;
        }
        let Some(&k) = by_evidence.get(&e.sequence) else {
            continue;
        };
        let a = &m.attempts[k];
        let mut grade = None;
        if a.result == Some("HIT") {
            hit(
                &mut s,
                a.attack.attack.as_ref().unwrap(),
                a.attack.nano_time,
                e.nano_time,
                &policy,
            );
            if e.client_tick.is_some_and(|t| {
                (-1..=1).any(|d| t.checked_add(d).is_some_and(|t| crits.contains(&t)))
            }) {
                s.server_crits += 1;
            }
            let attack = a.attack.attack.as_ref().unwrap();
            if attack.ray_contact
                && analytics::comparable(attack).is_some_and(|r| {
                    (r - policy.perfect_reach_target).abs() <= policy.perfect_reach_tolerance + 1e-9
                })
            {
                s.perfect_reach_hits += 1;
            }
            if attack.measurement.is_none() {
                grade = Some(
                    if attack.cooldown >= policy.perfect_cooldown {
                        "PERFECT"
                    } else if attack.cooldown >= policy.great_cooldown {
                        "GREAT"
                    } else if attack.cooldown >= 0.5 {
                        "NORMAL"
                    } else {
                        "POOR"
                    }
                    .into(),
                );
            }
        }
        evaluations.push(Evaluation {
            attack_sequence: a.attack.sequence,
            evidence_sequence: e.sequence,
            readiness: grade,
            confidence: Some("CONFIRMED".into()),
            combo: if a.result == Some("HIT") {
                s.current_combo
            } else {
                0
            },
            momentum: s.momentum,
            result: a.result.map(Into::into),
            ..Default::default()
        });
    }
    f.statistics = s;
    f.evaluations = evaluations;
}
fn momentum(s: &Statistics, time: i64) -> f64 {
    s.momentum * (-(delta(time, s.momentum_at).max(0) as f64) / 3e9).exp()
}
fn hit(s: &mut Statistics, a: &Attack, time: i64, evidence: i64, p: &Policy) {
    if let Some(reach) = analytics::comparable(a) {
        s.reach_sum += reach;
        s.reach_count += 1;
        s.max_reach = s.max_reach.max(reach);
        s.reach_distribution[(reach as usize).min(7)] += 1;
        let i = s.recent_reach_index as usize % 16;
        s.recent_reach[i] = reach;
        s.recent_reach_index += 1;
        s.recent_reach_count = (s.recent_reach_count + 1).min(16);
    }
    if a.ray_contact {
        if a.aim_y > 0.75 {
            s.high_aim += 1;
        } else if a.aim_y < 0.25 {
            s.low_aim += 1;
        } else if (analytics::face_horizontal(a) - 0.5).abs() > 0.35 {
            s.edge_aim += 1;
        } else {
            s.center_aim += 1;
        }
    }
    if a.critical_conditions {
        s.critical_conditions += 1;
    }
    if a.sprinting {
        s.sprint_hits += 1;
    }
    s.cooldown_sum += a.cooldown;
    if s.last_hit > 0 {
        let interval = delta(time, s.last_hit) as f64 / 1e9;
        s.recent_interval = interval;
        s.interval_sum += interval;
        s.interval_squared += interval * interval;
        s.interval_count += 1;
        if interval > p.combo_seconds {
            s.current_combo = 0;
        }
    }
    if s.current_combo == 0 {
        s.combo_start = time;
    }
    s.current_combo += 1;
    s.longest_combo = s.longest_combo.max(s.current_combo);
    s.last_hit = time;
    s.momentum = (momentum(s, evidence) + 0.2).min(1.);
    s.momentum_at = evidence;
}

pub(crate) fn pool(f: &mut FightRecord) {
    let Some(session) = &f.session else {
        return;
    };
    if f.schema_version < 6
        || f.events.is_empty()
        || session.exchanges.is_empty()
        || session.exchanges.iter().any(|c| c.events.is_empty())
    {
        return;
    }
    let mut p = Statistics::default();
    let mut evaluations = Vec::new();
    let mut counted = BTreeSet::new();
    for child in &session.exchanges {
        let s = &child.statistics;
        p.spam_clicks += s.spam_clicks;
        p.not_counted_incoming_swings += s.not_counted_incoming_swings;
        p.stuns += s.stuns;
        p.local_stuns += s.local_stuns;
        p.non_melee_damage += s.non_melee_damage;
        p.local_shield_not_blocked += s.local_shield_not_blocked;
        p.total_inputs += s.total_inputs;
        p.unique_attacks += s.unique_attacks;
        p.deduplicated_attacks += s.deduplicated_attacks;
        p.excluded_swings += s.excluded_swings;
        p.eligible_misses += s.eligible_misses;
        p.uncertain_shield += s.uncertain_shield;
        p.attempts += s.attempts;
        p.hits += s.hits;
        p.empty_misses += s.empty_misses;
        p.uncertain += s.uncertain;
        p.contacts += s.contacts;
        p.critical_conditions += s.critical_conditions;
        p.sprint_hits += s.sprint_hits;
        p.jump_candidates += s.jump_candidates;
        p.incoming += s.incoming;
        p.perfect_reach_hits += s.perfect_reach_hits;
        p.unattributed_incoming += s.unattributed_incoming;
        p.third_party_incoming += s.third_party_incoming;
        p.attacks_into_shield += s.attacks_into_shield;
        p.shield_blocks += s.shield_blocks;
        p.estimated_shield_blocks += s.estimated_shield_blocks;
        p.shield_disables += s.shield_disables;
        p.local_shield_blocks += s.local_shield_blocks;
        p.local_shield_disabled += s.local_shield_disabled;
        p.server_crits += s.server_crits;
        p.opponent_crits += s.opponent_crits;
        p.combo_count += s.combo_count;
        p.combo_hits += s.combo_hits;
        p.damage_taken += s.damage_taken;
        p.reach_sum += s.reach_sum;
        p.reach_count += s.reach_count;
        p.cooldown_sum += s.cooldown_sum;
        p.interval_sum += s.interval_sum;
        p.interval_squared += s.interval_squared;
        p.interval_count += s.interval_count;
        p.center_aim += s.center_aim;
        p.edge_aim += s.edge_aim;
        p.high_aim += s.high_aim;
        p.low_aim += s.low_aim;
        p.strict_version = p.strict_version.max(s.strict_version);
        p.measurement_version = p.measurement_version.max(s.measurement_version);
        p.longest_combo = p.longest_combo.max(s.longest_combo);
        p.longest_opponent_combo = p.longest_opponent_combo.max(s.longest_opponent_combo);
        p.longest_unanswered = p.longest_unanswered.max(s.longest_unanswered);
        p.max_reach = p.max_reach.max(s.max_reach);
        p.spam_click_sequences
            .extend(s.spam_click_sequences.clone());
        p.not_counted_incoming_sequences
            .extend(s.not_counted_incoming_sequences.clone());
        p.stun_follow_ups.extend(s.stun_follow_ups.clone());
        p.stun_incoming_damage
            .extend(s.stun_incoming_damage.clone());
        p.stun_outgoing_damage
            .extend(s.stun_outgoing_damage.clone());
        p.stun_incoming_follow_ups
            .extend(s.stun_incoming_follow_ups.clone());
        p.attack_canonical_ids
            .extend(s.attack_canonical_ids.clone());
        p.attack_samples.extend(s.attack_samples.clone());
        p.attack_classifications
            .extend(s.attack_classifications.clone());
        p.attack_timings.extend(s.attack_timings.clone());
        p.combat_exchanges.extend(s.combat_exchanges.clone());
        for (a, b) in p.reach_distribution.iter_mut().zip(&s.reach_distribution) {
            *a += b;
        }
        evaluations.extend(child.evaluations.clone());
        counted.extend(child.events.iter().map(|e| e.sequence));
    }
    p.strict_version = 3;
    p.measurement_version = 1;
    let mut orphan = Observations::new();
    for e in &f.events {
        if counted.contains(&e.sequence) {
            continue;
        }
        if analytics::outgoing(e) {
            orphan.observe(e, true, &mut p);
        } else if e.kind == "DAMAGE_IN" {
            p.non_melee_damage += 1;
        }
    }
    f.statistics = p;
    f.evaluations = evaluations;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn attack(seq: i64, tick: i64, millis: i64, w: &str) -> CombatEvent {
        CombatEvent {
            sequence: seq,
            nano_time: millis * 1_000_000,
            client_tick: Some(tick),
            kind: "ATTACK".into(),
            actor: Some("local".into()),
            target: Some("opponent".into()),
            attack: Some(Attack {
                cooldown: 1.,
                measurement: Some(Measurement {
                    weapon: Some(w.into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    fn proof(seq: i64, tick: i64, millis: i64) -> CombatEvent {
        let mut e = attack(seq, tick, millis, "axe");
        e.kind = "HURT".into();
        e.attack = None;
        e
    }
    #[test]
    fn distinct_real_packets_keep_identity_even_at_identical_time() {
        let mut s = Statistics::default();
        let mut o = Observations::new();
        let a = attack(1, 1, 0, "axe");
        let mut b = a.clone();
        b.sequence = 2;
        assert!(!o.observe(&a, true, &mut s).1);
        assert!(!o.observe(&b, true, &mut s).1);
        assert_eq!(s.unique_attacks, 2);
        let mut empty = b.clone();
        empty.sequence = 3;
        empty.detail = Some("empty-swing".into());
        assert!(o.observe(&empty, true, &mut s).1);
        assert_eq!(s.attack_canonical_ids[&3], 2);
    }
    #[test]
    fn fifo_responses_resolve_earliest_compatible_packet() {
        let mut m = Matcher::new(250., false);
        m.add(&attack(1, 1, 0, "sword"));
        m.add(&attack(2, 1, 1, "axe"));
        assert_eq!(m.matched(&proof(3, 2, 2), "DISABLED"), Some(1));
        assert_eq!(m.matched(&proof(4, 2, 3), "HIT"), Some(0));
        assert!(m.matched(&proof(5, 2, 4), "HIT").is_none());
    }
    #[test]
    fn pickaxe_cannot_disable_and_expired_evidence_is_not_reused() {
        let mut m = Matcher::new(250., false);
        m.add(&attack(1, 1, 0, "minecraft:diamond_pickaxe"));
        assert!(m.matched(&proof(2, 2, 1), "DISABLED").is_none());
        assert!(m.matched(&proof(3, 7, 251), "HIT").is_none());
    }
    #[test]
    fn stun_window_requires_both_time_and_ticks_and_absorbs_only_one() {
        let a = attack(1, 10, 0, "axe");
        assert!(follow_up(&a, &attack(2, 13, 150, "sword")));
        assert!(!follow_up(&a, &attack(2, 14, 150, "sword")));
        assert!(!follow_up(&a, &attack(2, 13, 151, "sword")));
        let mut m = Matcher::new(250., false);
        m.add(&a);
        m.add(&attack(2, 11, 50, "sword"));
        m.add(&attack(3, 12, 100, "sword"));
        m.matched(&proof(4, 13, 110), "DISABLED");
        assert_eq!(m.attempts[0].child, Some(1));
        assert_eq!(m.attempts[1].parent, Some(0));
        assert_eq!(m.attempts[2].parent, None);
    }
    #[test]
    fn separate_shield_response_detaches_child_without_replacing_it() {
        let mut m = Matcher::new(250., false);
        m.add(&attack(1, 1, 0, "axe"));
        m.add(&attack(2, 2, 50, "sword"));
        m.add(&attack(3, 3, 100, "axe"));
        m.matched(&proof(4, 3, 110), "DISABLED");
        m.matched(&proof(5, 3, 120), "BLOCKED");
        assert_eq!(m.attempts[0].child, None);
        assert_eq!(m.attempts[1].parent, None);
        assert_eq!(m.attempts[2].parent, None);
    }
    #[test]
    fn spam_thresholds_and_confirmation_order_are_exact() {
        let h = proof(1, 10, 0);
        let mut a = attack(2, 20, 999, "axe");
        assert!(spam(&a, Some(&h)));
        a.client_tick = Some(21);
        assert!(!spam(&a, Some(&h)));
        a.attack.as_mut().unwrap().cooldown = 0.49;
        assert!(spam(&a, None));
        a.attack.as_mut().unwrap().cooldown = 0.5;
        assert!(!spam(&a, None));
        a.sequence = 1;
        assert!(!spam(&a, Some(&h)));
        a.sequence = 2;
        a.client_tick = None;
        a.nano_time = 500_000_000;
        assert!(spam(&a, Some(&h)));
        a.nano_time += 1;
        assert!(!spam(&a, Some(&h)));
        a.detail = Some("empty-swing".into());
        a.attack.as_mut().unwrap().cooldown = 0.;
        assert!(!spam(&a, None));
    }
    #[test]
    fn boundaries_clear_matching_and_stun_links() {
        let mut m = Matcher::new(250., false);
        m.add(&attack(1, 1, 0, "axe"));
        m.matched(&proof(2, 1, 1), "DISABLED");
        m.boundary();
        m.add(&attack(3, 2, 50, "sword"));
        assert_eq!(m.attempts[1].parent, None);
        m.boundary();
        assert!(m.matched(&proof(4, 2, 60), "HIT").is_none());
    }
}
