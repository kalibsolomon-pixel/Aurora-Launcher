//! Private schemas 1?5 wire types. Additive members survive unchanged in raw storage.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct FightRecord {
    pub schema_version: i32,
    pub id: String,
    pub start_epoch_millis: i64,
    pub end_epoch_millis: i64,
    pub duration_nanos: i64,
    pub opponent: Option<String>,
    pub outcome: Option<String>,
    pub reason: Option<String>,
    pub statistics: Statistics,
    pub events: Vec<CombatEvent>,
    pub policy: Option<Policy>,
    pub evaluations: Vec<Evaluation>,
    pub opponent_label: Option<String>,
    pub match_evidence: Option<MatchEvidence>,
    pub resolution: Option<Resolution>,
    pub session: Option<SessionInfo>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for FightRecord {
    fn default() -> Self {
        Self {
            schema_version: Default::default(),
            id: Default::default(),
            start_epoch_millis: Default::default(),
            end_epoch_millis: Default::default(),
            duration_nanos: Default::default(),
            opponent: Default::default(),
            outcome: Default::default(),
            reason: Default::default(),
            statistics: Default::default(),
            events: Default::default(),
            policy: Default::default(),
            evaluations: Default::default(),
            opponent_label: Default::default(),
            match_evidence: Default::default(),
            resolution: Default::default(),
            session: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Policy {
    pub correlation_millis: f64,
    pub combo_seconds: f64,
    pub jump_window_millis: f64,
    pub perfect_cooldown: f64,
    pub great_cooldown: f64,
    pub perfect_reach_target: f64,
    pub perfect_reach_tolerance: f64,
    pub timing_tolerance_ticks: f64,
    pub jump_window_ticks: i32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            correlation_millis: Default::default(),
            combo_seconds: Default::default(),
            jump_window_millis: Default::default(),
            perfect_cooldown: Default::default(),
            great_cooldown: Default::default(),
            perfect_reach_target: Default::default(),
            perfect_reach_tolerance: Default::default(),
            timing_tolerance_ticks: Default::default(),
            jump_window_ticks: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Evaluation {
    pub attack_sequence: i64,
    pub evidence_sequence: i64,
    pub readiness: Option<String>,
    pub confidence: Option<String>,
    pub combo: i32,
    pub momentum: f64,
    pub result: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Evaluation {
    fn default() -> Self {
        Self {
            attack_sequence: Default::default(),
            evidence_sequence: Default::default(),
            readiness: Default::default(),
            confidence: Default::default(),
            combo: Default::default(),
            momentum: Default::default(),
            result: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SessionInfo {
    pub start_tick: i64,
    pub end_tick: i64,
    pub world: Option<String>,
    pub termination: Option<String>,
    pub participants: Vec<Participant>,
    pub exchanges: Vec<FightRecord>,
    pub announcements: Vec<EvidenceEntry>,
    pub manual_start_epoch_millis: Option<i64>,
    pub manual_start_tick: Option<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for SessionInfo {
    fn default() -> Self {
        Self {
            start_tick: Default::default(),
            end_tick: Default::default(),
            world: Default::default(),
            termination: Default::default(),
            participants: Default::default(),
            exchanges: Default::default(),
            announcements: Default::default(),
            manual_start_epoch_millis: Default::default(),
            manual_start_tick: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Participant {
    pub id: Option<String>,
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Participant {
    fn default() -> Self {
        Self {
            id: Default::default(),
            label: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct MatchEvidence {
    pub server: Option<String>,
    pub local_player: Option<String>,
    pub round_id: Option<String>,
    pub confidence: Option<String>,
    pub entries: Vec<EvidenceEntry>,
    pub timeline_start_nanos: Option<i64>,
    pub match_id: Option<String>,
    pub round_number: Option<i32>,
    pub rounds: Vec<Round>,
    pub server_session_id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for MatchEvidence {
    fn default() -> Self {
        Self {
            server: Default::default(),
            local_player: Default::default(),
            round_id: Default::default(),
            confidence: Default::default(),
            entries: Default::default(),
            timeline_start_nanos: Default::default(),
            match_id: Default::default(),
            round_number: Default::default(),
            rounds: Default::default(),
            server_session_id: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Round {
    pub number: i32,
    pub start_nanos: i64,
    pub end_nanos: Option<i64>,
    pub outcome: Option<String>,
    pub rule: Option<String>,
    pub confidence: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Round {
    fn default() -> Self {
        Self {
            number: Default::default(),
            start_nanos: Default::default(),
            end_nanos: Default::default(),
            outcome: Default::default(),
            rule: Default::default(),
            confidence: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct EvidenceEntry {
    pub nano_time: i64,
    pub kind: Option<String>,
    pub rule: Option<String>,
    pub origin: Option<String>,
    pub text: Option<String>,
    pub accepted: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for EvidenceEntry {
    fn default() -> Self {
        Self {
            nano_time: Default::default(),
            kind: Default::default(),
            rule: Default::default(),
            origin: Default::default(),
            text: Default::default(),
            accepted: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Resolution {
    pub classification: Option<String>,
    pub source: Option<String>,
    pub confidence: Option<String>,
    pub scope: Option<String>,
    pub established_nanos: Option<i64>,
    pub evidence_index: Option<i32>,
    pub event_sequence: Option<i64>,
    pub notes: Vec<String>,
    pub derived: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Resolution {
    fn default() -> Self {
        Self {
            classification: Default::default(),
            source: Default::default(),
            confidence: Default::default(),
            scope: Default::default(),
            established_nanos: Default::default(),
            evidence_index: Default::default(),
            event_sequence: Default::default(),
            notes: Default::default(),
            derived: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct CombatEvent {
    pub sequence: i64,
    pub nano_time: i64,
    pub kind: String,
    pub actor: Option<String>,
    pub target: Option<String>,
    pub confidence: String,
    pub attack: Option<Attack>,
    pub detail: Option<String>,
    pub client_tick: Option<i64>,
    pub context: Option<Context>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for CombatEvent {
    fn default() -> Self {
        Self {
            sequence: Default::default(),
            nano_time: Default::default(),
            kind: Default::default(),
            actor: Default::default(),
            target: Default::default(),
            confidence: Default::default(),
            attack: Default::default(),
            detail: Default::default(),
            client_tick: Default::default(),
            context: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Context {
    pub attribution: Option<String>,
    pub damage_type: Option<String>,
    pub direct: Option<bool>,
    pub health: Option<f64>,
    pub absorption: Option<f64>,
    pub vx: Option<f64>,
    pub vy: Option<f64>,
    pub vz: Option<f64>,
    pub ground: Option<bool>,
    pub sprinting: Option<bool>,
    pub movement_valid: Option<bool>,
    pub shield: Option<String>,
    pub subject: Option<String>,
    pub target_blocking: Option<bool>,
    pub target_facing: Option<bool>,
    pub distance: Option<f64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Context {
    fn default() -> Self {
        Self {
            attribution: Default::default(),
            damage_type: Default::default(),
            direct: Default::default(),
            health: Default::default(),
            absorption: Default::default(),
            vx: Default::default(),
            vy: Default::default(),
            vz: Default::default(),
            ground: Default::default(),
            sprinting: Default::default(),
            movement_valid: Default::default(),
            shield: Default::default(),
            subject: Default::default(),
            target_blocking: Default::default(),
            target_facing: Default::default(),
            distance: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Attack {
    pub cooldown: f64,
    pub reach: f64,
    pub aim_x: f64,
    pub aim_y: f64,
    pub aim_z: f64,
    pub sprinting: bool,
    pub critical_conditions: bool,
    pub ray_contact: bool,
    pub measurement: Option<Measurement>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Attack {
    fn default() -> Self {
        Self {
            cooldown: Default::default(),
            reach: Default::default(),
            aim_x: Default::default(),
            aim_y: Default::default(),
            aim_z: Default::default(),
            sprinting: Default::default(),
            critical_conditions: Default::default(),
            ray_contact: Default::default(),
            measurement: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Measurement {
    pub weapon: Option<String>,
    pub attack_speed: f64,
    pub cooldown_ticks: f64,
    pub timing: Option<TimingResult>,
    pub eye: Option<Point3>,
    pub r#box: Option<Bounds>,
    pub ideal_region: Option<Region>,
    pub aim_quality: Option<String>,
    pub mode: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Measurement {
    fn default() -> Self {
        Self {
            weapon: Default::default(),
            attack_speed: Default::default(),
            cooldown_ticks: Default::default(),
            timing: Default::default(),
            eye: Default::default(),
            r#box: Default::default(),
            ideal_region: Default::default(),
            aim_quality: Default::default(),
            mode: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Point3 {
    fn default() -> Self {
        Self {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            min_x: Default::default(),
            min_y: Default::default(),
            min_z: Default::default(),
            max_x: Default::default(),
            max_y: Default::default(),
            max_z: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Region {
    pub nearest: Option<Point3>,
    pub ideal: Option<Point3>,
    pub ideal_radius: f64,
    pub good_radius: f64,
    pub peripheral_radius: f64,
    pub reachable: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Region {
    fn default() -> Self {
        Self {
            nearest: Default::default(),
            ideal: Default::default(),
            ideal_radius: Default::default(),
            good_radius: Default::default(),
            peripheral_radius: Default::default(),
            reachable: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct TimingResult {
    pub grade: Option<String>,
    pub interval_ticks: f64,
    pub expected_ticks: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for TimingResult {
    fn default() -> Self {
        Self {
            grade: Default::default(),
            interval_ticks: Default::default(),
            expected_ticks: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct CombatInterval {
    pub start_nanos: i64,
    pub end_nanos: i64,
    pub ended_by: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for CombatInterval {
    fn default() -> Self {
        Self {
            start_nanos: Default::default(),
            end_nanos: Default::default(),
            ended_by: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Statistics {
    pub strict_version: i32,
    pub stuns: i32,
    pub local_stuns: i32,
    pub spam_clicks: i32,
    pub not_counted_incoming_swings: i32,
    pub stun_follow_ups: BTreeMap<i64, i64>,
    pub stun_incoming_follow_ups: BTreeMap<i64, i64>,
    pub stun_incoming_damage: std::collections::BTreeSet<i64>,
    pub stun_outgoing_damage: std::collections::BTreeSet<i64>,
    pub spam_click_sequences: std::collections::BTreeSet<i64>,
    pub not_counted_incoming_sequences: std::collections::BTreeSet<i64>,

    pub non_melee_damage: i32,
    pub local_shield_not_blocked: i32,
    pub measurement_version: i32,
    pub total_inputs: i32,
    pub unique_attacks: i32,
    pub deduplicated_attacks: i32,
    pub excluded_swings: i32,
    pub eligible_misses: i32,
    pub uncertain_shield: i32,
    pub attack_canonical_ids: BTreeMap<i64, i64>,
    pub attack_samples: BTreeMap<i64, i64>,
    pub attack_classifications: BTreeMap<i64, String>,
    pub attack_timings: BTreeMap<i64, TimingResult>,
    pub exchange_timeout_seconds: f64,
    pub combat_exchanges: Vec<CombatInterval>,
    pub attempts: i32,
    pub hits: i32,
    pub empty_misses: i32,
    pub uncertain: i32,
    pub contacts: i32,
    pub critical_conditions: i32,
    pub sprint_hits: i32,
    pub jump_candidates: i32,
    pub current_combo: i32,
    pub longest_combo: i32,
    pub incoming: i32,
    pub perfect_reach_hits: i32,
    pub unattributed_incoming: i32,
    pub third_party_incoming: i32,
    pub attacks_into_shield: i32,
    pub shield_blocks: i32,
    pub estimated_shield_blocks: i32,
    pub shield_disables: i32,
    pub local_shield_blocks: i32,
    pub local_shield_disabled: i32,
    pub server_crits: i32,
    pub opponent_crits: i32,
    pub combo_count: i32,
    pub combo_hits: i32,
    pub longest_opponent_combo: i32,
    pub longest_unanswered: i32,
    pub damage_taken: f64,
    pub center_aim: i32,
    pub edge_aim: i32,
    pub high_aim: i32,
    pub low_aim: i32,
    pub reach_distribution: Vec<i32>,
    pub recent_reach: Vec<f64>,
    pub recent_reach_index: i32,
    pub recent_reach_count: i32,
    pub recent_interval: f64,
    pub momentum_at: i64,
    pub reach_sum: f64,
    pub max_reach: f64,
    pub cooldown_sum: f64,
    pub interval_sum: f64,
    pub interval_squared: f64,
    pub reach_count: i32,
    pub interval_count: i32,
    pub last_hit: i64,
    pub combo_start: i64,
    pub momentum: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Statistics {
    fn default() -> Self {
        Self {
            strict_version: 0,
            stuns: Default::default(),
            local_stuns: Default::default(),
            spam_clicks: Default::default(),
            not_counted_incoming_swings: Default::default(),
            stun_follow_ups: Default::default(),
            stun_incoming_follow_ups: Default::default(),
            stun_incoming_damage: Default::default(),
            stun_outgoing_damage: Default::default(),
            spam_click_sequences: Default::default(),
            not_counted_incoming_sequences: Default::default(),

            non_melee_damage: 0,
            local_shield_not_blocked: 0,
            measurement_version: Default::default(),
            total_inputs: Default::default(),
            unique_attacks: Default::default(),
            deduplicated_attacks: Default::default(),
            excluded_swings: Default::default(),
            eligible_misses: Default::default(),
            uncertain_shield: Default::default(),
            attack_canonical_ids: Default::default(),
            attack_samples: Default::default(),
            attack_classifications: Default::default(),
            attack_timings: Default::default(),
            exchange_timeout_seconds: 3.0,
            combat_exchanges: Default::default(),
            attempts: Default::default(),
            hits: Default::default(),
            empty_misses: Default::default(),
            uncertain: Default::default(),
            contacts: Default::default(),
            critical_conditions: Default::default(),
            sprint_hits: Default::default(),
            jump_candidates: Default::default(),
            current_combo: Default::default(),
            longest_combo: Default::default(),
            incoming: Default::default(),
            perfect_reach_hits: Default::default(),
            unattributed_incoming: Default::default(),
            third_party_incoming: Default::default(),
            attacks_into_shield: Default::default(),
            shield_blocks: Default::default(),
            estimated_shield_blocks: Default::default(),
            shield_disables: Default::default(),
            local_shield_blocks: Default::default(),
            local_shield_disabled: Default::default(),
            server_crits: Default::default(),
            opponent_crits: Default::default(),
            combo_count: Default::default(),
            combo_hits: Default::default(),
            longest_opponent_combo: Default::default(),
            longest_unanswered: Default::default(),
            damage_taken: Default::default(),
            center_aim: Default::default(),
            edge_aim: Default::default(),
            high_aim: Default::default(),
            low_aim: Default::default(),
            reach_distribution: vec![0; 8],
            recent_reach: vec![0.0; 16],
            recent_reach_index: Default::default(),
            recent_reach_count: Default::default(),
            recent_interval: Default::default(),
            momentum_at: Default::default(),
            reach_sum: Default::default(),
            max_reach: Default::default(),
            cooldown_sum: Default::default(),
            interval_sum: Default::default(),
            interval_squared: Default::default(),
            reach_count: Default::default(),
            interval_count: Default::default(),
            last_hit: Default::default(),
            combo_start: Default::default(),
            momentum: Default::default(),
            extra: BTreeMap::new(),
        }
    }
}
