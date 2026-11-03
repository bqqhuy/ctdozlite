//! Phone-approved conversation/message rounds on the authenticated live WebSocket.
use super::{
    assets,
    codec::{Codec, KeyPair},
    import, plan, policy, wire,
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

static SYNC_OWNER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
pub(super) struct Batch {
    pub metadata: bool,
    pub index: u64,
    pub url: String,
}
#[derive(Default)]
struct Round {
    number: usize,
    batches: Vec<Batch>,
    seen: HashMap<u64, (bool, String, i64)>,
    failed: HashSet<String>,
    last_index: Option<u64>,
    confirmed: bool,
    idle: bool,
    activity: u64,
}
impl Round {
    fn progress(&self, wait: u64) -> String {
        let received = self.seen.len();
        if received == 0 {
            return format!(
                "Tải về: lượt {} • {} • chờ {} giây",
                self.number,
                if self.confirmed {
                    "điện thoại đã xác nhận; chờ gói đầu"
                } else {
                    "chờ xác nhận trên điện thoại"
                },
                wait
            );
        }
        let max = self.seen.keys().max().copied().unwrap_or(0);
        let missing = (0..=self.last_index.unwrap_or(max))
            .filter(|idx| !self.seen.contains_key(idx))
            .count();
        let count=match self.last_index {Some(last)=>format!("{received}/{} mô tả gói • còn thiếu {missing} idx",last+1),None=>format!("{received} mô tả gói • idx lớn nhất {max} • thiếu {missing} idx trong 0–{max} • chưa nhận dấu cuối")};
        format!(
            "Tải về: lượt {} • {count} • {} {wait} giây",
            self.number,
            if self.idle {
                "điện thoại tạm nghỉ; đã chờ"
            } else {
                "chờ gói tiếp"
            }
        )
    }

    fn trace(&self,id:&str,wait:u64) {
        let max=self.seen.keys().max().copied();
        let missing=self.last_index.or(max).map_or(0,|last|(0..=last).filter(|idx|!self.seen.contains_key(idx)).count());
        crate::transfer_diagnostics::round(id,crate::transfer_diagnostics::RoundSummary{round:self.number,received:self.seen.len(),max_index:max,final_index:self.last_index,missing,confirmed:self.confirmed,idle:self.idle,active_wait_seconds:wait});
    }
    fn complete(&self, _wanted: &[String]) -> bool {
        self.last_index
            .is_some_and(|last| (0..=last).all(|idx| self.seen.contains_key(&idx)))
    }
    fn apply(&mut self, event: &WsEvent, id: &str) -> Result<bool> {
        if event.cmd == crate::socket::CMD_SYNC_REQUEST
            && event
                .payload
                .get("error_code")
                .and_then(integer)
                .is_some_and(|c| c != 0)
        {
            return Err(policy::rejection(
                &event.payload,
                self.confirmed || !self.seen.is_empty(),
            ));
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
            let status = data.get("status").and_then(integer).unwrap_or(0);
            let idx = data.get("idx").and_then(Value::as_u64);
            let last = data
                .get("isLast")
                .is_some_and(|v| v.as_bool() == Some(true) || integer(v) == Some(1));
            let error = data
                .get("err")
                .or_else(|| data.get("errorCode"))
                .and_then(integer)
                .unwrap_or(0);
            let scopes = data
                .get("scopes")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            crate::transfer_diagnostics::phone(
                id,
                crate::transfer_diagnostics::PhoneControl {
                    index: idx,
                    last,
                    scopes,
                    error,
                    status,
                    batch_type: data.get("batchType").and_then(integer).unwrap_or(0),
                    command: event.cmd,
                    sub_command: event.sub_cmd,
                    round: self.number,
                },
            );
            match act {
                "transfer_status" => {
                    self.activity += 1;
                    match status {
                        1 => self.idle = false,
                        2 => self.idle = true,
                        3 => waiting = true,
                        4 | 5 => {
                            self.confirmed = true;
                            self.idle = false;
                        }
                        6 => anyhow::bail!("Điện thoại đã từ chối đồng bộ"),
                        7 => anyhow::bail!("Đồng bộ đã được hủy trên điện thoại"),
                        8 => anyhow::bail!(
                            "Điện thoại đang bận đồng bộ; hãy kết thúc phiên cũ rồi thử lại"
                        ),
                        9 => anyhow::bail!("Điện thoại không đủ dung lượng để chuẩn bị lịch sử"),
                        10 => anyhow::bail!("Zalo trên điện thoại đã đăng xuất"),
                        _ => {}
                    }
                }
                "upload_batch" => {
                    let index = idx.context("Phone batch index missing")?;
                    ensure!(index < 2048, "Phone batch index exceeds limit");
                    let metadata = data.get("batchType").and_then(integer) == Some(2);
                    if error != 0 {
                        ensure!(
                            !metadata,
                            "Điện thoại báo lỗi metadata (mã {error}); không thể giải mã phiên"
                        );
                        let scopes = data
                            .get("scopes")
                            .and_then(Value::as_array)
                            .context("Gói lỗi không xác định phạm vi")?;
                        ensure!(!scopes.is_empty(), "Gói lỗi không xác định phạm vi");
                        for scope in scopes {
                            let partition = scope
                                .get("partition")
                                .and_then(Value::as_str)
                                .context("Gói lỗi thiếu định danh hội thoại")?;
                            ensure!(
                                partition.len() <= 512 && self.failed.len() < 100_000,
                                "Phone scope limit"
                            );
                            self.failed.insert(partition.to_owned());
                        }
                    }
                    let url = data
                        .get("msgUrl")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    ensure!(url.len() <= 16384, "Phone batch URL exceeds limit");
                    if last {
                        ensure!(
                            self.last_index.is_none_or(|old| old == index),
                            "Conflicting phone final index"
                        );
                        ensure!(
                            self.seen.keys().all(|idx| *idx <= index),
                            "Phone index exceeds final batch"
                        );
                        self.last_index = Some(index);
                    }
                    ensure!(
                        self.last_index.is_none_or(|last| index <= last),
                        "Phone index exceeds final batch"
                    );
                    if let Some(old) = self.seen.get(&index) {
                        ensure!(
                            old == &(metadata, url, error),
                            "Conflicting duplicate phone batch"
                        );
                    } else {
                        self.seen.insert(index, (metadata, url.clone(), error));
                        self.activity += 1;
                        if !url.is_empty() && error == 0 {
                            self.batches.push(Batch {
                                metadata,
                                index,
                                url,
                            });
                        }
                    }
                }
                _ if act.to_ascii_lowercase().contains("error") => {
                    return Err(policy::rejection(
                        &data,
                        self.confirmed || !self.seen.is_empty(),
                    ))
                }
                _ => {}
            }
        }
        Ok(waiting)
    }
}
fn integer(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str()?.parse().ok())
}
fn request(id: &str, crypto: &Crypto, wanted: &[plan::Query]) -> Value {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let queries = request_queries(wanted, now);
    json!({"data":{"syncId":id,"syncType":0,"ek":B64.encode(&crypto.ephemeral.public),"ik":B64.encode(&crypto.identity.public),
        "toDevice":0,"tempKey":"","deviceName":"ctdozlite Windows ARM64","req":{"type":if wanted.is_empty(){"conversation"}else{"message"},
        "priority":if wanted.is_empty(){0}else{2},"batchSize":2000,"queries":queries},"ver":1,"ussidx":0},"reqId":format!("req_sync_{id}")})
}
fn request_queries(wanted: &[plan::Query], now: u64) -> Vec<Value> {
    if wanted.is_empty() {
        vec![json!({"partition":"","from":0,"to":now,"limit":2147483647})]
    } else {
        wanted
            .iter()
            .map(|p| json!({"partition":p.partition,"from":p.from,"to":p.to,"limit":2147483647}))
            .collect()
    }
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
struct RunningRound {
    id: String,
    files: tokio::sync::mpsc::Receiver<Result<super::pipeline::Staged>>,
    decoder_ready: tokio::sync::watch::Sender<bool>,
    receiver: Option<tokio::task::JoinHandle<Result<HashSet<String>>>>,
    downloader: Option<tokio::task::JoinHandle<()>>,
    stop: tokio::sync::watch::Sender<bool>,
}
impl Drop for RunningRound {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        if let Some(task) = &self.receiver {
            task.abort();
        }
        if let Some(task) = &self.downloader {
            task.abort();
        }
    }
}
impl RunningRound {
    #[allow(clippy::too_many_arguments)]
    async fn open<F: FnMut(String) + Send + 'static>(
        session: &Session,
        transport: &SyncTransport,
        epoch: u64,
        cancel: Arc<AtomicBool>,
        crypto: &Crypto,
        wanted: &[plan::Query],
        progress: Arc<std::sync::Mutex<F>>,
        number: usize,
    ) -> Result<Self> {
        let id = sync_id()?;
        let packet = request(&id, crypto, wanted);
        ensure!(
            serde_json::to_vec(&packet)?.len() <= policy::PAYLOAD_BUDGET,
            "Yêu cầu đồng bộ vượt giới hạn payload cục bộ"
        );
        journal(session, Some(id.clone())).await?;
        let mut events = transport.subscribe();
        // Keep the journal if delivery is uncertain; the next explicit sync disposes it.
        if let Err(error) = transport.request(epoch, packet).await {
            if transport.dispose_reason(epoch, &id, 2).await.is_ok() {
                journal(session, None).await?;
            }
            return Err(error);
        }
        let (batch_tx, batch_rx) = tokio::sync::mpsc::channel(2048);
        let (decoder_ready, ready) = tokio::sync::watch::channel(false);
        let (files, downloader) = super::pipeline::start(
            session.clone(),
            batch_rx,
            cancel.clone(),
            progress.clone(),
            number,
            ready,
            id.clone(),
        );
        let account = session.clone();
        let transfer = transport.clone();
        let sync = id.clone();
        let (stop, mut stopped) = tokio::sync::watch::channel(false);
        let receiver = tokio::spawn(async move {
            let mut state = Round {
                number,
                ..Default::default()
            };
            let started = std::time::Instant::now();
            let mut active_wait = Duration::ZERO;
            let mut previous = std::time::Instant::now();
            let mut tick = tokio::time::interval(Duration::from_millis(200));
            let mut last_display = std::time::Instant::now();
            loop {
                tokio::select! {
                    _=stopped.changed()=>anyhow::bail!("Đã dừng nhận lịch sử"),
                    _=tick.tick()=>{
                        ensure!(account.is_active() && !cancel.load(Ordering::Acquire),"Đã dừng đồng bộ điện thoại");
                        ensure!(transfer.epoch()==Some(epoch),"Mất kết nối khi đồng bộ điện thoại; dữ liệu đã lưu được giữ");
                        let now=std::time::Instant::now();if !state.idle{active_wait+=now.duration_since(previous);}previous=now;
                        ensure!(started.elapsed()<Duration::from_secs(1800),"Phiên đồng bộ quá lâu; đã dừng và giữ dữ liệu đã nhập");
                        ensure!(active_wait<Duration::from_secs(if state.confirmed || !state.seen.is_empty(){300}else{180}),"Chưa đầy đủ: hết thời gian chờ điện thoại gửi gói tiếp theo; dữ liệu đã nhập được giữ");
                        if last_display.elapsed()>=Duration::from_secs(1){state.trace(&sync,active_wait.as_secs());if let Ok(mut p)=progress.lock(){p(state.progress(active_wait.as_secs()));}last_display=now;}
                    },
                    event=events.recv()=>{
                        let event=event.map_err(|error|match error {tokio::sync::broadcast::error::RecvError::Lagged(n)=>anyhow::anyhow!("Luồng control bỏ lỡ {n} sự kiện; không thể xác nhận lịch sử đầy đủ"),_=>anyhow::anyhow!("Luồng nhận lịch sử gián đoạn")})?;
                        let before=state.activity;
                        let waiting=state.apply(&event,&sync)?;
                        if state.activity!=before{active_wait=Duration::ZERO;previous=std::time::Instant::now();}
                        if waiting{if let Ok(mut p)=progress.lock(){p("Tải về: chờ xác nhận trên điện thoại…".into());}}
                        for batch in state.batches.drain(..){batch_tx.try_send(batch).map_err(|_|anyhow::anyhow!("Hàng đợi gói đồng bộ vượt giới hạn"))?;}
                        if state.activity!=before {if let Ok(mut p)=progress.lock(){p(state.progress(0));}last_display=std::time::Instant::now();}
                        if state.complete(&[]){state.trace(&sync,0);return Ok(state.failed);}
                    }
                }
            }
        });
        Ok(Self {
            id,
            files,
            decoder_ready,
            receiver: Some(receiver),
            downloader: Some(downloader),
            stop,
        })
    }
    async fn stop_tasks(&mut self) -> Result<HashSet<String>> {
        let _ = self.stop.send(true);
        self.files.close();
        while self.files.try_recv().is_ok() {}
        if let Some(mut task) = self.downloader.take() {
            if tokio::time::timeout(Duration::from_secs(5), &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
        match self.receiver.take() {
            Some(mut task) => match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
                Ok(result) => result.map_err(anyhow::Error::from).and_then(|r| r),
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    anyhow::bail!("Đã buộc dừng tác vụ nhận lịch sử cũ");
                }
            },
            None => Ok(HashSet::new()),
        }
    }

    async fn dispose(
        &mut self,
        session: &Session,
        transport: &SyncTransport,
        epoch: u64,
        success: bool,
        cancel: &AtomicBool,
    ) -> Result<HashSet<String>> {
        let received = self.stop_tasks().await;
        let reason = if cancel.load(Ordering::Acquire) {
            0
        } else if success && received.as_ref().is_ok_and(|failed| failed.is_empty()) {
            1
        } else {
            2
        };
        if transport
            .dispose_reason(epoch, &self.id, reason)
            .await
            .is_ok()
        {
            journal(session, None).await?;
        } else {
            crate::logger::log_error(
                "phone cleanup",
                "Session disposal deferred to next explicit sync",
            );
        }
        if let Some(rate) = received
            .as_ref()
            .err()
            .and_then(|e| e.downcast_ref::<policy::RateLimited>())
        {
            let path = session.history_path().with_extension("sync-limits.json");
            let delay = rate.0;
            tokio::task::spawn_blocking(move || -> Result<()> {
                let mut limits = policy::Limits::load(&path);
                limits.cooldown_until = (crate::login::unix_now() as u64).saturating_add(delay);
                crate::storage::atomic_write(&path, &serde_json::to_vec(&limits)?)
            })
            .await??;
        }
        received
    }
}
async fn journal(session: &Session, id: Option<String>) -> Result<()> {
    let path = session.history_path().with_extension("sync-session.json");
    tokio::task::spawn_blocking(move || {
        crate::storage::atomic_write(&path, &serde_json::to_vec(&id)?)
    })
    .await?
}
async fn clean_old(session: &Session, transport: &SyncTransport, epoch: u64) -> Result<()> {
    let path = session.history_path().with_extension("sync-session.json");
    let old = tokio::task::spawn_blocking(move || -> Result<Option<String>> {
        if !path.exists() {
            return Ok(None);
        }
        Ok(serde_json::from_slice(&crate::limits::read_file(
            &path, 4096,
        )?)?)
    })
    .await??;
    if let Some(id) = old {
        ensure!(
            id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Invalid saved sync identity"
        );
        transport.dispose_reason(epoch, &id, 3).await?;
        journal(session, None).await?;
    }
    Ok(())
}
async fn decoded_file(
    crypto: Crypto,
    staged: super::pipeline::Staged,
    conversations: bool,
    cancel: Arc<AtomicBool>,
    session: Session,
    sync_id: String,
    round: usize,
) -> Result<(Crypto, Vec<Value>)> {
    tokio::task::spawn_blocking(move || {
        let mut trace = crate::transfer_diagnostics::BatchWork::start(
            &sync_id,
            round,
            staged.batch.index,
            crate::transfer_diagnostics::BatchStage::Decode,
        );
        let bytes = crate::limits::read_file(&staged.path, 64 * 1024 * 1024)?;
        let result = decode_batch(
            crypto,
            bytes,
            staged.batch.metadata,
            conversations,
            cancel,
            session,
        );
        if let Ok((_, objects)) = &result {
            trace.finish(objects.len());
        }
        result
    })
    .await?
}
/// Request phone approval once, then import history without pausing the live-chat transport.
/// # Errors
/// Returns bounded download/decode/identity/store failures; committed pages remain available.
pub(crate) async fn download<F: FnMut(String) + Send + 'static, G: FnMut(Thread) + Send>(
    session: &Session,
    transport: SyncTransport,
    cancel: Arc<AtomicBool>,
    full_rescan: bool,
    recent_only: bool,
    mut progress: F,
    mut page: G,
) -> Result<String> {
    let _owner = SYNC_OWNER.lock().await;
    ensure!(
        session.is_active() && session.login.is_some(),
        "Chưa đăng nhập"
    );
    let mut diagnostic = crate::transfer_diagnostics::PhoneRun::start(cancel.clone());
    let epoch = transport
        .epoch()
        .context("WebSocket chưa sẵn sàng; chờ kết nối rồi thử lại")?;
    let limit_file = session.history_path().with_extension("sync-limits.json");
    let cooldown =
        tokio::task::spawn_blocking(move || policy::Limits::load(&limit_file).cooldown_until)
            .await?;
    let now = crate::login::unix_now() as u64;
    ensure!(
        cooldown <= now,
        "Máy chủ giới hạn tần suất; thử lại sau {} giây",
        cooldown.saturating_sub(now)
    );
    progress("Đang chuẩn bị bộ giải mã đồng bộ điện thoại…".into());
    let cache = session
        .cache_path()
        .parent()
        .context("Account cache unavailable")?
        .to_owned();
    let bytes = guarded(assets::codec(&cache), session, &cancel, &transport, epoch).await?;
    let codec_bytes = bytes.clone();
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
    clean_old(session, &transport, epoch).await?;
    let progress = Arc::new(std::sync::Mutex::new(progress));
    let mut run = RunningRound::open(
        session,
        &transport,
        epoch,
        cancel.clone(),
        &crypto,
        &[],
        progress.clone(),
        0,
    )
    .await?;
    let mut conversations = Vec::new();
    let result = async {
        let mut pending = std::collections::VecDeque::new();
        while let Some(file) = run.files.recv().await {
            let file = file?;
            if !file.batch.metadata && crypto.record.is_none() {
                ensure!(
                    pending.len() < 8,
                    "Chưa nhận khóa metadata; đã đạt giới hạn chờ"
                );
                pending.push_back(file);
                continue;
            }
            pending.push_front(file);
            while let Some(file) = pending.pop_front() {
                let (next, objects) = decoded_file(
                    crypto,
                    file,
                    true,
                    cancel.clone(),
                    session.clone(),
                    run.id.clone(),
                    0,
                )
                .await?;
                crypto = next;
                if crypto.record.is_some() {
                    let _ = run.decoder_ready.send(true);
                }
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
        }
        ensure!(pending.is_empty(), "Không nhận được metadata giải mã");
        Ok::<_, anyhow::Error>(crypto)
    }
    .await;
    let received = run
        .dispose(session, &transport, epoch, result.is_ok(), &cancel)
        .await;
    let crypto = result?;
    ensure!(
        received?.is_empty(),
        "Danh sách hội thoại có gói lỗi; chưa thể lập kế hoạch đầy đủ"
    );
    let ordered = plan::conversations(&conversations)?;
    let partitions: Vec<_> = ordered.iter().map(|c| c.partition.clone()).collect();
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
    let owners: HashMap<_, _> = threads
        .iter()
        .map(|(p, t)| (p.clone(), t.id.clone()))
        .collect();
    let account = session.clone();
    let check_owners = owners.clone();
    let mut coverage = tokio::task::spawn_blocking(
        move || -> Result<HashMap<String, crate::store::PhoneSyncRange>> {
            ensure!(account.is_active(), "Đã đăng xuất; dừng đồng bộ");
            let store = crate::store::MessageStore::open(account.history_path())?;
            if full_rescan {
                store.reset_phone_sync_ranges()?;
            }
            Ok(store
                .phone_sync_ranges()?
                .into_iter()
                .filter(|(p, r)| check_owners.get(p) == Some(&r.thread_id))
                .collect())
        },
    )
    .await??;
    let threads = Arc::new(threads);
    let senders = Arc::new(senders);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let limit_path = session.history_path().with_extension("sync-limits.json");
    let read_path = limit_path.clone();
    let mut limits = tokio::task::spawn_blocking(move || policy::Limits::load(&read_path)).await?;
    let jobs = plan::queries(&ordered, &coverage, now, recent_only);
    let mut pending = policy::split(jobs, Some(limits.count.unwrap_or(30).min(30)))?;
    let emit = |text: String| {
        if let Ok(mut p) = progress.lock() {
            p(text);
        }
    };
    let identity = (
        crypto.identity.public.clone(),
        crypto.identity.private.clone(),
    );
    drop(crypto);
    let writer = super::pipeline::Writer::start(session.clone(), threads.clone());
    let mut senders = senders;
    let mut attempted_senders = HashSet::new();
    let mut inserted = 0;
    let mut unmapped = 0;
    let mut adjustments = 0;
    let mut number = 0;
    let mut failures = 0;
    let mut last_start = std::time::Instant::now() - Duration::from_secs(2);
    while let Some(wanted) = pending.pop_front() {
        ensure!(
            session.is_active() && !cancel.load(Ordering::Acquire),
            "Đã dừng đồng bộ"
        );
        guarded(
            async {
                tokio::time::sleep(Duration::from_secs(2).saturating_sub(last_start.elapsed()))
                    .await;
                Ok(())
            },
            session,
            &cancel,
            &transport,
            epoch,
        )
        .await?;
        last_start = std::time::Instant::now();
        number += 1;
        emit(format!(
            "Tải về: lượt {number} • {} hội thoại • chờ điện thoại",
            wanted.len()
        ));
        let bytes = codec_bytes.clone();
        let public = identity.0.clone();
        let private = identity.1.clone();
        let mut crypto = tokio::task::spawn_blocking(move || -> Result<Crypto> {
            let mut codec = Codec::new(&bytes)?;
            let ephemeral = codec.key_pair()?;
            Ok(Crypto {
                codec,
                identity: KeyPair { public, private },
                ephemeral,
                record: None,
            })
        })
        .await??;
        let mut run = RunningRound::open(
            session,
            &transport,
            epoch,
            cancel.clone(),
            &crypto,
            &wanted,
            progress.clone(),
            number,
        )
        .await?;
        let before_unmapped = unmapped;
        let result = async {
            let mut waiting = std::collections::VecDeque::new();
            while let Some(staged) = run.files.recv().await {
                let staged = staged?;
                if !staged.batch.metadata && crypto.record.is_none() {
                    ensure!(
                        waiting.len() < 8,
                        "Chưa nhận khóa metadata; đã đạt giới hạn chờ"
                    );
                    waiting.push_back(staged);
                    continue;
                }
                waiting.push_front(staged);
                while let Some(staged) = waiting.pop_front() {
                    let batch_index=staged.batch.index;
                    emit(format!(
                        "Nhập CSDL: {inserted} tin mới • đang giải mã gói {}",
                        staged.batch.index
                    ));
                    let (next, objects) =
                        decoded_file(crypto, staged, false, cancel.clone(), session.clone(),run.id.clone(),number)
                            .await?;
                    crypto = next;
                    if crypto.record.is_some() {
                        let _ = run.decoder_ready.send(true);
                    }
                    emit(format!("Nhập CSDL: {inserted} tin mới • đang nhập gói {batch_index}"));
                    for object in objects {
                        ensure!(session.is_active() && !cancel.load(Ordering::Acquire),"Đã dừng nhập lịch sử; dữ liệu đã lưu được giữ");
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
                            .filter(|id| {
                                !id.is_empty()
                                    && !senders.contains_key(id)
                                    && !attempted_senders.contains(id)
                            })
                            .collect::<HashSet<_>>()
                            .into_iter()
                            .collect();
                        let mut resolving=if unknown.is_empty(){None}else{
                            emit(format!("Nhập CSDL: {inserted} tin mới • đang xác định người gửi cho gói {batch_index}"));
                            Some(crate::transfer_diagnostics::BatchWork::start(&run.id,number,batch_index,crate::transfer_diagnostics::BatchStage::ResolveSenders))
                        };
                        for ids in unknown.chunks(50) {
                            let resolved = crate::chat::resolve_sync_ids(session, ids)
                                .await
                                .unwrap_or_default();
                            attempted_senders.extend(ids.iter().cloned());
                            Arc::make_mut(&mut senders).extend(resolved);
                        }
                        if let Some(trace)=&mut resolving{trace.finish(unknown.len());}
                        emit(format!("Nhập CSDL: {inserted} tin mới • đang nhập gói {batch_index}"));
                        let mut trace=crate::transfer_diagnostics::BatchWork::start(&run.id,number,batch_index,crate::transfer_diagnostics::BatchStage::Import);
                        let imported = writer.save(object, senders.clone()).await?;
                        trace.finish(imported.inserted);
                        inserted += imported.inserted;
                        unmapped += imported.unmapped;
                        for preview in imported.previews {
                            page(preview);
                            // Bound UI event production even when a phone batch has many partitions.
                            tokio::task::yield_now().await;
                        }
                    }
                    emit(format!("Nhập CSDL: {inserted} tin mới • đã nhập gói {batch_index} • chờ gói tiếp theo"));
                }
            }
            ensure!(waiting.is_empty(), "Không nhận được metadata giải mã");
            Ok::<_, anyhow::Error>(())
        }
        .await;
        let received = run
            .dispose(session, &transport, epoch, result.is_ok(), &cancel)
            .await;
        result?;
        let failed = match received {
            Ok(failed) => failed,
            Err(error) => {
                let Some(limit) = error.downcast_ref::<policy::LimitRejection>() else {
                    return Err(error);
                };
                ensure!(
                    adjustments < 4 && wanted.len() > 1,
                    "Đã dừng điều chỉnh giới hạn đồng bộ"
                );
                let maximum = limit
                    .0
                    .unwrap_or(wanted.len().div_ceil(2))
                    .min(wanted.len() - 1)
                    .max(1);
                adjustments += 1;
                limits.count = Some(maximum);
                let mut rest = wanted;
                rest.extend(pending.into_iter().flatten());
                pending = policy::split(rest, Some(maximum))?;
                continue;
            }
        };
        failures += failed.len();
        if unmapped == before_unmapped {
            let mut completed: Vec<_> = wanted
                .into_iter()
                .filter(|q| !failed.contains(&q.partition))
                .collect();
            plan::checkpoint_completed(&mut coverage, &mut completed, &owners)?;
            let ranges: Vec<_> = coverage
                .iter()
                .map(|(p, r)| (p.clone(), r.clone()))
                .collect();
            let account = session.clone();
            tokio::task::spawn_blocking(move || -> Result<()> {
                ensure!(account.is_active(), "Đã đăng xuất");
                crate::store::MessageStore::open(account.history_path())?
                    .save_phone_sync_ranges(&ranges)
            })
            .await??;
        }
        if adjustments > 0 {
            let path = limit_path.clone();
            let data = serde_json::to_vec(&limits)?;
            tokio::task::spawn_blocking(move || crate::storage::atomic_write(&path, &data))
                .await??;
        }
        emit(format!("Nhập CSDL: {inserted} tin mới • hoàn tất lượt {number} • {} hội thoại lỗi chưa ghi checkpoint",failed.len()));
    }
    diagnostic.finish();
    Ok(format!("Đã hoàn tất tải và nhập lịch sử: {inserted} tin mới; {unmapped} tin chưa xác định hội thoại được giữ riêng; {failures} hội thoại lỗi cần đồng bộ lại."))
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
    fn batch(index: u64, last: bool, metadata: bool) -> WsEvent {
        WsEvent {
            cmd: 601,
            sub_cmd: 0,
            version: 1,
            payload: json!({"controls":[{"content":{"act_type":"transfer_sync2","act":"upload_batch","data":{"syncId":"current","idx":index,"isLast":last,"batchType":if metadata{2}else{1},"scopes":[{"partition":if metadata{"metadata_current"}else{"oneone/b"}}],"msgUrl":format!("https://example.invalid/{index}")}}}]}),
        }
    }
    #[test]
    fn completion_waits_for_every_index_even_when_final_arrives_first() {
        let mut r = Round::default();
        r.apply(&batch(2, true, false), "current").unwrap();
        r.apply(&batch(0, false, true), "current").unwrap();
        assert!(!r.complete(&[]));
        r.apply(&batch(1, false, false), "current").unwrap();
        assert!(r.complete(&["a".into(), "b".into()]));
    }
    #[test]
    fn duplicate_does_not_fill_missing_index_and_bad_index_is_rejected() {
        let mut r = Round::default();
        r.apply(&batch(1, true, false), "current").unwrap();
        r.apply(&batch(1, true, false), "current").unwrap();
        assert!(!r.complete(&[]));
        assert!(r.apply(&batch(2, false, false), "current").is_err());
        assert!(r.apply(&batch(2048, false, false), "current").is_err());
    }
    #[test]
    fn foreign_session_does_not_complete() {
        let mut r = Round::default();
        r.apply(&batch(0, true, true), "other").unwrap();
        assert!(!r.complete(&[]));
    }
    #[test]
    fn mobile_idle_and_terminal_statuses_are_explicit() {
        let status = |n| WsEvent {
            cmd: 601,
            sub_cmd: 0,
            version: 1,
            payload: json!({"controls":[{"content":{"act_type":"transfer_sync2","act":"transfer_status","data":{"syncId":"current","status":n}}}]}),
        };
        let mut r = Round::default();
        r.apply(&status(2), "current").unwrap();
        assert!(r.idle);
        r.apply(&status(1), "current").unwrap();
        assert!(!r.idle);
        for n in 6..=10 {
            assert!(r.apply(&status(n), "current").is_err());
        }
    }
}

#[cfg(test)]
mod range_request_tests {
    use super::*;
    #[test]
    fn message_request_keeps_partition_order_and_exact_range_bounds() {
        let wanted = vec![
            plan::Query {
                partition: "group/recent".into(),
                from: 100,
                to: 500,
            },
            plan::Query {
                partition: "oneone/older".into(),
                from: 0,
                to: 250,
            },
        ];
        let values = request_queries(&wanted, 999);
        assert_eq!(
            values[0],
            json!({"partition":"group/recent", "from":100, "to":500, "limit":2147483647})
        );
        assert_eq!(values[1]["partition"], "oneone/older");
        assert_eq!(values[1]["to"], 250);
        assert_eq!(
            request_queries(&[], 999)[0],
            json!({"partition":"", "from":0, "to":999, "limit":2147483647})
        );
    }
}

#[cfg(test)]
mod batch_completion_tests {
    use super::*;
    fn event(partition: &str, last: bool, err: i64) -> WsEvent {
        WsEvent {
            cmd: 601,
            sub_cmd: 0,
            version: 1,
            payload: json!({"controls":[{"content":{"act_type":"transfer_sync2","act":"upload_batch","data":{"syncId":"fixture","batchType":2,"idx":1,"scopes":[{"partition":partition}],"isLast":last,"err":err}}}]}),
        }
    }
    #[test]
    fn metadata_scope_is_not_partition_completion() {
        let mut r = Round::default();
        r.apply(&event("metadata_fixture", true, 0), "fixture")
            .unwrap();
        assert!(!r.complete(&["group/a".into(), "group/b".into()]));
        let mut first = event("group/a", false, 0);
        first.payload["controls"][0]["content"]["data"]["idx"] = json!(0);
        r.apply(&first, "fixture").unwrap();
        assert!(r.complete(&["group/a".into(), "group/b".into()]));
    }
    #[test]
    fn failed_metadata_aborts_but_message_scope_is_recorded_for_retry() {
        let mut r = Round::default();
        assert!(r.apply(&event("group/a", true, 42), "fixture").is_err());
        let mut message = event("group/a", true, 42);
        message.payload["controls"][0]["content"]["data"]["batchType"] = json!(1);
        r.apply(&message, "fixture").unwrap();
        assert!(r.failed.contains("group/a"));
        assert!(r.batches.is_empty());
    }
}

/// Dispose a journaled session once the newly authenticated socket becomes ready.
pub(crate) async fn clean_after_connect(session: Session, transport: SyncTransport) {
    let _owner = SYNC_OWNER.lock().await;
    for _ in 0..100 {
        if !session.is_active() {
            return;
        }
        if let Some(epoch) = transport.epoch() {
            if clean_old(&session, &transport, epoch).await.is_err() {
                crate::logger::log_error("phone cleanup", "Deferred stale-session cleanup");
            }
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod transport_round_tests {
    use super::*;
    #[test]
    fn six_hundred_out_of_order_batches_across_300_control_pages_complete_only_with_every_idx() {
        let mut queues = crate::socket::queue_receiver::PullQueues::new(1);
        let now = std::time::Instant::now();
        let key = (crate::socket::CMD_CTRL_PULL_CTRL, 1);
        queues.register(key, "initial".into(), now);
        let mut round = Round::default();
        for page in 0..300u64 {
            let indexes = [page * 2 + 1, page * 2];
            let controls:Vec<_>=indexes.into_iter().map(|idx|json!({"content":{"act_type":"transfer_sync2","act":"upload_batch","data":{"syncId":"fixture","idx":idx,"isLast":idx==599,"batchType":if idx==0{2}else{1},"scopes":[{"partition":"oneone/fake"}],"msgUrl":format!("https://example.invalid/{idx}")}}})).collect();
            let payload = json!({"data":{"lastActionId":page+2,"more":if page<299{1}else{0},"controls":controls}});
            let wire = crate::socket::encode_frame(key.0, key.1, 1, Some(&payload));
            let (version, cmd, sub_cmd, decoded) = crate::socket::decode_frame(&wire).unwrap();
            let event = WsEvent {
                cmd,
                sub_cmd,
                version,
                payload: decoded,
            };
            round.apply(&event, "fixture").unwrap();
            let next = queues.page(key, &event.payload, now);
            assert_eq!(next.is_some(), page < 299);
            if page < 299 {
                queues.sent(key, format!("req-{page}"), now);
                assert!(!round.complete(&[]));
            }
            round.batches.clear();
        }
        assert_eq!(round.seen.len(), 600);
        assert!(round.complete(&[]));
        assert!(queues.due(now + Duration::from_secs(90)).is_empty());
    }
    #[test]
    fn received_descriptors_never_imply_completion_without_a_final_marker() {
        let mut round = Round::default();
        for index in 0..220u64 {
            let event = WsEvent {
                cmd: 603,
                sub_cmd: 1,
                version: 1,
                payload: json!({"controls":[{"content":{"act_type":"transfer_sync2","act":"upload_batch","data":{"syncId":"fixture","idx":index,"batchType":1,"msgUrl":"https://example.invalid/file"}}}]}),
            };
            round.apply(&event, "fixture").unwrap();
        }
        assert!(!round.complete(&[]));
        let progress = round.progress(80);
        assert!(progress.contains("220 mô tả"));
        assert!(progress.contains("chưa nhận dấu cuối"));
    }
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    fn running(receiver_done: Arc<AtomicBool>, download_done: Arc<AtomicBool>) -> RunningRound {
        let (tx, files) = tokio::sync::mpsc::channel(2);
        let (decoder_ready, _) = tokio::sync::watch::channel(false);
        let (stop, mut stopped) = tokio::sync::watch::channel(false);
        let receiver = tokio::spawn(async move {
            let _ = stopped.changed().await;
            receiver_done.store(true, Ordering::Release);
            Ok(HashSet::new())
        });
        let downloader = tokio::spawn(async move {
            tx.closed().await;
            download_done.store(true, Ordering::Release);
        });
        RunningRound {
            id: "fixture".into(),
            files,
            decoder_ready,
            receiver: Some(receiver),
            downloader: Some(downloader),
            stop,
        }
    }
    #[tokio::test]
    async fn cancelling_and_restarting_joins_both_old_tasks_before_the_next_round() {
        for _ in 0..2 {
            let received = Arc::new(AtomicBool::new(false));
            let downloaded = Arc::new(AtomicBool::new(false));
            let mut round = running(received.clone(), downloaded.clone());
            round.stop_tasks().await.unwrap();
            assert!(received.load(Ordering::Acquire));
            assert!(downloaded.load(Ordering::Acquire));
            assert!(round.receiver.is_none());
            assert!(round.downloader.is_none());
        }
    }
    #[test]
    fn late_idx_74_does_not_replace_the_max_idx_or_hide_the_32_missing_descriptors() {
        let missing = [
            75, 76, 79, 80, 82, 83, 84, 85, 88, 92, 94, 102, 108, 110, 114, 115, 117, 133, 136,
            138, 142, 144, 145, 146, 147, 151, 154, 159, 164, 166, 169, 172,
        ];
        let mut round = Round::default();
        for idx in (0..186u64)
            .filter(|idx| !missing.contains(idx) && *idx != 74)
            .chain(std::iter::once(74))
        {
            round.seen.insert(idx, (false, String::new(), 0));
        }
        assert_eq!(round.seen.len(), 154);
        let status = round.progress(120);
        assert!(status.contains("idx lớn nhất 185"));
        assert!(status.contains("thiếu 32 idx"));
        assert!(!round.complete(&[]));
    }
}
