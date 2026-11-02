//! Bounded Discord RPC v1 over native local IPC only.
use serde_json::{Value, json};
use std::io;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const FRAME_LIMIT: usize = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(2);

pub(super) trait Transport: Send {
    async fn connect(&mut self, id: &str) -> Result<(), Failure>;
    async fn publish(&mut self, activity: Option<&super::Activity>) -> Result<(), Failure>;
    fn disconnect(&mut self);
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Failure {
    Unavailable,
    Closed,
    Invalid,
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}
#[derive(Default)]
pub(super) struct Native {
    stream: Option<Box<dyn Stream>>,
    nonce: u64,
}

async fn write_frame<S: AsyncWrite + Unpin + ?Sized>(
    stream: &mut S,
    opcode: u32,
    body: &[u8],
) -> io::Result<()> {
    if body.len() > FRAME_LIMIT {
        return Err(io::ErrorKind::InvalidData.into());
    }
    stream.write_all(&opcode.to_le_bytes()).await?;
    stream.write_all(&(body.len() as u32).to_le_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await
}
async fn read_frame<S: AsyncRead + Unpin + ?Sized>(stream: &mut S) -> io::Result<(u32, Vec<u8>)> {
    let opcode = stream.read_u32_le().await?;
    let size = stream.read_u32_le().await? as usize;
    if size > FRAME_LIMIT {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut body = vec![0; size];
    stream.read_exact(&mut body).await?;
    Ok((opcode, body))
}
async fn response(stream: &mut dyn Stream, nonce: Option<&str>) -> Result<(), Failure> {
    // Ignore a bounded number of unrelated events and answer protocol pings.
    for _ in 0..8 {
        let (opcode, body) = read_frame(stream).await.map_err(|_| Failure::Closed)?;
        if opcode == 3 {
            write_frame(stream, 4, &body)
                .await
                .map_err(|_| Failure::Closed)?;
            continue;
        }
        if opcode != 1 {
            return Err(Failure::Closed);
        }
        let value: Value = serde_json::from_slice(&body).map_err(|_| Failure::Invalid)?;
        if value.get("evt").and_then(Value::as_str) == Some("ERROR") {
            return Err(Failure::Invalid);
        }
        if let Some(nonce) = nonce {
            if value.get("nonce").and_then(Value::as_str) == Some(nonce) {
                return if value.get("cmd").and_then(Value::as_str) == Some("SET_ACTIVITY") {
                    Ok(())
                } else {
                    Err(Failure::Invalid)
                };
            }
        } else if value.get("evt").and_then(Value::as_str) == Some("READY")
            && value.get("cmd").and_then(Value::as_str) == Some("DISPATCH")
        {
            return Ok(());
        }
    }
    Err(Failure::Invalid)
}
async fn open() -> Result<Box<dyn Stream>, Failure> {
    #[cfg(windows)]
    for index in 0..10 {
        // Fixed Discord endpoint; no frontend path or remotely reachable socket.
        if let Ok(pipe) = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(format!(r"\\?\pipe\discord-ipc-{index}"))
        {
            return Ok(Box::new(pipe));
        }
    }
    #[cfg(unix)]
    {
        let base = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
            .iter()
            .find_map(std::env::var_os)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "/tmp".into());
        for index in 0..10 {
            if let Ok(stream) =
                tokio::net::UnixStream::connect(base.join(format!("discord-ipc-{index}"))).await
            {
                return Ok(Box::new(stream));
            }
        }
    }
    Err(Failure::Unavailable)
}
impl Transport for Native {
    async fn connect(&mut self, id: &str) -> Result<(), Failure> {
        self.disconnect();
        let mut stream = tokio::time::timeout(DEADLINE, open())
            .await
            .map_err(|_| Failure::Unavailable)??;
        let result = tokio::time::timeout(DEADLINE, async {
            write_frame(
                stream.as_mut(),
                0,
                &serde_json::to_vec(&json!({"v":1,"client_id":id})).unwrap(),
            )
            .await
            .map_err(|_| Failure::Closed)?;
            response(stream.as_mut(), None).await
        })
        .await
        .map_err(|_| Failure::Closed)?;
        result?;
        self.stream = Some(stream);
        Ok(())
    }
    async fn publish(&mut self, activity: Option<&super::Activity>) -> Result<(), Failure> {
        let stream = self.stream.as_mut().ok_or(Failure::Closed)?;
        self.nonce = self.nonce.wrapping_add(1);
        let nonce = self.nonce.to_string();
        let payload = json!({"cmd":"SET_ACTIVITY","args":{"pid":std::process::id(),"activity":activity},"nonce":nonce});
        tokio::time::timeout(DEADLINE, async {
            write_frame(stream.as_mut(), 1, &serde_json::to_vec(&payload).unwrap())
                .await
                .map_err(|_| Failure::Closed)?;
            response(stream.as_mut(), Some(&nonce)).await
        })
        .await
        .map_err(|_| Failure::Closed)?
    }
    fn disconnect(&mut self) {
        self.stream = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn oversized_frame_is_rejected_before_allocation() {
        let (mut writer, mut reader) = tokio::io::duplex(32);
        writer.write_u32_le(1).await.unwrap();
        writer.write_u32_le((FRAME_LIMIT + 1) as u32).await.unwrap();
        assert_eq!(
            read_frame(&mut reader).await.unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[tokio::test]
    async fn malformed_and_error_responses_are_sanitized() {
        for body in [
            b"not json".as_slice(),
            br#"{"evt":"ERROR","data":{"secret":"do not expose"}}"#,
        ] {
            let (mut writer, mut reader) = tokio::io::duplex(1024);
            write_frame(&mut writer, 1, body).await.unwrap();
            assert_eq!(response(&mut reader, None).await, Err(Failure::Invalid));
        }
    }
    #[tokio::test]
    async fn handshake_and_command_acknowledgement_are_strict() {
        let (mut writer, mut reader) = tokio::io::duplex(1024);
        write_frame(&mut writer, 1, br#"{"evt":"READY","cmd":"DISPATCH"}"#)
            .await
            .unwrap();
        response(&mut reader, None).await.unwrap();
        write_frame(
            &mut writer,
            1,
            br#"{"cmd":"SET_ACTIVITY","nonce":"2","data":null}"#,
        )
        .await
        .unwrap();
        response(&mut reader, Some("2")).await.unwrap();
    }
}
