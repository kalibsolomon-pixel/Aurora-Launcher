use super::super::wire::FightRecord;
use super::{Combos, Dominance, num, ratio, resolution, text};
use serde_json::Value;
use std::collections::BTreeMap;

#[allow(clippy::too_many_arguments)]
pub(crate) fn of(
    f: &FightRecord,
    hits: &[Value],
    misses: &[Value],
    combos: &Combos,
    d: &Dominance,
    reach: &Value,
    shields: &Value,
    incoming: &[Value],
    timing: &[Value],
    availability: &str,
) -> Vec<String> {
    let r = resolution(f);
    let mut out = vec![format!(
        "Outcome: {} ({}{}).",
        r.label,
        r.source,
        if r.derived {
            ", derived from an older recording"
        } else {
            ""
        }
    )];
    let mut best = None;
    for s in combos.streaks.iter().filter(|s| s.local) {
        if best.is_none_or(|b: &super::combos::Streak| s.seqs.len() > b.seqs.len()) {
            best = Some(s);
        }
    }
    if let Some(s) = best.filter(|s| s.seqs.len() >= 2) {
        let words = [
            "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        ];
        let number = words
            .get(s.seqs.len())
            .map(|s| s.to_string())
            .unwrap_or_else(|| s.seqs.len().to_string());
        out.push(format!(
            "You landed {number} consecutive hits {}",
            match s.ended_by {
                "RETALIATION" => "before an attributed return hit.",
                "TRADE" => "before a same-tick trade.",
                "DISENGAGE" => "before both players disengaged.",
                "ROUND_BOUNDARY" => "before the round ended.",
                _ => "and the recording ended without a return hit.",
            }
        ));
    } else if !hits.is_empty() {
        out.push("No two of your confirmed hits landed consecutively.".into());
    }
    if f.session.is_some() {
        out.push(
            if hits.is_empty() && misses.is_empty() {
                "No combat was recorded in this manual session."
            } else {
                "Manual sessions have no tracked opponent, so opponent combos are unavailable."
            }
            .into(),
        );
    } else if availability != "Observed" {
        out.push(format!("Some incoming damage could not be attributed to the opponent, so opponent combos are {}.",availability.to_lowercase()));
    }
    if num(reach, "eligible") > 0.0 {
        out.push(format!("Your reach was within the configured target window ({:.2} ± {:.2} m) on {} of {} eligible hits.",num(reach,"target"),num(reach,"tolerance"),num(reach,"within")as usize,num(reach,"eligible")as usize));
    }
    if f.session.is_some() && hits.is_empty() && misses.is_empty() {
        return out;
    }
    out.push(match d.availability{"INCOMPLETE"=>"Incoming-hit attribution was incomplete, so encounter dominance could not be established confidently.".into(),"AVAILABLE"=>format!("During {} s of observed active combat you held the advantage {}% of the time and the opponent {}%.",super::format::number(d.active,1),super::format::number(ratio(100.0*d.local,d.active).unwrap_or(f64::NAN),0),super::format::number(ratio(100.0*d.opponent,d.active).unwrap_or(f64::NAN),0)),_=>"Too few confirmed exchanges were observed to estimate encounter dominance.".into()});
    if shields["available"] == true && num(shields, "attacksIntoShield") > 0.0 {
        out.push(format!("{} of your {} attacks into a raised shield were confirmed blocks{}. Confirmed blocks are excluded from the accuracy denominator.",num(shields,"confirmedBlocks")as usize,num(shields,"attacksIntoShield")as usize,if num(shields,"estimatedBlocks")>0.0{format!(" and {} were estimated blocks",num(shields,"estimatedBlocks")as usize)}else{String::new()}));
    }
    let att = incoming.iter().filter(|r| r["attributed"] == true).count();
    if att > 0 {
        out.push(format!("{} of {} attributed incoming hits had an observed jump within ±{} ticks ({} on the same tick). Whether a jump reduced knockback is not measured.",incoming.iter().filter(|r|matches!(text(r,"jumpClass"),"IDEAL_CANDIDATE"|"ATTEMPT_CANDIDATE")).count(),att,f.policy().jump_window_ticks,incoming.iter().filter(|r|text(r,"jumpClass")=="IDEAL_CANDIDATE").count()));
    }
    let mut weapons: BTreeMap<&str, [usize; 2]> = BTreeMap::new();
    for t in timing {
        if !matches!(text(t, "grade"), "PERFECT" | "TOO_FAST" | "TOO_SLOW") {
            continue;
        }
        let c = weapons.entry(text(t, "weapon")).or_default();
        c[1] += 1;
        if text(t, "grade") == "PERFECT" {
            c[0] += 1;
        }
    }
    for (weapon, c) in weapons {
        out.push(format!("With {}, {} of {} graded attack intervals matched the weapon's cooldown. Cooldown readiness is not a judgement of tactical timing.",super::format::short_item(weapon),c[0],c[1]));
    }
    out
}
