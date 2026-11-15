//! Presentation projections over canonical analytics; no filesystem access.
use super::{
    Error,
    analytics::{self, Analytics, format, metrics},
    store::Entry,
    view::*,
    wire::{FightRecord, MatchEvidence, Policy},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn array<'a>(d: &'a Value, key: &str) -> &'a [Value] {
    d[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn field(d: &Value, key: &str) -> Option<f64> {
    d[key].as_f64()
}
fn submetrics(all: &BTreeMap<String, MetricValue>, keys: &[&str]) -> BTreeMap<String, MetricValue> {
    keys.iter()
        .filter_map(|k| all.get(*k).cloned().map(|v| ((*k).into(), v)))
        .collect()
}
fn point(x: f64, y: Option<f64>, row: &Value, label: String) -> ChartPoint {
    ChartPoint {
        x,
        y,
        sequence: row["seq"].as_i64().or_else(|| row["sequence"].as_i64()),
        category: row["grade"]
            .as_str()
            .or_else(|| row["classification"].as_str())
            .or_else(|| row["type"].as_str())
            .unwrap_or("")
            .into(),
        label,
        availability: if y.is_some() {
            Availability::Available
        } else {
            Availability::Unavailable
        },
        key: row
            .get("fightKey")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
    }
}
fn chart(
    id: &str,
    title: &str,
    kind: &str,
    x_unit: &str,
    metric: &str,
    series: Vec<ChartSeries>,
    bands: Vec<ChartBand>,
    description: &str,
) -> ChartView {
    ChartView {
        id: id.into(),
        title: title.into(),
        kind: kind.into(),
        x_unit: x_unit.into(),
        y_definition: metrics::definition(metric),
        series,
        bands,
        description: description.into(),
    }
}
fn scalar_string(v: &Value) -> String {
    match v {
        Value::Null => "-".into(),
        Value::String(s) => s.chars().take(4096).collect(),
        _ => v.to_string().chars().take(4096).collect(),
    }
}
pub(crate) fn table(id: &str, title: &str, data: &Value, key: Option<&FightKey>) -> TableView {
    let rows: Vec<Value> = if let Some(rows) = data.as_array() {
        rows.iter()
            .take(2000)
            .map(|v| {
                if id == "turning_points" && v.is_array() {
                    json!({"time":v[0], "dominance":v[1]})
                } else {
                    v.clone()
                }
            })
            .collect()
    } else if let Some(rows) = data.as_object() {
        rows.iter()
            .take(2000)
            .map(|(k, v)| {
                let mut row = v.as_object().cloned().unwrap_or_default();
                row.insert("name".into(), k.clone().into());
                if !v.is_object() {
                    row.insert("value".into(), v.clone());
                }
                Value::Object(row)
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut columns = Vec::new();
    let mut view_rows = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let mut cells = BTreeMap::new();
        let mut values = BTreeMap::new();
        if let Some(obj) = r.as_object() {
            for (k, v) in obj {
                if k == "fightKey" {
                    continue;
                }
                if !columns.contains(k) {
                    columns.push(k.clone());
                }
                cells.insert(k.clone(), scalar_string(v));
                if let Some(n) = v.as_f64() {
                    let metric = if metrics::definitions().iter().any(|d| d.key == *k) {
                        metrics::metric(k, Some(n))
                    } else {
                        let unit = match k.as_str() {
                            "time" | "t" | "start" | "end" | "duration" | "from" | "to" => "s",
                            "reach" | "estimatedReach" | "distance" | "mean" | "deviation"
                            | "max" => "m",
                            "tick" | "offsetTicks" | "jumpOffsetTicks" | "interval"
                            | "expected" | "delta" => "ticks",
                            "damage" | "health" | "absorption" => "HP",
                            "knockback" | "speedAfter" => "blocks/tick",
                            "attackSpeed" => "attacks/s",
                            _ => "",
                        };
                        metrics::extra(
                            k,
                            k,
                            unit,
                            "Recorded analytical value; availability and evidence follow the selected scope.",
                            Some(n),
                        )
                    };
                    values.insert(k.clone(), metric);
                }
            }
        }
        view_rows.push(TableRow {
            id: r["seq"]
                .as_i64()
                .or_else(|| r["sequence"].as_i64())
                .map(|n| n.to_string())
                .unwrap_or_else(|| i.to_string()),
            sequence: r["seq"].as_i64().or_else(|| r["sequence"].as_i64()),
            key: r
                .get("fightKey")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .or_else(|| key.cloned()),
            cells,
            metrics: values,
        });
    }
    TableView {
        id: id.into(),
        title: title.into(),
        columns,
        rows: view_rows,
    }
}

pub(crate) fn tabs(
    f: &FightRecord,
    a: &Analytics,
    weapon: &Option<String>,
    timing_result: &Option<String>,
) -> (TabView, TabView, TabView, TabView) {
    tabs_value(f, &a.derived, weapon, timing_result)
}
fn tabs_value(
    f: &FightRecord,
    d: &Value,
    weapon: &Option<String>,
    timing_result: &Option<String>,
) -> (TabView, TabView, TabView, TabView) {
    let all = metrics::values(d);
    let mut overview = TabView {
        metrics: submetrics(
            &all,
            &[
                "accuracy",
                "hits",
                "averageReach",
                "reachWithin",
                "criticalRate",
                "averageCombo",
                "longestCombo",
                "dominanceShare",
                "averageInterval",
                "intervalDeviation",
                "fights",
            ],
        ),
        interpretations: array(d, "interpretations")
            .iter()
            .skip(1)
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        ..Default::default()
    };
    // Overview recent average reach uses eligible ray contacts, unlike History/trends.
    overview.metrics.insert(
        "averageReach".into(),
        metrics::metric("averageReach", field(&d["reach"], "mean")),
    );
    let mut offense=TabView{metrics:submetrics(&all,&["accuracy","hits","attempts","emptyMisses","uncertain","averageReach","maxReach","reachWithin","reachDeviation","reachError","aimCentral","aimDistance","timingPerfect","criticalRate","sprintRate","serverCrits"]),notices:vec!["Uncertain attacks stay in the accuracy denominator. Critical conditions do not establish damage, and sprint state does not establish knockback.".into()],..Default::default()};
    let hits: Vec<_> = array(d, "hits")
        .iter()
        .filter(|h| {
            weapon
                .as_ref()
                .is_none_or(|w| h["weapon"].as_str() == Some(w))
        })
        .cloned()
        .collect();
    let reach = analytics::reach_summary(
        &hits,
        analytics::num(&d["reach"], "target"),
        analytics::num(&d["reach"], "tolerance"),
    );
    for (key, value) in [
        ("averageReach", field(&reach, "mean")),
        ("maxReach", field(&reach, "max")),
        ("reachWithin", field(&reach, "withinPercent")),
        ("reachDeviation", field(&reach, "deviation")),
        ("reachError", field(&reach, "meanAbsoluteError")),
    ] {
        offense
            .metrics
            .insert(key.into(), metrics::metric(key, value));
    }
    for (key, label, unit, value, help) in [
        (
            "reachEligible",
            "Eligible reach samples",
            "",
            field(&reach, "eligible"),
            "Confirmed, comparable ray-contact hits only.",
        ),
        (
            "reachTarget",
            "Frozen reach target",
            "m",
            field(&reach, "target"),
            "Target recorded with this scope, independent of current preferences.",
        ),
        (
            "reachTolerance",
            "Frozen reach tolerance",
            "m",
            field(&reach, "tolerance"),
            "Inclusive target window, with a 1e-9 geometric tolerance.",
        ),
        (
            "aimDistanceDeviation",
            "Aim consistency",
            "m",
            field(&d["aim"], "distanceDeviation"),
            "Population standard deviation of distance to the frozen ideal region.",
        ),
        (
            "timingTolerance",
            "Frozen timing tolerance",
            "ticks",
            Some(f.policy().timing_tolerance_ticks),
            "Mechanical cooldown interval tolerance; not tactical timing.",
        ),
    ] {
        offense
            .metrics
            .insert(key.into(), metrics::extra(key, label, unit, help, value));
    }
    for key in [
        "totalInputs",
        "uniqueAttacks",
        "excludedSwings",
        "deduplicatedAttacks",
        "eligibleMisses",
        "uncertainShield",
        "unresolved",
    ] {
        offense.metrics.insert(key.into(),metrics::extra(key,key,"","Frozen canonical input policy. Historical recordings retain their original classification.",field(&d["summary"],key)));
    }
    let s = &d["summary"];
    offense.metrics.insert(
        "accuracyDenominator".into(),
        metrics::extra(
            "accuracyDenominator",
            "Eligible accuracy attempts",
            "count",
            "Attack attempts minus confirmed shield blocks. Uncertain results remain eligible.",
            Some(analytics::num(s, "attempts") - analytics::num(&d["shields"], "confirmedBlocks")),
        ),
    );
    if let Some(m) = offense.metrics.get_mut("eligibleMisses") {
        m.definition.label = "Canonical miss classifications".into();
        m.definition.help="MISS classifications under the current frozen input policy. Unavailable for older recording policies; this diagnostic can differ from the historical aimed-swing counter.".into();
    }
    let outcomes = json!([{"category":"HIT","count":s["hits"]},{"category":"SHIELD_BLOCK","count":d["shields"]["confirmedBlocks"]},{"category":"ESTIMATED_BLOCK","count":d["shields"]["estimatedBlocks"]},{"category":"UNCERTAIN","count":(analytics::num(s,"uncertain")-analytics::num(&d["shields"],"estimatedBlocks")).max(0.0)},{"category":"MISS","count":s["emptyMisses"]}]);
    offense.charts.push(chart("attack_outcomes","Attack outcomes","stackedBar","","attempts",vec![ChartSeries{label:"Outcome counts".into(),points:outcomes.as_array().unwrap().iter().enumerate().map(|(i,r)|{let mut p=point(i as f64,field(r,"count"),r,r["category"].as_str().unwrap().into());p.category=r["category"].as_str().unwrap().into();p}).collect()}],vec![],"Accuracy is confirmed hits out of eligible attempts. Confirmed shield blocks are excluded; uncertain results remain in the denominator."));
    let mut bins = [0usize; 42];
    for v in array(&reach, "values").iter().filter_map(Value::as_f64) {
        let bin = if v < 2.0 {
            0
        } else if v >= 4.0 {
            41
        } else {
            1 + (((v - 2.0) / 0.05 + 1e-9).floor() as usize).min(39)
        };
        bins[bin] += 1;
    }
    let target = analytics::num(&reach, "target");
    let tolerance = analytics::num(&reach, "tolerance");
    let band = ChartBand {
        start: target - tolerance,
        end: target + tolerance,
        label: "Frozen target window".into(),
        lane: "reachTarget".into(),
    };
    offense.charts.push(chart("reach_histogram","Reach distribution","histogram","m","hits",vec![ChartSeries{label:"Eligible hit reach".into(),points:bins.iter().enumerate().map(|(i,n)|{let x=if i==0{1.95}else if i==41{4.05}else{2.0+(i as f64-0.5)*0.05};let mut p=point(x,Some(*n as f64),&Value::Null,format!("{x:.3} m: {n} confirmed hits"));p.category=if(x-target).abs()<=tolerance+1e-9{"within"}else{"outside"}.into();p}).collect()}],vec![band.clone()],"0.05 m interior buckets, underflow below 2 m and overflow at 4 m; target shading classifies bucket midpoints."));
    offense.charts.push(chart(
        "reach_scatter",
        "Confirmed reach and uncertain estimates",
        "scatter",
        "s",
        "averageReach",
        vec![
            ChartSeries {
                label: "Confirmed hit".into(),
                points: hits
                    .iter()
                    .map(|h| {
                        point(
                            analytics::num(h, "t"),
                            field(h, "reach"),
                            h,
                            "Confirmed observed reach".into(),
                        )
                    })
                    .collect(),
            },
            ChartSeries {
                label: "No hit / uncertain".into(),
                points: array(d, "misses")
                    .iter()
                    .map(|h| {
                        let mut p = point(
                            analytics::num(h, "t"),
                            field(h, "estimatedReach"),
                            h,
                            "Estimated reach; not a confirmed hit".into(),
                        );
                        p.category = "hollow".into();
                        p
                    })
                    .collect(),
            },
        ],
        vec![],
        "Distances use frozen geometry. Creative reach is unavailable for survival comparison.",
    ));
    offense.tables.push(table(
        "reach_by_weapon",
        "Reach by weapon",
        &reach["byWeapon"],
        None,
    ));
    let aim_samples: Vec<_> = array(&d["aim"], "samples")
        .iter()
        .filter(|s| {
            weapon
                .as_ref()
                .is_none_or(|w| s["weapon"].as_str() == Some(w))
        })
        .cloned()
        .collect();
    let mut grades = [0; 4];
    let mut distances = Vec::new();
    let mut density = [0; 72];
    for r in &aim_samples {
        if r["hit"] == true {
            if let Some(g) = ["IDEAL", "GOOD", "PERIPHERAL", "POOR"]
                .iter()
                .position(|g| r["grade"].as_str() == Some(*g))
            {
                grades[g] += 1;
                if let Some(v) = field(r, "distance") {
                    distances.push(v);
                }
            }
            let x = analytics::num(r, "x");
            let z = analytics::num(r, "z");
            let h = if x.min(1.0 - x) < z.min(1.0 - z) {
                z
            } else {
                x
            };
            let col = ((h * 6.0) as usize).min(5);
            let row = (((1.0 - analytics::num(r, "y")) * 12.0) as usize).min(11);
            density[row * 6 + col] += 1;
        }
    }
    offense.metrics.insert(
        "aimCentral".into(),
        metrics::metric(
            "aimCentral",
            analytics::ratio(
                100.0 * (grades[0] + grades[1]) as f64,
                grades.iter().sum::<i32>() as f64,
            ),
        ),
    );
    offense.metrics.insert(
        "aimDistance".into(),
        metrics::metric("aimDistance", analytics::mean(&distances)),
    );
    offense.metrics.insert(
        "aimDistanceDeviation".into(),
        metrics::extra(
            "aimDistanceDeviation",
            "Aim consistency",
            "m",
            "Population standard deviation of graded confirmed hit distance.",
            analytics::sd(&distances),
        ),
    );
    offense.charts.push(chart("aim_contacts","Measured face contacts","scatter","normalized face","aimDistance",vec![ChartSeries{label:"Contacts".into(),points:aim_samples.iter().map(|r|{let x=analytics::num(r,"x");let z=analytics::num(r,"z");point(if x.min(1.0-x)<z.min(1.0-z){z}else{x},field(r,"y"),r,format!("{}; {}",r["grade"],r["face"]))}).collect()}],vec![],"Measured contacts only. Hollow contacts did not establish a hit; grades are geometric, not damage."));
    offense.charts.push(chart(
        "aim_density",
        "Confirmed-hit contact density",
        "heatmap",
        "column (6)",
        "hits",
        (0..12)
            .map(|r| ChartSeries {
                label: format!("row {r}"),
                points: (0..6)
                    .map(|c| {
                        point(
                            c as f64,
                            Some(density[r * 6 + c] as f64),
                            &Value::Null,
                            format!("column {c}, row {r}: {} hits", density[r * 6 + c]),
                        )
                    })
                    .collect(),
            })
            .collect(),
        vec![],
        "6 by 12 normalized hit-only count grid. It does not reconstruct unobserved impacts.",
    ));
    offense.tables.push(table(
        "aim_samples",
        "Measured aim samples",
        &json!(aim_samples),
        None,
    ));
    offense.tables.push(table(
        "aim_by_weapon",
        "Aim grades by weapon",
        &d["aim"]["byWeapon"],
        None,
    ));
    offense.tables.push(table(
        "aim_by_reach",
        "Aim grades by reach band",
        &d["aim"]["byReach"],
        None,
    ));
    let timing: Vec<_> = array(d, "timingSamples")
        .iter()
        .filter(|t| {
            weapon
                .as_ref()
                .is_none_or(|w| t["weapon"].as_str() == Some(w))
        })
        .filter(|t| {
            timing_result
                .as_ref()
                .is_none_or(|filter| match filter.as_str() {
                    "HITS" => t["outcome"] == "HIT",
                    "NO_HIT" => t["outcome"] != "HIT",
                    _ => true,
                })
        })
        .cloned()
        .collect();
    let graded: Vec<_> = timing
        .iter()
        .filter(|t| {
            matches!(
                t["grade"].as_str(),
                Some("PERFECT" | "TOO_FAST" | "TOO_SLOW")
            )
        })
        .collect();
    let deltas: Vec<_> = graded.iter().filter_map(|t| field(t, "delta")).collect();
    offense.metrics.insert(
        "timingPerfect".into(),
        metrics::metric(
            "timingPerfect",
            analytics::ratio(
                100.0 * graded.iter().filter(|t| t["grade"] == "PERFECT").count() as f64,
                graded.len() as f64,
            ),
        ),
    );
    for (key, label, value) in [
        (
            "timingGraded",
            "Graded intervals",
            Some(graded.len() as f64),
        ),
        (
            "timingMeanAbsoluteDelta",
            "Mean absolute cooldown difference",
            analytics::mean(&deltas.iter().map(|v| v.abs()).collect::<Vec<_>>()),
        ),
        (
            "timingDeviation",
            "Cooldown difference consistency",
            analytics::sd(&deltas),
        ),
    ] {
        offense.metrics.insert(key.into(),metrics::extra(key,label,if key=="timingGraded"{""}else{"ticks"},"Mechanical readiness, not tactical quality. Raw intervals from different weapons are not pooled.",value));
    }
    let tolerance = f.policy().timing_tolerance_ticks;
    let timing_band = ChartBand {
        start: -tolerance,
        end: tolerance,
        label: "Frozen timing tolerance".into(),
        lane: "yTolerance".into(),
    };
    for (id, title, seconds) in [
        ("timing_ordinal", "Cooldown timing by attempt", false),
        ("timing_seconds", "Cooldown timing over time", true),
    ] {
        let mut c = chart(
            id,
            title,
            "timing",
            if seconds { "s" } else { "attempt ordinal" },
            "timingPerfect",
            vec![ChartSeries {
                label: "Interval minus expected cooldown".into(),
                points: timing
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let mut p = point(
                            if seconds {
                                analytics::num(t, "t")
                            } else {
                                i as f64 + 1.0
                            },
                            field(t, "delta"),
                            t,
                            format!("{}; {}", t["grade"], t["outcome"]),
                        );
                        p.category = format!(
                            "{}:{}",
                            t["grade"].as_str().unwrap_or("UNAVAILABLE"),
                            t["outcome"].as_str().unwrap_or("UNRESOLVED")
                        );
                        p
                    })
                    .collect(),
            }],
            vec![timing_band.clone()],
            "Hit square, shield diamond, no-hit hollow circle; openers and ungraded resets stay unavailable.",
        );
        c.y_definition = metrics::extra(
            "timingDelta",
            "Interval minus expected cooldown",
            "ticks",
            "Frozen mechanical interval difference.",
            None,
        )
        .definition;
        offense.charts.push(c);
    }
    offense.tables.push(table(
        "timing_latest",
        "Newest 50 selected timing samples",
        &json!(timing.iter().rev().take(50).rev().collect::<Vec<_>>()),
        None,
    ));
    let mut per_weapon: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for t in &timing {
        per_weapon
            .entry(t["weapon"].as_str().unwrap_or("unknown").into())
            .or_default()
            .push(t);
    }
    let weapon_rows:Vec<_>=per_weapon.into_iter().map(|(weapon,rows)|{let graded:Vec<_>=rows.iter().filter(|r|matches!(r["grade"].as_str(),Some("PERFECT"|"TOO_FAST"|"TOO_SLOW"))).collect();let deltas:Vec<_>=graded.iter().filter_map(|r|field(r,"delta")).collect();json!({"weapon":weapon,"graded":graded.len(),"perfect":graded.iter().filter(|r|r["grade"]=="PERFECT").count(),"meanAbsoluteDelta":analytics::mean(&deltas.iter().map(|v|v.abs()).collect::<Vec<_>>()),"deviation":analytics::sd(&deltas)})}).collect();
    offense.tables.push(table(
        "timing_by_weapon",
        "Timing by weapon",
        &json!(weapon_rows),
        None,
    ));

    let mut defense=TabView{metrics:submetrics(&all,&["incoming","unattributedIncoming","attacksIntoShield","shieldBlocks","estimatedShieldBlocks","shieldDisables","hitsAfterDisable","localShieldBlocks","damageTaken","knockback","jumpCandidates"]),notices:vec!["Damage dealt, prevented damage and successful jump resets are not measured. Third-party damage is not opponent retaliation.".into()],..Default::default()};
    let incoming = array(d, "incomingDetail");
    let attributed: Vec<_> = incoming
        .iter()
        .filter(|r| r["attributed"] == true)
        .collect();
    for (key, label, unit, value) in [
        (
            "thirdPartyIncoming",
            "Third-party damage",
            "",
            field(s, "thirdPartyIncoming"),
        ),
        (
            "unexplainedHealthDrops",
            "Unexplained health drops",
            "",
            field(s, "unexplainedHealthDrops"),
        ),
        (
            "opponentCrits",
            "Critical animations on you",
            "",
            Some(incoming.iter().filter(|r| r["crit"] == true).count() as f64),
        ),
        (
            "meanKnockback",
            "Mean observed horizontal knockback",
            "blocks/tick",
            analytics::mean(
                &attributed
                    .iter()
                    .filter_map(|r| field(r, "knockback"))
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "healthCoverage",
            "Attributed hits with observed health drop",
            "",
            Some(
                attributed
                    .iter()
                    .filter(|r| r["damage"].as_f64().is_some())
                    .count() as f64,
            ),
        ),
        (
            "localShieldDisabled",
            "Your shield disabled",
            "",
            field(&d["shields"], "localDisabled"),
        ),
        (
            "bypassedShields",
            "Hits into raised shields",
            "",
            field(&d["shields"], "bypassed"),
        ),
        ("damageDealt", "Damage dealt", "HP", None),
        ("jumpResetSuccess", "Successful jump resets", "", None),
    ] {
        defense.metrics.insert(key.into(),metrics::extra(key,label,unit,"Observed context only; missing evidence cannot establish prevention, dealt damage or reset success.",value));
    }
    defense.tables.push(table(
        "incoming_latest",
        "Incoming hit observations",
        &json!(incoming),
        None,
    ));
    defense.tables.push(table(
        "shield_events",
        "Shield observations",
        &d["shields"]["events"],
        None,
    ));
    defense.tables.push(table(
        "jump_hits",
        "Attributed incoming jump timing",
        &json!(attributed),
        None,
    ));
    defense.tables.push(table(
        "jump_effect",
        "Experimental post-knockback speed comparison",
        &if d["jumpEffect"].is_object() {
            json!([d["jumpEffect"]])
        } else {
            json!([])
        },
        None,
    ));
    let categories = [
        "IDEAL_CANDIDATE",
        "ATTEMPT_CANDIDATE",
        "OUTSIDE_WINDOW",
        "NO_JUMP",
        "EXCLUDED",
        "UNAVAILABLE",
    ];
    defense.charts.push(chart(
        "jump_classes",
        "Incoming-hit jump categories",
        "bar",
        "category",
        "jumpCandidates",
        vec![ChartSeries {
            label: "Attributed hits".into(),
            points: categories
                .iter()
                .enumerate()
                .map(|(i, class)| {
                    let mut p = point(
                        i as f64,
                        Some(incoming.iter().filter(|r| r["jumpClass"] == *class).count() as f64),
                        &Value::Null,
                        (*class).into(),
                    );
                    p.category = (*class).into();
                    p
                })
                .collect(),
        }],
        vec![],
        "Actual jumps, not jump input. Same-tick/nearby candidates do not establish success.",
    ));
    let mut offsets = [0; 21];
    for offset in attributed
        .iter()
        .filter_map(|r| r["jumpOffsetTicks"].as_i64())
    {
        offsets[(offset.clamp(-10, 10) + 10) as usize] += 1;
    }
    defense.charts.push(chart("jump_offsets","Signed client-tick jump offsets","histogram","ticks","jumpCandidates",vec![ChartSeries{label:"Incoming-hit pairing".into(),points:offsets.iter().enumerate().map(|(i,n)|point(i as f64-10.0,Some(*n as f64),&Value::Null,format!("{} ticks: {n}",i as i64-10))).collect()}],vec![ChartBand{start:-(f.policy().jump_window_ticks as f64),end:f.policy().jump_window_ticks as f64,label:"Candidate window; zero is same tick".into(),lane:"candidate".into()}],"Offsets clamp to -10/+10 for display only. Pairing uses nearest actual jump with earlier sequence on ties."));
    defense.charts.push(chart(
        "shield_lanes",
        "Shield observations",
        "lanes",
        "s",
        "shieldBlocks",
        ["local", "opponent"]
            .into_iter()
            .map(|subject| ChartSeries {
                label: subject.into(),
                points: array(&d["shields"], "events")
                    .iter()
                    .filter(|r| r["subject"] == subject)
                    .map(|r| {
                        point(
                            analytics::num(r, "time"),
                            Some(1.0),
                            r,
                            r["type"].as_str().unwrap_or("").into(),
                        )
                    })
                    .collect(),
            })
            .collect(),
        vec![],
        "Sound observations identify shield subject; local attacker identity is unavailable.",
    ));
    let jump_lanes = vec![
        ChartSeries {
            label: "Attributed incoming".into(),
            points: attributed
                .iter()
                .filter_map(|r| {
                    field(r, "tick")
                        .map(|tick| point(tick, Some(0.0), r, "Attributed incoming hit".into()))
                })
                .collect(),
        },
        ChartSeries {
            label: "Actual jumps".into(),
            points: f
                .events
                .iter()
                .filter(|e| e.kind == "JUMP")
                .filter_map(|e| {
                    e.client_tick.map(|tick| {
                        let mut p =
                            point(tick as f64, Some(1.0), &Value::Null, "Actual jump".into());
                        p.sequence = Some(e.sequence);
                        p
                    })
                })
                .collect(),
        },
    ];
    let pairing_bands = attributed
        .iter()
        .filter_map(|r| {
            Some(ChartBand {
                start: field(r, "tick")?,
                end: field(r, "tick")? + field(r, "jumpOffsetTicks")?,
                label: format!("Signed offset {} ticks", r["jumpOffsetTicks"]),
                lane: "jumpPair".into(),
            })
        })
        .collect();
    defense.charts.push(chart("jump_lanes","Incoming and actual jumps","lanes","client ticks","jumpCandidates",jump_lanes,pairing_bands,"Shared client-tick lanes with signed pairing offsets. Actual jumps, not inputs; no world reconstruction."));

    let mut flow=TabView{metrics:submetrics(&all,&["comboCount","comboHits","longestCombo","averageCombo","longestOpponentCombo","longestUnanswered","dominanceShare"]),interpretations:array(d,"interpretations").iter().filter_map(Value::as_str).map(str::to_owned).collect(),notices:vec!["Combos use evidence time and attributed incoming hits; same-tick trades extend neither side's streak.".into()],..Default::default()};
    for (key, label, unit, value) in [
        (
            "combatActiveSeconds",
            "Recorded active combat",
            "s",
            field(s, "combatActiveSeconds"),
        ),
        (
            "recordedSeconds",
            "Recorded duration",
            "s",
            field(s, "duration"),
        ),
        (
            "dominanceActiveSeconds",
            "Observed dominance coverage",
            "s",
            field(&d["dominance"], "active"),
        ),
        (
            "opponentDominanceShare",
            "Opponent advantage share",
            "%",
            field(&d["dominance"], "opponentShare"),
        ),
        (
            "neutralDominanceShare",
            "Neutral share",
            "%",
            field(&d["dominance"], "neutralShare"),
        ),
        (
            "dominanceIncompleteSeconds",
            "Incomplete attribution",
            "s",
            field(&d["dominance"], "incomplete"),
        ),
        (
            "dominanceInsufficientSeconds",
            "Insufficient evidence",
            "s",
            field(&d["dominance"], "insufficient"),
        ),
        (
            "dominanceChanges",
            "Advantage changes",
            "",
            field(&d["dominance"], "changes"),
        ),
        (
            "exchangeTimeoutSeconds",
            "Frozen exchange timeout",
            "s",
            field(d, "exchangeTimeoutSeconds"),
        ),
    ] {
        flow.metrics.insert(key.into(),metrics::extra(key,label,unit,"Dominance is a deterministic decayed evidence share, not win probability or physical momentum.",value));
    }
    let bands: Vec<_> = array(&d["combos"], "streaks")
        .iter()
        .filter(|r| analytics::num(r, "length") >= 2.0)
        .map(|r| ChartBand {
            start: analytics::num(r, "start"),
            end: analytics::num(r, "end"),
            label: format!("{} hits; {}", r["length"], r["endedBy"]),
            lane: r["side"].as_str().unwrap_or("").into(),
        })
        .collect();
    flow.charts.push(chart("combos","Confirmed hit streaks","combo","s","comboCount",["LOCAL","OPPONENT"].into_iter().map(|side|ChartSeries{label:side.into(),points:array(&d["combos"],"marks").iter().filter(|r|r["side"]==side).map(|r|{let mut p=point(analytics::num(r,"t"),Some(if side=="LOCAL"{1.0}else{-1.0}),r,format!("{}{}",side,if r["traded"]==true{" same-tick trade"}else{""}));p.category=if r["traded"]==true{"trade"}else{"hit"}.into();p}).collect()}).collect(),bands,"Two-sided evidence timeline with trade marks and combo bands. Boundaries split recorded streaks."));
    let step = analytics::num(&d["dominance"], "step");
    let states = d["dominance"]["states"].as_str().unwrap_or("").as_bytes();
    flow.charts.push(chart("dominance","Encounter dominance","dominance","s","dominance",vec![ChartSeries{label:"Decayed confirmed evidence".into(),points:array(&d["dominance"],"values").iter().enumerate().map(|(i,v)|{let mut p=point(i as f64*step,v.as_f64(),&Value::Null,format!("{} s; {}",format::number(i as f64*step,2),states.get(i).copied().unwrap_or(b'I')as char));p.category=(states.get(i).copied().unwrap_or(b'I')as char).to_string();if p.category=="C"{p.availability=Availability::Incomplete;}p}).collect()}],vec![],"D=(P-N)/(P+N+1), tau 3 s. +/-0.2 advantage threshold; insufficient gaps and incomplete attribution are explicit. Shares exclude both."));
    flow.charts.push(chart("active_exchanges","Frozen combat-active intervals","timeline","s","fights",vec![],array(d,"combatExchanges").iter().map(|e|ChartBand{start:analytics::num(e,"start"),end:analytics::num(e,"end"),label:e["endedBy"].as_str().unwrap_or("").into(),lane:"activeExchange".into()}).collect(),"Active intervals include the recorded timeout tail; these differ from hit-streak exchanges."));
    for (id, title, data) in [
        ("combos", "Combo detail", &d["combos"]["streaks"]),
        (
            "hit_marks",
            "Evidence-time hit marks",
            &d["combos"]["marks"],
        ),
        (
            "combat_exchanges",
            "Combat-active exchanges",
            &d["combatExchanges"],
        ),
        (
            "streak_exchanges",
            "Historical hit-streak exchanges",
            &d["combos"]["exchanges"],
        ),
        (
            "turning_points",
            "Dominance turning points",
            &d["dominance"]["turningPoints"],
        ),
    ] {
        flow.tables.push(table(id, title, data, None));
    }
    if f.session.is_some() {
        flow.notices.push("Entire manual sessions have no tracked opponent. Select one participant exchange for opponent combos and dominance.".into());
    }
    super::cards::focused_flow(&mut flow, d);
    (overview, offense, defense, flow)
}

pub(crate) fn events(f: &FightRecord, a: &Analytics) -> Vec<EventView> {
    a.events
        .iter()
        .map(|e| EventView {
            sequence: e.sequence,
            time: f.seconds(e.nano_time),
            tick: e.client_tick,
            kind: e.kind.clone(),
            confidence: e.confidence.clone(),
            detail: e.detail.clone().unwrap_or_default(),
            outcome: a.derived["attemptOutcomes"][e.sequence.to_string()]
                .as_str()
                .map(str::to_owned),
            lane: match e.kind.as_str() {
                "ATTACK" | "HURT" => "attacks",
                "DAMAGE_IN" => "incoming",
                "SHIELD" => "shields",
                "JUMP" => "jumps",
                "SPRINT" => "sprint",
                _ => "events",
            }
            .into(),
            attributed_incoming: analytics::attributed(e, f.opponent.as_deref()),
            eligible_attempt: analytics::outgoing(e)
                && f.statistics
                    .attack_classifications
                    .get(&e.sequence)
                    .is_none_or(|c| c != "EXCLUDED"),
        })
        .collect()
}

fn checkpoint(f: &FightRecord, a: &Analytics, t: f64, events: &[EventView]) -> PlaybackCheckpoint {
    let hits = array(&a.derived, "hits")
        .iter()
        .filter(|h| analytics::num(h, "t") <= t + 1e-9)
        .count();
    let attempts = events
        .iter()
        .filter(|e| e.eligible_attempt && e.time <= t + 1e-9)
        .count();
    let incoming = events
        .iter()
        .filter(|e| e.attributed_incoming && e.time <= t + 1e-9)
        .count();
    let mut streak = 0;
    for mark in a
        .combos
        .marks
        .iter()
        .rev()
        .filter(|m| f.seconds(m.nanos) <= t + 1e-9)
    {
        if !mark.local || mark.traded {
            break;
        }
        streak += 1;
    }
    let index = ((t / a.dominance.step + 0.5).floor() as usize).min(a.dominance.values.len() - 1);
    PlaybackCheckpoint {
        time: t,
        hits,
        attempts,
        incoming,
        local_streak: streak,
        dominance: if f.session.is_some() {
            None
        } else {
            a.dominance.values[index]
        },
        dominance_state: if f.session.is_some() {
            "I".into()
        } else {
            a.dominance.states[index].to_string()
        },
    }
}
pub(crate) fn playback(
    f: &FightRecord,
    a: &Analytics,
    t: f64,
    filter: Option<&str>,
) -> Result<PlaybackView, Error> {
    const KINDS: [&str; 14] = [
        "ALL",
        "All",
        "ATTACK",
        "HURT",
        "DAMAGE_IN",
        "SHIELD",
        "CRIT",
        "JUMP",
        "SPRINT",
        "HEALTH",
        "VELOCITY",
        "DEATH",
        "MOTION",
        "JUMP_INPUT",
    ];
    let filter = filter.unwrap_or("ALL");
    if !KINDS.contains(&filter) {
        return Err(Error::new(
            "pulsar_invalid_filter",
            "Unsupported playback event filter.",
        ));
    }
    let t = t.clamp(0.0, f.duration());
    let events = events(f, a);
    let mut visible: Vec<_> = events
        .iter()
        .rev()
        .filter(|e| {
            e.time <= t + 1e-9
                && (if matches!(filter, "ALL" | "All") {
                    e.kind != "MOTION"
                } else {
                    e.kind == filter
                })
        })
        .take(200)
        .cloned()
        .collect();
    visible.reverse();
    let current = checkpoint(f, a, t, &events);
    let cumulative = BTreeMap::from([
        (
            "hits".into(),
            metrics::metric("hits", Some(current.hits as f64)),
        ),
        (
            "attempts".into(),
            metrics::metric("attempts", Some(current.attempts as f64)),
        ),
        (
            "incoming".into(),
            metrics::metric("incoming", Some(current.incoming as f64)),
        ),
        (
            "longestCombo".into(),
            metrics::extra(
                "currentStreak",
                "Current unanswered streak",
                "",
                "Evidence-time local marks; trades and attributed return hits reset the run.",
                Some(current.local_streak as f64),
            ),
        ),
        (
            "dominance".into(),
            metrics::metric("dominance", current.dominance),
        ),
    ]);
    let mut boundaries: Vec<_> = f
        .evidence()
        .rounds
        .iter()
        .map(|r| ChartBand {
            start: f.seconds(r.start_nanos),
            end: r.end_nanos.map(|n| f.seconds(n)).unwrap_or(f.duration()),
            label: format!("Round {}", r.number),
            lane: "rounds".into(),
        })
        .collect();
    boundaries.extend(f.statistics.combat_exchanges.iter().map(|e| ChartBand {
        start: f.seconds(e.start_nanos),
        end: f.seconds(e.end_nanos),
        label: e.ended_by.clone().unwrap_or("Exchange".into()),
        lane: "activeExchanges".into(),
    }));
    let mut times: Vec<_> = events
        .iter()
        .map(|e| e.time.clamp(0.0, f.duration()))
        .chain(
            a.combos
                .marks
                .iter()
                .map(|m| f.seconds(m.nanos).clamp(0.0, f.duration())),
        )
        .chain(
            (0..a.dominance.values.len()).map(|i| (i as f64 * a.dominance.step).min(f.duration())),
        )
        .chain([0.0, f.duration()])
        .collect();
    times.sort_by(f64::total_cmp);
    times.dedup();
    // Counts/streak are generated in core. The playback UI only seeks this cached snapshot.
    let mut hit_times: Vec<_> = array(&a.derived, "hits")
        .iter()
        .map(|h| analytics::num(h, "t"))
        .collect();
    hit_times.sort_by(f64::total_cmp);
    let attempt_times: Vec<_> = events
        .iter()
        .filter(|e| e.eligible_attempt)
        .map(|e| e.time)
        .collect();
    let incoming_times: Vec<_> = events
        .iter()
        .filter(|e| e.attributed_incoming)
        .map(|e| e.time)
        .collect();
    let mut streak = 0;
    let mark_runs: Vec<_> = a
        .combos
        .marks
        .iter()
        .map(|m| {
            if m.local && !m.traded {
                streak += 1;
            } else {
                streak = 0;
            }
            (f.seconds(m.nanos), streak)
        })
        .collect();
    let checkpoints = times
        .into_iter()
        .map(|time| {
            let index = ((time / a.dominance.step + 0.5).floor() as usize)
                .min(a.dominance.values.len() - 1);
            let marks = mark_runs.partition_point(|(t, _)| *t <= time + 1e-9);
            PlaybackCheckpoint {
                time,
                hits: hit_times.partition_point(|t| *t <= time + 1e-9),
                attempts: attempt_times.partition_point(|t| *t <= time + 1e-9),
                incoming: incoming_times.partition_point(|t| *t <= time + 1e-9),
                local_streak: marks.checked_sub(1).map(|i| mark_runs[i].1).unwrap_or(0),
                dominance: if f.session.is_some() {
                    None
                } else {
                    a.dominance.values[index]
                },
                dominance_state: if f.session.is_some() {
                    "I".into()
                } else {
                    a.dominance.states[index].to_string()
                },
            }
        })
        .collect();
    Ok(PlaybackView{duration_seconds:f.duration(),playhead_seconds:t,speeds:vec![0.25,0.5,1.0,2.0,4.0],lanes:vec!["rounds","activeExchanges","attacks","incoming","shields","jumps","sprint","events"].into_iter().map(str::to_owned).collect(),events,visible_events:visible,boundaries,cumulative,notices:vec!["Canonical attempts use attack time; streak marks use evidence time. No world movement or damage dealt is reconstructed.".into()],checkpoints})
}

pub(crate) fn trends(entries: &[&Entry]) -> Vec<ChartView> {
    ["accuracy","averageReach","criticalRate","averageInterval","sprintRate"].into_iter().map(|key|chart(&format!("trend_{key}"),&metrics::definition(key).label,"line","recording",key,vec![ChartSeries{label:metrics::definition(key).label,points:entries.iter().rev().enumerate().map(|(i,e)|{let v=analytics::summary(&e.record)[key].as_f64();let mut p=point(i as f64,v,&Value::Null,format!("{}; {}. Fight {}. {}",format::short_date(e.record.start_epoch_millis),format::value(v,&metrics::definition(key).unit),e.record.opponent_label.as_deref().or(e.record.opponent.as_deref()).unwrap_or("Manual session"), if key == "accuracy" { format!("{} confirmed hits from {} eligible attempts", e.record.statistics.hits, e.record.statistics.attempts - e.record.statistics.shield_blocks) } else { String::new() }));p.key=Some(e.key.clone());p}).collect()}],vec![],"Chronological recorded sufficient statistics. Missing denominators are gaps; never sort formatted strings.")).collect()
}

pub(crate) fn pool(
    entries: &[&Entry],
    timelines: &[(FightKey, FightRecord, Analytics)],
    weapon: &Option<String>,
    timing: &Option<String>,
) -> (TabView, TabView, TabView, TabView) {
    let mut f = entries
        .first()
        .map(|e| e.record.clone())
        .unwrap_or_else(|| FightRecord {
            policy: Some(Policy::historical_default()),
            match_evidence: Some(MatchEvidence::default()),
            ..Default::default()
        });
    f.session = None;
    let mut d = timelines
        .first()
        .map(|(_, _, a)| a.derived.clone())
        .unwrap_or_else(|| analytics::analyze(&f).derived);
    let mut summary = json!({});
    let max_keys = [
        "longestCombo",
        "longestOpponentCombo",
        "longestUnanswered",
        "maxReach",
    ];
    let count_keys = [
        "attempts",
        "hits",
        "emptyMisses",
        "uncertain",
        "contacts",
        "criticalConditions",
        "sprintHits",
        "jumpCandidates",
        "incoming",
        "comboCount",
        "comboHits",
        "unattributedIncoming",
        "thirdPartyIncoming",
        "longestCombo",
        "longestOpponentCombo",
        "longestUnanswered",
        "serverCrits",
        "damageTaken",
    ];
    for key in count_keys {
        let values: Vec<_> = entries
            .iter()
            .filter_map(|e| analytics::summary(&e.record)[key].as_f64())
            .collect();
        summary[key] = if values.is_empty() {
            Value::Null
        } else {
            json!(if max_keys.contains(&key) {
                values.into_iter().fold(0.0_f64, f64::max)
            } else {
                values.iter().sum()
            })
        };
    }
    let sum = |read: fn(&super::wire::Statistics) -> f64| {
        entries
            .iter()
            .map(|e| read(&e.record.statistics))
            .sum::<f64>()
    };
    let attempts = sum(|s| s.attempts as f64);
    let hits = sum(|s| s.hits as f64);
    let blocks = sum(|s| s.shield_blocks as f64);
    let intervals = sum(|s| s.interval_count as f64);
    let mean_interval = analytics::ratio(sum(|s| s.interval_sum), intervals);
    for (key, value) in [
        (
            "accuracy",
            analytics::ratio(100.0 * hits, attempts - blocks),
        ),
        (
            "averageReach",
            analytics::ratio(sum(|s| s.reach_sum), sum(|s| s.reach_count as f64)),
        ),
        (
            "maxReach",
            (sum(|s| s.reach_count as f64) > 0.0).then(|| {
                entries
                    .iter()
                    .map(|e| e.record.statistics.max_reach)
                    .fold(0.0_f64, f64::max)
            }),
        ),
        (
            "criticalRate",
            analytics::ratio(100.0 * sum(|s| s.critical_conditions as f64), hits),
        ),
        (
            "sprintRate",
            analytics::ratio(100.0 * sum(|s| s.sprint_hits as f64), hits),
        ),
        ("averageInterval", mean_interval),
        (
            "intervalDeviation",
            mean_interval.map(|m| {
                (sum(|s| s.interval_squared) / intervals - m * m)
                    .max(0.0)
                    .sqrt()
            }),
        ),
    ] {
        summary[key] = json!(value);
    }
    summary["contextAvailable"] = json!(entries.iter().any(|e| e.record.schema_version >= 3));
    summary["opponentComboAvailability"] = "Estimated".into();
    summary["duration"] = json!(entries.iter().map(|e| e.record.duration()).sum::<f64>());
    let measurement = entries
        .iter()
        .any(|e| e.record.statistics.measurement_version > 0);
    for (key, get) in [
        (
            "totalInputs",
            (|s: &super::wire::Statistics| s.total_inputs) as fn(&super::wire::Statistics) -> i32,
        ),
        ("uniqueAttacks", |s: &super::wire::Statistics| {
            s.unique_attacks
        }),
        ("excludedSwings", |s: &super::wire::Statistics| {
            s.excluded_swings
        }),
        ("deduplicatedAttacks", |s: &super::wire::Statistics| {
            s.deduplicated_attacks
        }),
        ("eligibleMisses", |s: &super::wire::Statistics| {
            s.eligible_misses
        }),
        ("uncertainShield", |s: &super::wire::Statistics| {
            s.uncertain_shield
        }),
    ] {
        summary[key] = json!(measurement.then(|| {
            entries
                .iter()
                .filter(|e| e.record.statistics.measurement_version > 0)
                .map(|e| get(&e.record.statistics) as i64)
                .sum::<i64>()
        }));
    }
    summary["unresolved"] =
        json!(analytics::num(&summary, "uncertain") - analytics::num(&summary, "uncertainShield"));
    summary["measurementVersion"] = json!(i32::from(measurement));
    summary["accuracyDenominator"] = json!(attempts - blocks);
    for key in [
        "hits",
        "misses",
        "timingSamples",
        "timings",
        "incomingDetail",
        "jumps",
    ] {
        d[key] = json!(
            timelines
                .iter()
                .rev()
                .flat_map(
                    |(fight_key, _, a)| array(&a.derived, key).iter().cloned().map(
                        move |mut row| {
                            if let Some(obj) = row.as_object_mut() {
                                obj.insert(
                                    "fightKey".into(),
                                    serde_json::to_value(fight_key).unwrap(),
                                );
                            }
                            row
                        }
                    )
                )
                .collect::<Vec<_>>()
        );
    }
    let mut aim_samples = Vec::new();
    let mut aim_grades = [0_i64; 4];
    let mut distances = Vec::new();
    let mut by_weapon: BTreeMap<String, [i64; 4]> = BTreeMap::new();
    let mut by_reach: BTreeMap<String, [i64; 4]> = BTreeMap::new();
    for (key, _, a) in timelines.iter().rev() {
        for sm in array(&a.derived["aim"], "samples") {
            let mut sm = sm.clone();
            sm["fightKey"] = json!(key);
            if sm["hit"] == true {
                if let Some(g) = ["IDEAL", "GOOD", "PERIPHERAL", "POOR"]
                    .iter()
                    .position(|g| sm["grade"] == *g)
                {
                    aim_grades[g] += 1;
                    by_weapon
                        .entry(sm["weapon"].as_str().unwrap_or("unknown").into())
                        .or_default()[g] += 1;
                    by_reach
                        .entry(sm["band"].as_str().unwrap_or("unknown").into())
                        .or_default()[g] += 1;
                    if let Some(v) = field(&sm, "distance") {
                        distances.push(v);
                    }
                }
            }
            aim_samples.push(sm);
        }
    }
    d["aim"] = json!({"samples":aim_samples,"grades":aim_grades,"byWeapon":by_weapon,"byReach":by_reach,"meanDistance":analytics::mean(&distances),"distanceDeviation":analytics::sd(&distances)});
    let all_hits = array(&d, "hits");
    let target = timelines
        .first()
        .map(|(_, f, _)| f.policy().perfect_reach_target)
        .unwrap_or(3.0);
    let tolerance = timelines
        .first()
        .map(|(_, f, _)| f.policy().perfect_reach_tolerance)
        .unwrap_or(0.06);
    d["reach"] = analytics::reach_summary(all_hits, target, tolerance);
    let mut shields = json!({"available":entries.iter().any(|e|e.record.schema_version>=3)});
    for (key, get) in [
        (
            "attacksIntoShield",
            (|s: &super::wire::Statistics| s.attacks_into_shield)
                as fn(&super::wire::Statistics) -> i32,
        ),
        ("confirmedBlocks", |s: &super::wire::Statistics| {
            s.shield_blocks
        }),
        ("estimatedBlocks", |s: &super::wire::Statistics| {
            s.estimated_shield_blocks
        }),
        ("disables", |s: &super::wire::Statistics| s.shield_disables),
        ("localBlocks", |s: &super::wire::Statistics| {
            s.local_shield_blocks
        }),
        ("localDisabled", |s: &super::wire::Statistics| {
            s.local_shield_disabled
        }),
    ] {
        shields[key] = json!(
            entries
                .iter()
                .map(|e| get(&e.record.statistics) as i64)
                .sum::<i64>()
        );
    }
    for key in ["hitsAfterDisable", "bypassed"] {
        shields[key] = if !timelines.iter().any(|(_, f, _)| f.schema_version >= 3) {
            Value::Null
        } else {
            json!(
                timelines
                    .iter()
                    .map(|(_, _, a)| analytics::num(&a.derived["shields"], key))
                    .sum::<f64>()
            )
        };
    }
    shields["events"] = json!(
        timelines
            .iter()
            .rev()
            .flat_map(|(_, _, a)| array(&a.derived["shields"], "events").iter().cloned())
            .collect::<Vec<_>>()
    );
    d["shields"] = shields;
    let combo_lengths: Vec<f64> = timelines
        .iter()
        .flat_map(|(_, _, a)| {
            a.combos
                .streaks
                .iter()
                .filter(|s| s.local && s.seqs.len() >= 2)
                .map(|s| s.seqs.len() as f64)
        })
        .collect();
    summary["averageCombo"] = json!(analytics::mean(&combo_lengths));
    summary["averageLocalCombo"] = summary["averageCombo"].clone();
    d["summary"] = summary;
    let local: f64 = timelines.iter().map(|(_, _, a)| a.dominance.local).sum();
    let opponent: f64 = timelines.iter().map(|(_, _, a)| a.dominance.opponent).sum();
    let neutral: f64 = timelines.iter().map(|(_, _, a)| a.dominance.neutral).sum();
    d["dominance"]["localShare"] =
        json!(analytics::ratio(100.0 * local, local + opponent + neutral));
    d["dominance"]["opponentShare"] = json!(analytics::ratio(
        100.0 * opponent,
        local + opponent + neutral
    ));
    d["dominance"]["neutralShare"] = json!(analytics::ratio(
        100.0 * neutral,
        local + opponent + neutral
    ));
    let (mut overview, mut offense, mut defense, mut flow) = tabs_value(&f, &d, weapon, timing);
    overview
        .metrics
        .extend(super::cards::pooled(entries, timelines));
    if !timelines.iter().any(|(_, f, _)| f.schema_version >= 3) {
        defense
            .metrics
            .insert("knockback".into(), metrics::metric("knockback", None));
    }
    overview.metrics.insert(
        "fights".into(),
        metrics::metric("fights", Some(entries.len() as f64)),
    );
    let eligible: f64 = timelines
        .iter()
        .map(|(_, _, a)| analytics::num(&a.derived["reach"], "eligible"))
        .sum();
    let within: f64 = timelines
        .iter()
        .map(|(_, _, a)| analytics::num(&a.derived["reach"], "within"))
        .sum();
    overview.metrics.insert(
        "reachWithin".into(),
        metrics::metric("reachWithin", analytics::ratio(100.0 * within, eligible)),
    );
    // Latest incoming/shield tables preserve the reference's per-record origin.
    if let Some((_, latest, a)) = timelines.first() {
        let latest_tabs = tabs(latest, a, weapon, timing);
        defense.tables = latest_tabs.2.tables;
        defense.charts = latest_tabs.2.charts;
        flow.metrics.extend(pooled_flow_metrics(entries, timelines));
        offense.notices.push("Distributions include loaded timelines only; counts and sufficient-stat rates include all selected recordings.".into());
        return (overview, offense, defense, flow);
    }
    (overview, offense, defense, flow)
}
pub(crate) fn pooled_flow_metrics(
    entries: &[&Entry],
    timelines: &[(FightKey, FightRecord, Analytics)],
) -> BTreeMap<String, MetricValue> {
    let (mut local, mut opponent, mut neutral) = (0.0, 0.0, 0.0);
    let mut counts = [0; 3];
    for (_, _, a) in timelines {
        local += a.dominance.local;
        opponent += a.dominance.opponent;
        neutral += a.dominance.neutral;
        counts[match a.dominance.availability {
            "AVAILABLE" => 0,
            "INCOMPLETE" => 1,
            _ => 2,
        }] += 1;
    }
    let mut result = BTreeMap::new();
    result.insert(
        "pooledDominanceShare".into(),
        metrics::extra(
            "pooledDominanceShare",
            "Pooled advantage share",
            "%",
            "Duration-weighted complete active samples; never an average of fight percentages.",
            analytics::ratio(100.0 * local, local + opponent + neutral),
        ),
    );
    result.insert(
        "pooledOpponentDominanceShare".into(),
        metrics::extra(
            "pooledOpponentDominanceShare",
            "Pooled opponent advantage share",
            "%",
            "Duration-weighted complete active samples.",
            analytics::ratio(100.0 * opponent, local + opponent + neutral),
        ),
    );
    for (i, label) in ["available", "incomplete", "insufficient"]
        .into_iter()
        .enumerate()
    {
        let key = format!("dominance_{label}");
        result.insert(
            key.clone(),
            metrics::extra(
                &key,
                &format!("Recordings: {label}"),
                "",
                "Fight-level evidence availability in the loaded timeline subset.",
                Some(counts[i] as f64),
            ),
        );
    }
    for key in [
        "comboCount",
        "comboHits",
        "longestCombo",
        "longestOpponentCombo",
        "longestUnanswered",
    ] {
        let values = entries
            .iter()
            .map(|e| analytics::summary(&e.record)[key].as_f64().unwrap_or(0.0));
        let value = if key.starts_with("longest") {
            values.fold(0.0_f64, f64::max)
        } else {
            values.sum()
        };
        result.insert(key.into(), metrics::metric(key, Some(value)));
    }
    let lengths: Vec<f64> = timelines
        .iter()
        .flat_map(|(_, _, a)| {
            a.combos
                .streaks
                .iter()
                .filter(|s| s.local && s.seqs.len() >= 2)
                .map(|s| s.seqs.len() as f64)
        })
        .collect();
    result.insert(
        "averageCombo".into(),
        metrics::metric("averageCombo", analytics::mean(&lengths)),
    );
    result
}
