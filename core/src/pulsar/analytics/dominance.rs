use super::super::wire::FightRecord;
use super::{Combos, num, ratio};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone)]
pub(crate) struct Dominance {
    pub step: f64,
    pub values: Vec<Option<f64>>,
    pub states: Vec<char>,
    pub active: f64,
    pub local: f64,
    pub opponent: f64,
    pub neutral: f64,
    pub insufficient: f64,
    pub availability: &'static str,
    pub changes: usize,
    pub longest: Value,
    pub swing: Value,
    pub turning: Vec<[f64; 2]>,
}
impl Dominance {
    pub(crate) fn payload(&self) -> Value {
        json!({"step":self.step,"values":self.values.iter().map(|v|v.map(|v|(v*1e4+0.5).floor()/1e4)).collect::<Vec<_>>(),"states":self.states.iter().collect::<String>(),"availability":self.availability,"availabilityLabel":match self.availability{"AVAILABLE"=>"Available",_=>"Insufficient evidence"},"active":self.active,"localShare":ratio(100.0*self.local,self.active),"opponentShare":ratio(100.0*self.opponent,self.active),"neutralShare":ratio(100.0*self.neutral,self.active),"insufficient":self.insufficient,"changes":self.changes,"longest":self.longest,"swing":self.swing,"turningPoints":self.turning,"model":{"tau":3.0,"threshold":0.2,"minMass":0.35,"hit":1.0,"streakBonus":0.25,"shieldBlocked":0.25,"shieldDisabled":0.5,"localBlock":0.25}})
    }
}
pub(crate) fn analyze(f: &FightRecord, combos: &Combos, shields: &[Value]) -> Dominance {
    let mut inputs: Vec<(f64, f64, &str)> = Vec::new();
    let mut weights = BTreeMap::new();
    for s in &combos.streaks {
        for (k, seq) in s.seqs.iter().enumerate() {
            weights.insert(
                *seq,
                (if s.local { 1.0 } else { -1.0 }) * (1.0 + if k > 0 { 0.25 } else { 0.0 }),
            );
        }
    }
    for m in &combos.marks {
        inputs.push((
            f.seconds(m.nanos),
            *weights
                .get(&m.seq)
                .unwrap_or(&if m.local { 1.0 } else { -1.0 }),
            "HIT",
        ));
    }
    for s in shields {
        let weight = match (
            s["type"].as_str().unwrap_or(""),
            s["subject"].as_str().unwrap_or(""),
        ) {
            ("BLOCK", "opponent") => -0.25,
            ("DISABLE", "opponent") => 0.5,
            ("BLOCK", "local") => 0.25,
            ("DISABLE", "local") => -0.5,
            _ => continue,
        };
        inputs.push((num(s, "time"), weight, "SHIELD"));
    }
    for e in &f.statistics.combat_exchanges {
        inputs.push((f.seconds(e.start_nanos), 0.0, "EXCHANGE_RESET"));
        inputs.push((f.seconds(e.end_nanos), 0.0, "EXCHANGE_RESET"));
    }
    inputs.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then((a.2 != "EXCHANGE_RESET").cmp(&(b.2 != "EXCHANGE_RESET")))
    });
    if f.session.is_some() {
        inputs.clear();
    }
    let input_count = inputs.iter().filter(|i| i.1 != 0.0).count();
    let span = f.duration().max(inputs.last().map(|i| i.0).unwrap_or(0.0));
    let step = 0.05_f64.max(span / 2000.0);
    let n = ((span / step).ceil() as usize + 1).max(2);
    let mut d = Dominance {
        step,
        values: Vec::with_capacity(n),
        states: Vec::with_capacity(n),
        active: 0.0,
        local: 0.0,
        opponent: 0.0,
        neutral: 0.0,
        insufficient: 0.0,
        availability: "INSUFFICIENT",
        changes: 0,
        longest: Value::Null,
        swing: Value::Null,
        turning: Vec::new(),
    };
    let (mut p, mut q, mut at) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut i, mut interval) = (0, 0);
    for k in 0..n {
        let t = k as f64 * step;
        while i < inputs.len() && inputs[i].0 <= t {
            let (time, weight, kind) = inputs[i];
            i += 1;
            let decay = (-(time - at).max(0.0) / 3.0).exp();
            p *= decay;
            q *= decay;
            at = at.max(time);
            match kind {
                "EXCHANGE_RESET" => {
                    p = 0.0;
                    q = 0.0;
                }
                _ => {
                    if weight > 0.0 {
                        p += weight;
                    } else {
                        q -= weight;
                    }
                }
            }
        }
        let decay = (-(t - at) / 3.0).exp();
        let (pt, qt) = (p * decay, q * decay);
        let value = (pt - qt) / (pt + qt + 1.0);
        let mut state = if pt + qt < 0.35 {
            'I'
        } else if value >= 0.2 {
            'L'
        } else if value <= -0.2 {
            'O'
        } else {
            'N'
        };
        if f.statistics.measurement_version > 0 {
            while interval < f.statistics.combat_exchanges.len()
                && t >= f.seconds(f.statistics.combat_exchanges[interval].end_nanos)
            {
                interval += 1;
            }
            if interval >= f.statistics.combat_exchanges.len()
                || t < f.seconds(f.statistics.combat_exchanges[interval].start_nanos)
            {
                state = 'I';
            }
        }
        d.states.push(state);
        d.values.push(if state == 'I' { None } else { Some(value) });
        match state {
            'L' => d.local += step,
            'O' => d.opponent += step,
            'N' => d.neutral += step,
            _ => d.insufficient += step,
        }
    }
    d.active = d.local + d.opponent + d.neutral;
    let mut start = 0;
    let mut best_length = -1.0;
    let mut last = None;
    for k in 1..=n {
        if k == n || d.states[k] != d.states[start] {
            let state = d.states[start];
            let end = if k == n {
                (n - 1) as f64 * step + step
            } else {
                k as f64 * step
            };
            let from = start as f64 * step;
            if matches!(state, 'L' | 'O') && end - from > best_length {
                best_length = end - from;
                d.longest =
                    json!({"side":if state=='L'{"LOCAL"}else{"OPPONENT"},"start":from,"end":end});
            }
            start = k;
        }
    }
    for (k, state) in d.states.iter().enumerate() {
        if matches!(state, 'L' | 'O') {
            if last.is_some_and(|s| s != *state) {
                d.changes += 1;
                d.turning.push([k as f64 * step, d.values[k].unwrap()]);
            }
            last = Some(*state);
        } else if matches!(state, 'I') {
            last = None;
        }
    }
    let window = (3.0 / step + 0.5).floor() as usize;
    let (mut maxq, mut minq): (VecDeque<usize>, VecDeque<usize>) =
        (VecDeque::new(), VecDeque::new());
    let mut swing_size = -1.0;
    for b in 0..n {
        if matches!(d.states[b], 'I') {
            maxq.clear();
            minq.clear();
            continue;
        }
        while maxq.front().is_some_and(|a| *a + window < b) {
            maxq.pop_front();
        }
        while minq.front().is_some_and(|a| *a + window < b) {
            minq.pop_front();
        }
        for a in [maxq.front().copied(), minq.front().copied()]
            .into_iter()
            .flatten()
        {
            let size = (d.values[b].unwrap() - d.values[a].unwrap()).abs();
            if size > swing_size + 1e-12 {
                d.swing = json!({"from":a as f64*step,"to":b as f64*step,"fromValue":d.values[a],"toValue":d.values[b]});
                swing_size = size;
            }
        }
        while maxq.back().is_some_and(|a| d.values[*a] <= d.values[b]) {
            maxq.pop_back();
        }
        maxq.push_back(b);
        while minq.back().is_some_and(|a| d.values[*a] >= d.values[b]) {
            minq.pop_back();
        }
        minq.push_back(b);
    }
    d.availability = if d.active < 1.0 || input_count < 3 {
        "INSUFFICIENT"
    } else {
        "AVAILABLE"
    };
    d
}
