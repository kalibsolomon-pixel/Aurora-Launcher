//! Minecraft Java Edition server-status protocol (client side).
//!
//! A deliberately small, bounded implementation of the modern status ping:
//! one handshake with next-state `Status`, one empty status request, one
//! VarInt-framed JSON response, then the socket is dropped. Every read and
//! write is deadline-bounded, the response is size-capped before parsing, and
//! malformed framing fails closed. The response body itself is untrusted
//! server data and is normalized by [`crate::server_enrichment`] before it can
//! reach a presentation DTO.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The status handshake intentionally sends the "unknown" protocol version
/// (`-1`): the version is only meaningful for login, and every mainstream
/// implementation answers status pings regardless of the value.
const STATUS_PROTOCOL_VERSION: i32 = -1;
/// A VarInt is at most five bytes; longer continuations are malformed.
const MAX_VARINT_BYTES: usize = 5;
/// Hard cap on one status response packet. Real responses (including a
/// maximum-size favicon and a player sample) stay far below this.
pub const MAX_STATUS_PAYLOAD: usize = 1024 * 1024;
/// Hard cap on the JSON string inside the response packet.
const MAX_JSON_BYTES: usize = MAX_STATUS_PAYLOAD;

/// Bounded timeouts for one status exchange: `connect_timeout` bounds both
/// connection setup and each individual socket read (an idle peer cannot hold
/// the exchange open), and `total_timeout` bounds the whole exchange. Defaults
/// suit a Home-widget enrichment request; tests narrow them.
#[derive(Clone, Copy, Debug)]
pub struct QueryLimits {
    pub connect_timeout: Duration,
    pub total_timeout: Duration,
}
impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(3),
            total_timeout: Duration::from_secs(5),
        }
    }
}

/// The raw (still untrusted) facts one status response carries. Only the
/// fields presentation needs are extracted; everything else a server sends is
/// ignored.
#[derive(Debug, Clone)]
pub struct StatusResponse {
    pub version_text: Option<String>,
    pub players_online: Option<u32>,
    pub players_max: Option<u32>,
    /// The `description` component exactly as the server serialized it; the
    /// MOTD converter owns turning this into safe segments.
    pub description: serde_json::Value,
    /// The favicon exactly as sent (`data:…;base64,…`) or absent.
    pub favicon: Option<String>,
    /// Wall-clock time from starting the connection to the complete response.
    pub latency: Duration,
}

pub fn write_varint(buffer: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buffer.push(byte);
        if value == 0 {
            return;
        }
    }
}

pub fn read_varint(bytes: &[u8], position: &mut usize) -> Result<u32, ()> {
    let mut value: u32 = 0;
    for index in 0..MAX_VARINT_BYTES {
        let byte = *bytes.get(*position).ok_or(())?;
        *position += 1;
        value |= u32::from(byte & 0x7F) << (7 * index);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(())
}

/// The bytes of the two packets a status ping sends. `handshake_host` is the
/// address the player would type (virtual-host routing), not a resolved SRV
/// target; `connect_port` is the port actually being contacted.
pub fn status_request(handshake_host: &str, connect_port: u16) -> Result<Vec<u8>, ()> {
    if handshake_host.len() > 255 || handshake_host.is_empty() {
        return Err(());
    }
    let mut host_bytes = Vec::with_capacity(handshake_host.len());
    write_varint(&mut host_bytes, handshake_host.len() as u32);
    host_bytes.extend_from_slice(handshake_host.as_bytes());

    let mut handshake = Vec::with_capacity(host_bytes.len() + 16);
    let mut inner = Vec::new();
    write_varint(&mut inner, STATUS_PROTOCOL_VERSION as u32);
    inner.extend_from_slice(&host_bytes);
    inner.extend_from_slice(&connect_port.to_be_bytes());
    write_varint(&mut inner, 1); // next state: Status
    // The outer length covers the whole packet, including the packet-id byte.
    write_varint(&mut handshake, inner.len() as u32 + 1);
    handshake.push(0x00); // packet id: Handshake
    handshake.extend_from_slice(&inner);

    let mut request = Vec::with_capacity(2);
    write_varint(&mut request, 1);
    request.push(0x00); // packet id: Status Request

    handshake.extend_from_slice(&request);
    Ok(handshake)
}

/// Parses one complete status response packet (length-prefixed, without any
/// TCP transport concerns) into its JSON payload bytes.
pub fn parse_status_packet(bytes: &[u8]) -> Result<&[u8], ()> {
    let mut position = 0;
    let length = read_varint(bytes, &mut position)?;
    let packet_id = read_varint(bytes, &mut position)?;
    if packet_id != 0x00 {
        return Err(());
    }
    let json_length = read_varint(bytes, &mut position)? as usize;
    if json_length > MAX_JSON_BYTES {
        return Err(());
    }
    let end = position.checked_add(json_length).ok_or(())?;
    let json = bytes.get(position..end).ok_or(())?;
    // The framing must account for exactly the JSON payload; a length that
    // over- or under-runs the packet is malformed.
    if length as usize + read_varint_span(bytes, 0)? != bytes.len() {
        return Err(());
    }
    std::str::from_utf8(json).map_err(|_| ())?;
    Ok(json)
}

fn read_varint_span(bytes: &[u8], from: usize) -> Result<usize, ()> {
    let mut position = from;
    read_varint(bytes, &mut position)?;
    Ok(position - from)
}

/// Performs one bounded status exchange against `connect_host:connect_port`,
/// presenting `handshake_host` in the handshake. Fails closed on timeout,
/// malformed framing, non-UTF-8 payloads, or a response over the size cap.
pub async fn query(
    handshake_host: &str,
    connect_host: &str,
    connect_port: u16,
    limits: QueryLimits,
) -> Result<StatusResponse, ()> {
    let started = std::time::Instant::now();
    let exchange = async {
        let connect = tokio::time::timeout(
            limits.connect_timeout,
            TcpStream::connect((connect_host, connect_port)),
        )
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
        let mut stream = connect;
        let request = status_request(handshake_host, connect_port)?;
        stream.write_all(&request).await.map_err(|_| ())?;
        stream.flush().await.map_err(|_| ())?;

        let mut payload = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            if payload.len() > MAX_STATUS_PAYLOAD {
                return Err(());
            }
            let read = tokio::time::timeout(limits.connect_timeout, stream.read(&mut chunk))
                .await
                .map_err(|_| ())?
                .map_err(|_| ())?;
            if read == 0 {
                break;
            }
            payload.extend_from_slice(&chunk[..read]);
            if let Some(json) = complete_packet(&payload)? {
                let json = json.to_vec();
                let latency = started.elapsed();
                return Ok((json, latency));
            }
        }
        Err(())
    };
    let (json, latency) = tokio::time::timeout(limits.total_timeout, exchange)
        .await
        .map_err(|_| ())??;
    let document: serde_json::Value = serde_json::from_slice(&json).map_err(|_| ())?;
    let object = document.as_object().ok_or(())?;
    let version_text = object
        .get("version")
        .and_then(|value| value.get("name"))
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let players = object.get("players");
    let players_online = players
        .and_then(|value| value.get("online"))
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok());
    let players_max = players
        .and_then(|value| value.get("max"))
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok());
    let favicon = object
        .get("favicon")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let description = object
        .get("description")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(StatusResponse {
        version_text: version_text.filter(|text| !text.is_empty()),
        players_online,
        players_max,
        description,
        favicon: favicon.filter(|text| !text.is_empty()),
        latency,
    })
}

/// Returns the JSON slice once `payload` contains one complete, well-formed
/// status packet. A packet that declares more than the payload cap, or that
/// trails extra bytes that do not complete a second frame, is rejected.
fn complete_packet(payload: &[u8]) -> Result<Option<&[u8]>, ()> {
    if payload.is_empty() {
        return Ok(None);
    }
    // A response larger than the cap may already have declared its length;
    // stop reading before buffering beyond it.
    let mut position = 0;
    let header = match read_varint(payload, &mut position) {
        Ok(header) => header as usize,
        Err(()) => {
            // Five continuation bytes without a terminator is malformed.
            if payload.len() >= MAX_VARINT_BYTES {
                return Err(());
            }
            return Ok(None);
        }
    };
    if header > MAX_STATUS_PAYLOAD {
        return Err(());
    }
    let end = position.checked_add(header).ok_or(())?;
    if payload.len() < end {
        return Ok(None);
    }
    let packet = &payload[..end];
    if payload.len() > end {
        // The protocol sends exactly one response packet; surplus bytes are
        // treated as malformed framing rather than silently ignored.
        return Err(());
    }
    parse_status_packet(packet).map(Some)
}

/// Whether a Minecraft client would attempt an SRV lookup for this host:
/// only multi-label DNS names. IP literals and single-label names have no
/// SRV semantics.
pub fn should_attempt_srv(handshake_host: &str) -> bool {
    let unbracketed = handshake_host.trim_start_matches('[').trim_end_matches(']');
    if unbracketed.parse::<std::net::Ipv4Addr>().is_ok()
        || unbracketed.parse::<std::net::Ipv6Addr>().is_ok()
    {
        return false;
    }
    let label_count = unbracketed.split('.').count();
    label_count > 1 && unbracketed.parse::<std::net::IpAddr>().is_err()
}

/// One SRV record as the system resolver answered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrvRecord {
    pub priority: u16,
    pub weight: u16,
    pub port: u16,
    pub target: String,
}

/// Deterministic record selection matching client behavior: lowest priority
/// first, then highest weight. Returns `None` when no usable record exists.
pub fn select_srv_record(records: &[SrvRecord]) -> Option<&SrvRecord> {
    records
        .iter()
        .filter(|record| {
            crate::gameplay_history::ServerTarget::parse(format!(
                "{}:{}",
                record.target, record.port
            ))
            .is_ok()
        })
        .min_by_key(|record| (record.priority, u16::MAX - record.weight))
}

/// Resolves `_minecraft._tcp.<host>` through the platform system resolver.
/// Only Windows implements the native SRV query today; other platforms fall
/// back to the literal endpoint, which is documented behavior rather than a
/// silent guess.
pub fn resolve_srv(handshake_host: &str) -> Option<(String, u16)> {
    if !should_attempt_srv(handshake_host) {
        return None;
    }
    resolve_srv_platform(handshake_host)
}

#[cfg(windows)]
fn resolve_srv_platform(handshake_host: &str) -> Option<(String, u16)> {
    use windows_sys::Win32::NetworkManagement::Dns::{
        DNS_QUERY_STANDARD, DNS_RECORDA, DNS_RECORDW, DNS_TYPE_SRV, DnsFree, DnsFreeRecordList,
        DnsQuery_W,
    };

    let query = format!("_minecraft._tcp.{handshake_host}");
    let mut wide: Vec<u16> = query.encode_utf16().collect();
    wide.push(0);
    let mut records: *mut DNS_RECORDA = std::ptr::null_mut();
    // DNS_QUERY_STANDARD: cached answers are acceptable, no wildcarding.
    let status = unsafe {
        DnsQuery_W(
            wide.as_ptr(),
            DNS_TYPE_SRV,
            DNS_QUERY_STANDARD,
            std::ptr::null_mut(),
            &mut records,
            std::ptr::null_mut(),
        )
    };
    if status != 0 || records.is_null() {
        return None;
    }
    let mut parsed: Vec<SrvRecord> = Vec::new();
    let mut current = records;
    while !current.is_null() {
        let record = unsafe { &*(current as *const DNS_RECORDW) };
        if record.wType == DNS_TYPE_SRV {
            let entry = unsafe { record.Data.SRV };
            let mut target = Vec::new();
            let mut cursor = entry.pNameTarget;
            while !cursor.is_null() && target.len() < 255 {
                let character = unsafe { *cursor };
                if character == 0 {
                    break;
                }
                target.push(character);
                cursor = unsafe { cursor.add(1) };
            }
            if let Ok(target) = String::from_utf16(&target) {
                parsed.push(SrvRecord {
                    priority: entry.wPriority,
                    weight: entry.wWeight,
                    port: entry.wPort,
                    target,
                });
            }
        }
        current = record.pNext.cast();
    }
    unsafe { DnsFree(records.cast(), DnsFreeRecordList) };
    let selected = select_srv_record(&parsed)?;
    Some((
        selected.target.trim_end_matches('.').to_owned(),
        selected.port,
    ))
}

#[cfg(not(windows))]
fn resolve_srv_platform(_handshake_host: &str) -> Option<(String, u16)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "queries the live system resolver and a public Minecraft server"]
    async fn live_srv_status_exchange_diagnostic() {
        for host in ["minemen.club", "pvphq.com"] {
            let resolved = resolve_srv(host);
            eprintln!("[live] {host} SRV -> {resolved:?}");
            let (connect_host, connect_port) =
                resolved.clone().unwrap_or_else(|| (host.to_owned(), 25565));
            match query(host, &connect_host, connect_port, QueryLimits::default()).await {
                Ok(response) => eprintln!(
                    "[live] {host} -> version={:?} players={:?}/{:?} favicon={} desc={}",
                    response.version_text,
                    response.players_online,
                    response.players_max,
                    response
                        .favicon
                        .as_deref()
                        .is_some_and(|f| f.starts_with("data:")),
                    serde_json::to_string(&response.description).unwrap_or_default()
                ),
                Err(()) => eprintln!("[live] {host} status query FAILED"),
            }
        }
    }

    #[test]
    fn varint_roundtrip_matches_protocol_boundaries() {
        for value in [0u32, 1, 127, 128, 255, 25565, 2097151, 2147483647, u32::MAX] {
            let mut buffer = Vec::new();
            write_varint(&mut buffer, value);
            assert!(buffer.len() <= MAX_VARINT_BYTES);
            let mut position = 0;
            assert_eq!(read_varint(&buffer, &mut position).unwrap(), value);
            assert_eq!(position, buffer.len());
        }
        let mut buffer = Vec::new();
        write_varint(&mut buffer, 25565);
        assert_eq!(buffer, vec![0xDD, 0xC7, 0x01]);
    }

    #[test]
    fn varint_rejects_truncated_and_overlong_continuations() {
        assert_eq!(read_varint(&[0x80], &mut 0), Err(()));
        assert_eq!(read_varint(&[0x80; 5], &mut 0), Err(()));
        assert_eq!(read_varint(&[0x80; 4], &mut 0), Err(()));
        assert_eq!(read_varint(&[], &mut 0), Err(()));
    }

    #[test]
    fn handshake_bytes_are_the_canonical_status_shape() {
        let bytes = status_request("example.invalid", 25565).unwrap();
        assert_eq!(
            bytes,
            vec![
                0x19, // handshake packet length (25: payload plus packet id)
                0x00, // handshake packet id
                0xFF, 0xFF, 0xFF, 0xFF, 0x0F, // protocol version -1
                0x0F, // host length (15)
                b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'i', b'n', b'v', b'a', b'l', b'i',
                b'd', 0x63, 0xDD, // port 25565 (u16 big-endian)
                0x01, // next state: status
                0x01, 0x00, // status request packet
            ]
        );
        assert!(status_request("", 25565).is_err());
        let long = "a".repeat(256);
        assert!(status_request(&long, 25565).is_err());
    }

    fn packet(payload: &[u8]) -> Vec<u8> {
        let mut inner = Vec::new();
        write_varint(&mut inner, 0x00); // packet id
        write_varint(&mut inner, payload.len() as u32);
        inner.extend_from_slice(payload);
        let mut framed = Vec::new();
        write_varint(&mut framed, inner.len() as u32);
        framed.extend_from_slice(&inner);
        framed
    }

    #[test]
    fn complete_packet_extracts_json_only_when_framing_is_exact() {
        let json = br#"{"players":{"online":1}}"#;
        let bytes = packet(json);
        assert_eq!(complete_packet(&bytes).unwrap().unwrap(), json);
        // Partial reads wait for more bytes.
        assert!(
            complete_packet(&bytes[..bytes.len() - 2])
                .unwrap()
                .is_none()
        );
        // Trailing bytes beyond one packet are malformed.
        let mut trailing = bytes.clone();
        trailing.extend_from_slice(b"xx");
        assert!(complete_packet(&trailing).is_err());
    }

    #[test]
    fn malformed_packets_fail_closed() {
        // Wrong packet id.
        let mut inner = Vec::new();
        write_varint(&mut inner, 0x02);
        write_varint(&mut inner, 0);
        let mut wrong_id = Vec::new();
        write_varint(&mut wrong_id, inner.len() as u32);
        wrong_id.extend_from_slice(&inner);
        assert!(complete_packet(&wrong_id).is_err());
        // Declared payload longer than the cap.
        let mut huge = Vec::new();
        write_varint(&mut huge, (MAX_STATUS_PAYLOAD + 1) as u32);
        assert!(complete_packet(&huge).is_err());
        // Five continuation bytes without a terminator.
        assert!(complete_packet(&[0x80; 5]).is_err());
        // Declared JSON length overrun.
        let mut overrun = Vec::new();
        let mut body = Vec::new();
        write_varint(&mut body, 0x00);
        write_varint(&mut body, 200);
        body.extend_from_slice(br#"{}"#);
        write_varint(&mut overrun, body.len() as u32);
        overrun.extend_from_slice(&body);
        assert!(complete_packet(&overrun).is_err());
        // Non-UTF-8 JSON payload.
        let mut binary = Vec::new();
        let mut body = Vec::new();
        write_varint(&mut body, 0x00);
        write_varint(&mut body, 2);
        body.extend_from_slice(&[0xFF, 0xFE]);
        write_varint(&mut binary, body.len() as u32);
        binary.extend_from_slice(&body);
        assert!(complete_packet(&binary).is_err());
    }

    #[test]
    fn srv_attempt_predicate_covers_addresses_and_labels() {
        assert!(should_attempt_srv("example.invalid"));
        assert!(!should_attempt_srv("127.0.0.1"));
        assert!(!should_attempt_srv("[::1]"));
        assert!(!should_attempt_srv("localhost"));
        assert!(!should_attempt_srv("::1"));
    }

    #[test]
    fn srv_selection_prefers_low_priority_then_weight_and_validity() {
        let records = vec![
            SrvRecord {
                priority: 10,
                weight: 5,
                port: 25566,
                target: "a.example.invalid".into(),
            },
            SrvRecord {
                priority: 5,
                weight: 1,
                port: 25567,
                target: "b.example.invalid".into(),
            },
            SrvRecord {
                priority: 5,
                weight: 9,
                port: 25568,
                target: "c.example.invalid".into(),
            },
            // A root-target or invalid port record must never be selected.
            SrvRecord {
                priority: 1,
                weight: 9,
                port: 25565,
                target: ".".into(),
            },
        ];
        let selected = select_srv_record(&records).unwrap();
        assert_eq!(selected.target, "c.example.invalid");
        assert_eq!(selected.port, 25568);
        assert!(select_srv_record(&[]).is_none());
    }
}
