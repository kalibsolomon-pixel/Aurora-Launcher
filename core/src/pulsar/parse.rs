use super::{Error, wire::*};
use serde_json::Value;
use std::{fs, io::Read, path::Path, sync::OnceLock, time::Duration};

pub(crate) const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const MAX_CANDIDATES: usize = 2000;
pub(crate) const MAX_TIMELINE_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_TIMELINE_EVENTS: usize = 50_000;

pub(crate) fn canonical_id(id: &str) -> Result<(), Error> {
    let parsed = uuid::Uuid::parse_str(id).map_err(|_| {
        Error::new(
            "pulsar_invalid_id",
            "A canonical lowercase fight UUID is required.",
        )
    })?;
    if parsed.hyphenated().to_string() != id {
        return Err(Error::new(
            "pulsar_invalid_id",
            "A canonical lowercase fight UUID is required.",
        ));
    }
    Ok(())
}

/// Reject reparse points on the root itself and every descendant, not only the leaf.
pub(crate) fn safe_path(root: &Path, path: &Path) -> Result<(), Error> {
    if !root.is_absolute() || path.strip_prefix(root).is_err() {
        return Err(Error::new(
            "pulsar_unsafe_path",
            "The path is outside its Pulsar root.",
        ));
    }
    for ancestor in root.ancestors().skip(1) {
        if crate::install::coordination::redirected(&fs::symlink_metadata(ancestor)?) {
            return Err(Error::new(
                "pulsar_unsafe_path",
                "A Pulsar root ancestor is a link or reparse point.",
            ));
        }
    }
    let mut current = root.to_path_buf();
    for component in
        std::iter::once(None).chain(path.strip_prefix(root).unwrap().components().map(Some))
    {
        if let Some(component) = component {
            if !matches!(component, std::path::Component::Normal(_)) {
                return Err(Error::new(
                    "pulsar_unsafe_path",
                    "Noncanonical path component.",
                ));
            }
            current.push(component);
        }
        let meta = fs::symlink_metadata(&current)?;
        if crate::install::coordination::redirected(&meta) {
            return Err(Error::new(
                "pulsar_unsafe_path",
                "Links and reparse points are not recordings.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn read_bytes(root: &Path, path: &Path, max: u64) -> Result<Vec<u8>, Error> {
    safe_path(root, path)?;
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() {
        return Err(Error::new("pulsar_unsafe_path", "Expected a regular file."));
    }
    if meta.len() > max {
        return Err(Error::new(
            "pulsar_oversize",
            "The file exceeds the bounded read limit.",
        ));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Do not follow a leaf replaced with a reparse point between metadata and open.
        options.custom_flags(0x0020_0000);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || crate::install::coordination::redirected(&meta) {
        return Err(Error::new(
            "pulsar_unsafe_path",
            "Expected a regular non-link file.",
        ));
    }
    let mut bytes = Vec::with_capacity((meta.len().min(max).min(1024 * 1024)) as usize);
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(Error::new(
            "pulsar_oversize",
            "The file grew beyond the bounded read limit.",
        ));
    }
    safe_path(root, path)?;
    Ok(bytes)
}

pub(crate) fn read_fight(root: &Path, path: &Path) -> Result<(FightRecord, Vec<u8>), Error> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::new("pulsar_invalid_id", "Invalid recording filename."))?;
    let id = name.strip_suffix(".json").ok_or_else(|| {
        Error::new(
            "pulsar_invalid_id",
            "Only finalized JSON recordings are accepted.",
        )
    })?;
    canonical_id(id)?;
    let mut last = Error::new("pulsar_io", "Recording is unavailable.");
    for attempt in 0..3 {
        let result = read_bytes(root, path, MAX_FILE_BYTES)
            .and_then(|bytes| decode(&bytes).map(|f| (f, bytes)));
        match result {
            Ok((f, bytes)) if f.id == id => return Ok((f, bytes)),
            Ok(_) => {
                return Err(Error::new(
                    "pulsar_id_mismatch",
                    "The recording ID does not match its filename.",
                ));
            }
            Err(e) if matches!(e.code, "pulsar_io" | "pulsar_malformed") => last = e,
            Err(e) => return Err(e),
        }
        if attempt < 2 {
            std::thread::sleep(Duration::from_millis(25 * (attempt + 1)));
        }
    }
    Err(last)
}

/// Validate known field types before normalization, retaining additive unknown fields.
fn validate(value: &Value, rule: &Value, key: &str, depth: usize) -> Result<(), Error> {
    let bad = || {
        Error::new(
            "pulsar_malformed",
            "A recording contains an invalid field or integer range.",
        )
    };
    if depth > 64 {
        return Err(Error::new(
            "pulsar_depth",
            "The recording exceeds the nesting limit.",
        ));
    }
    if let Some(choices) = rule.get("anyOf").and_then(Value::as_array) {
        if value.is_null() {
            return Ok(());
        }
        return validate(value, &choices[0], key, depth + 1);
    }
    if let Some(reference) = rule.get("$ref").and_then(Value::as_str) {
        let name = reference.rsplit('/').next().unwrap();
        static SCHEMA: OnceLock<Value> = OnceLock::new();
        let schema =
            SCHEMA.get_or_init(|| serde_json::from_str(include_str!("wire-schema.json")).unwrap());
        if let Some(variants) = schema["enums"].get(name).and_then(Value::as_array) {
            return if variants.contains(value) {
                Ok(())
            } else {
                Err(bad())
            };
        }
        let object = value.as_object().ok_or_else(bad)?;
        if name == "FightRecord" {
            if !object.get("statistics").is_some_and(Value::is_object)
                || !object.get("events").is_some_and(Value::is_array)
            {
                return Err(bad());
            }
            if !object
                .get("schemaVersion")
                .and_then(Value::as_i64)
                .is_some_and(|v| (1..=5).contains(&v))
            {
                return Err(Error::new(
                    "pulsar_unsupported_schema",
                    "Unsupported recording schema; source preserved.",
                ));
            }
        }
        for (field, field_rule) in schema["types"][name].as_object().ok_or_else(bad)? {
            if let Some(v) = object.get(field) {
                validate(v, field_rule, field, depth + 1)?;
            }
        }
        return Ok(());
    }
    match rule["type"].as_str().unwrap_or("") {
        "integer" => {
            let n = value.as_i64().ok_or_else(bad)?;
            if !matches!(
                key,
                "startEpochMillis"
                    | "endEpochMillis"
                    | "durationNanos"
                    | "attackSequence"
                    | "evidenceSequence"
                    | "startTick"
                    | "endTick"
                    | "manualStartEpochMillis"
                    | "manualStartTick"
                    | "timelineStartNanos"
                    | "startNanos"
                    | "endNanos"
                    | "nanoTime"
                    | "establishedNanos"
                    | "eventSequence"
                    | "sequence"
                    | "clientTick"
                    | "momentumAt"
                    | "lastHit"
                    | "comboStart"
                    | "attackCanonicalIds"
                    | "attackSamples"
            ) && i32::try_from(n).is_err()
            {
                return Err(bad());
            }
        }
        "number" => {
            if !value.as_f64().is_some_and(f64::is_finite) {
                return Err(bad());
            }
        }
        "boolean" => {
            if !value.is_boolean() {
                return Err(bad());
            }
        }
        "string" => {
            if !value.is_string() {
                return Err(bad());
            }
        }
        "array" => {
            for v in value.as_array().ok_or_else(bad)? {
                validate(v, &rule["items"], key, depth + 1)?;
            }
        }
        "object" => {
            for (k, v) in value.as_object().ok_or_else(bad)? {
                if k.parse::<i64>().is_err() {
                    return Err(bad());
                }
                validate(v, &rule["additionalProperties"], key, depth + 1)?;
            }
        }
        _ => return Err(bad()),
    }
    Ok(())
}

fn normalize(value: &mut Value, name: &str) {
    if let Some(obj) = value.as_object_mut() {
        // Nullable collection constructors copy null to an empty collection.
        let fields: &[(&str, bool)] = match name {
            "FightRecord" => &[("evaluations", false)],
            "MatchEvidence" => &[("entries", false), ("rounds", false)],
            "Statistics" => &[
                ("attackCanonicalIds", true),
                ("attackSamples", true),
                ("attackClassifications", true),
                ("attackTimings", true),
                ("combatExchanges", false),
                ("reachDistribution", false),
                ("recentReach", false),
            ],
            "Resolution" => &[("notes", false)],
            _ => &[],
        };
        for (field, map) in fields {
            if obj.get(*field).is_some_and(Value::is_null) {
                obj.insert(
                    (*field).into(),
                    if *map {
                        serde_json::json!({})
                    } else {
                        serde_json::json!([])
                    },
                );
            }
        }
        if name == "FightRecord" {
            if let Some(stats) = obj.get_mut("statistics") {
                normalize(stats, "Statistics");
            }
            if let Some(evidence) = obj.get_mut("matchEvidence") {
                normalize(evidence, "MatchEvidence");
            }
            if let Some(resolution) = obj.get_mut("resolution") {
                normalize(resolution, "Resolution");
            }
            if let Some(exchanges) = obj
                .get_mut("session")
                .and_then(|s| s.get_mut("exchanges"))
                .and_then(Value::as_array_mut)
            {
                for ex in exchanges {
                    normalize(ex, "FightRecord");
                }
            }
        }
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<FightRecord, Error> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(Error::new("pulsar_oversize", "Recording exceeds 512 MiB."));
    }
    let mut value: Value = serde_json::from_slice(bytes).map_err(|_| {
        Error::new(
            "pulsar_malformed",
            "The recording is not complete, valid JSON.",
        )
    })?;
    fn depth(value: &Value, level: usize) -> Result<(), Error> {
        if level > 64 {
            return Err(Error::new(
                "pulsar_depth",
                "The recording exceeds the nesting limit.",
            ));
        }
        match value {
            Value::Array(values) => {
                for v in values {
                    depth(v, level + 1)?;
                }
            }
            Value::Object(values) => {
                for v in values.values() {
                    depth(v, level + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    // Additive unknown fields have the same depth budget as known schema fields.
    depth(&value, 0)?;
    validate(
        &value,
        &serde_json::json!({"$ref":"#/$defs/FightRecord"}),
        "",
        0,
    )?;
    normalize(&mut value, "FightRecord");
    let mut fight: FightRecord = serde_json::from_value(value).map_err(|_| {
        Error::new(
            "pulsar_malformed",
            "The recording does not match its wire schema.",
        )
    })?;
    validate_fight(&mut fight, 0)?;
    Ok(fight)
}

fn validate_fight(f: &mut FightRecord, depth: usize) -> Result<(), Error> {
    if !(1..=5).contains(&f.schema_version) {
        return Err(Error::new(
            "pulsar_unsupported_schema",
            "Unsupported recording schema; source preserved.",
        ));
    }
    canonical_id(&f.id)?;
    if depth > 1 || (depth == 1 && f.session.is_some()) {
        return Err(Error::new(
            "pulsar_depth",
            "Nested manual sessions are not supported.",
        ));
    }
    if f.duration_nanos < 0
        || f.events.len()
            > if f.session.is_some() {
                1_000_000
            } else {
                10_000
            }
    {
        return Err(Error::new(
            "pulsar_event_limit",
            "Invalid duration or excessive event count.",
        ));
    }
    let mut sequence = -1;
    let mut time = -1;
    for e in &mut f.events {
        if e.kind.is_empty()
            || e.confidence.is_empty()
            || e.sequence <= sequence
            || e.nano_time < time
        {
            return Err(Error::new(
                "pulsar_event_order",
                "Events must have increasing sequences and nondecreasing timestamps.",
            ));
        }
        sequence = e.sequence;
        time = e.nano_time;
        if let Some(m) = e.attack.as_mut().and_then(|a| a.measurement.as_mut()) {
            m.attack_speed = m.attack_speed.max(0.0);
            m.cooldown_ticks = m.cooldown_ticks.max(0.0);
        }
    }
    // Negative sufficient-stat counts would manufacture invalid rates.
    let stats = serde_json::to_value(&f.statistics).unwrap();
    for (key, value) in stats.as_object().unwrap() {
        if value.is_i64()
            && !matches!(key.as_str(), "lastHit" | "comboStart" | "momentumAt")
            && value.as_i64().unwrap() < 0
        {
            return Err(Error::new(
                "pulsar_invalid_statistics",
                "Negative metric counters are not supported.",
            ));
        }
    }
    if f.statistics.shield_blocks > f.statistics.attempts
        || f.statistics.hits > f.statistics.attempts
    {
        return Err(Error::new(
            "pulsar_invalid_statistics",
            "Hit/block counts exceed attack attempts.",
        ));
    }
    if f.outcome.is_none() {
        f.outcome = Some("UNKNOWN".into());
    }
    let p = f.policy.get_or_insert_with(Policy::historical_default);
    if p.perfect_reach_target <= 0.0 {
        p.perfect_reach_target = 3.0;
        p.perfect_reach_tolerance = 0.06;
        p.timing_tolerance_ticks = 1.0;
        p.jump_window_ticks = 4;
    }
    if f.match_evidence.is_none() {
        f.match_evidence = Some(MatchEvidence {
            confidence: Some("OBSERVED".into()),
            ..Default::default()
        });
    }
    if let Some(session) = &mut f.session {
        if session.exchanges.len() > 10_000 || session.participants.len() > 10_000 {
            return Err(Error::new(
                "pulsar_event_limit",
                "Manual session scope limit exceeded.",
            ));
        }
        for ex in &mut session.exchanges {
            validate_fight(ex, depth + 1)?;
        }
    }
    Ok(())
}

impl Policy {
    pub(crate) fn historical_default() -> Self {
        Self {
            correlation_millis: 250.0,
            combo_seconds: 3.0,
            jump_window_millis: 150.0,
            perfect_cooldown: 0.95,
            great_cooldown: 0.85,
            perfect_reach_target: 3.0,
            perfect_reach_tolerance: 0.06,
            timing_tolerance_ticks: 1.0,
            jump_window_ticks: 4,
            ..Default::default()
        }
    }
}
impl FightRecord {
    pub(crate) fn policy(&self) -> &Policy {
        self.policy.as_ref().expect("validated policy")
    }
    pub(crate) fn evidence(&self) -> &MatchEvidence {
        self.match_evidence
            .as_ref()
            .expect("validated match evidence")
    }
    pub(crate) fn origin(&self) -> i64 {
        self.evidence()
            .timeline_start_nanos
            .or_else(|| self.events.first().map(|e| e.nano_time))
            .unwrap_or(0)
    }
    pub(crate) fn seconds(&self, nanos: i64) -> f64 {
        (nanos as i128 - self.origin() as i128) as f64 / 1e9
    }
    pub(crate) fn duration(&self) -> f64 {
        self.duration_nanos as f64 / 1e9
    }
    pub(crate) fn summary_only(&mut self) {
        self.events.clear();
        self.evaluations.clear();
        self.statistics.attack_canonical_ids.clear();
        self.statistics.attack_samples.clear();
        self.statistics.attack_classifications.clear();
        self.statistics.attack_timings.clear();
        self.statistics.combat_exchanges.clear();
        if let Some(s) = &mut self.session {
            for ex in &mut s.exchanges {
                ex.summary_only();
            }
        }
    }
}
