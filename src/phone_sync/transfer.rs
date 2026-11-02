//! Phone-approved conversation/message rounds on the authenticated live WebSocket.
use super::{
    assets,
    codec::{Codec, KeyPair},
    import, wire,
};
use crate::{
    chat::Thread,
    session::Session,
    socket::{SyncTransport, WsEvent},
};
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use zeroize::Zeroizing;

fn routing_id(id: &str) -> bool {
    !id.is_empty() && id != "0" && id.bytes().all(|b| b.is_ascii_digit())
}

struct Crypto {
    codec: Codec,
    identity: KeyPair,
    ephemeral: KeyPair,
    record: Option<Zeroizing<Vec<u8>>>,
}
#[derive(Clone)]
struct Batch {
    metadata: bool,
    index: u64,
    url: String,
}
#[derive(Default)]
struct Round {
    batches: Vec<Batch>,
    seen: HashSet<(bool, u64)>,
    covered: HashSet<String>,
    last: bool,
    confirmed: bool,
}
impl Round {
    fn complete(&self, wanted: &[String]) -> bool {
        self.last || (!wanted.is_empty() && wanted.iter().all(|p| self.covered.contains(p)))
    }
    fn apply(&mut self, event: &WsEvent, id: &str) -> Result<bool> {
        if event.cmd == crate::socket::CMD_SYNC_REQUEST {
            if let Some(code) = event
                .payload
                .get("error_code")
                .and_then(Value::as_i64)
                .filter(|code| *code != 0)
            {
                anyhow::bail!("Máy chủ từ chối yêu cầu đồng bộ điện thoại (mã {code})");
            }
        }
        let controls = event
            .payload
            .pointer("/data/controls")
            .or_else(|| event.payload.get("controls"))
            .and_then(Value::as_array);
        let mut waiting = false;
        for control in controls.into_iter().flatten() {
            let Some(content) = control.get("content") else {
                continue;
            };
            if content.get("act_type").and_then(Value::as_str) != Some("transfer_sync2") {
                continue;
            }
            let Some(raw) = content.get("data") else {
                continue;
            };
            let data = if let Some(text) = raw.as_str() {
                serde_json::from_str::<Value>(text).context("Invalid phone transfer control")?
            } else {
                raw.clone()
            };
            if data.get("syncId").and_then(Value::as_str) != Some(id) {
                continue;
            }
            let act = content.get("act").and_then(Value::as_str).unwrap_or("");
            match act {
                "transfer_status" => match data.get("status").and_then(Value::as_u64) {
                    Some(3) => waiting = true,
                    Some(4 | 5) => self.confirmed = true,
                    _ => {}
                },
                "upload_batch" => {
                    self.last |= data
                        .get("isLast")
                        .is_some_and(|v| v.as_bool() == Some(true) || v.as_u64() == Some(1));
                    for scope in data
                        .get("scopes")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(p) = scope.get("partition").and_then(Value::as_str) {
                            ensure!(
                                p.len() <= 512 && self.covered.len() < 100_000,
                                "Phone scope limit"
                            );
                            self.covered.insert(p.into());
                        }
                    }
                    if let Some(url) = data
                        .get("msgUrl")
                        .and_then(Value::as_str)
                        .filter(|u| !u.is_empty())
                    {
                        ensure!(
                            url.len() <= 16_384 && self.batches.len() < 2048,
                            "Phone batch limit"
                        );
                        let metadata = data.get("batchType").and_then(Value::as_u64) == Some(2);
                        let index = data
                            .get("idx")
                            .and_then(Value::as_u64)
                            .context("Phone batch index missing")?;
                        if self.seen.insert((metadata, index)) {
                            self.batches.push(Batch {
                                metadata,
                                index,
                                url: url.into(),
                            });
                        }
                    }
                }
                _ if act.to_ascii_lowercase().contains("error") => {
                    anyhow::bail!("Điện thoại hoặc máy chủ từ chối đồng bộ")
                }
                _ => {}
            }
        }
        Ok(waiting)
    }
}
fn request(id: &str, crypto: &Crypto, wanted: &[String]) -> Value {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let queries = if wanted.is_empty() {
        vec![json!({"partition":"","from":0,"to":now,"limit":2147483647})]
    } else {
        wanted
            .iter()
            .map(|p| json!({"partition":p,"from":0,"to":now,"limit":2147483647}))
            .collect()
    };
    json!({"data":{"syncId":id,"syncType":0,"ek":B64.encode(&crypto.ephemeral.public),"ik":B64.encode(&crypto.identity.public),
        "toDevice":0,"tempKey":"","deviceName":"ctdozlite Windows ARM64","req":{"type":if wanted.is_empty(){"conversation"}else{"message"},
        "priority":if wanted.is_empty(){0}else{2},"batchSize":2000,"queries":queries},"ver":1,"ussidx":0},"reqId":format!("req_sync_{id}")})
}
fn sync_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    crate::random::fill(&mut bytes)?;
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    let h: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    ))
}
async fn guarded<T>(
    operation: impl Future<Output = Result<T>>,
    session: &Session,
    cancel: &AtomicBool,
    transport: &SyncTransport,
    epoch: u64,
) -> Result<T> {
    ensure!(
        !cancel.load(Ordering::Relaxed) && session.is_active(),
        "Đã dừng đồng bộ điện thoại; giữ dữ liệu đã lưu"
    );
    ensure!(
        transport.epoch() == Some(epoch),
        "Kết nối đồng bộ điện thoại đã thay đổi; thử lại"
    );
    tokio::pin!(operation);
    loop {
        tokio::select! {
            result=&mut operation=>return result,
            _=tokio::time::sleep(Duration::from_millis(200))=>{
                ensure!(!cancel.load(Ordering::Relaxed) && session.is_active(),"Đã dừng đồng bộ điện thoại; giữ dữ liệu đã lưu");
                ensure!(transport.epoch()==Some(epoch),"Mất kết nối khi đồng bộ điện thoại; dữ liệu đã lưu được giữ. Kết nối lại rồi bấm đồng bộ.");
            }
        }
    }
}
async fn round<F: FnMut(String)>(
    session: &Session,
    transport: &SyncTransport,
    epoch: u64,
    cancel: &AtomicBool,
    crypto: &Crypto,
    wanted: &[String],
    progress: &mut F,
) -> Result<Vec<Batch>> {
    let id = sync_id()?;
    let mut events = transport.subscribe();
    let outcome=guarded(async {
        transport.request(epoch,request(&id,crypto,wanted)).await?;
        let mut state=Round::default();
        let timeout=tokio::time::sleep(Duration::from_secs(if wanted.is_empty(){180}else{600}));
        tokio::pin!(timeout);
        loop {
            tokio::select! {
                _=&mut timeout=>anyhow::bail!("Hết thời gian chờ điện thoại; dữ liệu đã lưu được giữ. Mở Zalo trên điện thoại và thử lại."),
                event=events.recv()=>{
                    let event=event.map_err(|_|anyhow::anyhow!("Luồng đồng bộ điện thoại gián đoạn; thử lại sau khi kết nối ổn định"))?;
                    if state.apply(&event,&id)? {progress("Chờ xác nhận trên điện thoại: mở Zalo và chọn Đồng bộ ngay…".into());}
                    if state.complete(wanted) {
                        ensure!(state.confirmed || !state.batches.is_empty(),"Điện thoại chưa xác nhận đồng bộ");
                        state.batches.sort_by_key(|b|(!b.metadata,b.index));
                        return Ok(state.batches);
                    }
                }
            }
        }
    },session,cancel,transport,epoch).await;
    // Cleanup is attempted even on cancellation, rejection, timeout or a failed request.
    // A disconnected socket cannot send disposal; never replay it on a new epoch.
    if transport.dispose(epoch, &id).await.is_err() {
        crate::logger::log_error("phone sync cleanup", "Transfer disposal could not be sent");
    }
    outcome
}

/// Request phone approval once, then import history without pausing the live-chat transport.
/// # Errors
/// Returns bounded download/decode/identity/store failures; committed pages remain available.
pub(crate) async fn download<F: FnMut(String), G: FnMut(Thread)>(
    session: &Session,
    transport: SyncTransport,
    cancel: Arc<AtomicBool>,
    mut progress: F,
    mut page: G,
) -> Result<String> {
    ensure!(
        session.is_active() && session.login.is_some(),
        "Chưa đăng nhập"
    );
    let epoch = transport
        .epoch()
        .context("WebSocket chưa sẵn sàng; chờ kết nối rồi thử lại")?;
    progress("Đang chuẩn bị bộ giải mã đồng bộ điện thoại…".into());
    let cache = session
        .cache_path()
        .parent()
        .context("Account cache unavailable")?
        .to_owned();
    let bytes = guarded(assets::codec(&cache), session, &cancel, &transport, epoch).await?;
    let mut crypto = tokio::task::spawn_blocking(move || -> Result<Crypto> {
        assets::verify(&bytes)?;
        let mut codec = Codec::new(&bytes)?;
        let identity = codec.key_pair()?;
        let ephemeral = codec.key_pair()?;
        Ok(Crypto {
            codec,
            identity,
            ephemeral,
            record: None,
        })
    })
    .await??;
    progress("Đang lấy danh bạ và metadata nhóm để nhận diện lịch sử…".into());
    let fresh = guarded(
        crate::chat::get_friends(session),
        session,
        &cancel,
        &transport,
        epoch,
    )
    .await?;
    let mut threads: HashMap<String, Thread> = fresh
        .iter()
        .filter(|t| routing_id(&t.id) && !t.presentation.global_id.is_empty())
        .map(|t| {
            let mut t = t.clone();
            t.messages.clear();
            (
                format!(
                    "{}/{}",
                    if t.is_group { "group" } else { "oneone" },
                    t.presentation.global_id
                ),
                t,
            )
        })
        .collect();
    let mut senders: HashMap<String, String> = fresh
        .iter()
        .filter(|t| !t.is_group && routing_id(&t.id) && !t.presentation.global_id.is_empty())
        .map(|t| (t.presentation.global_id.clone(), t.id.clone()))
        .collect();
    progress("Đang gửi yêu cầu đồng bộ; hãy mở Zalo trên điện thoại và xác nhận…".into());
    let batches = round(
        session,
        &transport,
        epoch,
        &cancel,
        &crypto,
        &[],
        &mut progress,
    )
    .await?;
    let mut conversations = Vec::new();
    for batch in batches {
        let data = guarded(
            assets::download(&batch.url, 64 * 1024 * 1024),
            session,
            &cancel,
            &transport,
            epoch,
        )
        .await?;
        let cancel_decode = cancel.clone();
        let account_decode = session.clone();
        let (next, objects) = tokio::task::spawn_blocking(move || {
            decode_batch(
                crypto,
                data,
                batch.metadata,
                true,
                cancel_decode,
                account_decode,
            )
        })
        .await??;
        crypto = next;
        for object in objects {
            for c in object
                .get("conversations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                ensure!(conversations.len() < 100_000, "Phone conversation limit");
                conversations.push(c.clone());
            }
        }
    }
    let mut partitions = Vec::new();
    for c in &conversations {
        let id = c
            .get("convId")
            .and_then(Value::as_str)
            .context("Missing phone conversation ID")?;
        ensure!(
            !id.is_empty() && id.len() <= 512 && !id.contains('/'),
            "Invalid phone conversation ID"
        );
        let prefix = if c.get("convType").and_then(Value::as_u64) == Some(2) {
            "group"
        } else {
            "oneone"
        };
        partitions.push(format!("{prefix}/{id}"));
    }
    partitions.sort();
    partitions.dedup();
    // Non-friends and self may be absent from the friend roster. Resolve real IDs,
    // never synthesize sendable recipients from names or opaque partition strings.
    let unknown: Vec<_> = partitions
        .iter()
        .filter_map(|p| p.strip_prefix("oneone/"))
        .filter(|id| !senders.contains_key(*id))
        .map(str::to_owned)
        .collect();
    for ids in unknown.chunks(50) {
        let resolved = guarded(
            crate::chat::resolve_sync_ids(session, ids),
            session,
            &cancel,
            &transport,
            epoch,
        )
        .await?;
        for (global, id) in resolved {
            senders.insert(global.clone(), id.clone());
            let template = fresh
                .iter()
                .find(|t| t.id == id && !t.is_group)
                .cloned()
                .unwrap_or(Thread {
                    id: id.clone(),
                    name: id.clone(),
                    is_group: false,
                    presentation: Default::default(),
                    unread: Default::default(),
                    preview: String::new(),
                    time: String::new(),
                    avatar: None,
                    messages: Vec::new(),
                });
            threads.insert(format!("oneone/{global}"), template);
        }
    }
    let mut inserted = 0usize;
    let mut unmapped = 0usize;
    let total = partitions.len().div_ceil(30);
    for (index, wanted) in partitions.chunks(30).enumerate() {
        progress(format!(
            "Điện thoại: yêu cầu đợt {}/{total}; đã lưu {inserted} tin mới…",
            index + 1
        ));
        crypto = tokio::task::spawn_blocking(move || -> Result<Crypto> {
            crypto.ephemeral = crypto.codec.key_pair()?;
            crypto.record = None;
            Ok(crypto)
        })
        .await??;
        let batches = round(
            session,
            &transport,
            epoch,
            &cancel,
            &crypto,
            wanted,
            &mut progress,
        )
        .await?;
        for (batch_index, batch) in batches.into_iter().enumerate() {
            progress(format!(
                "Điện thoại: đợt {}/{total}, gói {}; đã lưu {inserted} tin mới…",
                index + 1,
                batch_index + 1
            ));
            let data = guarded(
                assets::download(&batch.url, 64 * 1024 * 1024),
                session,
                &cancel,
                &transport,
                epoch,
            )
            .await?;
            let cancel_decode = cancel.clone();
            let account_decode = session.clone();
            let (next, objects) = tokio::task::spawn_blocking(move || {
                decode_batch(
                    crypto,
                    data,
                    batch.metadata,
                    false,
                    cancel_decode,
                    account_decode,
                )
            })
            .await??;
            crypto = next;
            for object in objects {
                let unknown: Vec<_> = object
                    .get("partitions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .flat_map(|p| {
                        p.get("messages")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                    })
                    .map(|m| import::number(m, "senderId"))
                    .filter(|id| !id.is_empty() && !senders.contains_key(id))
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                for ids in unknown.chunks(50) {
                    let resolved = guarded(
                        crate::chat::resolve_sync_ids(session, ids),
                        session,
                        &cancel,
                        &transport,
                        epoch,
                    )
                    .await?;
                    senders.extend(resolved);
                }
                let account = session.clone();
                let map = threads.clone();
                let senders = senders.clone();
                let imported = tokio::task::spawn_blocking(move || -> Result<import::Imported> {
                    ensure!(account.is_active(), "Đã đăng xuất; dừng nhập lịch sử");
                    let mut store = crate::store::MessageStore::open(account.history_path())?;
                    import::save(&mut store, &object, &map, &senders, account.account_key())
                })
                .await??;
                inserted += imported.inserted;
                unmapped += imported.unmapped;
                for preview in imported.previews {
                    page(preview);
                    // Bound UI event production even when a phone batch has many partitions.
                    tokio::time::sleep(Duration::from_millis(16)).await;
                }
            }
        }
    }
    Ok(format!("Đã nhận lịch sử điện thoại: {inserted} tin mới, {} hội thoại được yêu cầu; {unmapped} tin chưa xác định hội thoại được giữ riêng. Chỉ khôi phục dữ liệu điện thoại còn giữ; tệp đính kèm phụ thuộc liên kết còn hạn.",partitions.len()))
}
fn decode_batch(
    mut state: Crypto,
    bytes: Vec<u8>,
    metadata: bool,
    conversations: bool,
    canceled: Arc<AtomicBool>,
    account: Session,
) -> Result<(Crypto, Vec<Value>)> {
    let mut out = Vec::new();
    let mut total = 0;
    for frame in wire::frames(&bytes)? {
        ensure!(
            !canceled.load(Ordering::Relaxed) && account.is_active(),
            "Đã dừng giải mã lịch sử điện thoại"
        );
        let plaintext = if metadata {
            let (plain, record) = state
                .codec
                .metadata(frame, &state.identity, &state.ephemeral)?;
            state.record = Some(record);
            plain
        } else {
            state.codec.message(
                frame,
                state
                    .record
                    .as_deref()
                    .context("Phone message arrived without metadata")?,
            )?
        };
        let expanded = Zeroizing::new(wire::expand(&plaintext)?);
        total += expanded.len();
        ensure!(
            total <= 64 * 1024 * 1024,
            "Decoded phone batch exceeds budget"
        );
        out.push(wire::chunk(&expanded, conversations)?);
    }
    Ok((state, out))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_rejection_does_not_wait_for_phone_timeout() {
        let event = WsEvent {
            cmd: crate::socket::CMD_SYNC_REQUEST,
            sub_cmd: 0,
            version: 1,
            payload: json!({"error_code":114}),
        };
        assert!(Round::default()
            .apply(&event, "synthetic")
            .unwrap_err()
            .to_string()
            .contains("114"));
    }
    #[test]
    fn opaque_roster_fallback_is_not_a_routing_id() {
        assert!(!routing_id("global-opaque-partition"));
        assert!(!routing_id("0"));
        assert!(!routing_id(""));
        assert!(routing_id("123456789"));
    }
    #[test]
    fn completion_requires_all_requested_scopes_and_matching_session() {
        let mut state = Round::default();
        let event = |id: &str, partition: &str| WsEvent {
            cmd: 601,
            sub_cmd: 0,
            version: 1,
            payload: json!({"data":{"controls":[{"content":{"act_type":"transfer_sync2","act":"upload_batch","data":json!({"syncId":id,"scopes":[{"partition":partition}],"isLast":0}).to_string()}}]}}),
        };
        state.apply(&event("other", "oneone/a"), "current").unwrap();
        assert!(!state.complete(&["oneone/a".into()]));
        state
            .apply(&event("current", "oneone/a"), "current")
            .unwrap();
        assert!(!state.complete(&["oneone/a".into(), "group/b".into()]));
        state
            .apply(&event("current", "group/b"), "current")
            .unwrap();
        assert!(state.complete(&["oneone/a".into(), "group/b".into()]));
        assert!(!state.complete(&[]));
    }
}
