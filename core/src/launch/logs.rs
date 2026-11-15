//! Bounded, redacted session output and the explicit public-log upload boundary.
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

pub const MAX_LINES: usize = 10_000;
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
// mclo.gs accepts up to 10 MiB / 25,000 lines. Our live buffer is smaller.
const UPLOAD_BYTES: usize = MAX_BYTES;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub text: String,
    pub level: String,
    pub stream: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSnapshot {
    pub lines: Vec<LogLine>,
    pub discarded_lines: u64,
}

#[derive(Default)]
struct Buffer {
    lines: VecDeque<LogLine>,
    bytes: usize,
    discarded: u64,
}

pub struct Session {
    buffer: Mutex<Buffer>,
    changes: tokio::sync::watch::Sender<u64>,
}

fn sessions() -> &'static Mutex<HashMap<String, Arc<Session>>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Arc<Session>>>> = OnceLock::new();
    SESSIONS.get_or_init(Default::default)
}

pub fn session(instance: &str) -> Arc<Session> {
    sessions()
        .lock()
        .unwrap()
        .entry(instance.into())
        .or_insert_with(|| {
            Arc::new(Session {
                buffer: Mutex::new(Buffer::default()),
                changes: tokio::sync::watch::channel(0).0,
            })
        })
        .clone()
}

impl Session {
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.subscribe()
    }
    pub fn clear(&self) {
        *self.buffer.lock().unwrap() = Buffer::default();
        self.changed();
    }
    pub fn changed(&self) {
        self.changes
            .send_modify(|version| *version = version.wrapping_add(1));
    }
    pub fn push(&self, text: &str, stream: &str, secrets: &[String]) {
        let text = redact(text, secrets);
        let mut buffer = self.buffer.lock().unwrap();
        if text.len() > MAX_BYTES {
            buffer.discarded += 1;
            return; // Never retain a fragment of a secret-bearing overlong line.
        }
        buffer.bytes += text.len();
        buffer.lines.push_back(LogLine {
            level: level(&text).into(),
            text,
            stream: stream.into(),
        });
        while buffer.lines.len() > MAX_LINES || buffer.bytes > MAX_BYTES {
            if let Some(line) = buffer.lines.pop_front() {
                buffer.bytes -= line.text.len();
                buffer.discarded += 1;
            }
        }
        drop(buffer);
        self.changed();
    }
    pub fn snapshot(&self) -> LogSnapshot {
        let b = self.buffer.lock().unwrap();
        LogSnapshot {
            lines: b.lines.iter().cloned().collect(),
            discarded_lines: b.discarded,
        }
    }
}

pub fn level(text: &str) -> &'static str {
    if text.contains("FATAL") {
        "fatal"
    } else if text.contains("ERROR")
        || text.contains("Exception:")
        || text.trim_start().starts_with("Caused by:")
    {
        "error"
    } else if text.contains("WARN") {
        "warn"
    } else if text.trim_start().starts_with("at ") || text.trim_start().starts_with("... ") {
        "trace"
    } else {
        "info"
    }
}

/// Exact session credentials plus labelled credentials even if a mod logs them
/// in a different representation. Runs before anything enters the shared ring.
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut text = text.to_owned();
    // Longer values first so a prefix secret cannot expose the rest of a token.
    let mut secrets: Vec<_> = secrets.iter().filter(|s| !s.is_empty()).collect();
    secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for secret in secrets {
        text = text.replace(secret, "[redacted]");
    }
    for key in [
        "--accessToken",
        "--uuid",
        "access_token",
        "accessToken",
        "refresh_token",
        "sessionId",
        "sessionid",
        "clientToken",
        "Bearer",
        "Authorization",
        "uuid",
    ] {
        let mut offset = 0;
        loop {
            let lower = text.to_ascii_lowercase();
            let Some(found) = lower[offset..].find(&key.to_ascii_lowercase()) else {
                break;
            };
            let after = offset + found + key.len();
            let mut start = after;
            while start < text.len()
                && matches!(
                    text.as_bytes()[start],
                    b' ' | b'\t' | b'=' | b':' | b'"' | b'\''
                )
            {
                start += 1;
            }
            if text[start..].starts_with("[redacted]") {
                offset = start + "[redacted]".len();
                continue;
            }
            if start == after {
                offset = after;
                continue;
            }
            let end = if key == "Authorization" {
                text[start..]
                    .find(['\n', '\r'])
                    .map_or(text.len(), |n| start + n)
            } else {
                text[start..]
                    .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ']' | '}'))
                    .map_or(text.len(), |n| start + n)
            };
            if end > start {
                text.replace_range(start..end, "[redacted]");
                offset = start + "[redacted]".len();
            } else {
                offset = after;
            }
        }
    }
    text
}

pub fn upload_redact(text: &str, home: Option<&str>, username: Option<&str>) -> String {
    let mut text = redact(text, &[]);
    if let Some(home) = home.filter(|s| !s.is_empty()) {
        for path in [
            home.to_owned(),
            home.replace('\\', "/"),
            home.replace('\\', "\\\\"),
        ] {
            text = replace_ascii_case_insensitive(&text, &path, "~");
        }
    }
    // Other Windows user directories can appear in crash reports too.
    for prefix in ["\\\\Users\\\\", "\\Users\\", "/Users/"] {
        let mut offset = 0;
        loop {
            let lower = text.to_ascii_lowercase();
            let Some(n) = lower[offset..].find(&prefix.to_ascii_lowercase()) else {
                break;
            };
            let start = offset + n + prefix.len();
            let end = text[start..]
                .find(['\\', '/', '"', '\n', '\r'])
                .map_or(text.len(), |n| start + n);
            text.replace_range(start..end, "[user]");
            offset = start + 6;
        }
    }
    if let Some(name) = username.filter(|s| !s.is_empty()) {
        text = replace_ascii_case_insensitive(&text, name, "[user]");
    }
    text
}

fn replace_ascii_case_insensitive(text: &str, needle: &str, replacement: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    let mut result = String::new();
    let mut start = 0;
    while let Some(n) = lower[start..].find(&needle) {
        let n = start + n;
        result.push_str(&text[start..n]);
        result.push_str(replacement);
        start = n + needle.len();
    }
    result.push_str(&text[start..]);
    result
}

pub struct UploadRequest {
    pub content: String,
    pub truncated: bool,
}

impl UploadRequest {
    pub fn build(snapshot: &LogSnapshot, home: Option<&str>, username: Option<&str>) -> Self {
        let text = snapshot
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let text = upload_redact(&text, home, username);
        let mut start = text.len().saturating_sub(UPLOAD_BYTES - 128);
        while !text.is_char_boundary(start) {
            start += 1;
        }
        if start > 0 {
            start = text[start..]
                .find('\n')
                .map_or(text.len(), |n| start + n + 1);
        }
        let truncated = start > 0 || snapshot.discarded_lines > 0;
        Self {
            content: format!(
                "{}{}",
                if truncated {
                    "[Older output truncated by Aurora Launcher]\n"
                } else {
                    ""
                },
                &text[start..]
            ),
            truncated,
        }
    }
    pub fn http_request(&self, client: &reqwest::Client) -> reqwest::RequestBuilder {
        // URL encoding is explicit: no feature addition to the shared HTTP client.
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("content", &self.content)
            .finish();
        client
            .post("https://api.mclo.gs/1/log")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    pub url: String,
    pub id: String,
    #[serde(default)]
    pub truncated: bool,
}

/// The HTTP seam takes an already-built request. Tests substitute a transport
/// and exercise the same status/response path without making any connection.
pub async fn upload_using<F, Fut>(
    upload: UploadRequest,
    client: &reqwest::Client,
    send: F,
) -> Result<UploadResult, &'static str>
where
    F: FnOnce(reqwest::Request) -> Fut,
    Fut: std::future::Future<Output = Result<(u16, Vec<u8>), &'static str>>,
{
    let request = upload
        .http_request(client)
        .build()
        .map_err(|_| "The upload request could not be built.")?;
    let (status, bytes) = send(request).await?;
    if !(200..300).contains(&status) {
        return Err("mclo.gs rejected the upload. Try again later.");
    }
    if bytes.len() > 64 * 1024 {
        return Err("mclo.gs returned an oversized response.");
    }
    parse_upload_response(&bytes, upload.truncated)
}

pub fn parse_upload_response(bytes: &[u8], truncated: bool) -> Result<UploadResult, &'static str> {
    #[derive(Deserialize)]
    struct Response {
        success: bool,
        url: Option<String>,
        id: Option<String>,
    }
    let response: Response =
        serde_json::from_slice(bytes).map_err(|_| "mclo.gs returned an invalid response.")?;
    let (Some(raw), Some(id)) = (response.url, response.id) else {
        return Err("mclo.gs did not return a public link.");
    };
    let url = url::Url::parse(&raw).map_err(|_| "mclo.gs returned an invalid link.")?;
    if !response.success
        || id.is_empty()
        || !id.bytes().all(|b| b.is_ascii_alphanumeric())
        || url.scheme() != "https"
        || url.host_str() != Some("mclo.gs")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != format!("/{id}")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("mclo.gs did not return a valid public link.");
    }
    Ok(UploadResult {
        url: url.into(),
        id,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_ring_redacts_before_observers() {
        let session = session("ring-test");
        session.clear();
        for _ in 0..MAX_LINES + 4 {
            session.push("INFO secret-value", "stdout", &["secret-value".into()]);
        }
        let s = session.snapshot();
        assert_eq!(s.lines.len(), MAX_LINES);
        assert_eq!(s.discarded_lines, 4);
        assert!(s.lines.iter().all(|l| l.text == "INFO [redacted]"));
        for _ in 0..4 {
            session.push(&"x".repeat(MAX_BYTES / 2), "stderr", &[]);
        }
        assert!(
            session
                .snapshot()
                .lines
                .iter()
                .map(|l| l.text.len())
                .sum::<usize>()
                <= MAX_BYTES
        );
    }
    #[test]
    fn labelled_auth_and_overlapping_secrets_are_removed() {
        let text = redact(
            "--accessToken TOKEN --uuid UUID access_token=OTHER sessionId: SESSION Authorization: Bearer BEARER",
            &[],
        );
        for s in ["TOKEN", "UUID", "OTHER", "SESSION", "BEARER"] {
            assert!(!text.contains(s), "{text}");
        }
        assert_eq!(
            redact("prefix-long", &["prefix".into(), "prefix-long".into()]),
            "[redacted]"
        );
        assert_eq!(
            redact("Authorization: Basic private-credentials", &[]),
            "Authorization: [redacted]"
        );
        assert_eq!(redact("--uuid [redacted]", &[]), "--uuid [redacted]");
    }
    #[test]
    fn upload_removes_home_and_windows_usernames() {
        let text = upload_redact(
            r"C:\Users\Player\logs c:/users/player/logs C:\Users\Other\file Player --accessToken secret --uuid identity",
            Some(r"C:\Users\Player"),
            Some("Player"),
        );
        for value in ["Player", "Other", "secret", "identity"] {
            assert!(!text.contains(value), "{text}");
        }
        assert!(text.contains('~'));
    }
    #[test]
    fn upload_request_is_form_encoded_without_network() {
        let s = LogSnapshot {
            lines: vec![LogLine {
                text: "INFO a&b=é\n--accessToken secret".into(),
                level: "info".into(),
                stream: "stdout".into(),
            }],
            discarded_lines: 2,
        };
        let upload = UploadRequest::build(&s, None, None);
        crate::downloads::ensure_rustls_crypto_provider();
        let request = upload
            .http_request(&reqwest::Client::new())
            .build()
            .unwrap();
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(request.url().as_str(), "https://api.mclo.gs/1/log");
        let body = request.body().unwrap().as_bytes().unwrap();
        let form = url::form_urlencoded::parse(body).collect::<Vec<_>>();
        assert_eq!(form.len(), 1);
        assert_eq!(form[0].0, "content");
        assert_eq!(form[0].1, upload.content);
        assert!(!upload.content.contains("secret"));
        assert!(upload.truncated);
    }
    #[test]
    fn upload_response_rejects_remote_or_malformed_links() {
        assert!(
            parse_upload_response(
                br#"{"success":true,"url":"https://mclo.gs/Ab12","id":"Ab12"}"#,
                false
            )
            .is_ok()
        );
        for text in [
            r#"{"success":false}"#,
            r#"{"success":true,"url":"https://evil.test/Ab12","id":"Ab12"}"#,
            r#"{"success":true,"url":"https://mclo.gs/wrong","id":"Ab12"}"#,
            "no",
        ] {
            assert!(parse_upload_response(text.as_bytes(), false).is_err());
        }
    }
    #[tokio::test]
    async fn upload_transport_is_mocked_and_rejections_are_sanitized() {
        crate::downloads::ensure_rustls_crypto_provider();
        let client = reqwest::Client::new();
        let upload = UploadRequest {
            content: "INFO already redacted".into(),
            truncated: true,
        };
        let response = upload_using(upload, &client, |request| async move {
            assert_eq!(request.url().as_str(), "https://api.mclo.gs/1/log");
            let form = url::form_urlencoded::parse(request.body().unwrap().as_bytes().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(form[0].1, "INFO already redacted");
            Ok((
                200,
                br#"{"success":true,"url":"https://mclo.gs/Ab12","id":"Ab12"}"#.to_vec(),
            ))
        })
        .await
        .unwrap();
        assert_eq!(response.id, "Ab12");
        assert!(response.truncated);
        let error = upload_using(
            UploadRequest {
                content: "x".into(),
                truncated: false,
            },
            &client,
            |_| async { Ok((429, b"untrusted secret body".to_vec())) },
        )
        .await
        .unwrap_err();
        assert!(!error.contains("secret"));
    }
    #[test]
    fn upload_cap_drops_oldest_complete_lines() {
        let snapshot = LogSnapshot {
            lines: (0..5000)
                .map(|n| LogLine {
                    text: format!("{n}: {}", "é".repeat(512)),
                    level: "info".into(),
                    stream: "stdout".into(),
                })
                .collect(),
            discarded_lines: 0,
        };
        let upload = UploadRequest::build(&snapshot, None, None);
        assert!(upload.truncated);
        assert!(upload.content.len() <= UPLOAD_BYTES);
        assert!(upload.content.contains("4999:"));
        assert!(!upload.content.contains("\n0:"));
    }
}
