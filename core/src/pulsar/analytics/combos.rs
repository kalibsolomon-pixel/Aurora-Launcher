use super::super::wire::*;
use super::{attributed, hit_evaluation};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub(crate) struct Mark {
    pub seq: i64,
    pub nanos: i64,
    pub tick: Option<i64>,
    pub local: bool,
    pub traded: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct Streak {
    pub local: bool,
    pub seqs: Vec<i64>,
    pub start: i64,
    pub end: i64,
    pub ended_by: &'static str,
}
#[derive(Debug, Clone, Default)]
struct Exchange {
    start: i64,
    end: i64,
    local: usize,
    opponent: usize,
    longest_local: usize,
    longest_opponent: usize,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct Combos {
    pub marks: Vec<Mark>,
    pub streaks: Vec<Streak>,
    exchanges: Vec<Exchange>,
    pub longest_unanswered: usize,
}
impl Combos {
    pub(crate) fn count(&self) -> usize {
        self.streaks
            .iter()
            .filter(|s| s.local && s.seqs.len() >= 2)
            .count()
    }
    pub(crate) fn hits(&self) -> usize {
        self.streaks
            .iter()
            .filter(|s| s.local && s.seqs.len() >= 2)
            .map(|s| s.seqs.len())
            .sum()
    }
    pub(crate) fn longest(&self, local: bool) -> usize {
        self.streaks
            .iter()
            .filter(|s| s.local == local)
            .map(|s| s.seqs.len())
            .max()
            .unwrap_or(0)
    }
    pub(crate) fn average(&self) -> Option<f64> {
        super::ratio(self.hits() as f64, self.count() as f64)
    }
    pub(crate) fn payload(&self, f: &FightRecord) -> Value {
        json!({"marks":self.marks.iter().map(|m|json!({"seq":m.seq,"t":f.seconds(m.nanos),"side":side(m.local),"traded":m.traded})).collect::<Vec<_>>(),
            "streaks":self.streaks.iter().map(|s|json!({"side":side(s.local),"length":s.seqs.len(),"start":f.seconds(s.start),"end":f.seconds(s.end),"seqs":s.seqs,"endedBy":s.ended_by})).collect::<Vec<_>>(),
            "exchanges":self.exchanges.iter().map(|e|json!({"start":f.seconds(e.start),"end":f.seconds(e.end),"local":e.local,"opponent":e.opponent,"longestLocal":e.longest_local,"longestOpponent":e.longest_opponent})).collect::<Vec<_>>(),"gap":f.policy().combo_seconds})
    }
}
pub(crate) fn side(local: bool) -> &'static str {
    if local { "LOCAL" } else { "OPPONENT" }
}
fn finish(
    current: &mut Option<Streak>,
    ended_by: &'static str,
    streaks: &mut Vec<Streak>,
    exchange: &mut Exchange,
) {
    if let Some(mut s) = current.take() {
        s.ended_by = ended_by;
        if s.local {
            exchange.longest_local = exchange.longest_local.max(s.seqs.len());
        } else {
            exchange.longest_opponent = exchange.longest_opponent.max(s.seqs.len());
        }
        streaks.push(s);
    }
}
pub(crate) fn analyze(f: &FightRecord, events: &[CombatEvent]) -> Combos {
    if f.session.is_some() {
        return Combos::default();
    }
    let by_seq: BTreeMap<i64, &CombatEvent> = events.iter().map(|e| (e.sequence, e)).collect();
    let mut result = Combos::default();
    for ev in f.evaluations.iter().filter(|e| hit_evaluation(e)) {
        if let Some(at) = by_seq
            .get(&ev.evidence_sequence)
            .or_else(|| by_seq.get(&ev.attack_sequence))
        {
            result.marks.push(Mark {
                seq: ev.attack_sequence,
                nanos: at.nano_time,
                tick: at.client_tick,
                local: true,
                traded: false,
            });
        }
    }
    let mut opponent_ticks = BTreeSet::new();
    for e in events
        .iter()
        .filter(|e| attributed(e, f.opponent.as_deref()))
    {
        if let Some(tick) = e.client_tick {
            opponent_ticks.insert(tick);
        }
        result.marks.push(Mark {
            seq: e.sequence,
            nanos: e.nano_time,
            tick: e.client_tick,
            local: false,
            traded: false,
        });
    }
    result.marks.sort_by_key(|m| (m.nanos, m.seq));
    let local_ticks: BTreeSet<i64> = result
        .marks
        .iter()
        .filter(|m| m.local)
        .filter_map(|m| m.tick)
        .collect();
    for m in &mut result.marks {
        m.traded = m.tick.is_some_and(|t| {
            if m.local {
                opponent_ticks.contains(&t)
            } else {
                local_ticks.contains(&t)
            }
        });
    }
    let mut boundaries: Vec<i64> = f
        .evidence()
        .rounds
        .iter()
        .flat_map(|r| std::iter::once(r.start_nanos).chain(r.end_nanos))
        .collect();
    boundaries.sort();
    boundaries.dedup();
    let mut exchange_boundaries: Vec<i64> = f
        .statistics
        .combat_exchanges
        .iter()
        .flat_map(|e| [e.start_nanos, e.end_nanos])
        .collect();
    exchange_boundaries.sort();
    exchange_boundaries.dedup();
    let mut current: Option<Streak> = None;
    let mut exchange = Exchange {
        start: -1,
        ..Default::default()
    };
    let mut last: Option<i64> = None;
    let mut run_side = None;
    let mut run = 0;
    let mut boundary = 0;
    let mut ex_boundary = 0;
    for m in &result.marks {
        let mut round = false;
        let mut measurement = false;
        while ex_boundary < exchange_boundaries.len() && exchange_boundaries[ex_boundary] <= m.nanos
        {
            measurement |= last.is_some_and(|t| exchange_boundaries[ex_boundary] > t);
            ex_boundary += 1;
        }
        while boundary < boundaries.len() && boundaries[boundary] <= m.nanos {
            round |= last.is_some_and(|t| boundaries[boundary] > t);
            boundary += 1;
        }
        if round {
            run = 0;
            run_side = None;
        }
        if last.is_some_and(|t| {
            round
                || measurement
                || (m.nanos as i128 - t as i128) > (f.policy().combo_seconds * 1e9) as i128
        }) {
            finish(
                &mut current,
                if round { "ROUND_BOUNDARY" } else { "DISENGAGE" },
                &mut result.streaks,
                &mut exchange,
            );
            result.exchanges.push(exchange);
            exchange = Exchange {
                start: -1,
                ..Default::default()
            };
        }
        if exchange.start < 0 {
            exchange.start = m.nanos;
        }
        exchange.end = m.nanos;
        last = Some(m.nanos);
        if m.local {
            exchange.local += 1;
        } else {
            exchange.opponent += 1;
        }
        if m.traded {
            run = 0;
            run_side = None;
        } else if run_side == Some(m.local) {
            run += 1;
        } else {
            run_side = Some(m.local);
            run = 1;
        }
        if run_side == Some(true) {
            result.longest_unanswered = result.longest_unanswered.max(run);
        }
        if m.traded {
            finish(&mut current, "TRADE", &mut result.streaks, &mut exchange);
            continue;
        }
        if current.as_ref().is_some_and(|s| s.local != m.local) {
            finish(
                &mut current,
                "RETALIATION",
                &mut result.streaks,
                &mut exchange,
            );
        }
        let s = current.get_or_insert_with(|| Streak {
            local: m.local,
            seqs: Vec::new(),
            start: m.nanos,
            end: m.nanos,
            ended_by: "RECORDING_END",
        });
        s.seqs.push(m.seq);
        s.end = m.nanos;
    }
    finish(
        &mut current,
        "RECORDING_END",
        &mut result.streaks,
        &mut exchange,
    );
    if exchange.start >= 0 {
        result.exchanges.push(exchange);
    }
    result
}
