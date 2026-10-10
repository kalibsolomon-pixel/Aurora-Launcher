//! Launcher-owned local gameplay history. No frontend path or argument authority.
use crate::instances::InstanceId;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const SCHEMA: u32 = 2;
const MAX_RECENT_SERVERS: usize = 100;
const MAX_FAVORITE_SERVERS: usize = 100;
const MAX_SESSIONS: usize = 256;
const MAX_VISITS: usize = 16;
const MAX_DAYS: usize = 3660;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorldSaveId(String);
impl WorldSaveId {
    pub fn parse(value: String) -> Result<Self, &'static str> {
        if value.is_empty()
            || value.len() > 255
            || value.chars().count() > 128
            || value == "."
            || value == ".."
            || value.ends_with(['.', ' '])
            || value.trim() != value
            || value.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
            || value.chars().any(|c| c.is_control())
            || crate::launch::activity_bridge::sanitize(&value) != value
        {
            return Err("invalid world identity");
        }
        let stem = value.split('.').next().unwrap_or("");
        if matches!(
            stem.to_ascii_uppercase().as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        ) {
            return Err("reserved world identity");
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for WorldSaveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorldSaveId[private]")
    }
}
impl TryFrom<String> for WorldSaveId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}
impl From<WorldSaveId> for String {
    fn from(value: WorldSaveId) -> String {
        value.0
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServerTarget(String);
impl ServerTarget {
    pub fn parse(value: String) -> Result<Self, &'static str> {
        if value.is_empty() || value.len() > 255 || value.trim() != value || !value.is_ascii() {
            return Err("invalid server target");
        }
        let (host, port) = if let Some(tail) = value.strip_prefix('[') {
            let (ip, rest) = tail.split_once(']').ok_or("invalid server target")?;
            let ip: std::net::Ipv6Addr = ip.parse().map_err(|_| "invalid server target")?;
            let port = rest
                .strip_prefix(':')
                .map(parse_port)
                .transpose()?
                .unwrap_or(25565);
            (format!("[{ip}]"), port)
        } else {
            let (host, port) = match value.rsplit_once(':') {
                Some((host, port)) => (host, parse_port(port)?),
                None => (value.as_str(), 25565),
            };
            if host.is_empty() || host.len() > 253 || host.ends_with('.') {
                return Err("invalid server target");
            }
            if host.parse::<std::net::Ipv4Addr>().is_err()
                && host
                    .split('.')
                    .all(|label| label.bytes().all(|b| b.is_ascii_digit()))
            {
                return Err("invalid IPv4 target");
            }
            if host.parse::<std::net::Ipv4Addr>().is_err()
                && !host.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err("invalid server target");
            }
            (host.to_ascii_lowercase(), port)
        };
        Ok(Self(format!("{host}:{port}")))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
fn parse_port(value: &str) -> Result<u16, &'static str> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid port");
    }
    value
        .parse::<u16>()
        .map_err(|_| "invalid port")
        .and_then(|port| {
            if port == 0 {
                Err("invalid port")
            } else {
                Ok(port)
            }
        })
}
impl fmt::Debug for ServerTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ServerTarget[private]")
    }
}
impl TryFrom<String> for ServerTarget {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parsed = Self::parse(value.clone())?;
        if parsed.as_str() != value {
            return Err("noncanonical server target");
        }
        Ok(parsed)
    }
}
impl From<ServerTarget> for String {
    fn from(value: ServerTarget) -> String {
        value.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum QuickLaunchTarget {
    Singleplayer {
        instance: InstanceId,
        world: WorldSaveId,
    },
    Multiplayer {
        instance: InstanceId,
        server: ServerTarget,
    },
}
impl fmt::Debug for QuickLaunchTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QuickLaunchTarget[private]")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Unknown,
    MainMenu,
    Singleplayer,
    Multiplayer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Normal,
    Abnormal,
    Interrupted,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Visit {
    mode: Mode,
    started_at: u64,
    ended_at: u64,
    duration_ms: u64,
    world: Option<WorldSaveId>,
    server: Option<ServerTarget>,
    display: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRecord {
    id: String,
    instance: InstanceId,
    started_at: u64,
    ended_at: Option<u64>,
    duration_ms: u64,
    outcome: Option<Outcome>,
    mode: Mode,
    visits: Vec<Visit>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DayTotal {
    day: u64,
    instance: InstanceId,
    duration_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    schema_version: u32,
    sessions: Vec<SessionRecord>,
    archive: Vec<DayTotal>,
    servers: Vec<ServerHistory>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ServerHistory {
    instance: InstanceId,
    server: ServerTarget,
    display: String,
    last_played_at: u64,
    duration_ms: u64,
    favorite: bool,
}
fn target_id(mode: Mode, instance: &InstanceId, target: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(if mode == Mode::Singleplayer {
        b"world".as_slice()
    } else {
        b"server".as_slice()
    });
    digest.update([0]);
    digest.update(instance.as_str().as_bytes());
    digest.update([0]);
    digest.update(target.as_bytes());
    format!("{:x}", digest.finalize())
}
impl Default for Document {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA,
            sessions: Vec::new(),
            archive: Vec::new(),
            servers: Vec::new(),
        }
    }
}
impl Document {
    fn validate(&self) -> Result<(), ()> {
        if self.schema_version != SCHEMA
            || self.sessions.len() > MAX_SESSIONS
            || self.archive.len() > 8192
        {
            return Err(());
        }
        let mut ids = std::collections::HashSet::new();
        let mut server_ids = std::collections::HashSet::new();
        if self.servers.iter().filter(|s| s.favorite).count() > MAX_FAVORITE_SERVERS
            || self.servers.iter().filter(|s| !s.favorite).count() > MAX_RECENT_SERVERS
            || self.servers.iter().any(|s| {
                !server_ids.insert(target_id(Mode::Multiplayer, &s.instance, s.server.as_str()))
                    || s.display.len() > 512
                    || crate::launch::activity_bridge::sanitize(&s.display) != s.display
            })
        {
            return Err(());
        }
        let mut archive_keys = std::collections::HashSet::new();
        for day in &self.archive {
            if !archive_keys.insert((day.day, day.instance.as_str()))
                || day.duration_ms > 86_400_000
            {
                return Err(());
            }
        }
        for session in &self.sessions {
            if uuid::Uuid::parse_str(&session.id)
                .map_err(|_| ())?
                .to_string()
                != session.id
                || !ids.insert(&session.id)
                || session.visits.len() > MAX_VISITS
                || session.duration_ms > 3660 * 86400 * 1000
            {
                return Err(());
            }
            if let Some(end) = session.ended_at {
                if end < session.started_at || session.outcome.is_none() {
                    return Err(());
                }
            } else if session.outcome.is_some() {
                return Err(());
            }
            for visit in &session.visits {
                if visit.ended_at < visit.started_at
                    || visit.display.as_ref().is_some_and(|s| {
                        s.len() > 512 || crate::launch::activity_bridge::sanitize(s) != *s
                    })
                    || (visit.mode != Mode::Singleplayer && visit.world.is_some())
                    || (visit.mode != Mode::Multiplayer && visit.server.is_some())
                {
                    return Err(());
                }
            }
        }
        if self
            .archive
            .iter()
            .any(|day| day.duration_ms > 86400 * 1000 * 1000)
        {
            return Err(());
        }
        Ok(())
    }
    fn retain(&mut self) -> Result<(), ()> {
        // Summaries live in this same document independently of session retention.
        // Fold before pruning visits/sessions; never derive launch targets from labels.
        for session in &self.sessions {
            for visit in &session.visits {
                let Some(server) = &visit.server else {
                    continue;
                };
                if visit.mode != Mode::Multiplayer {
                    continue;
                }
                let display = visit
                    .display
                    .clone()
                    .unwrap_or_else(|| "Multiplayer server".into());
                if let Some(entry) = self
                    .servers
                    .iter_mut()
                    .find(|s| s.instance == session.instance && s.server == *server)
                {
                    if visit.ended_at >= entry.last_played_at {
                        entry.last_played_at = visit.ended_at;
                        entry.display = display;
                    }
                } else {
                    self.servers.push(ServerHistory {
                        instance: session.instance.clone(),
                        server: server.clone(),
                        display,
                        last_played_at: visit.ended_at,
                        duration_ms: 0,
                        favorite: false,
                    });
                }
            }
        }
        for entry in &mut self.servers {
            let duration: u64 = self
                .sessions
                .iter()
                .filter(|s| s.instance == entry.instance)
                .flat_map(|s| &s.visits)
                .filter(|v| v.server.as_ref() == Some(&entry.server))
                .map(|v| v.duration_ms)
                .sum();
            entry.duration_ms = entry.duration_ms.max(duration);
        }
        self.servers.sort_by(|a, b| {
            b.last_played_at.cmp(&a.last_played_at).then_with(|| {
                target_id(Mode::Multiplayer, &a.instance, a.server.as_str()).cmp(&target_id(
                    Mode::Multiplayer,
                    &b.instance,
                    b.server.as_str(),
                ))
            })
        });
        let mut recent = 0;
        self.servers.retain(|s| {
            if s.favorite {
                true
            } else {
                recent += 1;
                recent <= MAX_RECENT_SERVERS
            }
        });
        self.sessions.sort_by_key(|s| s.started_at);
        while self.sessions.len() > MAX_SESSIONS {
            let index = self
                .sessions
                .iter()
                .position(|s| s.ended_at.is_some())
                .ok_or(())?;
            let removed = self.sessions.remove(index);
            if removed.ended_at.is_some() {
                let mut daily = BTreeMap::new();
                add_daily(&mut daily, removed.started_at, removed.duration_ms);
                for (day, ms) in daily {
                    if let Some(bucket) = self
                        .archive
                        .iter_mut()
                        .find(|b| b.day == day && b.instance == removed.instance)
                    {
                        bucket.duration_ms = bucket.duration_ms.saturating_add(ms);
                    } else {
                        self.archive.push(DayTotal {
                            day,
                            instance: removed.instance.clone(),
                            duration_ms: ms,
                        });
                    }
                }
            }
        }
        let cutoff = now().saturating_div(86400).saturating_sub(MAX_DAYS as u64);
        self.archive.retain(|d| d.day >= cutoff);
        self.archive.sort_by(|a, b| {
            a.day
                .cmp(&b.day)
                .then_with(|| a.instance.as_str().cmp(b.instance.as_str()))
        });
        if self.archive.len() > 8192 {
            self.archive.drain(..self.archive.len() - 8192);
        }
        Ok(())
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn memory() -> &'static Mutex<HashMap<PathBuf, Document>> {
    static DOCS: OnceLock<Mutex<HashMap<PathBuf, Document>>> = OnceLock::new();
    DOCS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn read(path: &Path) -> Result<Document, ()> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Document::default()),
        Err(_) => return Err(()),
    };
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(());
    }
    // Only the exact known schema-1 shape migrates. Unknown/malformed documents
    // remain untouched. The next successful atomic mutation writes schema 2.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Legacy {
        schema_version: u32,
        sessions: Vec<SessionRecord>,
        archive: Vec<DayTotal>,
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| ())?;
    let doc: Document = if value.get("schemaVersion").and_then(|v| v.as_u64()) == Some(1) {
        let old: Legacy = serde_json::from_value(value).map_err(|_| ())?;
        if old.schema_version != 1 {
            return Err(());
        }
        let mut migrated = Document {
            schema_version: SCHEMA,
            sessions: old.sessions,
            archive: old.archive,
            servers: Vec::new(),
        };
        migrated.validate()?;
        migrated.retain()?;
        migrated
    } else {
        serde_json::from_value(value).map_err(|_| ())?
    };
    doc.validate()?;
    Ok(doc)
}
fn write(path: &Path, doc: &Document) -> Result<(), ()> {
    doc.validate()?;
    read(path)?;
    fs::create_dir_all(path.parent().ok_or(())?).map_err(|_| ())?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<(), ()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| ())?;
        serde_json::to_writer(&mut file, doc).map_err(|_| ())?;
        use std::io::Write;
        file.write_all(b"\n").map_err(|_| ())?;
        file.sync_all().map_err(|_| ())?;
        fs::rename(&temporary, path).map_err(|_| ())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
#[derive(Clone)]
pub struct Store {
    path: PathBuf,
}
impl Store {
    /// Resolves an opaque history ID against persisted visits. The caller must
    /// validate the returned target again against current managed state.
    pub fn quick_target(&self, mode: Mode, id: &str) -> Result<Option<QuickLaunchTarget>, ()> {
        use sha2::{Digest, Sha256};
        if id.len() != 64
            || !id.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !matches!(mode, Mode::Singleplayer | Mode::Multiplayer)
        {
            return Err(());
        }
        let map = memory().lock().map_err(|_| ())?;
        let doc = map.get(&self.path).ok_or(())?;
        if mode == Mode::Multiplayer {
            return Ok(doc
                .servers
                .iter()
                .find(|s| target_id(mode, &s.instance, s.server.as_str()) == id)
                .map(|s| QuickLaunchTarget::Multiplayer {
                    instance: s.instance.clone(),
                    server: s.server.clone(),
                }));
        }
        for session in &doc.sessions {
            for visit in &session.visits {
                if visit.mode != mode {
                    continue;
                }
                let target = match mode {
                    Mode::Singleplayer => visit.world.as_ref().map(WorldSaveId::as_str),
                    Mode::Multiplayer => visit.server.as_ref().map(ServerTarget::as_str),
                    _ => None,
                };
                let Some(target) = target else {
                    continue;
                };
                let mut digest = Sha256::new();
                digest.update(if mode == Mode::Singleplayer {
                    b"world".as_slice()
                } else {
                    b"server".as_slice()
                });
                digest.update([0]);
                digest.update(session.instance.as_str().as_bytes());
                digest.update([0]);
                digest.update(target.as_bytes());
                if format!("{:x}", digest.finalize()) == id {
                    return Ok(Some(match mode {
                        Mode::Singleplayer => QuickLaunchTarget::Singleplayer {
                            instance: session.instance.clone(),
                            world: WorldSaveId::parse(target.to_owned()).map_err(|_| ())?,
                        },
                        Mode::Multiplayer => QuickLaunchTarget::Multiplayer {
                            instance: session.instance.clone(),
                            server: ServerTarget::try_from(target.to_owned()).map_err(|_| ())?,
                        },
                        _ => unreachable!(),
                    }));
                }
            }
        }
        Ok(None)
    }
    pub fn open(path: PathBuf) -> Result<Self, ()> {
        let mut map = memory().lock().map_err(|_| ())?;
        if !map.contains_key(&path) {
            let mut doc = read(&path)?;
            let mut changed = false;
            for session in &mut doc.sessions {
                if session.ended_at.is_none() {
                    session.ended_at = Some(
                        session
                            .started_at
                            .saturating_add(session.duration_ms / 1000),
                    );
                    session.outcome = Some(Outcome::Interrupted);
                    changed = true;
                }
            }
            if changed {
                write(&path, &doc)?;
            }
            map.insert(path.clone(), doc);
        }
        Ok(Self { path })
    }
    fn edit(&self, edit: impl FnOnce(&mut Document)) -> Result<(), ()> {
        let mut map = memory().lock().map_err(|_| ())?;
        let current = map.get(&self.path).ok_or(())?;
        let mut next = current.clone();
        edit(&mut next);
        next.retain()?;
        write(&self.path, &next)?;
        map.insert(self.path.clone(), next);
        Ok(())
    }
    pub fn start(&self, instance: InstanceId) -> Result<Recorder, ()> {
        let id = uuid::Uuid::new_v4().to_string();
        let started_at = now();
        self.edit(|doc| {
            doc.sessions.push(SessionRecord {
                id: id.clone(),
                instance,
                started_at,
                ended_at: None,
                duration_ms: 0,
                outcome: None,
                mode: Mode::Unknown,
                visits: Vec::new(),
            })
        })?;
        Ok(Recorder {
            store: self.clone(),
            id,
            started_at,
            clock: Instant::now(),
            current: Mutex::new(None),
        })
    }
    pub fn sessions(&self) -> Result<Vec<SessionRecord>, ()> {
        Ok(memory()
            .lock()
            .map_err(|_| ())?
            .get(&self.path)
            .ok_or(())?
            .sessions
            .clone())
    }
    pub fn playtime(&self, instance: Option<&InstanceId>, at: u64) -> Result<Playtime, ()> {
        let map = memory().lock().map_err(|_| ())?;
        let doc = map.get(&self.path).ok_or(())?;
        let mut buckets: BTreeMap<u64, u64> = BTreeMap::new();
        for day in &doc.archive {
            if instance.is_none_or(|id| *id == day.instance) {
                *buckets.entry(day.day).or_default() += day.duration_ms;
            }
        }
        for session in &doc.sessions {
            if instance.is_none_or(|id| *id == session.instance) {
                add_daily(&mut buckets, session.started_at, session.duration_ms);
            }
        }
        let day = at / 86400;
        let sum = |days: u64| {
            buckets
                .range(day.saturating_sub(days - 1)..=day)
                .map(|(_, ms)| *ms)
                .sum()
        };
        Ok(Playtime {
            today_ms: sum(1),
            last_7_days_ms: sum(7),
            last_30_days_ms: sum(30),
            all_time_ms: buckets.values().sum(),
        })
    }
    pub fn daily_playtime(
        &self,
        instance: Option<&InstanceId>,
        at: u64,
        days: usize,
    ) -> Result<Vec<DailyPlaytime>, ()> {
        if !(1..=30).contains(&days) {
            return Err(());
        }
        let map = memory().lock().map_err(|_| ())?;
        let doc = map.get(&self.path).ok_or(())?;
        let mut buckets = BTreeMap::new();
        for day in &doc.archive {
            if instance.is_none_or(|id| *id == day.instance) {
                *buckets.entry(day.day).or_insert(0u64) += day.duration_ms;
            }
        }
        for session in &doc.sessions {
            if instance.is_none_or(|id| *id == session.instance) {
                add_daily(&mut buckets, session.started_at, session.duration_ms);
            }
        }
        let today = at / 86400;
        Ok((0..days)
            .rev()
            .map(|back| {
                let day = today.saturating_sub(back as u64);
                DailyPlaytime {
                    day,
                    duration_ms: *buckets.get(&day).unwrap_or(&0),
                }
            })
            .collect())
    }
    pub fn recent(
        &self,
        mode: Mode,
        instance: Option<&InstanceId>,
        limit: usize,
    ) -> Result<Vec<RecentTarget>, ()> {
        use sha2::{Digest, Sha256};
        if !matches!(mode, Mode::Singleplayer | Mode::Multiplayer) {
            return Err(());
        }
        let map = memory().lock().map_err(|_| ())?;
        let doc = map.get(&self.path).ok_or(())?;
        if mode == Mode::Multiplayer {
            return Ok(doc
                .servers
                .iter()
                .filter(|s| instance.is_none_or(|id| *id == s.instance))
                .take(limit.min(MAX_RECENT_SERVERS + MAX_FAVORITE_SERVERS))
                .map(|s| RecentTarget {
                    id: target_id(mode, &s.instance, s.server.as_str()),
                    instance_id: s.instance.to_string(),
                    display_name: s.display.clone(),
                    last_played_at: s.last_played_at,
                    duration_ms: s.duration_ms,
                    available: true,
                    favorite: s.favorite,
                })
                .collect());
        }
        let mut entries = HashMap::<String, RecentTarget>::new();
        for session in &doc.sessions {
            if !instance.is_none_or(|id| *id == session.instance) {
                continue;
            }
            for visit in &session.visits {
                if visit.mode != mode {
                    continue;
                }
                let target = match mode {
                    Mode::Singleplayer => visit.world.as_ref().map(WorldSaveId::as_str),
                    Mode::Multiplayer => visit.server.as_ref().map(ServerTarget::as_str),
                    _ => None,
                };
                let Some(target) = target else {
                    continue;
                };
                let mut digest = Sha256::new();
                digest.update(if mode == Mode::Singleplayer {
                    b"world".as_slice()
                } else {
                    b"server".as_slice()
                });
                digest.update([0]);
                digest.update(session.instance.as_str().as_bytes());
                digest.update([0]);
                digest.update(target.as_bytes());
                let id = format!("{:x}", digest.finalize());
                let entry = entries.entry(id.clone()).or_insert_with(|| RecentTarget {
                    id,
                    instance_id: session.instance.as_str().to_owned(),
                    display_name: String::new(),
                    last_played_at: 0,
                    duration_ms: 0,
                    available: true,
                    favorite: false,
                });
                entry.duration_ms = entry.duration_ms.saturating_add(visit.duration_ms);
                if visit.ended_at >= entry.last_played_at {
                    entry.last_played_at = visit.ended_at;
                    entry.display_name = visit.display.clone().unwrap_or_else(|| {
                        if mode == Mode::Singleplayer {
                            "Singleplayer world"
                        } else {
                            "Multiplayer server"
                        }
                        .to_owned()
                    });
                }
            }
        }
        let mut result: Vec<_> = entries.into_values().collect();
        result.sort_by(|a, b| {
            b.last_played_at
                .cmp(&a.last_played_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        result.truncate(limit.min(20));
        Ok(result)
    }
    pub fn set_server_favorite(&self, id: &str, favorite: bool) -> Result<(), ()> {
        // The existing process-wide history mutex serializes with bridge writes.
        let mut map = memory().lock().map_err(|_| ())?;
        let mut next = map.get(&self.path).ok_or(())?.clone();
        let entry = next
            .servers
            .iter()
            .position(|s| target_id(Mode::Multiplayer, &s.instance, s.server.as_str()) == id)
            .ok_or(())?;
        if favorite
            && !next.servers[entry].favorite
            && next.servers.iter().filter(|s| s.favorite).count() >= MAX_FAVORITE_SERVERS
        {
            return Err(());
        }
        next.servers[entry].favorite = favorite;
        next.retain()?;
        write(&self.path, &next)?;
        map.insert(self.path.clone(), next);
        Ok(())
    }
}
fn add_daily(buckets: &mut BTreeMap<u64, u64>, start: u64, duration_ms: u64) {
    let mut time_ms = start.saturating_mul(1000);
    let mut left = duration_ms;
    while left > 0 {
        let day = time_ms / 86_400_000;
        let take = left.min((day + 1).saturating_mul(86_400_000).saturating_sub(time_ms));
        if take == 0 {
            break;
        }
        *buckets.entry(day).or_default() += take;
        time_ms += take;
        left -= take;
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playtime {
    pub today_ms: u64,
    pub last_7_days_ms: u64,
    pub last_30_days_ms: u64,
    pub all_time_ms: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTarget {
    pub id: String,
    pub instance_id: String,
    pub display_name: String,
    pub last_played_at: u64,
    pub duration_ms: u64,
    pub available: bool,
    pub favorite: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPlaytime {
    pub day: u64,
    pub duration_ms: u64,
}
pub struct Recorder {
    store: Store,
    id: String,
    started_at: u64,
    clock: Instant,
    current: Mutex<
        Option<(
            Mode,
            Instant,
            u64,
            Option<WorldSaveId>,
            Option<ServerTarget>,
            Option<String>,
        )>,
    >,
}
impl Recorder {
    pub fn observe(
        &self,
        mode: Mode,
        world: Option<WorldSaveId>,
        server: Option<ServerTarget>,
        display: Option<String>,
    ) -> Result<(), ()> {
        let elapsed = self.clock.elapsed();
        let stamp = self.started_at.saturating_add(elapsed.as_secs());
        let mut current = self.current.lock().map_err(|_| ())?;
        if current.as_ref().is_some_and(|(m, _, _, w, s, d)| {
            *m == mode && *w == world && *s == server && *d == display
        }) {
            return Ok(());
        }
        let previous = current.take();
        self.store.edit(|doc| {
            if let Some(session) = doc.sessions.iter_mut().find(|s| s.id == self.id) {
                if let Some((m, started, at, w, s, d)) = previous {
                    if session.visits.len() == MAX_VISITS {
                        session.visits.remove(0);
                    }
                    {
                        session.visits.push(Visit {
                            mode: m,
                            started_at: at,
                            ended_at: stamp,
                            duration_ms: started.elapsed().as_millis() as u64,
                            world: w,
                            server: s,
                            display: d,
                        });
                    }
                }
                session.mode = mode;
                session.duration_ms = elapsed.as_millis() as u64;
            }
        })?;
        *current = Some((mode, Instant::now(), stamp, world, server, display));
        Ok(())
    }
    pub fn finish(&self, outcome: Outcome) -> Result<(), ()> {
        let elapsed = self.clock.elapsed();
        let stamp = self.started_at.saturating_add(elapsed.as_secs());
        let previous = self.current.lock().map_err(|_| ())?.take();
        self.store.edit(|doc| {
            if let Some(session) = doc.sessions.iter_mut().find(|s| s.id == self.id) {
                if session.ended_at.is_some() {
                    return;
                }
                if let Some((m, started, at, w, s, d)) = previous {
                    if session.visits.len() == MAX_VISITS {
                        session.visits.remove(0);
                    }
                    {
                        session.visits.push(Visit {
                            mode: m,
                            started_at: at,
                            ended_at: stamp,
                            duration_ms: started.elapsed().as_millis() as u64,
                            world: w,
                            server: s,
                            display: d,
                        });
                    }
                }
                session.ended_at = Some(stamp);
                session.duration_ms = elapsed.as_millis() as u64;
                session.outcome = Some(outcome);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn server_session(index: u64, address: &str, instance: InstanceId) -> SessionRecord {
        SessionRecord {
            id: uuid::Uuid::new_v4().to_string(),
            instance,
            started_at: index,
            ended_at: Some(index + 1),
            duration_ms: 1000,
            outcome: Some(Outcome::Normal),
            mode: Mode::Multiplayer,
            visits: vec![Visit {
                mode: Mode::Multiplayer,
                started_at: index,
                ended_at: index + 1,
                duration_ms: 1000,
                world: None,
                server: Some(ServerTarget::parse(address.into()).unwrap()),
                display: Some(format!("Server {index}")),
            }],
        }
    }
    #[test]
    fn server_retention_favorites_restart_and_old_quick_join() {
        let path = path();
        let store = Store::open(path.clone()).unwrap();
        assert!(
            store
                .recent(Mode::Multiplayer, None, 200)
                .unwrap()
                .is_empty()
        );
        store
            .edit(|doc| doc.sessions.push(server_session(1, "OLD.invalid", id())))
            .unwrap();
        let old = store.recent(Mode::Multiplayer, None, 200).unwrap()[0]
            .id
            .clone();
        store.set_server_favorite(&old, true).unwrap();
        for index in 2..=280 {
            store
                .edit(|doc| {
                    doc.sessions
                        .push(server_session(index, &format!("s{index}.invalid"), id()))
                })
                .unwrap();
        }
        let rows = store.recent(Mode::Multiplayer, None, 200).unwrap();
        assert_eq!(rows.len(), 101);
        assert_eq!(rows[0].last_played_at, 281);
        assert!(rows.last().unwrap().favorite);
        assert!(
            matches!(store.quick_target(Mode::Multiplayer, &old).unwrap(), Some(QuickLaunchTarget::Multiplayer { server, .. }) if server.as_str() == "old.invalid:25565")
        );
        memory().lock().unwrap().remove(&path);
        let restarted = Store::open(path.clone()).unwrap();
        assert_eq!(
            restarted
                .recent(Mode::Multiplayer, None, 200)
                .unwrap()
                .len(),
            101
        );
        assert!(
            restarted
                .recent(Mode::Multiplayer, None, 200)
                .unwrap()
                .last()
                .unwrap()
                .favorite
        );
        restarted
            .edit(|doc| {
                doc.sessions
                    .push(server_session(300, "old.invalid:25565", id()))
            })
            .unwrap();
        let joined = restarted.recent(Mode::Multiplayer, None, 200).unwrap();
        assert_eq!(joined[0].id, old);
        assert_eq!(joined.iter().filter(|s| s.id == old).count(), 1);
        assert!(
            restarted
                .set_server_favorite(&"f".repeat(64), true)
                .is_err()
        );
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn exact_legacy_migration_preserves_visits_and_instance_boundaries() {
        let path = path();
        let other = InstanceId::new("b".repeat(32)).unwrap();
        let legacy = serde_json::json!({"schemaVersion":1,"sessions":[server_session(1,"same.invalid",id()),server_session(2,"SAME.invalid:25565",other.clone())],"archive":[]});
        let bytes = serde_json::to_vec(&legacy).unwrap();
        fs::write(&path, &bytes).unwrap();
        let store = Store::open(path.clone()).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let rows = store.recent(Mode::Multiplayer, None, 200).unwrap();
        assert_eq!(rows.len(), 2);
        assert_ne!(rows[0].id, rows[1].id);
        assert_eq!(
            store
                .recent(Mode::Multiplayer, Some(&other), 200)
                .unwrap()
                .len(),
            1
        );
        store.set_server_favorite(&rows[0].id, true).unwrap();
        let persisted: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted["schemaVersion"], 2);
        assert_eq!(persisted["sessions"], legacy["sessions"]);
        fs::remove_file(path).unwrap();
    }
    fn id() -> InstanceId {
        InstanceId::new("1234567890abcdef1234567890abcdef").unwrap()
    }
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!("aurora-e1-history-{}.json", uuid::Uuid::new_v4()))
    }
    #[test]
    fn world_identity_is_one_safe_component_and_display_is_independent() {
        assert_eq!(
            WorldSaveId::parse("stable-save".into()).unwrap().as_str(),
            "stable-save"
        );
        for value in [
            "",
            ".",
            "..",
            "../x",
            "C:x",
            "a\\b",
            "/tmp/x",
            "CON.txt",
            "ends.",
            " bad",
            "x\0y",
            "x\u{202e}y",
        ] {
            assert!(WorldSaveId::parse(value.into()).is_err(), "{value:?}");
        }
        let target = QuickLaunchTarget::Singleplayer {
            instance: id(),
            world: WorldSaveId::parse("stable-save".into()).unwrap(),
        };
        assert!(!format!("{target:?}").contains("stable-save"));
    }
    #[test]
    fn server_target_is_canonical_and_private() {
        assert_eq!(
            ServerTarget::parse("EXAMPLE.invalid".into())
                .unwrap()
                .as_str(),
            "example.invalid:25565"
        );
        assert_eq!(
            ServerTarget::parse("example.invalid:25565".into())
                .unwrap()
                .as_str(),
            "example.invalid:25565"
        );
        assert_eq!(
            ServerTarget::parse("[::1]:25566".into()).unwrap().as_str(),
            "[::1]:25566"
        );
        for value in [
            "",
            "user@host",
            "http://host",
            "host/path",
            "host:0",
            "host:65536",
            "host:-1",
            "host:abc",
            "::1",
            "a..b",
            "-a.test",
            "host\n",
        ] {
            assert!(ServerTarget::parse(value.into()).is_err(), "{value:?}");
        }
        assert!(
            !format!(
                "{:?}",
                ServerTarget::parse("secret.invalid".into()).unwrap()
            )
            .contains("secret")
        );
    }
    #[test]
    fn daily_buckets_and_window_aggregation() {
        let mut days = BTreeMap::new();
        add_daily(&mut days, 86_399, 2500);
        assert_eq!(days.get(&0), Some(&1000));
        assert_eq!(days.get(&1), Some(&1500));
        let file = path();
        let store = Store::open(file.clone()).unwrap();
        store
            .edit(|doc| {
                let base = now() / 86400 - 30;
                for day in base..=base + 30 {
                    doc.archive.push(DayTotal {
                        day,
                        instance: id(),
                        duration_ms: 1000,
                    });
                }
            })
            .unwrap();
        let summary = store
            .playtime(Some(&id()), (now() / 86400) * 86400)
            .unwrap();
        assert_eq!(
            (
                summary.today_ms,
                summary.last_7_days_ms,
                summary.last_30_days_ms,
                summary.all_time_ms
            ),
            (1000, 7000, 30000, 31000)
        );
        let daily = store.daily_playtime(Some(&id()), now(), 7).unwrap();
        assert_eq!(daily.len(), 7);
        assert!(daily.iter().all(|bucket| bucket.duration_ms == 1000));
        assert!(store.daily_playtime(None, now(), 31).is_err());
        let _ = fs::remove_file(file);
    }
    #[test]
    fn opaque_recent_ids_resolve_only_the_stored_instance_and_target() {
        let file = path();
        let store = Store::open(file.clone()).unwrap();
        let record = store.start(id()).unwrap();
        record
            .observe(
                Mode::Singleplayer,
                Some(WorldSaveId::parse("safe-save".into()).unwrap()),
                None,
                Some("Friendly title".into()),
            )
            .unwrap();
        record
            .observe(
                Mode::Multiplayer,
                None,
                Some(ServerTarget::parse("EXAMPLE.invalid".into()).unwrap()),
                Some("Private server".into()),
            )
            .unwrap();
        record.finish(Outcome::Normal).unwrap();
        let worlds = store.recent(Mode::Singleplayer, None, 5).unwrap();
        let servers = store.recent(Mode::Multiplayer, None, 5).unwrap();
        assert_eq!(worlds.len(), 1);
        assert_eq!(servers.len(), 1);
        assert!(
            matches!(store.quick_target(Mode::Singleplayer, &worlds[0].id).unwrap(), Some(QuickLaunchTarget::Singleplayer { instance, world }) if instance == id() && world.as_str() == "safe-save")
        );
        assert!(
            matches!(store.quick_target(Mode::Multiplayer, &servers[0].id).unwrap(), Some(QuickLaunchTarget::Multiplayer { instance, server }) if instance == id() && server.as_str() == "example.invalid:25565")
        );
        assert!(
            store
                .quick_target(Mode::Multiplayer, &worlds[0].id)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .quick_target(Mode::Singleplayer, &"0".repeat(64))
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .quick_target(Mode::Singleplayer, "../../world")
                .is_err()
        );
        assert!(
            !format!(
                "{:?}",
                store
                    .quick_target(Mode::Multiplayer, &servers[0].id)
                    .unwrap()
                    .unwrap()
            )
            .contains("example.invalid")
        );
        let _ = fs::remove_file(file);
    }
    #[test]
    fn session_start_observe_exit_and_round_trip() {
        let file = path();
        let store = Store::open(file.clone()).unwrap();
        let record = store.start(id()).unwrap();
        assert_eq!(store.sessions().unwrap().len(), 1);
        record.observe(Mode::MainMenu, None, None, None).unwrap();
        record
            .observe(
                Mode::Singleplayer,
                Some(WorldSaveId::parse("stable".into()).unwrap()),
                None,
                Some("Old title".into()),
            )
            .unwrap();
        record
            .observe(
                Mode::Singleplayer,
                Some(WorldSaveId::parse("stable".into()).unwrap()),
                None,
                Some("New title".into()),
            )
            .unwrap();
        record.finish(Outcome::Normal).unwrap();
        let loaded = read(&file).unwrap();
        assert_eq!(loaded.sessions.len(), 1);
        assert!(loaded.sessions[0].ended_at.is_some());
        assert_eq!(loaded.sessions[0].visits.len(), 3);
        assert_eq!(
            loaded.sessions[0].visits[1].world,
            loaded.sessions[0].visits[2].world
        );
        let recent = store.recent(Mode::Singleplayer, Some(&id()), 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].display_name, "New title");
        assert!(!format!("{recent:?}").contains("stable"));
        let _ = fs::remove_file(file);
    }
    #[test]
    fn malformed_future_and_atomic_failure_preserve_original() {
        let file = path();
        fs::write(&file, b"broken").unwrap();
        let original = fs::read(&file).unwrap();
        assert!(Store::open(file.clone()).is_err());
        assert_eq!(fs::read(&file).unwrap(), original);
        fs::write(
            &file,
            b"{\"schemaVersion\":99,\"sessions\":[],\"archive\":[]}",
        )
        .unwrap();
        assert!(Store::open(file.clone()).is_err());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(&file).unwrap()).unwrap()["schemaVersion"],
            99
        );
        let _ = fs::remove_file(file);
    }
    #[test]
    fn no_bridge_abnormal_session_and_retention_keep_playtime() {
        let file = path();
        let store = Store::open(file.clone()).unwrap();
        let session = store.start(id()).unwrap();
        session.finish(Outcome::Abnormal).unwrap();
        let first = store.sessions().unwrap().remove(0);
        assert_eq!(first.mode, Mode::Unknown);
        assert_eq!(first.outcome, Some(Outcome::Abnormal));
        let base = now();
        store
            .edit(|doc| {
                doc.sessions.clear();
                for index in 0..=MAX_SESSIONS {
                    doc.sessions.push(SessionRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        instance: id(),
                        started_at: base - index as u64,
                        ended_at: Some(base - index as u64 + 1),
                        duration_ms: 1000,
                        outcome: Some(Outcome::Normal),
                        mode: Mode::Unknown,
                        visits: Vec::new(),
                    });
                }
            })
            .unwrap();
        assert_eq!(store.sessions().unwrap().len(), MAX_SESSIONS);
        assert_eq!(
            store.playtime(Some(&id()), base).unwrap().all_time_ms,
            (MAX_SESSIONS as u64 + 1) * 1000
        );
        let _ = fs::remove_file(file);
    }
    #[test]
    fn repeated_server_visits_update_recency_without_exposing_address() {
        let file = path();
        let store = Store::open(file.clone()).unwrap();
        let start = now();
        let server = ServerTarget::parse("EXAMPLE.invalid".into()).unwrap();
        store
            .edit(|doc| {
                doc.sessions.push(SessionRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    instance: id(),
                    started_at: start,
                    ended_at: Some(start + 10),
                    duration_ms: 10_000,
                    outcome: Some(Outcome::Normal),
                    mode: Mode::Multiplayer,
                    visits: vec![
                        Visit {
                            mode: Mode::Multiplayer,
                            started_at: start,
                            ended_at: start + 4,
                            duration_ms: 4000,
                            world: None,
                            server: Some(server.clone()),
                            display: Some("Old label".into()),
                        },
                        Visit {
                            mode: Mode::Multiplayer,
                            started_at: start + 4,
                            ended_at: start + 10,
                            duration_ms: 6000,
                            world: None,
                            server: Some(server),
                            display: Some("New label".into()),
                        },
                    ],
                });
            })
            .unwrap();
        let recent = store.recent(Mode::Multiplayer, None, 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].display_name, "New label");
        assert_eq!(recent[0].duration_ms, 10_000);
        assert!(!format!("{recent:?}").contains("example.invalid"));
        let _ = fs::remove_file(file);
    }
}
