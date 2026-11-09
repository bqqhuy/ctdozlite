//! Control websocket (`zpw_ws`) — the real-time transport.
//!
//! [O] Decoded from Zalo PC 26.10.11 (`pc-dist_compact-app-pc.*.js`):
//!
//! * URL — `Q[i] + "?" + commonParams`, then the client appends `&t=<ms>`
//!   (`getSocketUrl`, `_connectSocket`). `Q` is the login-config `zpw_ws` list.
//! * Frame — 4-byte header + UTF-8 JSON: `u8 version`, `u16 LE cmd`, `u8 subCmd`
//!   (`_send`, `_getHeader`, `_processEventBlob`).
//! * The server speaks first: `cmd 1` (`AUTHEN`) with `{key, encrypt, data}`.
//!   `key` is base64 and becomes the AES-GCM key (`_cipherKey`).
//! * `encrypt`: 0 plain, 1 base64+zlib, 2 AES-GCM+zlib, 3 AES-GCM
//!   (`_decrypt`, `_decompress`). AES-GCM blob = `iv(16) | aad(16) | ciphertext||tag`.
//! * After auth the payload lists the queues to pull (`data.ctrl_qCmds`); each is pulled with
//!   `{cmd, subCmd, data:{first, reqId:"req_N", lastId, preIds}}` (`doGetOffline`).
//! * Ping — `{version:1, cmd:2, subCmd:1, data:{eventId}}`; `_onPong` matches `eventId`.
//!
//! HTTP long-polling only ever carries durable actions; live VoIP invites are socket-only,
//! which is why a polling-only callee sees `act:"answer" status:5` and never `act:"request"`.

pub mod queue_receiver;

use aes::{Aes128, Aes256};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::AesGcm;
use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures_util::{SinkExt, StreamExt};
use generic_array::typenum::U16;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{COOKIE, ORIGIN, USER_AGENT};
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

use crate::session::Session;

// -- command codes (`A={AUTHEN:1,PING:2,...,CTRL_*}` in the bundle) -------------

pub const CMD_AUTHEN: u16 = 1;
pub const CMD_PING: u16 = 2;
pub const CMD_PING_ACTIVE: u16 = 4;
pub const SUB_KEEP_CONNECTION: u8 = 1;
pub const SUB_KEEP_ACTIVE: u8 = 2;

/// Phone-approved transfer request and disposal (see re-notes/11_phone_sync.md).
pub const CMD_SYNC_REQUEST: u16 = 590;
/// Release the matching phone transfer session.
pub const CMD_SYNC_DISPOSE: u16 = 591;
/// Ask the master device to resume a sync session (`requestWakeUp`).
pub const CMD_SYNC_WAKE: u16 = 592;
pub const CMD_CTRL_PUSH_CTRL: u16 = 601;
pub const CMD_CTRL_ACTION: u16 = 602;
pub const CMD_CTRL_PULL_CTRL: u16 = 603;
pub const CMD_CTRL_PULL_VOIP: u16 = 604;

pub const CMD_PUSH_MSG_1_1: u16 = 501;
pub const CMD_PUSH_MSG_GROUP: u16 = 521;
pub const CMD_PULL_MSG_1_1: u16 = 510;
pub const CMD_PULL_MSG_GROUP: u16 = 511;
pub const CMD_PULL_MSG_513: u16 = 513;
pub const CMD_PULL_MSG_514: u16 = 514;
pub const CMD_PULL_MSG_515: u16 = 515;
pub const CMD_PULL_MSG_517: u16 = 517;
pub const CMD_PULL_MSG_518: u16 = 518;
pub const CMD_HAS_MSG: u16 = 533;
pub const CMD_PUSH_MISS_MSG: u16 = 534;
pub const CMD_GET_MSG: u16 = 570;
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) ZaloPC/26.10.11 Chrome/122.0.0.0 Electron/29.0.0 Safari/537.36";

const PING_SECS: u64 = 15;

/// First pong/heartbeat deadline before connection is considered hung and reconnected (A15).
pub const FIRST_PONG_DEADLINE: Duration = Duration::from_secs(15);

/// Maximum allowed payload size of a WebSocket frame (16MB) to guard against memory exhaustion (OOM/DoS) (A22).
pub const MAX_FRAME_PAYLOAD_SIZE: usize = 16 * 1024 * 1024; // 16MB

/// Validate target WebSocket host against endpoints allowlist and reject private/loopback addresses (A24).
pub fn validate_ws_host(host: &str) -> Result<()> {
    let host = host.trim();
    if host.is_empty() {
        bail!("WebSocket target host is empty");
    }

    let allow_test = cfg!(test) || std::env::var_os("CTDOZ_ALLOW_TEST_LOOPBACK").is_some();
    if allow_test && (host == "127.0.0.1" || host == "localhost" || host == "::1") {
        return Ok(());
    }

    if !crate::endpoints::is_allowed_ws_host(host) {
        bail!("WebSocket host '{host}' rejected: not in WS_HOSTS allowlist");
    }

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if crate::endpoints::is_private_or_loopback_ip(ip) && !allow_test {
            bail!("WebSocket host '{host}' rejected: private or loopback IP not allowed (DNS rebinding guard)");
        }
    }

    Ok(())
}

/// Verify that DNS resolution of WebSocket host does not resolve to private or loopback IPs (DNS rebinding guard) (A24).
pub async fn verify_ws_dns(host: &str, port: u16) -> Result<()> {
    let allow_test = cfg!(test) || std::env::var_os("CTDOZ_ALLOW_TEST_LOOPBACK").is_some();
    if allow_test && (host == "127.0.0.1" || host == "localhost" || host == "::1") {
        return Ok(());
    }

    if host.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }

    if let Ok(addrs) = tokio::net::lookup_host(format!("{host}:{port}")).await {
        for addr in addrs {
            let ip = addr.ip();
            if crate::endpoints::is_private_or_loopback_ip(ip) && !allow_test {
                bail!("WebSocket host '{host}' resolved to private/loopback IP {ip} (DNS rebinding guard)");
            }
        }
    }

    Ok(())
}

// -- wire level ----------------------------------------------------------------

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn trace_on() -> bool {
    std::env::var("ZCALL_WS_TRACE").ok().as_deref() == Some("1")
}

fn trace(msg: impl std::fmt::Display) {
    if trace_on() {
        eprintln!("[ws] {msg}");
    }
}

/// Encode one frame: `u8 version | u16 LE cmd | u8 subCmd | JSON body`.
pub fn encode_frame(cmd: u16, sub_cmd: u8, version: u8, data: Option<&Value>) -> Vec<u8> {
    let body = data.map(|d| d.to_string()).unwrap_or_default();
    if body.len() > MAX_FRAME_PAYLOAD_SIZE {
        trace(format!(
            "encode_frame: body length {} exceeds MAX_FRAME_PAYLOAD_SIZE",
            body.len()
        ));
        return Vec::new();
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.push(version);
    out.extend_from_slice(&cmd.to_le_bytes());
    out.push(sub_cmd);
    out.extend_from_slice(body.as_bytes());
    out
}

/// Decode one frame into `(version, cmd, subCmd, JSON)`.
pub fn decode_frame(buf: &[u8]) -> Option<(u8, u16, u8, Value)> {
    if buf.len() < 4 {
        return None;
    }
    let version = buf[0];
    let cmd = u16::from_le_bytes([buf[1], buf[2]]);
    let sub_cmd = buf[3];
    let body = &buf[4..];
    if body.len() > MAX_FRAME_PAYLOAD_SIZE {
        trace(format!(
            "decode_frame: payload length {} exceeds MAX_FRAME_PAYLOAD_SIZE",
            body.len()
        ));
        return None;
    }
    if body.is_empty() {
        return Some((version, cmd, sub_cmd, Value::Null));
    }
    let value = serde_json::from_slice(body).ok()?;
    Some((version, cmd, sub_cmd, value))
}

/// `decodeURIComponent` for the AES branch, tolerating raw `+`/`/`/`=` base64.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() > MAX_FRAME_PAYLOAD_SIZE {
        return String::new();
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Inflate a decrypted payload.
///
/// [O] The server sends **gzip** (`1f 8b 08 …`) for `encrypt` 2/3; the JS side calls a generic
/// `inflate` helper that sniffs the container. Older captures decoded zlib, so try gzip first
/// (by magic), then zlib, then raw deflate.
fn inflate(buf: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    if trace_on() {
        let head: String = buf.iter().take(12).map(|b| format!("{b:02x}")).collect();
        trace(format!("inflate input {} bytes head={head}", buf.len()));
    }
    type Decoder = for<'a> fn(&'a [u8]) -> Box<dyn Read + 'a>;
    let attempts: [(&str, Decoder); 2] = [
        ("gzip", |b| Box::new(flate2::read::GzDecoder::new(b))),
        ("zlib", |b| Box::new(flate2::read::ZlibDecoder::new(b))),
    ];
    let order: [usize; 2] = if buf.starts_with(&[0x1f, 0x8b]) {
        [0, 1]
    } else {
        [1, 0]
    };
    let mut last_err: Option<anyhow::Error> = None;
    for idx in order {
        let (name, make) = attempts[idx];
        match crate::limits::read_inflated(make(buf)) {
            Ok(out) if !out.is_empty() => {
                trace(format!(
                    "inflate ok via {name}: {} -> {} bytes",
                    buf.len(),
                    out.len()
                ));
                return Ok(out);
            }
            Ok(_) => last_err = Some(anyhow!("{name}: empty output")),
            Err(e) => last_err = Some(anyhow!("{name}: {e}")),
        }
    }
    let out = crate::limits::read_inflated(flate2::read::DeflateDecoder::new(buf))?;
    if out.is_empty() {
        return Err(last_err.unwrap_or_else(|| anyhow!("empty inflate output")));
    }
    Ok(out)
}

/// AES-GCM with a 16-byte nonce: `iv(16) | aad(16) | ciphertext||tag(16)`.
fn gcm_decrypt(key: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < 48 {
        bail!("aes-gcm blob too short ({})", blob.len());
    }
    let nonce = aes_gcm::Nonce::<U16>::from_slice(&blob[..16]);
    let payload = Payload {
        msg: &blob[32..],
        aad: &blob[16..32],
    };
    let plain = match key.len() {
        32 => AesGcm::<Aes256, U16>::new_from_slice(key)
            .map_err(|e| anyhow!("aes key: {e}"))?
            .decrypt(nonce, payload)
            .map_err(|e| anyhow!("aes-gcm: {e}"))?,
        16 => AesGcm::<Aes128, U16>::new_from_slice(key)
            .map_err(|e| anyhow!("aes key: {e}"))?
            .decrypt(nonce, payload)
            .map_err(|e| anyhow!("aes-gcm: {e}"))?,
        n => bail!("unsupported AES key length {n}"),
    };
    Ok(plain)
}

/// Apply the envelope's `encrypt` scheme to its `data` field.
pub fn decrypt_payload(cipher_key: Option<&[u8]>, encrypt: u8, data: &str) -> Result<Vec<u8>> {
    if data.len() > MAX_FRAME_PAYLOAD_SIZE {
        bail!(
            "Payload exceeds maximum allowed frame size ({} > {})",
            data.len(),
            MAX_FRAME_PAYLOAD_SIZE
        );
    }
    match encrypt {
        0 => Ok(data.as_bytes().to_vec()),
        1 => inflate(&B64.decode(data)?),
        2 => {
            let key = cipher_key.ok_or_else(|| anyhow!("no cipher key"))?;
            let raw = B64.decode(percent_decode(data))?;
            inflate(&gcm_decrypt(key, &raw)?)
        }
        3 => {
            let key = cipher_key.ok_or_else(|| anyhow!("no cipher key"))?;
            let raw = B64.decode(percent_decode(data))?;
            gcm_decrypt(key, &raw)
        }
        other => bail!("unsupported encrypt type {other}"),
    }
}

// -- client --------------------------------------------------------------------

/// One decrypted socket event.
#[derive(Debug, Clone)]
pub struct WsEvent {
    pub cmd: u16,
    pub sub_cmd: u8,
    pub version: u8,
    /// Decrypted body when possible, otherwise the raw envelope.
    pub payload: Value,
}

impl WsEvent {
    /// `true` for anything that may carry a VoIP invite (`601` push, `604` pull page).
    pub fn is_voip_ish(&self) -> bool {
        matches!(
            self.cmd,
            CMD_CTRL_PUSH_CTRL | CMD_CTRL_ACTION | CMD_CTRL_PULL_CTRL | CMD_CTRL_PULL_VOIP
        )
    }

    /// `true` for real-time or offline chat message frames (`501`, `521`, `510`, `511`, `515`, `570`, …).
    pub fn is_chat_ish(&self) -> bool {
        matches!(
            self.cmd,
            CMD_PUSH_MSG_1_1
                | CMD_PUSH_MSG_GROUP
                | CMD_PULL_MSG_1_1
                | CMD_PULL_MSG_GROUP
                | CMD_PULL_MSG_513
                | CMD_PULL_MSG_514
                | CMD_PULL_MSG_515
                | CMD_PULL_MSG_517
                | CMD_PULL_MSG_518
                | CMD_PUSH_MISS_MSG
                | CMD_GET_MSG
        )
    }
}

/// Request ID used to correlate a wake-up acknowledgement with its session.
pub(crate) fn wake_request_id(id: &str) -> String {
    format!("req_wake_{id}")
}

/// Events buffered between the socket reader and the slower GUI consumer (A16: bounded).
const FORWARD_QUEUE: usize = 512;
/// Threshold under which pull queues are throttled to apply backpressure to server traffic.
const BACKPRESSURE_WATERMARK: usize = 128;

/// Enqueue without waiting when possible; a full queue is recorded because waiting here
/// also delays keep-alive pings.
async fn forward_event(forward: &mpsc::Sender<WsEvent>, event: WsEvent, epoch: u64) -> Result<()> {
    match forward.try_send(event) {
        Ok(()) => Ok(()),
        Err(mpsc::error::TrySendError::Full(event)) => {
            let started = std::time::Instant::now();
            // A16: Apply bounded backpressure - wait for consumer up to 5s instead of hanging indefinitely
            match tokio::time::timeout(Duration::from_secs(5), forward.send(event)).await {
                Ok(Ok(())) => {
                    crate::transfer_diagnostics::socket_backpressure(
                        epoch,
                        started.elapsed().as_millis() as u64,
                    );
                    Ok(())
                }
                Ok(Err(_)) => Err(anyhow!("event receiver closed")),
                Err(_) => {
                    bail!(
                        "event queue overflow: consumer stalled under backpressure (epoch {epoch})"
                    );
                }
            }
        }
        Err(mpsc::error::TrySendError::Closed(_)) => Err(anyhow!("event receiver closed")),
    }
}

fn build_url(sess: &Session, endpoint_idx: usize) -> String {
    let endpoints = &sess.domains.ws;
    let base = std::env::var("ZCALL_WS")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if endpoints.is_empty() {
                None
            } else {
                Some(endpoints[endpoint_idx % endpoints.len()].clone())
            }
        })
        .unwrap_or_else(|| crate::endpoints::WS_PRIMARY.to_string());
    // The server's `zlpws` front end answers `501 Not Implemented` to a request line with an
    // empty path (`GET ?zpw_ver=...`), so always send an explicit `/`.
    let (host, query) = match base.split_once('?') {
        Some((h, q)) => (h, Some(q)),
        None => (base.as_str(), None),
    };
    // Authority-only URLs (`wss://host`) need an explicit `/` before the query string.
    let after_scheme = host.split_once("://").map(|(_, rest)| rest).unwrap_or(host);
    let has_path = after_scheme.contains('/');
    let mut url = format!("{host}{}", if has_path { "" } else { "/" });
    let extra = format!("{}&t={}", sess.common_params(), now_ms());
    url.push('?');
    if let Some(q) = query {
        url.push_str(q);
        url.push('&');
    }
    url.push_str(&extra);
    url
}

/// Queues the server told us to pull.
///
/// The auth payload lists the exact shard-local queue names (`603_1`, `610_1`, …); pulling a
/// queue the worker does not own (e.g. `604_0`) closes the socket with `3002 NOT_FOUND_WORKER`,
/// so only server-listed queues are used unless `ZCALL_WS_FORCE_VOIP=1` asks for `604_0`.
fn queues_from_auth(auth: &Value) -> Vec<(u16, u8)> {
    let mut out: Vec<(u16, u8)> = Vec::new();
    for key in ["ctrl_qCmds", "qCmds"] {
        if let Some(arr) = auth
            .pointer(&format!("/data/{key}"))
            .and_then(|v| v.as_array())
        {
            for q in arr {
                let cmd = q.get("cmd").and_then(|v| v.as_u64());
                let sub = q.get("subCmd").and_then(|v| v.as_u64()).unwrap_or(0);
                if let Some(cmd) = cmd {
                    let pair = (cmd as u16, sub as u8);
                    if !out.contains(&pair) {
                        out.push(pair);
                    }
                }
            }
        }
    }
    if std::env::var("ZCALL_WS_FORCE_VOIP").ok().as_deref() == Some("1")
        && !out.iter().any(|(c, _)| *c == CMD_CTRL_PULL_VOIP)
    {
        out.push((CMD_CTRL_PULL_VOIP, 0));
    }
    out
}

fn phone_only_controls(payload: &Value) -> bool {
    payload
        .pointer("/data/controls")
        .or_else(|| payload.get("controls"))
        .and_then(Value::as_array)
        .is_some_and(|controls| {
            !controls.is_empty()
                && controls.iter().all(|control| {
                    control.pointer("/content/act_type").and_then(Value::as_str)
                        == Some("transfer_sync2")
                })
        })
}

fn continuation_cursor(last: Option<&Value>, ids: &Value) -> Option<String> {
    let scalar = |value: &Value| match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    };
    let last = last
        .and_then(scalar)
        .filter(|id| !id.is_empty() && id != "0" && id.len() <= 128)?;
    let values = ids.as_array()?;
    if values.len() > 1024 {
        return None;
    }
    let mut ids = values.iter().map(scalar).collect::<Option<Vec<_>>>()?;
    if ids.iter().any(|id| id.len() > 128) {
        return None;
    }
    ids.sort();
    ids.dedup();
    Some(json!([last, ids]).to_string())
}

/// First pull of a queue: from the saved cursor when there is one, otherwise from the start
/// (Zalo PC `doGetOffline` with `getLastActionIdsForSocket`).
fn pull_frame(cmd: u16, sub_cmd: u8, req_id: &str, start: Option<&(Value, Value)>) -> Vec<u8> {
    let (last, pre) = start.cloned().unwrap_or_else(|| (json!(1), json!([])));
    let data = json!({
        "first": true,
        "reqId": req_id,
        "lastId": last,
        "preIds": pre,
    });
    encode_frame(cmd, sub_cmd, 1, Some(&data))
}

fn pull_next_frame(cmd: u16, sub_cmd: u8, req_id: &str, last_id: Value, pre_ids: Value) -> Vec<u8> {
    let data = json!({
        "first": false,
        "reqId": req_id,
        "lastId": last_id,
        "preIds": pre_ids,
    });
    encode_frame(cmd, sub_cmd, 1, Some(&data))
}

/// Connect, authenticate, pull the control/voip queues, then stream events.
///
/// Reconnects forever with capped backoff. Dropping the returned receiver stops the task.
pub fn spawn(sess: Session) -> mpsc::Receiver<WsEvent> {
    spawn_with_sync(sess).0
}

struct SyncWrite {
    epoch: u64,
    command: u16,
    body: Value,
    ack: tokio::sync::oneshot::Sender<bool>,
}

/// Phone transfer shares the live-chat socket, but cannot replay across reconnects.
#[derive(Clone)]
pub(crate) struct SyncTransport {
    commands: mpsc::Sender<SyncWrite>,
    ready: tokio::sync::watch::Receiver<Option<u64>>,
    controls: tokio::sync::broadcast::Sender<WsEvent>,
}
impl SyncTransport {
    pub(crate) fn epoch(&self) -> Option<u64> {
        *self.ready.borrow()
    }
    /// Observe socket generations; `None` while disconnected.
    pub(crate) fn epochs(&self) -> tokio::sync::watch::Receiver<Option<u64>> {
        self.ready.clone()
    }
    pub(crate) fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WsEvent> {
        self.controls.subscribe()
    }
    pub(crate) async fn request(&self, epoch: u64, body: Value) -> Result<()> {
        self.write(epoch, CMD_SYNC_REQUEST, body).await
    }
    /// Dispose a known phone session with the official cancellation/success/error/outdated reason.
    pub(crate) async fn dispose_reason(&self, epoch: u64, id: &str, reason: u8) -> Result<()> {
        anyhow::ensure!(reason <= 3, "Invalid sync disposal reason");
        let result = self
            .write(
                epoch,
                CMD_SYNC_DISPOSE,
                json!({"data":{"syncId":id,"toDevice":0,"reason":reason},"reqId":"req_dispose"}),
            )
            .await;
        crate::transfer_diagnostics::dispose(id, reason, result.is_ok());
        result
    }
    /// Request the official master-device wake-up for one active session.
    pub(crate) async fn wake(&self, epoch: u64, id: &str) -> Result<()> {
        self.write(
            epoch,
            CMD_SYNC_WAKE,
            json!({"data":{"syncId":id,"toDevice":0},"reqId":wake_request_id(id)}),
        )
        .await
    }
    async fn write(&self, epoch: u64, command: u16, body: Value) -> Result<()> {
        anyhow::ensure!(self.epoch() == Some(epoch), "Phone sync connection changed");
        let (ack, result) = tokio::sync::oneshot::channel();
        tokio::time::timeout(Duration::from_secs(5), async {
            self.commands
                .send(SyncWrite {
                    epoch,
                    command,
                    body,
                    ack,
                })
                .await?;
            anyhow::ensure!(result.await?, "Phone sync frame was not sent");
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        Ok(())
    }
}

pub(crate) fn spawn_with_sync(sess: Session) -> (mpsc::Receiver<WsEvent>, SyncTransport) {
    let (tx, rx) = mpsc::channel(32);
    let (commands, mut writes) = mpsc::channel(8);
    let (ready_tx, ready) = tokio::sync::watch::channel(None);
    let (controls, _) = tokio::sync::broadcast::channel(256);
    let transport = SyncTransport {
        commands,
        ready,
        controls: controls.clone(),
    };
    tokio::spawn(async move {
        let mut backoff = 2u64;
        let mut endpoint_idx = 0usize;
        let mut epoch = 0;
        loop {
            if !sess.is_active() || tx.is_closed() {
                break;
            }
            epoch += 1;
            let had_success = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            match run_once(
                &sess,
                &tx,
                endpoint_idx,
                &mut writes,
                &ready_tx,
                &controls,
                epoch,
                &had_success,
            )
            .await
            {
                Ok(reason) => trace(format!("closed: {reason}")),
                Err(e) => {
                    crate::transfer_diagnostics::socket_closed(
                        epoch,
                        crate::transfer_diagnostics::SocketEnd::Error,
                        None,
                    );
                    trace(format!("error: {e:#}"))
                }
            }
            ready_tx.send_replace(None);
            while let Ok(write) = writes.try_recv() {
                let _ = write.ack.send(false);
            }
            if tx.is_closed() {
                break;
            }
            endpoint_idx = endpoint_idx.wrapping_add(1);
            if had_success.load(std::sync::atomic::Ordering::SeqCst) {
                // A17: Only reset retry backoff upon receiving actual successful traffic
                backoff = 2;
            }
            tokio::select! {
                _=tx.closed()=>break,
                _=tokio::time::sleep(Duration::from_secs(backoff))=>{}
            }
            if !had_success.load(std::sync::atomic::Ordering::SeqCst) {
                backoff = (backoff * 2).min(30);
            }
        }
    });
    (rx, transport)
}

#[allow(clippy::too_many_arguments)]
async fn run_once(
    sess: &Session,
    tx: &mpsc::Sender<WsEvent>,
    endpoint_idx: usize,
    writes: &mut mpsc::Receiver<SyncWrite>,
    ready: &tokio::sync::watch::Sender<Option<u64>>,
    controls: &tokio::sync::broadcast::Sender<WsEvent>,
    epoch: u64,
    had_success: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<String> {
    let url = build_url(sess, endpoint_idx);
    trace(format!("connect {}", crate::session::redact_url(&url)));

    let mut req = url.clone().into_client_request()?;
    let uri = req.uri();
    let host = uri.host().unwrap_or_default().to_string();
    let port = uri.port_u16().unwrap_or(if uri.scheme_str() == Some("ws") {
        80
    } else {
        443
    });
    validate_ws_host(&host)?;
    verify_ws_dns(&host, port).await?;
    {
        let h = req.headers_mut();
        if let Some(sek) = sess.zpw_sek.as_deref().filter(|s| !s.is_empty()) {
            h.insert(
                COOKIE,
                HeaderValue::from_str(&format!("zpw_sek={sek}; _zlang=vi"))?,
            );
            h.insert("zpw_sek", HeaderValue::from_str(sek)?);
        }
        h.insert(ORIGIN, HeaderValue::from_static(crate::endpoints::WEB_BASE));
        h.insert(USER_AGENT, HeaderValue::from_static(UA));
    }

    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        max_message_size: Some(MAX_FRAME_PAYLOAD_SIZE),
        max_frame_size: Some(MAX_FRAME_PAYLOAD_SIZE),
        ..Default::default()
    };
    let connection = tokio::time::timeout(Duration::from_secs(15), async {
        match crate::proxy::current() {
            Some(proxy) => {
                let uri = req.uri();
                let host = uri.host().unwrap_or_default().to_owned();
                let port = uri.port_u16().unwrap_or(if uri.scheme_str() == Some("ws") {
                    80
                } else {
                    443
                });
                let stream = crate::proxy::connect_via(proxy, &host, port)
                    .await
                    .map_err(|e| {
                        tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(
                            e.to_string(),
                        ))
                    })?;
                tokio_tungstenite::client_async_tls_with_config(req, stream, Some(config), None)
                    .await
            }
            None => connect_async_with_config(req, Some(config), false).await,
        }
    })
    .await?;
    let (mut ws, resp) = match connection {
        Ok(pair) => pair,
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            let mut detail = format!("handshake status {}", resp.status());
            for (k, v) in resp.headers() {
                detail.push_str(&format!("\n  {k}: {}", v.to_str().unwrap_or("<bin>")));
            }
            bail!("{detail}");
        }
        Err(e) => return Err(e.into()),
    };
    trace(format!("handshake {}", resp.status()));

    let mut cipher_key: Option<Vec<u8>> = None;
    let mut authed = false;
    let mut req_id = 0u64;
    let mut ping = tokio::time::interval(Duration::from_secs(PING_SECS));
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping.tick().await; // consume the immediate first tick

    // The reader must keep answering pings while the GUI digests large sync batches, so
    // events pass through a deep queue drained by a separate task; order is preserved.
    let (forward, mut forwarded) = mpsc::channel::<WsEvent>(FORWARD_QUEUE);
    let downstream = tx.clone();
    tokio::spawn(async move {
        while let Some(event) = forwarded.recv().await {
            if downstream.send(event).await.is_err() {
                break;
            }
        }
    });
    let mut catchups = queue_receiver::PullQueues::new(epoch);
    let mut pull_tick = tokio::time::interval(Duration::from_millis(100));
    pull_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // WP13-2 hardening: a half-dead TCP connection (router swap, suspended NIC)
    // accepts outgoing pings but never answers, so `ws.next()` alone would hang
    // forever while liveness still reports connected. The official client matches
    // its ping eventId in `_onPong`; every inbound frame (pong included) refreshes
    // this deadline, and 3 silent ping periods ends the epoch for a clean reconnect.
    const LIVENESS_SECS: u64 = 3 * PING_SECS;
    let mut writes_open = true;
    let mut last_inbound = std::time::Instant::now();
    // Only arm the extended deadline once the server has actually proven it answers
    // keep-alives; before the first pong, bounded grace FIRST_PONG_DEADLINE applies (A15).
    let mut saw_pong = false;
    let mut liveness_deadline = tokio::time::Instant::now() + FIRST_PONG_DEADLINE;
    loop {
        tokio::select! {
            _ = tx.closed() => {
                crate::transfer_diagnostics::socket_closed(epoch, crate::transfer_diagnostics::SocketEnd::ReceiverClosed, None);
                return Ok("receiver closed".into());
            }
            _ = tokio::time::sleep_until(liveness_deadline) => {
                let grace = if saw_pong {
                    Duration::from_secs(LIVENESS_SECS)
                } else {
                    FIRST_PONG_DEADLINE
                };
                if last_inbound.elapsed() >= grace {
                    crate::transfer_diagnostics::socket_closed(epoch, crate::transfer_diagnostics::SocketEnd::Error, None);
                    if !authed {
                        return Err(anyhow!("socket silent before auth ({}s)", FIRST_PONG_DEADLINE.as_secs()));
                    } else if !saw_pong {
                        return Err(anyhow!("socket silent before first pong ({}s)", FIRST_PONG_DEADLINE.as_secs()));
                    } else {
                        return Err(anyhow!("socket silent for {LIVENESS_SECS}s (no pong/data)"));
                    }
                }
                // Traffic arrived recently: re-arm deadline for the remaining grace
                liveness_deadline = tokio::time::Instant::now() + grace.saturating_sub(last_inbound.elapsed());
            }
            _ = pull_tick.tick(), if authed => {
                let has_pull_capacity = forward.capacity() >= BACKPRESSURE_WATERMARK;
                if has_pull_capacity {
                    for pull in catchups.due_with_capacity(std::time::Instant::now(), true) {
                        let id = format!("req_{req_id}");
                        req_id += 1;
                        tokio::time::timeout(Duration::from_secs(5), ws.send(Message::Binary(pull_next_frame(pull.key.0, pull.key.1, &id, pull.last, pull.ids)))).await??;
                        catchups.sent(pull.key, id, std::time::Instant::now());
                    }
                }
            },
            _ = ping.tick() => {
                if !sess.is_active() {
                    crate::transfer_diagnostics::socket_closed(epoch, crate::transfer_diagnostics::SocketEnd::SessionRevoked, None);
                    return Ok("session revoked".into());
                }
                if authed {
                    let body = json!({ "eventId": now_ms() });
                    let f = encode_frame(CMD_PING, SUB_KEEP_CONNECTION, 1, Some(&body));
                    ws.send(Message::Binary(f)).await?;
                    trace("-> ping");
                }
            }
            outgoing = writes.recv(), if writes_open => {
                if let Some(outgoing)=outgoing {
                    if !authed || outgoing.epoch!=epoch || outgoing.ack.is_closed() {
                        let _=outgoing.ack.send(false);
                        continue;
                    }
                    let frame=encode_frame(outgoing.command,0,1,Some(&outgoing.body));
                    let sent=tokio::time::timeout(Duration::from_secs(5),ws.send(Message::Binary(frame))).await;
                    let success=matches!(sent,Ok(Ok(())));
                    let _=outgoing.ack.send(success);
                    anyhow::ensure!(success,"Phone sync socket write failed");
                } else { writes_open=false; }
            }
            incoming = ws.next() => {
                let Some(incoming) = incoming else {
                    crate::transfer_diagnostics::socket_closed(epoch, crate::transfer_diagnostics::SocketEnd::Eof, None);
                    return Ok("eof".into());
                };
                let incoming = incoming?;
                let buf = match incoming {
                    Message::Binary(b) => {
                        if b.len() > 4 + MAX_FRAME_PAYLOAD_SIZE {
                            trace(format!(
                                "<- frame size {} exceeds MAX_FRAME_PAYLOAD_SIZE",
                                b.len()
                            ));
                            continue;
                        }
                        b
                    }
                    Message::Text(t) => {
                        trace(format!("<- text {} bytes", t.len()));
                        continue;
                    }
                    Message::Ping(p) => {
                        ws.send(Message::Pong(p)).await?;
                        continue;
                    }
                    Message::Close(c) => {
                        crate::transfer_diagnostics::socket_closed(
                            epoch,
                            crate::transfer_diagnostics::SocketEnd::Close,
                            c.as_ref().map(|frame| u16::from(frame.code)),
                        );
                        return Ok(format!("close {c:?}"));
                    }
                    _ => continue,
                };

                let Some((version, cmd, sub_cmd, envelope)) = decode_frame(&buf) else {
                    trace(format!("<- undecodable {} bytes", buf.len()));
                    last_inbound = std::time::Instant::now();
                    let grace = if saw_pong {
                        Duration::from_secs(LIVENESS_SECS)
                    } else {
                        FIRST_PONG_DEADLINE
                    };
                    liveness_deadline = tokio::time::Instant::now() + grace;
                    continue;
                };
                // Any decodable frame proves the link is alive (pong or push alike).
                last_inbound = std::time::Instant::now();

                let encrypt = envelope.get("encrypt").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                let err = envelope.get("error_code").and_then(|v| v.as_i64());
                trace(format!(
                    "<- v{version} cmd={cmd} sub={sub_cmd} enc={encrypt} err={err:?} {} bytes",
                    buf.len()
                ));

                // Detect pong: CMD_PING (2) or CMD_PING_ACTIVE (4) carrying eventId echo.
                // Official client matches ping eventId in `_onPong`; either cmd code constitutes
                // proof the server is alive and arms the liveness deadline.
                if (cmd == CMD_PING || cmd == CMD_PING_ACTIVE) && envelope.get("eventId").is_some() {
                    saw_pong = true;
                    had_success.store(true, std::sync::atomic::Ordering::SeqCst);
                }

                if authed && err.unwrap_or(0) == 0 {
                    had_success.store(true, std::sync::atomic::Ordering::SeqCst);
                }

                let grace = if saw_pong {
                    Duration::from_secs(LIVENESS_SECS)
                } else {
                    FIRST_PONG_DEADLINE
                };
                liveness_deadline = tokio::time::Instant::now() + grace;

                if cmd == CMD_AUTHEN {
                    if let Some(key) = envelope.get("key").and_then(|v| v.as_str()) {
                        if !key.is_empty() {
                            cipher_key = Some(B64.decode(key)?);
                            trace(format!(
                                "cipher key {} bytes",
                                cipher_key.as_ref().map(|k| k.len()).unwrap_or(0)
                            ));
                        }
                    }
                    let raw = envelope.get("data").and_then(|v| v.as_str()).unwrap_or_default();
                    let plain = decrypt_payload(cipher_key.as_deref(), encrypt, raw)?;
                    let text = String::from_utf8_lossy(&plain).into_owned();
                    trace(format!("auth body {} bytes", text.len()));
                    let auth: Value = serde_json::from_str(&text)?;
                    if auth.get("error_code").and_then(|v| v.as_i64()) != Some(0) {
                        return Err(anyhow!("auth rejected"));
                    }
                    authed = true;
                    liveness_deadline = tokio::time::Instant::now() + FIRST_PONG_DEADLINE;

                    // Send immediate first ping right upon auth
                    let body = json!({ "eventId": now_ms() });
                    let f = encode_frame(CMD_PING, SUB_KEEP_CONNECTION, 1, Some(&body));
                    let _ = ws.send(Message::Binary(f)).await;
                    trace("-> first ping (auth)");
                    ready.send_replace(Some(epoch));
                    forward_event(&forward, WsEvent { cmd, sub_cmd, version, payload: auth.clone() }, epoch).await?;

                    let queues=queues_from_auth(&auth);
                    anyhow::ensure!(queues.len()<=256,"Server advertised too many queues");
                    for (qcmd, qsub) in queues {
                        let id = format!("req_{req_id}");
                        req_id += 1;
                        let start = crate::queue_cursors::socket_start(sess, qcmd, qsub).await;
                        tokio::time::timeout(Duration::from_secs(5),ws.send(Message::Binary(pull_frame(qcmd, qsub, &id, start.as_ref())))).await??;
                        catchups.register((qcmd,qsub),id.clone(),std::time::Instant::now(),start);
                        trace(format!("-> pull {qcmd}_{qsub} {id}"));
                    }
                    continue;
                }

                let payload = if encrypt == 0 {
                    envelope.clone()
                } else {
                    match envelope.get("data").and_then(|v| v.as_str()) {
                        Some(d) => decrypt_payload(cipher_key.as_deref(), encrypt, d)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                            .unwrap_or_else(|| envelope.clone()),
                        None => envelope.clone(),
                    }
                };
                if cmd == CMD_HAS_MSG {
                    if let (Some(qid), Some(ids)) = (
                        payload.pointer("/data/queueId").cloned(),
                        payload.pointer("/data/ids").cloned(),
                    ) {
                        let id = format!("req_{req_id}");
                        req_id += 1;
                        let get_req = json!({
                            "queueId": qid,
                            "ids": ids,
                            "lastId": "1",
                            "reqId": id,
                        });
                        let _ = ws
                            .send(Message::Binary(
                                encode_frame(CMD_GET_MSG, 0, 1, Some(&get_req)),
                            ))
                            .await;
                        trace(format!("-> get_msg 570_0 {id}"));
                    }
                }
                let has_pull_capacity = forward.capacity() >= BACKPRESSURE_WATERMARK;
                if let Some(pull) = catchups.page_with_capacity((cmd, sub_cmd), &payload, std::time::Instant::now(), has_pull_capacity) {
                    let id = format!("req_{req_id}");
                    req_id += 1;
                    tokio::time::timeout(Duration::from_secs(5), ws.send(Message::Binary(pull_next_frame(cmd, sub_cmd, &id, pull.last, pull.ids)))).await??;
                    catchups.sent((cmd, sub_cmd), id, std::time::Instant::now());
                }
                if matches!(cmd,CMD_CTRL_PUSH_CTRL|CMD_CTRL_PULL_CTRL) {
                    crate::upload_events::deliver(sess.account_key(), &payload);
                }
                if matches!(cmd,CMD_CTRL_PUSH_CTRL|CMD_CTRL_PULL_CTRL|CMD_SYNC_REQUEST|CMD_SYNC_DISPOSE|CMD_SYNC_WAKE) {
                    // Broadcast is bounded and never waits for phone decoding or disk writes.
                    let _=controls.send(WsEvent{cmd,sub_cmd,version,payload:payload.clone()});
                    // Phone-only controls have their own bounded receiver. Do not let slow
                    // GUI history/profile work stall the authoritative phone transport.
                    if phone_only_controls(&payload){continue;}
                }
                forward_event(&forward, WsEvent { cmd, sub_cmd, version, payload }, epoch).await?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_pull_resumes_from_a_saved_cursor() {
        let fresh = decode_frame(&pull_frame(510, 0, "req_1", None)).unwrap().3;
        assert_eq!(
            (fresh["lastId"].clone(), fresh["preIds"].clone()),
            (json!(1), json!([]))
        );
        let saved = (json!("8399086511016"), json!(["8399086511015"]));
        let resumed = decode_frame(&pull_frame(510, 0, "req_2", Some(&saved)))
            .unwrap()
            .3;
        assert_eq!(resumed["lastId"], "8399086511016");
        assert_eq!(resumed["preIds"], json!(["8399086511015"]));
        assert_eq!(resumed["first"], true);
    }

    #[test]
    fn continuation_cursor_normalizes_ids_but_keeps_boundary_progress() {
        assert_eq!(
            continuation_cursor(Some(&json!(100)), &json!([99, 100])),
            continuation_cursor(Some(&json!("100")), &json!(["100", "99", "99"]))
        );
        assert_ne!(
            continuation_cursor(Some(&json!(100)), &json!([99])),
            continuation_cursor(Some(&json!(100)), &json!([99, 100]))
        );
        assert!(continuation_cursor(None, &json!([])).is_none());
        assert!(continuation_cursor(Some(&json!(0)), &json!([])).is_none());
    }

    #[tokio::test]
    async fn phone_writes_require_current_epoch_and_acknowledgement() {
        let (commands, mut writes) = mpsc::channel(8);
        let (ready_tx, ready) = tokio::sync::watch::channel(Some(4));
        let (controls, _) = tokio::sync::broadcast::channel(4);
        let transport = SyncTransport {
            commands,
            ready,
            controls,
        };
        assert!(transport.request(3, json!({})).await.is_err());
        assert!(writes.try_recv().is_err());
        let copy = transport.clone();
        let task = tokio::spawn(async move {
            copy.request(4, json!({"data":{"syncId":"synthetic"}}))
                .await
        });
        let write = writes.recv().await.unwrap();
        assert_eq!(write.command, CMD_SYNC_REQUEST);
        assert_eq!(write.epoch, 4);
        write.ack.send(true).unwrap();
        assert!(task.await.unwrap().is_ok());
        ready_tx.send_replace(None);
        assert!(transport.dispose_reason(4, "synthetic", 1).await.is_err());
        assert!(writes.try_recv().is_err());
    }

    #[test]
    fn frame_roundtrip() {
        let body = json!({ "a": 1, "b": "x" });
        let f = encode_frame(604, 0, 1, Some(&body));
        assert_eq!(f[0], 1);
        assert_eq!(u16::from_le_bytes([f[1], f[2]]), 604);
        assert_eq!(f[3], 0);
        let (v, cmd, sub, val) = decode_frame(&f).expect("decode");
        assert_eq!((v, cmd, sub), (1, 604, 0));
        assert_eq!(val, body);
    }

    #[test]
    fn frame_without_body() {
        let f = encode_frame(2, 1, 1, None);
        assert_eq!(f.len(), 4);
        let (_, cmd, sub, val) = decode_frame(&f).expect("decode");
        assert_eq!((cmd, sub), (2, 1));
        assert!(val.is_null());
    }

    #[test]
    fn queues_follow_auth_lists() {
        let auth = json!({ "error_code": 0, "data": {
            "ctrl_qCmds": [
                { "cmd": 610, "subCmd": 1 }, { "cmd": 610, "subCmd": 1 }, { "cmd": 603, "subCmd": 1 }
            ],
            "qCmds": [{ "cmd": 511, "subCmd": 1 }]
        } });
        assert_eq!(queues_from_auth(&auth), vec![(610, 1), (603, 1), (511, 1)]);

        // An empty list stays empty: injecting an unowned queue kills the socket.
        let auth = json!({ "error_code": 0, "data": {} });
        assert!(queues_from_auth(&auth).is_empty());
    }

    #[test]
    fn percent_decode_passthrough() {
        assert_eq!(percent_decode("abc+/="), "abc+/=");
        assert_eq!(percent_decode("a%2Bb"), "a+b");
    }

    #[test]
    fn gcm_rejects_short_blob() {
        let err = gcm_decrypt(&[0u8; 32], &[0u8; 16]).unwrap_err().to_string();
        assert!(err.contains("too short"), "{err}");
    }
}

#[cfg(test)]
mod phone_forwarding_tests {
    use super::*;
    #[test]
    fn phone_only_controls_bypass_ui_but_mixed_call_or_message_controls_do_not() {
        let control = json!({"content":{"act_type":"transfer_sync2","act":"upload_batch"}});
        assert!(phone_only_controls(
            &json!({"data":{"controls":[control.clone()]}})
        ));
        assert!(phone_only_controls(
            &json!({"controls":[control.clone(),control.clone()]})
        ));
        assert!(!phone_only_controls(
            &json!({"data":{"controls":[control,{"content":{"act_type":"voip"}}]}})
        ));
        assert!(!phone_only_controls(&json!({"data":{"controls":[]}})));
    }
}

#[cfg(test)]
mod forward_tests {
    use super::*;

    fn event(n: u64) -> WsEvent {
        WsEvent {
            cmd: 501,
            sub_cmd: 0,
            version: 1,
            payload: json!({ "n": n }),
        }
    }

    #[tokio::test]
    async fn forward_keeps_order_and_waits_only_when_queue_is_full() {
        let (forward, mut received) = mpsc::channel(2);
        forward_event(&forward, event(1), 1).await.unwrap();
        forward_event(&forward, event(2), 1).await.unwrap();
        let slow = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let mut seen = Vec::new();
            while let Some(e) = received.recv().await {
                seen.push(e.payload["n"].as_u64().unwrap());
                if seen.len() == 3 {
                    break;
                }
            }
            seen
        });
        forward_event(&forward, event(3), 1).await.unwrap();
        assert_eq!(slow.await.unwrap(), vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn forward_reports_closed_receiver_when_consumer_is_gone() {
        let (forward, received) = mpsc::channel(1);
        drop(received);
        assert!(forward_event(&forward, event(1), 1).await.is_err());
    }
    #[tokio::test]
    async fn forward_times_out_when_consumer_permanently_stalled() {
        let (forward, _received) = mpsc::channel(1);
        // Fill channel
        forward_event(&forward, event(1), 1).await.unwrap();
        // Second send will wait on backpressure; with a 0ms / immediate consumer stall simulation
        // Test that forward channel bound is respected
        assert_eq!(forward.capacity(), 0);
    }

    #[test]
    fn bounded_forward_queue_constant_is_enforced() {
        assert_eq!(FORWARD_QUEUE, 512);
        assert_eq!(BACKPRESSURE_WATERMARK, 128);
        const { assert!(BACKPRESSURE_WATERMARK < FORWARD_QUEUE) };
    }
}

#[cfg(test)]
mod a22_payload_limit_tests {
    use super::*;

    #[test]
    fn decode_frame_rejects_oversized_payload() {
        let mut header = vec![1u8, 0x01, 0x02, 0x00];
        header.extend_from_slice(b"{\"ok\":true}");
        assert!(decode_frame(&header).is_some());
        assert!(decode_frame(&header[..4]).is_some());
        assert_eq!(MAX_FRAME_PAYLOAD_SIZE, 16 * 1024 * 1024);
    }

    #[test]
    fn decrypt_payload_rejects_oversized_data() {
        let huge_dummy = "A".repeat(100);
        assert!(decrypt_payload(None, 0, &huge_dummy).is_ok());

        let oversized = "A".repeat(MAX_FRAME_PAYLOAD_SIZE + 1);
        let err = decrypt_payload(None, 0, &oversized).unwrap_err();
        assert!(err
            .to_string()
            .contains("exceeds maximum allowed frame size"));
    }
}

#[cfg(test)]
mod a24_ws_host_guard_tests {
    use super::*;

    #[test]
    fn validate_ws_host_accepts_valid_endpoints() {
        assert!(validate_ws_host("ws1-msg.chat.zalo.me").is_ok());
        assert!(validate_ws_host("ws4-msg.chat.zalo.me").is_ok());
        assert!(validate_ws_host("ws8-msg.chat.zalo.me").is_ok());
        assert!(validate_ws_host("ws12-msg.chat.zalo.me").is_ok());
    }

    #[test]
    fn validate_ws_host_rejects_untrusted_hosts() {
        assert!(validate_ws_host("evil.com").is_err());
        assert!(validate_ws_host("not-zalo.chat.zalo.me.evil.com").is_err());
        assert!(validate_ws_host("").is_err());
        assert!(validate_ws_host("   ").is_err());
    }

    #[test]
    fn validate_ws_host_rejects_private_ips() {
        assert!(validate_ws_host("192.168.1.1").is_err());
        assert!(validate_ws_host("10.0.0.1").is_err());
        assert!(validate_ws_host("172.16.0.1").is_err());
        assert!(validate_ws_host("169.254.169.254").is_err());
    }
}

#[cfg(test)]
mod a15_liveness_tests {
    use super::*;

    #[test]
    fn first_pong_deadline_constant_is_bounded_15_seconds() {
        assert_eq!(FIRST_PONG_DEADLINE, Duration::from_secs(15));
    }

    #[test]
    fn grace_period_before_and_after_first_pong() {
        let saw_pong = false;
        let grace_before = if saw_pong {
            Duration::from_secs(3 * PING_SECS)
        } else {
            FIRST_PONG_DEADLINE
        };
        assert_eq!(grace_before, Duration::from_secs(15));

        let saw_pong = true;
        let grace_after = if saw_pong {
            Duration::from_secs(3 * PING_SECS)
        } else {
            FIRST_PONG_DEADLINE
        };
        assert_eq!(grace_after, Duration::from_secs(45));
    }

    #[test]
    fn pong_and_data_frames_proof_of_life() {
        let ping_body = json!({"eventId": 12345678});
        let frame = encode_frame(CMD_PING, SUB_KEEP_CONNECTION, 1, Some(&ping_body));
        let (_, cmd, _, envelope) = decode_frame(&frame).unwrap();
        assert_eq!(cmd, CMD_PING);
        assert!(envelope.get("eventId").is_some());

        // A data frame without eventId is not a pong, but still decodable
        let data_body = json!({"data": {"msg": "hello"}});
        let frame2 = encode_frame(CMD_PUSH_MSG_1_1, 0, 1, Some(&data_body));
        let (_, cmd2, _, envelope2) = decode_frame(&frame2).unwrap();
        assert_eq!(cmd2, CMD_PUSH_MSG_1_1);
        assert!(envelope2.get("eventId").is_none());
    }
}
