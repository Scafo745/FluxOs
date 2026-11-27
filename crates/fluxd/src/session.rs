//! Relay to the console user's flux-session (crates/flux-session): terminal,
//! files, processes and app launching for the web UI.
//!
//! fluxd authenticates the paired screen, then talks to flux-session over
//! its unix socket in `/run/user/<uid>/fluxos/`. flux-session runs as that
//! user, so everything it does has the user's permissions, never fluxd's.
//! Browsers cannot send headers on a WebSocket, so a terminal starts with an
//! authenticated request for a single-use ticket that expires in 30 seconds.

use std::collections::HashMap;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::engine::EngineError;

const TICKET_TTL: Duration = Duration::from_secs(30);
const CALL_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_REPLY: u64 = 32 << 20;
const MAX_FRAME: usize = 1 << 20;
const DATA: u8 = 0;
const RESIZE: u8 = 1;
const EXIT: u8 = 2;

pub struct Ticket {
    pub principal: String,
    pub cols: u16,
    pub rows: u16,
    created: Instant,
}

pub struct SessionLink {
    user: Option<String>,
    socket: Option<PathBuf>,
    tickets: Mutex<HashMap<String, Ticket>>,
}

fn uid_of(user: &str) -> Option<u32> {
    std::fs::read_to_string("/etc/passwd").ok()?.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.first() == Some(&user)).then(|| f.get(2)?.parse().ok()).flatten()
    })
}

fn offline(msg: impl Into<String>) -> EngineError {
    EngineError::new(503, "session_offline", msg)
}

impl SessionLink {
    pub fn new(user: Option<String>, socket: Option<PathBuf>) -> Self {
        Self { user, socket, tickets: Mutex::new(HashMap::new()) }
    }

    pub fn configured(&self) -> bool {
        self.user.is_some() || self.socket.is_some()
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    /// The socket, checked to belong to the console user.
    fn socket_path(&self) -> Result<PathBuf, EngineError> {
        if let Some(p) = &self.socket {
            return Ok(p.clone());
        }
        let user = self.user.as_deref().ok_or_else(|| {
            EngineError::new(404, "no_session", "no console user is configured (services.fluxos.console.user)")
        })?;
        let uid = uid_of(user).ok_or_else(|| offline(format!("user {user} does not exist")))?;
        let path = PathBuf::from(format!("/run/user/{uid}/fluxos/session.sock"));
        let md =
            std::fs::metadata(&path).map_err(|_| offline(format!("{user} is not logged in to a FluxOS session")))?;
        if !md.file_type().is_socket() || md.uid() != uid {
            return Err(EngineError::new(
                503,
                "session_untrusted",
                "the session socket does not belong to the console user",
            ));
        }
        Ok(path)
    }

    async fn connect(&self) -> Result<UnixStream, EngineError> {
        let path = self.socket_path()?;
        UnixStream::connect(&path).await.map_err(|e| offline(format!("flux-session is not running ({e})")))
    }

    /// One request, one JSON reply.
    pub async fn call(&self, req: Value) -> Result<Value, EngineError> {
        let mut s = self.connect().await?;
        let io = async {
            s.write_all(format!("{req}\n").as_bytes()).await?;
            let mut buf = Vec::new();
            (&mut s).take(MAX_REPLY).read_to_end(&mut buf).await?;
            Ok::<_, std::io::Error>(buf)
        };
        let buf = tokio::time::timeout(CALL_TIMEOUT, io)
            .await
            .map_err(|_| EngineError::new(504, "session_timeout", "flux-session did not answer in time"))?
            .map_err(|e| offline(format!("flux-session: {e}")))?;
        let v: Value = serde_json::from_slice(&buf)
            .map_err(|e| EngineError::internal(format!("flux-session sent invalid JSON: {e}")))?;
        if v["ok"] == json!(true) {
            Ok(v)
        } else {
            Err(EngineError::bad_request(v["error"].as_str().unwrap_or("flux-session refused the request").to_string()))
        }
    }

    pub fn issue_ticket(&self, principal: &str, cols: u16, rows: u16) -> Result<String, EngineError> {
        let ticket = format!("flxt_{}", crate::util::random_hex(24).map_err(EngineError::internal)?);
        let mut t = self.tickets.lock().expect("ticket lock");
        t.retain(|_, v| v.created.elapsed() < TICKET_TTL);
        t.insert(ticket.clone(), Ticket { principal: principal.into(), cols, rows, created: Instant::now() });
        Ok(ticket)
    }

    /// Single use: a redeemed or expired ticket is gone.
    pub fn redeem(&self, ticket: &str) -> Option<Ticket> {
        let t = self.tickets.lock().expect("ticket lock").remove(ticket)?;
        (t.created.elapsed() < TICKET_TTL).then_some(t)
    }

    /// Starts a shell in the user's session; the stream then carries frames.
    pub async fn open_terminal(&self, cols: u16, rows: u16) -> Result<(UnixStream, Value), EngineError> {
        let mut s = self.connect().await?;
        s.write_all(format!("{}\n", json!({ "op": "term", "cols": cols, "rows": rows })).as_bytes())
            .await
            .map_err(|e| offline(e.to_string()))?;
        // The reply line, read byte by byte so no frame bytes are consumed.
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        while line.len() < 4096 {
            let n = tokio::time::timeout(CALL_TIMEOUT, s.read(&mut byte))
                .await
                .map_err(|_| EngineError::new(504, "session_timeout", "flux-session did not start the shell"))?
                .map_err(|e| offline(e.to_string()))?;
            if n == 0 || byte[0] == b'\n' {
                break;
            }
            line.push(byte[0]);
        }
        let v: Value = serde_json::from_slice(&line).unwrap_or(json!({ "ok": false, "error": "no reply" }));
        if v["ok"] != json!(true) {
            return Err(EngineError::bad_request(v["error"].as_str().unwrap_or("could not start a shell").to_string()));
        }
        Ok((s, v))
    }
}

async fn write_frame(w: &mut (impl AsyncWriteExt + Unpin), kind: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 5);
    frame.push(kind);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    w.write_all(&frame).await
}

/// Pipes a browser WebSocket to a flux-session terminal. Binary messages are
/// keystrokes and output; text messages are JSON control (`resize`, `exit`).
pub async fn relay(ws: WebSocket, unix: UnixStream) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (mut rd, mut wr) = unix.into_split();
    let up = async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(b) => write_frame(&mut wr, DATA, &b).await?,
                Message::Text(t) => {
                    if let Ok(v) = serde_json::from_str::<Value>(t.as_str())
                        && v["type"] == "resize"
                    {
                        let size = json!({ "cols": v["cols"], "rows": v["rows"] }).to_string();
                        write_frame(&mut wr, RESIZE, size.as_bytes()).await?;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        wr.shutdown().await
    };
    let down = async move {
        let mut head = [0u8; 5];
        while rd.read_exact(&mut head).await.is_ok() {
            let len = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
            if len > MAX_FRAME {
                break;
            }
            let mut buf = vec![0u8; len];
            if rd.read_exact(&mut buf).await.is_err() {
                break;
            }
            match head[0] {
                DATA => {
                    if ws_tx.send(Message::Binary(buf.into())).await.is_err() {
                        break;
                    }
                }
                EXIT => {
                    let code = serde_json::from_slice::<Value>(&buf).map(|v| v["code"].clone()).unwrap_or(Value::Null);
                    let _ = ws_tx.send(Message::Text(json!({ "type": "exit", "code": code }).to_string().into())).await;
                    break;
                }
                _ => {}
            }
        }
        let _ = ws_tx.send(Message::Close(None)).await;
    };
    tokio::select! {
        _ = up => {}
        _ = down => {}
    }
}
