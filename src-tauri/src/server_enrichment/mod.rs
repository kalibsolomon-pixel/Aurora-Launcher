//! Bounded presentation enrichment for the Home Recent Servers widget.
//!
//! The launcher's gameplay history stays the sole source of truth for which
//! servers were visited, when, and through which instance; nothing here writes
//! history. This module answers a separate question — "what should that server
//! look like in the launcher?" — by querying the Minecraft server-status
//! protocol on demand for endpoints already present in persisted history, and
//! caching the sanitized presentation under a launcher-owned, bounded,
//! damage-tolerant store.
//!
//! Privacy: status queries go only to the server endpoint itself (the stored
//! connection target, resolved through the system resolver). No Aurora
//! infrastructure, provider, or third party receives server history, and
//! Discord preferences are untouched. Presentation output never carries the
//! endpoint address as connection authority and never feeds back into launch
//! authority: rejoin continues to resolve the same opaque history target ID
//! natively.

pub mod motd;
pub mod protocol;

use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use base64ct::{Base64, Encoding as _};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::gameplay_history::{Mode, ServerTarget, Store};
use crate::paths::ManagedPaths;

pub use motd::MotdSegment;
pub use protocol::QueryLimits;

const CACHE_SCHEMA: u32 = 1;
/// One status document (including a maximum bounded favicon as a data URL)
/// cannot exceed this; larger files are treated as damaged cache objects.
const MAX_CACHE_DOCUMENT: usize = 384 * 1024;
/// Decoded favicon bytes are bounded well beyond the vanilla 64×64 icon.
const MAX_FAVICON_BYTES: usize = 128 * 1024;
/// A server favicon must decode as a PNG no larger than this square.
const MAX_FAVICON_DIMENSION: u32 = 512;
/// Server-provided version text is sanitized and bounded for tooltips.
const MAX_VERSION_CHARS: usize = 64;
/// A successful status answer stays fresh for this long; repeated widget
/// navigation inside the window performs no network traffic.
const SUCCESS_TTL: u64 = 10 * 60;
/// Consecutive failures back off exponentially, capped, so unreachable
/// servers are not hammered on every widget load.
const FAILURE_BACKOFF_BASE: u64 = 60;
const FAILURE_BACKOFF_CAP: u64 = 30 * 60;
/// At most this many servers are contacted simultaneously during one refresh.
const REFRESH_CONCURRENCY: usize = 4;
/// History-driven bound on one refresh: `recent` itself is capped at 20.
const MAX_TARGETS: usize = 20;

/// The display names that carry no recognizable identity: Minecraft's own
/// default server-list name and Aurora's history fallback.
fn is_generic_server_name(name: &str) -> bool {
    matches!(name.trim(), "Minecraft Server" | "Multiplayer server")
}

/// The strongest truthful display identity for one enriched entry: the saved
/// server-list name when it is meaningful, otherwise the sanitized host.
fn resolve_name(display: Option<&str>, host: &str) -> String {
    match display.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) if !is_generic_server_name(name) => name.to_owned(),
        _ => host.trim_matches(['[', ']']).to_owned(),
    }
}

/// A normalized enrichment endpoint derived from a validated history target.
/// The cache key is the SHA-256 of the canonical `host:port` form, so cache
/// object names can never contain traversal or host-controlled characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Endpoint {
    host: String,
    port: u16,
    key: String,
}
impl Endpoint {
    pub fn from_target(server: &ServerTarget) -> Result<Self, ()> {
        let raw = server.as_str();
        let (host, port) = raw
            .rsplit_once(':')
            .and_then(|(host, port)| port.parse::<u16>().ok().map(|port| (host, port)))
            .ok_or(())?;
        if host.is_empty() || port == 0 {
            return Err(());
        }
        let mut digest = Sha256::new();
        digest.update(raw.as_bytes());
        Ok(Self {
            host: host.to_owned(),
            port,
            key: format!("{:x}", digest.finalize()),
        })
    }
    pub fn host(&self) -> &str {
        &self.host
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn cache_key(&self) -> &str {
        &self.key
    }
    /// The cache object name is derived purely from the hex digest.
    fn cache_path(paths: &ManagedPaths, key: &str) -> Option<PathBuf> {
        (key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| {
            paths
                .cache_dir()
                .join("server-enrichment")
                .join(format!("{key}.json"))
        })
    }
}

// ---------------------------------------------------------------------------
// Persisted presentation cache
//
// Non-authoritative, independently replaceable objects: malformed, oversized
// or future-schema documents are treated as absent, and only a freshly
// normalized (successful or failure-recorded) write replaces a stored one.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CachedPresentation {
    online: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version_text: Option<String>,
    #[serde(default)]
    motd: Vec<Vec<MotdSegment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    players_online: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    players_max: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    favicon: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CacheDocument {
    schema_version: u32,
    /// Canonical `host:port`; a document that names another endpoint is
    /// never served for this one.
    identity: String,
    saved_at: u64,
    failure_count: u32,
    /// Zero when the next widget load may refresh freely.
    next_attempt_at: u64,
    #[serde(default)]
    presentation: Option<CachedPresentation>,
}

fn read_document(paths: &ManagedPaths, endpoint: &Endpoint) -> Option<CacheDocument> {
    let path = Endpoint::cache_path(paths, endpoint.cache_key())?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take((MAX_CACHE_DOCUMENT + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_CACHE_DOCUMENT {
        return None;
    }
    let mut document: CacheDocument = serde_json::from_slice(&bytes).ok()?;
    // The cosmetic cache is not a trust authority. Revalidate stored imagery
    // without network traffic before exposing it through a native DTO.
    if let Some(presentation) = &mut document.presentation {
        presentation.favicon = presentation.favicon.as_deref().and_then(validate_favicon);
    }
    (document.schema_version == CACHE_SCHEMA
        && document.identity == format!("{}:{}", endpoint.host, endpoint.port))
    .then_some(document)
}

fn write_document(path: &Path, document: &CacheDocument) -> Option<()> {
    let bytes = serde_json::to_vec(document).ok()?;
    if bytes.len() > MAX_CACHE_DOCUMENT {
        return None;
    }
    let parent = path.parent()?;
    std::fs::create_dir_all(parent).ok()?;
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .ok()?;
        file.write_all(&bytes).ok()?;
        file.sync_all().ok()?;
        std::fs::rename(&temporary, path).ok()
    })();
    if result.is_none() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn failure_backoff(failures: u32) -> u64 {
    let exponent = failures.saturating_sub(1).min(5);
    FAILURE_BACKOFF_BASE
        .saturating_mul(1u64 << exponent)
        .min(FAILURE_BACKOFF_CAP)
}

fn in_flight() -> &'static Mutex<HashSet<String>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

// ---------------------------------------------------------------------------
// Favicon validation: a server-controlled string that must become renderable
// image data or nothing. Only data-URL PNG (decoded and dimension-checked)
// and JPEG (magic-checked) pass; SVG and every other media type are rejected.
// ---------------------------------------------------------------------------

fn validate_favicon(raw: &str) -> Option<String> {
    // Normalize MIME-style base64 line wrapping into one canonical data URL.
    let raw = raw.trim();
    let (media, encoded) = if let Some(encoded) = raw.strip_prefix("data:image/png;base64,") {
        ("image/png", encoded)
    } else if let Some(encoded) = raw.strip_prefix("data:image/jpeg;base64,") {
        ("image/jpeg", encoded)
    } else {
        return None;
    };
    if encoded.len() > MAX_FAVICON_BYTES.div_ceil(3) * 4 + 4096 {
        return None;
    }
    let encoded: String = encoded
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    let bytes = Base64::decode_vec(&encoded).ok()?;
    if bytes.is_empty() || bytes.len() > MAX_FAVICON_BYTES {
        return None;
    }
    match media {
        "image/png" => {
            let mut decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
            decoder.set_limits(png::Limits {
                bytes: 4 * 1024 * 1024,
            });
            let mut reader = decoder.read_info().ok()?;
            let info = reader.info();
            if info.width == 0
                || info.height == 0
                || info.width > MAX_FAVICON_DIMENSION
                || info.height > MAX_FAVICON_DIMENSION
            {
                return None;
            }
            // Full decode proves the payload is a structurally valid image.
            let mut pixels = vec![0u8; reader.output_buffer_size()];
            reader.next_frame(&mut pixels).ok()?;
        }
        _ => {
            if bytes.len() < 3 || bytes[0] != 0xFF || bytes[1] != 0xD8 || bytes[2] != 0xFF {
                return None;
            }
        }
    }
    Some(format!(
        "data:{media};base64,{}",
        Base64::encode_string(&bytes)
    ))
}

/// Normalizes one raw status response into cacheable presentation facts.
/// Every field is individually optional and bounded; a response whose only
/// defect is a hostile favicon still contributes its other fields.
fn normalize(response: &protocol::StatusResponse) -> CachedPresentation {
    let version_text = response
        .version_text
        .as_deref()
        .map(crate::launch::activity_bridge::sanitize)
        .filter(|text| !text.is_empty())
        .map(|text| text.chars().take(MAX_VERSION_CHARS).collect::<String>());
    CachedPresentation {
        online: true,
        version_text,
        motd: motd::convert(&response.description),
        players_online: response.players_online,
        players_max: response.players_max,
        latency_ms: Some(response.latency.as_millis().try_into().unwrap_or(u32::MAX)),
        favicon: response.favicon.as_deref().and_then(validate_favicon),
    }
}

/// The presentation DTO returned to the frontend. Deliberately carries no
/// endpoint address field, path, instance, or launch authority — only
/// display facts. (The display name may fall back to the sanitized host when
/// no meaningful identity exists, which is display text, not a connector.)
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentServerPresentation {
    pub target_id: String,
    pub status: &'static str,
    pub name: Option<String>,
    pub motd: Vec<Vec<MotdSegment>>,
    pub players_online: Option<u32>,
    pub players_max: Option<u32>,
    pub version_text: Option<String>,
    pub latency_ms: Option<u32>,
    pub favicon: Option<String>,
}

/// Presentation refresh options; defaults match product behavior, tests
/// narrow the timeouts and concurrency.
#[derive(Clone, Copy, Debug)]
pub struct RefreshOptions {
    pub limits: QueryLimits,
    pub concurrency: usize,
}
impl Default for RefreshOptions {
    fn default() -> Self {
        Self {
            limits: QueryLimits::default(),
            concurrency: REFRESH_CONCURRENCY,
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

struct RequestedTarget {
    id: String,
    display: Option<String>,
    endpoint: Endpoint,
}

/// Cache-first, on-demand enrichment for opaque recent-server target IDs.
///
/// Every ID is resolved natively through history (the frontend never supplies
/// an address); equivalent endpoints are deduplicated; fresh cache entries and
/// backoff windows skip the network; live queries run under bounded
/// concurrency; and one server's failure never fails the others. History is
/// never written.
pub async fn refresh(
    paths: &ManagedPaths,
    store: &Store,
    target_ids: &[String],
    options: RefreshOptions,
) -> Vec<RecentServerPresentation> {
    if target_ids.is_empty() || target_ids.len() > MAX_TARGETS {
        return Vec::new();
    }
    let displays: HashMap<String, String> = store
        .recent(Mode::Multiplayer, None, MAX_TARGETS)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.id, entry.display_name))
        .collect();

    let mut targets: Vec<RequestedTarget> = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut endpoints: Vec<Endpoint> = Vec::new();
    let mut by_endpoint: HashMap<String, Vec<usize>> = HashMap::new();
    for id in target_ids {
        if !seen_ids.insert(id.clone()) {
            continue;
        }
        let Ok(Some(crate::gameplay_history::QuickLaunchTarget::Multiplayer { server, .. })) =
            store.quick_target(Mode::Multiplayer, id)
        else {
            continue;
        };
        let Ok(endpoint) = Endpoint::from_target(&server) else {
            continue;
        };
        if !by_endpoint.contains_key(endpoint.cache_key()) {
            endpoints.push(endpoint.clone());
        }
        by_endpoint
            .entry(endpoint.cache_key().to_owned())
            .or_default()
            .push(targets.len());
        targets.push(RequestedTarget {
            id: id.clone(),
            display: displays.get(id).cloned(),
            endpoint,
        });
    }

    let now = now_secs();
    let mut documents: HashMap<String, CacheDocument> = HashMap::new();
    let mut work: Vec<Endpoint> = Vec::new();
    for endpoint in &endpoints {
        let document = read_document(paths, endpoint);
        let skip = document.as_ref().is_some_and(|document| {
            (document
                .presentation
                .as_ref()
                .is_some_and(|presentation| presentation.online)
                && now < document.saved_at.saturating_add(SUCCESS_TTL))
                || now < document.next_attempt_at
        });
        if !skip {
            work.push(endpoint.clone());
        }
        if let Some(document) = document {
            documents.insert(endpoint.cache_key().to_owned(), document);
        }
    }

    let paths = Arc::new(paths.clone());
    let queries = futures_util::stream::iter(work.into_iter().map(|endpoint| {
        let paths = Arc::clone(&paths);
        let limits = options.limits;
        async move {
            let result = query_endpoint(&paths, &endpoint, limits).await;
            (endpoint, result)
        }
    }))
    .buffer_unordered(options.concurrency.max(1));
    futures_util::pin_mut!(queries);
    while let Some((endpoint, result)) = queries.next().await {
        let key = endpoint.cache_key().to_owned();
        if result.is_none() {
            // An equivalent query for this endpoint is already in flight;
            // whatever the cache holds stays authoritative this pass.
            continue;
        }
        let mut document = documents.remove(&key).unwrap_or(CacheDocument {
            schema_version: CACHE_SCHEMA,
            identity: format!("{}:{}", endpoint.host, endpoint.port),
            saved_at: 0,
            failure_count: 0,
            next_attempt_at: 0,
            presentation: None,
        });
        document.saved_at = now_secs();
        match result.expect("in-flight skip handled above") {
            QueryOutcome::Reached(presentation) => {
                document.presentation = Some(presentation);
                document.failure_count = 0;
                document.next_attempt_at = 0;
            }
            QueryOutcome::Unreachable(retained) => {
                // A failed refresh retains the known-good presentation marked
                // offline; only the failure bookkeeping advances.
                document.presentation = Some(retained);
                document.failure_count = document.failure_count.saturating_add(1);
                document.next_attempt_at =
                    now_secs().saturating_add(failure_backoff(document.failure_count));
            }
        }
        if let Some(path) = Endpoint::cache_path(&paths, &key) {
            let _ = write_document(&path, &document);
        }
        documents.insert(key, document);
    }

    targets
        .into_iter()
        .map(|target| {
            let presentation = documents
                .get(target.endpoint.cache_key())
                .and_then(|document| document.presentation.as_ref());
            RecentServerPresentation {
                target_id: target.id,
                status: presentation
                    .map(|presentation| {
                        if presentation.online {
                            "online"
                        } else {
                            "offline"
                        }
                    })
                    .unwrap_or("offline"),
                name: Some(resolve_name(
                    target.display.as_deref(),
                    target.endpoint.host(),
                )),
                motd: presentation.map(|p| p.motd.clone()).unwrap_or_default(),
                players_online: presentation.and_then(|p| p.players_online),
                players_max: presentation.and_then(|p| p.players_max),
                version_text: presentation.and_then(|p| p.version_text.clone()),
                latency_ms: presentation.and_then(|p| p.latency_ms),
                favicon: presentation.and_then(|p| p.favicon.clone()),
            }
        })
        .collect()
}

/// The outcome of one endpoint's status query.
enum QueryOutcome {
    /// The server answered; the payload is the normalized presentation.
    Reached(CachedPresentation),
    /// The bounded query failed. The payload is the retained known-good
    /// presentation (if any) marked offline, so a temporarily unreachable
    /// server never erases its cached favicon or MOTD.
    Unreachable(CachedPresentation),
}

/// Performs one endpoint's status query. The connect target honors SRV
/// resolution for DNS hosts; the handshake keeps the stored host so
/// virtual-host routing sees the address the player used.
///
/// Returns `None` when an equivalent query is already running for this
/// endpoint (this pass falls back to the cache instead of duplicating work).
async fn query_endpoint(
    paths: &ManagedPaths,
    endpoint: &Endpoint,
    limits: QueryLimits,
) -> Option<QueryOutcome> {
    let handshake_host = endpoint.host.trim_start_matches('[').trim_end_matches(']');
    let (connect_host, connect_port) = protocol::resolve_srv(endpoint.host())
        .map(|(target, port)| (target, port))
        .unwrap_or_else(|| (handshake_host.to_owned(), endpoint.port));
    if !in_flight()
        .lock()
        .map(|mut set| set.insert(endpoint.cache_key().to_owned()))
        .unwrap_or(false)
    {
        return None;
    }
    let result = protocol::query(handshake_host, &connect_host, connect_port, limits).await;
    if let Ok(mut guard) = in_flight().lock() {
        guard.remove(endpoint.cache_key());
    }
    Some(match result {
        Ok(response) => QueryOutcome::Reached(normalize(&response)),
        Err(()) => {
            let mut presentation = read_document(paths, endpoint)
                .and_then(|document| document.presentation)
                .unwrap_or_default();
            presentation.online = false;
            presentation.latency_ms = None;
            presentation.players_online = None;
            presentation.players_max = None;
            QueryOutcome::Unreachable(presentation)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_history::Outcome;
    use crate::instances::InstanceId;
    use crate::test_support::RawTcpServer;
    use serde_json::json;
    use std::io::Read as _;
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn managed() -> ManagedPaths {
        ManagedPaths::from_app_local_data_dir(
            std::env::temp_dir().join(format!("aurora-server-enrichment-{}", uuid::Uuid::new_v4())),
        )
        .unwrap()
    }

    fn history() -> (ManagedPaths, Store) {
        let paths = managed();
        let store = Store::open(paths.gameplay_history_file()).unwrap();
        (paths, store)
    }

    fn instance() -> InstanceId {
        InstanceId::new("1234567890abcdef1234567890abcdef").unwrap()
    }

    /// Records one finished multiplayer visit through the public history API,
    /// returning the opaque recent-target id it produces. Distinct callers
    /// must pass distinct display names so the id can be located reliably.
    fn seed_server_visit(store: &Store, host: &str, display: Option<&str>) -> String {
        let recorder = store.start(instance()).unwrap();
        recorder
            .observe(
                Mode::Multiplayer,
                None,
                Some(ServerTarget::parse(host.to_owned()).unwrap()),
                display.map(str::to_owned),
            )
            .unwrap();
        recorder.finish(Outcome::Normal).unwrap();
        let recent = store.recent(Mode::Multiplayer, None, 20).unwrap();
        match display {
            Some(name) => recent.iter().find(|entry| entry.display_name == name),
            None => recent
                .iter()
                .find(|entry| entry.display_name == "Multiplayer server"),
        }
        .unwrap_or(&recent[0])
        .id
        .clone()
    }

    /// Builds a complete framed status-response packet for a JSON value.
    fn status_packet(payload: &serde_json::Value) -> Vec<u8> {
        let json = serde_json::to_vec(payload).unwrap();
        let mut inner = Vec::new();
        protocol::write_varint(&mut inner, 0x00);
        protocol::write_varint(&mut inner, json.len() as u32);
        inner.extend_from_slice(&json);
        let mut framed = Vec::new();
        protocol::write_varint(&mut framed, inner.len() as u32);
        framed.extend_from_slice(&inner);
        framed
    }

    /// Drains the client's handshake + request, then writes scripted bytes.
    fn answer_with(bytes: Vec<u8>) -> Arc<dyn Fn(TcpStream) + Send + Sync> {
        Arc::new(move |mut stream| {
            let mut buffer = [0u8; 512];
            let _ = stream.read(&mut buffer);
            let _ = stream.write_all(&bytes);
            let _ = stream.flush();
        })
    }

    fn minimal_status_json() -> serde_json::Value {
        json!({
            "version": { "name": "1.21.11", "protocol": 772 },
            "players": { "online": 7, "max": 100 },
            "description": { "text": "A friendly place" }
        })
    }

    fn small_png() -> Vec<u8> {
        let mut png_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_bytes, 64, 64);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&vec![127u8; 64 * 64 * 4]).unwrap();
        }
        png_bytes
    }

    fn png_favicon_field() -> String {
        format!(
            "data:image/png;base64,{}",
            Base64::encode_string(&small_png())
        )
    }

    fn fast_options() -> RefreshOptions {
        RefreshOptions {
            limits: QueryLimits {
                connect_timeout: Duration::from_millis(400),
                total_timeout: Duration::from_millis(800),
            },
            concurrency: 2,
        }
    }

    #[test]
    fn endpoint_identity_defaults_ports_and_hashes_safely() {
        let target = ServerTarget::parse("EXAMPLE.invalid".into()).unwrap();
        let endpoint = Endpoint::from_target(&target).unwrap();
        assert_eq!(endpoint.host(), "example.invalid");
        assert_eq!(endpoint.port(), 25565);
        let key = endpoint.cache_key().to_owned();
        assert_eq!(key.len(), 64);
        assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));
        // Default and explicit default ports normalize to one endpoint.
        let explicit = ServerTarget::parse("example.invalid:25565".into()).unwrap();
        assert_eq!(endpoint, Endpoint::from_target(&explicit).unwrap());
        // A different port is a different endpoint.
        let other = ServerTarget::parse("example.invalid:25566".into()).unwrap();
        assert_ne!(endpoint, Endpoint::from_target(&other).unwrap());
        // Cache object names are pure hex digests — never host bytes.
        let path = Endpoint::cache_path(&managed(), &key).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name.trim_end_matches(".json")
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        );
        // Pathological host text cannot reach path construction at all; a
        // valid-but-odd hostname can never traverse because cache object
        // names are hex digests.
        for hostile in ["../escape", "a\\b", "..", "host/path"] {
            assert!(
                ServerTarget::parse(hostile.to_owned()).is_err(),
                "{hostile}"
            );
        }
    }

    #[test]
    fn display_name_priority_and_generic_fallback() {
        assert_eq!(
            resolve_name(Some("  My Realm  "), "mc.example.invalid"),
            "My Realm"
        );
        assert_eq!(
            resolve_name(Some("Minecraft Server"), "mc.example.invalid"),
            "mc.example.invalid"
        );
        assert_eq!(
            resolve_name(Some("Multiplayer server"), "mc.example.invalid"),
            "mc.example.invalid"
        );
        assert_eq!(
            resolve_name(None, "mc.example.invalid"),
            "mc.example.invalid"
        );
        assert_eq!(
            resolve_name(Some("  "), "mc.example.invalid"),
            "mc.example.invalid"
        );
        assert_eq!(resolve_name(None, "[::1]"), "::1");
    }

    #[test]
    fn favicon_validation_accepts_only_bounded_decodable_images() {
        let valid = validate_favicon(&png_favicon_field()).unwrap();
        assert!(valid.starts_with("data:image/png;base64,"));
        // Non-image or unsupported media types never render.
        assert!(validate_favicon("data:image/svg+xml;base64,AAAA").is_none());
        assert!(validate_favicon("https://example.invalid/icon.png").is_none());
        assert!(validate_favicon("plain text").is_none());
        // Malformed payloads fail safely.
        assert!(validate_favicon("data:image/png;base64,AAAA").is_none());
        assert!(
            validate_favicon(&format!(
                "data:image/png;base64,{}",
                Base64::encode_string(b"<html>bytes")
            ))
            .is_none()
        );
        // Oversized decoded payloads are rejected before decoding.
        let oversized = vec![0u8; MAX_FAVICON_BYTES + 1];
        assert!(
            validate_favicon(&format!(
                "data:image/png;base64,{}",
                Base64::encode_string(&oversized)
            ))
            .is_none()
        );
        // Valid JPEG magic passes; anything else with a jpeg prefix fails.
        assert!(
            validate_favicon(&format!(
                "data:image/jpeg;base64,{}",
                Base64::encode_string(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00])
            ))
            .is_some()
        );
        assert!(
            validate_favicon(&format!(
                "data:image/jpeg;base64,{}",
                Base64::encode_string(&[0xFF, 0xD8, 0x00])
            ))
            .is_none()
        );
        // Invalid wrapped bytes still fail. Valid MIME-style wrapping is normalized.
        assert!(validate_favicon(&format!("data:image/png;base64,AA AA",)).is_none());
        let canonical = png_favicon_field();
        let (prefix, encoded) = canonical.split_once(',').unwrap();
        let wrapped = encoded
            .as_bytes()
            .chunks(76)
            .map(|chunk| std::str::from_utf8(chunk).unwrap())
            .collect::<Vec<_>>()
            .join("\r\n");
        assert_eq!(
            validate_favicon(&format!("{prefix},{wrapped}")),
            Some(canonical)
        );
        assert!(
            validate_favicon(&format!(
                " data:image/png;base64,{} ",
                Base64::encode_string(&small_png())
            ))
            .is_some()
        );
    }

    #[test]
    fn cache_documents_round_trip_and_damaged_cache_is_absent() {
        let paths = managed();
        let endpoint = Endpoint::from_target(
            &ServerTarget::parse("cache.example.invalid:25565".into()).unwrap(),
        )
        .unwrap();
        let path = Endpoint::cache_path(&paths, endpoint.cache_key()).unwrap();
        let document = CacheDocument {
            schema_version: CACHE_SCHEMA,
            identity: "cache.example.invalid:25565".into(),
            saved_at: 1_000,
            failure_count: 0,
            next_attempt_at: 0,
            presentation: Some(CachedPresentation {
                online: true,
                version_text: Some("1.21.11".into()),
                motd: vec![vec![MotdSegment {
                    text: "cached".into(),
                    color: None,
                    bold: false,
                    italic: false,
                    underline: false,
                }]],
                players_online: Some(3),
                players_max: Some(10),
                latency_ms: Some(12),
                favicon: None,
            }),
        };
        write_document(&path, &document).unwrap();
        assert_eq!(read_document(&paths, &endpoint).unwrap(), document);
        let mut poisoned = document.clone();
        poisoned.presentation.as_mut().unwrap().favicon =
            Some("https://example.invalid/favicon.png".into());
        write_document(&path, &poisoned).unwrap();
        assert_eq!(
            read_document(&paths, &endpoint)
                .unwrap()
                .presentation
                .unwrap()
                .favicon,
            None
        );
        // A document naming a different identity is never served.
        let mut alien = document.clone();
        alien.identity = "other.example.invalid:25565".into();
        write_document(&path, &alien).unwrap();
        assert!(read_document(&paths, &endpoint).is_none());
        // Damaged JSON and future schemas are treated as absent.
        std::fs::write(&path, b"{ broken").unwrap();
        assert!(read_document(&paths, &endpoint).is_none());
        let mut future = document.clone();
        future.schema_version = 99;
        write_document(&path, &future).unwrap();
        assert!(read_document(&paths, &endpoint).is_none());
        // Two identities never share a cache object.
        let other = Endpoint::from_target(
            &ServerTarget::parse("cache.example.invalid:25566".into()).unwrap(),
        )
        .unwrap();
        assert_ne!(
            path,
            Endpoint::cache_path(&paths, other.cache_key()).unwrap()
        );
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[test]
    fn failure_backoff_grows_and_caps() {
        assert_eq!(failure_backoff(1), 60);
        assert_eq!(failure_backoff(2), 120);
        assert_eq!(failure_backoff(3), 240);
        assert_eq!(failure_backoff(5), 960);
        assert_eq!(failure_backoff(6), 1800);
        assert_eq!(failure_backoff(9), 1800);
        assert_eq!(failure_backoff(100), 1800);
    }

    #[tokio::test]
    async fn existing_history_enriches_without_a_new_visit() {
        let (paths, store) = history();
        let server = RawTcpServer::spawn(answer_with(status_packet(&minimal_status_json())));
        let host = format!("127.0.0.1:{}", server.port());
        let id = seed_server_visit(&store, &host, Some("Minecraft Server"));
        let sessions_before = store.sessions().unwrap().len();

        let presentations = refresh(&paths, &store, &[id.clone()], fast_options()).await;
        assert_eq!(presentations.len(), 1);
        let presentation = &presentations[0];
        assert_eq!(presentation.target_id, id);
        assert_eq!(presentation.status, "online");
        // Generic display name falls back to the sanitized host.
        assert_eq!(presentation.name.as_deref(), Some("127.0.0.1"));
        assert_eq!(presentation.players_online, Some(7));
        assert_eq!(presentation.players_max, Some(100));
        assert_eq!(presentation.version_text.as_deref(), Some("1.21.11"));
        assert_eq!(presentation.motd.len(), 1);
        assert_eq!(presentation.motd[0][0].text, "A friendly place");
        // The DTO surface is presentation-only: no instance, endpoint, or
        // connection authority fields exist on it.
        let serialized: serde_json::Value = serde_json::to_value(presentation).unwrap();
        assert_eq!(
            serialized.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec![
                "favicon",
                "latencyMs",
                "motd",
                "name",
                "playersMax",
                "playersOnline",
                "status",
                "targetId",
                "versionText"
            ]
        );
        // History was not written: no new sessions.
        assert_eq!(store.sessions().unwrap().len(), sessions_before);
        // The presentation persisted to the cache.
        let endpoint =
            Endpoint::from_target(&ServerTarget::parse(host.to_owned()).unwrap()).unwrap();
        assert!(read_document(&paths, &endpoint).is_some());
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn favicon_round_trips_through_refresh_and_survives_offline() {
        let (paths, store) = history();
        let mut with_icon = minimal_status_json();
        with_icon["favicon"] = json!(png_favicon_field());
        let server = RawTcpServer::spawn(answer_with(status_packet(&with_icon)));
        let host = format!("127.0.0.1:{}", server.port());
        let id = seed_server_visit(&store, &host, Some("Named Realm"));
        let presentations = refresh(&paths, &store, &[id.clone()], fast_options()).await;
        let favicon = presentations[0].favicon.clone().unwrap();
        assert!(favicon.starts_with("data:image/png;base64,"));
        server.stop();
        // The server is now unreachable: the cached favicon and MOTD remain,
        // status reports offline, and no failure state replaces the row.
        let endpoint =
            Endpoint::from_target(&ServerTarget::parse(host.to_owned()).unwrap()).unwrap();
        let mut doc = read_document(&paths, &endpoint).unwrap();
        // Force a refresh attempt: expire both the success TTL and backoff.
        doc.saved_at = now_secs().saturating_sub(SUCCESS_TTL + 1);
        doc.next_attempt_at = 0;
        let path = Endpoint::cache_path(&paths, endpoint.cache_key()).unwrap();
        write_document(&path, &doc).unwrap();
        let presentations = refresh(&paths, &store, &[id], fast_options()).await;
        assert_eq!(presentations[0].status, "offline");
        assert_eq!(presentations[0].favicon.as_deref(), Some(favicon.as_str()));
        assert_eq!(presentations[0].motd.len(), 1);
        assert_eq!(presentations[0].players_online, None);
        assert_eq!(presentations[0].latency_ms, None);
        // Backoff was recorded after the failure.
        let doc = read_document(&paths, &endpoint).unwrap();
        assert_eq!(doc.failure_count, 1);
        assert!(doc.next_attempt_at > now_secs());
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn fresh_cache_skips_the_network_and_stale_cache_refreshes() {
        let (paths, store) = history();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let server = RawTcpServer::spawn(Arc::new(move |mut stream| {
            counter.fetch_add(1, Ordering::SeqCst);
            let mut buffer = [0u8; 512];
            let _ = stream.read(&mut buffer);
            let mut payload = minimal_status_json();
            payload["description"] = json!("refreshed");
            let _ = stream.write_all(&status_packet(&payload));
            let _ = stream.flush();
        }));
        let host = format!("127.0.0.1:{}", server.port());
        let id = seed_server_visit(&store, &host, None);
        let first = refresh(&paths, &store, &[id.clone()], fast_options()).await;
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        // A fresh success is served entirely from cache.
        let second = refresh(&paths, &store, &[id.clone()], fast_options()).await;
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(second, first);
        // Expiring the TTL allows one bounded refresh.
        let endpoint =
            Endpoint::from_target(&ServerTarget::parse(host.to_owned()).unwrap()).unwrap();
        let mut doc = read_document(&paths, &endpoint).unwrap();
        doc.saved_at = now_secs().saturating_sub(SUCCESS_TTL + 1);
        let path = Endpoint::cache_path(&paths, endpoint.cache_key()).unwrap();
        write_document(&path, &doc).unwrap();
        let third = refresh(&paths, &store, &[id], fast_options()).await;
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        assert_eq!(third[0].motd[0][0].text, "refreshed");
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn one_failing_server_never_fails_its_neighbors() {
        let (paths, store) = history();
        let good = RawTcpServer::spawn(answer_with(status_packet(&minimal_status_json())));
        let hanging = RawTcpServer::spawn(Arc::new(|mut stream| {
            let mut buffer = [0u8; 512];
            let _ = stream.read(&mut buffer);
            std::thread::sleep(Duration::from_millis(2_000));
        }));
        let good_host = format!("127.0.0.1:{}", good.port());
        let hanging_host = format!("127.0.0.1:{}", hanging.port());
        let good_id = seed_server_visit(&store, &good_host, Some("Good Realm"));
        let hanging_id = seed_server_visit(&store, &hanging_host, Some("Slow Realm"));
        let presentations = refresh(
            &paths,
            &store,
            &[good_id.clone(), hanging_id.clone()],
            fast_options(),
        )
        .await;
        assert_eq!(presentations.len(), 2);
        let good = presentations
            .iter()
            .find(|p| p.target_id == good_id)
            .unwrap();
        let hanging = presentations
            .iter()
            .find(|p| p.target_id == hanging_id)
            .unwrap();
        assert_eq!(good.status, "online");
        assert_eq!(good.motd[0][0].text, "A friendly place");
        assert_eq!(hanging.status, "offline");
        assert!(hanging.motd.is_empty());
        assert_eq!(hanging.name.as_deref(), Some("Slow Realm"));
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn duplicate_endpoints_are_queried_once_per_refresh() {
        let (paths, store) = history();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let server = RawTcpServer::spawn(Arc::new(move |mut stream| {
            counter.fetch_add(1, Ordering::SeqCst);
            let mut buffer = [0u8; 512];
            let _ = stream.read(&mut buffer);
            let _ = stream.write_all(&status_packet(&minimal_status_json()));
        }));
        let host = format!("127.0.0.1:{}", server.port());
        // One history entry per instance: two recent targets, one endpoint.
        let first = seed_server_visit(&store, &host, Some("Same place"));
        let recorder = store
            .start(InstanceId::new("abcdef1234567890abcdef1234567890").unwrap())
            .unwrap();
        recorder
            .observe(
                Mode::Multiplayer,
                None,
                Some(ServerTarget::parse(host.to_owned()).unwrap()),
                Some("Same place".to_owned()),
            )
            .unwrap();
        recorder.finish(Outcome::Normal).unwrap();
        let recent = store.recent(Mode::Multiplayer, None, 10).unwrap();
        assert_eq!(recent.len(), 2);
        let second = recent
            .iter()
            .find(|entry| entry.id != first)
            .unwrap()
            .id
            .clone();
        // Both the same id twice and two ids for one endpoint query once.
        let presentations = refresh(
            &paths,
            &store,
            &[first.clone(), first, second.clone()],
            fast_options(),
        )
        .await;
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(presentations.len(), 2);
        assert!(presentations.iter().all(|p| p.status == "online"));
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn refresh_concurrency_is_bounded() {
        let (paths, store) = history();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let active_counter = Arc::clone(&active);
        let peak_counter = Arc::clone(&peak);
        let hold: Arc<dyn Fn(TcpStream) + Send + Sync> = Arc::new(move |mut stream: TcpStream| {
            let current = active_counter.fetch_add(1, Ordering::SeqCst) + 1;
            peak_counter.fetch_max(current, Ordering::SeqCst);
            let mut buffer = [0u8; 512];
            let _ = stream.read(&mut buffer);
            std::thread::sleep(Duration::from_millis(150));
            let _ = stream.write_all(&status_packet(&minimal_status_json()));
            active_counter.fetch_sub(1, Ordering::SeqCst);
        });
        // Six distinct loopback endpoints, each on its own listener.
        let mut servers = Vec::new();
        let mut ids = Vec::new();
        for index in 0..6 {
            let server = RawTcpServer::spawn(Arc::clone(&hold));
            ids.push(seed_server_visit(
                &store,
                &format!("127.0.0.1:{}", server.port()),
                Some(&format!("Realm {index}")),
            ));
            servers.push(server);
        }
        let options = RefreshOptions {
            limits: QueryLimits {
                connect_timeout: Duration::from_secs(2),
                total_timeout: Duration::from_secs(4),
            },
            concurrency: 2,
        };
        let presentations = refresh(&paths, &store, &ids, options).await;
        assert_eq!(presentations.len(), ids.len());
        assert!(presentations.iter().all(|p| p.status == "online"));
        assert!(
            peak.load(Ordering::SeqCst) <= 2,
            "concurrency exceeded the bound: {}",
            peak.load(Ordering::SeqCst)
        );
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn malformed_protocol_answers_report_offline_without_cache_damage() {
        let (paths, store) = history();
        let garbage = RawTcpServer::spawn(answer_with(vec![0x80; 64]));
        let bad_json = RawTcpServer::spawn(answer_with({
            let mut inner = Vec::new();
            protocol::write_varint(&mut inner, 0x00);
            let payload = b"{ not json";
            protocol::write_varint(&mut inner, payload.len() as u32);
            inner.extend_from_slice(payload);
            let mut framed = Vec::new();
            protocol::write_varint(&mut framed, inner.len() as u32);
            framed.extend_from_slice(&inner);
            framed
        }));
        let mut ids = Vec::new();
        for (index, server) in [&garbage, &bad_json].iter().enumerate() {
            let host = format!("127.0.0.1:{}", server.port());
            ids.push(seed_server_visit(
                &store,
                &host,
                Some(&format!("Realm {index}")),
            ));
        }
        let presentations = refresh(&paths, &store, &ids, fast_options()).await;
        assert_eq!(presentations.len(), 2);
        assert!(presentations.iter().all(|p| p.status == "offline"));
        assert!(presentations.iter().all(|p| p.motd.is_empty()));
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn presentation_cache_never_influences_launch_targets() {
        let (paths, store) = history();
        let server = RawTcpServer::spawn(answer_with(status_packet(&minimal_status_json())));
        let host = format!("127.0.0.1:{}", server.port());
        let id = seed_server_visit(&store, &host, Some("Launch me"));
        let before = store.quick_target(Mode::Multiplayer, &id).unwrap();
        refresh(&paths, &store, &[id.clone()], fast_options()).await;
        let after = store.quick_target(Mode::Multiplayer, &id).unwrap();
        assert_eq!(before, after);
        // Enrichment of one target never unlocks another id.
        assert!(
            store
                .quick_target(Mode::Multiplayer, &"0".repeat(64))
                .unwrap()
                .is_none()
        );
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn offline_history_without_cache_stays_usable() {
        let (paths, store) = history();
        // Port 1 on loopback has no listener: refresh fails with no cache and
        // no history damage; the presentation degrades to name-only.
        let id = seed_server_visit(&store, "127.0.0.1:1", Some("Lonely Realm"));
        let presentations = refresh(&paths, &store, &[id.clone()], fast_options()).await;
        assert_eq!(presentations.len(), 1);
        assert_eq!(presentations[0].status, "offline");
        assert_eq!(presentations[0].name.as_deref(), Some("Lonely Realm"));
        assert_eq!(presentations[0].favicon, None);
        assert_eq!(presentations[0].motd.len(), 0);
        // History remains intact and authoritative.
        assert!(
            store
                .quick_target(Mode::Multiplayer, &id)
                .unwrap()
                .is_some()
        );
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }
}
