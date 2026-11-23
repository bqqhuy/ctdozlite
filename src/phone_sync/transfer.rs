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
/// Serializes read-modify-write of the account's session journal across parallel rounds.
static JOURNAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Incremented by the user's "wake the phone" action; each active round reacts once.
static WAKE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Last time any round of the running sync received a new batch, and which round it was.
/// Only one sync runs at a time (`SYNC_OWNER`), so this is scoped to that run.
static PEER_BATCH: std::sync::Mutex<Option<(std::time::Instant, usize)>> =
    std::sync::Mutex::new(None);
/// An Idle round waits this long while another round is served before it is woken.
const IDLE_PEER_WAIT: Duration = Duration::from_secs(60);
/// After that wake-up, a still Idle round gets this long before it is retried.
const IDLE_PEER_GRACE: Duration = Duration::from_secs(60);

/// What an Idle round should do while the phone serves other rounds.
#[derive(Debug, PartialEq, Eq)]
enum IdleAction {
    Wait,
    Wake,
    Retry,
}
/// Decide for an Idle round. `idle_for` is how long it has been Idle, `peer_served` whether
/// another round received a batch since it became Idle and `woken_for` how long ago this
/// rule already woke it. One wake-up only; the retry reopens the range after disposal.
fn idle_action(idle_for: Duration, peer_served: bool, woken_for: Option<Duration>) -> IdleAction {
    match woken_for {
        Some(after) if after >= IDLE_PEER_GRACE => IdleAction::Retry,
        Some(_) => IdleAction::Wait,
        None if peer_served && idle_for >= IDLE_PEER_WAIT => IdleAction::Wake,
        None => IdleAction::Wait,
    }
}
fn note_peer_batch(round: usize) {
    if let Ok(mut last) = PEER_BATCH.lock() {
        *last = Some((std::time::Instant::now(), round));
    }
}
fn peer_served_since(round: usize, since: std::time::Instant) -> bool {
    PEER_BATCH
        .lock()
        .ok()
        .and_then(|last| *last)
        .is_some_and(|(at, other)| other != round && at > since)
}

/// Message sessions open at once in test fixtures (production uses `Sync2PcConfig::max_concurrent_sync_items`).
#[cfg(test)]
const PARALLEL_ROUNDS: usize = 4;
/// Recent conversations fetched first with a short page (Zalo PC `sync_recent_chat`).
const PRIORITY_CONVERSATIONS: usize = 10;
const PRIORITY_MESSAGES: u32 = 100;
/// Spacing between session requests, as Zalo Web sends a wave.
const ROUND_START_GAP: Duration = Duration::from_millis(600);
/// A phone reporting Idle this long is treated as stalled (`sync_paused_timeout_ms`).
const IDLE_LIMIT: Duration = Duration::from_secs(600);
/// Hint the user after this much silence from a confirmed phone.
const STALL_HINT: Duration = Duration::from_secs(20);
/// Send one automatic wake-up after this much silence.
const AUTO_WAKE_AFTER: Duration = Duration::from_secs(30);
/// Minimum spacing between wake-up requests for one session.
const WAKE_SPACING: Duration = Duration::from_secs(15);
/// Silence after which a confirmed round is declared stalled.
const STALL_AFTER: Duration = Duration::from_secs(300);
/// Upper bound for one round, counted from the phone's confirmation.
const ROUND_CAP: Duration = Duration::from_secs(2 * 60 * 60);
/// Reconnects a single sync may resume across.
const MAX_RECONNECTS: u8 = 2;
/// Waiting for a new socket generation after a disconnect.
const RECONNECT_WAIT: Duration = Duration::from_secs(30);
/// Consecutive failed rounds that stop the whole sync.
const MAX_CONSECUTIVE_FAILURES: usize = 2;
/// Sessions remembered for cleanup; more than the parallel limit plus retries.
const JOURNAL_LIMIT: usize = 16;

/// The phone stopped delivering batches for a confirmed round.
#[derive(Debug)]
pub(super) struct Stalled;
impl std::fmt::Display for Stalled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Chưa đầy đủ: điện thoại ngừng gửi gói; dữ liệu đã nhập được giữ")
    }
}
impl std::error::Error for Stalled {}

/// The socket generation that carried a round ended.
#[derive(Debug)]
pub(super) struct Disconnected;
impl std::fmt::Display for Disconnected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Mất kết nối khi đồng bộ điện thoại; dữ liệu đã lưu được giữ")
    }
}
impl std::error::Error for Disconnected {}

/// Ask every active round to send a wake-up to the phone (rate limited per session).
pub(crate) fn request_wake() {
    WAKE_SEQ.fetch_add(1, Ordering::AcqRel);
}

type ActivePrioritySlot = (String, usize, Arc<std::sync::Mutex<TransferQueue>>);
static ACTIVE_PRIORITY_QUEUE: std::sync::Mutex<Option<ActivePrioritySlot>> =
    std::sync::Mutex::new(None);

/// Move missing ranges of the currently opened conversation to the front of the active
/// Sync2 transfer queue (`SELECT_CONVERSATION` / `addRangesToFrontInQueue`), scoped by
/// account owner and runtime generation. Returns `true` if any pending range was promoted.
pub(crate) fn prioritize_conversation(owner: &str, runtime_id: usize, thread_id: &str) -> bool {
    let queue_arc = {
        let guard = ACTIVE_PRIORITY_QUEUE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some((active_owner, active_runtime, q))
                if active_owner == owner && *active_runtime == runtime_id =>
            {
                Some(q.clone())
            }
            _ => None,
        }
    };
    let Some(queue_arc) = queue_arc else {
        return false;
    };
    let mut queue = queue_arc.lock().unwrap_or_else(|e| e.into_inner());
    queue.add_ranges_to_front_in_queue(thread_id)
}

struct ActivePriorityRegistration {
    owner: String,
    runtime_id: usize,
}

impl ActivePriorityRegistration {
    fn register(
        owner: String,
        runtime_id: usize,
        queue: Arc<std::sync::Mutex<TransferQueue>>,
    ) -> Self {
        if let Ok(mut slot) = ACTIVE_PRIORITY_QUEUE.lock() {
            *slot = Some((owner.clone(), runtime_id, queue));
        }
        Self { owner, runtime_id }
    }
}

impl Drop for ActivePriorityRegistration {
    fn drop(&mut self) {
        if let Ok(mut slot) = ACTIVE_PRIORITY_QUEUE.lock() {
            if matches!(slot.as_ref(), Some((o, r, _)) if o == &self.owner && *r == self.runtime_id)
            {
                *slot = None;
            }
        }
    }
}

const MAX_CONSECUTIVE_PRIORITY_POPS: usize = 2;

/// Thread-safe Sync2 job queue supporting open-conversation front prioritization
/// with deduplication and anti-starvation scheduling.
pub(super) struct TransferQueue {
    queue: std::collections::VecDeque<Job>,
    thread_partitions: HashMap<String, Vec<String>>,
    in_flight: HashSet<String>,
    front_promoted: HashSet<String>,
    consecutive_priority_pops: usize,
    next_number: usize,
}

impl TransferQueue {
    fn new(
        queue: std::collections::VecDeque<Job>,
        owners: &HashMap<String, String>,
        next_number: usize,
    ) -> Self {
        let mut thread_partitions: HashMap<String, Vec<String>> = HashMap::new();
        for (partition, thread_id) in owners {
            thread_partitions
                .entry(thread_id.clone())
                .or_default()
                .push(partition.clone());
        }
        Self {
            queue,
            thread_partitions,
            in_flight: HashSet::new(),
            front_promoted: HashSet::new(),
            consecutive_priority_pops: 0,
            next_number,
        }
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    fn allocate_number(&mut self) -> usize {
        self.next_number += 1;
        self.next_number
    }

    /// Promote pending full-range queries for `thread_id` (or raw partition key) to the front,
    /// skipping partitions already in-flight or already front-promoted to prevent duplicate jobs.
    fn add_ranges_to_front_in_queue(&mut self, thread_or_partition: &str) -> bool {
        let target_partitions: Vec<String> = self
            .thread_partitions
            .get(thread_or_partition)
            .cloned()
            .unwrap_or_else(|| {
                if thread_or_partition.contains('/') {
                    vec![thread_or_partition.to_owned()]
                } else {
                    Vec::new()
                }
            });
        let eligible: HashSet<String> = target_partitions
            .into_iter()
            .filter(|p| !self.in_flight.contains(p) && !self.front_promoted.contains(p))
            .collect();
        if eligible.is_empty() {
            return false;
        }

        // Do not pull out of jobs that are already front-promoted user priority jobs;
        // extract matching missing ranges from background jobs without duplicating them.
        let mut promoted_queries: Vec<plan::Query> = Vec::new();
        let mut seen_ranges: HashSet<(String, i64, i64)> = HashSet::new();
        let mut rebuilt = std::collections::VecDeque::with_capacity(self.queue.len());

        while let Some(mut job) = self.queue.pop_front() {
            if job.priority || job.user_prioritized {
                rebuilt.push_back(job);
                continue;
            }
            let mut remaining = Vec::with_capacity(job.wanted.len());
            for q in job.wanted.drain(..) {
                if eligible.contains(&q.partition) {
                    if seen_ranges.insert((q.partition.clone(), q.from, q.to)) {
                        promoted_queries.push(q);
                    }
                } else {
                    remaining.push(q);
                }
            }
            if !remaining.is_empty() {
                job.wanted = remaining;
                rebuilt.push_back(job);
            }
        }
        self.queue = rebuilt;

        if promoted_queries.is_empty() {
            return false;
        }
        for q in &promoted_queries {
            self.front_promoted.insert(q.partition.clone());
        }
        let number = self.allocate_number();
        self.queue.push_front(Job {
            wanted: promoted_queries,
            number,
            attempt: 0,
            priority: false,
            user_prioritized: true,
        });
        true
    }

    /// Pop next job while enforcing anti-starvation fairness for background jobs.
    fn pop_next(&mut self) -> Option<Job> {
        if self.queue.is_empty() {
            return None;
        }
        let job = if self.consecutive_priority_pops >= MAX_CONSECUTIVE_PRIORITY_POPS {
            if let Some(bg_idx) = self
                .queue
                .iter()
                .position(|j| !j.priority && !j.user_prioritized)
            {
                self.consecutive_priority_pops = 0;
                self.queue.remove(bg_idx)?
            } else {
                let front = self.queue.pop_front()?;
                if front.priority || front.user_prioritized {
                    self.consecutive_priority_pops += 1;
                } else {
                    self.consecutive_priority_pops = 0;
                }
                front
            }
        } else {
            let front = self.queue.pop_front()?;
            if front.priority || front.user_prioritized {
                self.consecutive_priority_pops += 1;
            } else {
                self.consecutive_priority_pops = 0;
            }
            front
        };
        for q in &job.wanted {
            self.front_promoted.remove(&q.partition);
            self.in_flight.insert(q.partition.clone());
        }
        Some(job)
    }

    fn finish_job(&mut self, wanted: &[plan::Query]) {
        for q in wanted {
            self.in_flight.remove(&q.partition);
        }
    }

    fn push_front_requeue(&mut self, job: Job) {
        self.finish_job(&job.wanted);
        self.queue.push_front(job);
    }

    fn push_back_retry(&mut self, job: Job) {
        self.finish_job(&job.wanted);
        self.queue.push_back(job);
    }
}

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
    /// The phone reported Active after Idle and has not sent a batch since.
    resumed: bool,
    activity: u64,
    /// Last diagnostic summary written: when, and the counters it carried.
    traced: Option<(std::time::Instant, TraceKey)>,
}
/// Counters whose change is worth a new diagnostic summary line.
type TraceKey = (usize, Option<u64>, Option<u64>, usize, bool, bool);
/// Unchanged round summaries are repeated at most this often.
const TRACE_EVERY: Duration = Duration::from_secs(10);
impl Round {
    #[cfg_attr(not(test), allow(dead_code))]
    fn new(number: usize) -> Self {
        Self {
            number,
            ..Default::default()
        }
    }

    fn progress(&self, wait: u64) -> String {
        let base = self.progress_counts(wait);
        if self.idle {
            return format!("{base} • iPhone đang tạm nghỉ: mở Zalo và giữ màn hình sáng");
        }
        if self.resumed {
            return format!("{base} • điện thoại đã hoạt động lại; đang chờ gửi tiếp");
        }
        if (self.confirmed || !self.seen.is_empty()) && wait >= STALL_HINT.as_secs() {
            return format!("{base} • điện thoại có thể đã khóa màn hình hoặc chuyển app: mở Zalo, giữ màn hình sáng hoặc bấm Đánh thức điện thoại");
        }
        base
    }
    fn progress_counts(&self, wait: u64) -> String {
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

    /// Counters for the diagnostic summary.
    fn trace_key(&self) -> TraceKey {
        let max = self.seen.keys().max().copied();
        let missing = self.last_index.or(max).map_or(0, |last| {
            (0..=last)
                .filter(|idx| !self.seen.contains_key(idx))
                .count()
        });
        (
            self.seen.len(),
            max,
            self.last_index,
            missing,
            self.confirmed,
            self.idle,
        )
    }
    /// Whether a summary is due: counters changed, `TRACE_EVERY` passed, or `force`.
    fn trace_due(&mut self, now: std::time::Instant, force: bool) -> bool {
        let key = self.trace_key();
        let due = force
            || self.traced.as_ref().is_none_or(|(at, previous)| {
                *previous != key || now.duration_since(*at) >= TRACE_EVERY
            });
        if due {
            self.traced = Some((now, key));
        }
        due
    }
    /// Write a round summary when it carries news (see `trace_due`).
    fn trace(&mut self, id: &str, wait: u64, force: bool) {
        if !self.trace_due(std::time::Instant::now(), force) {
            return;
        }
        let (received, max_index, final_index, missing, confirmed, idle) = self.trace_key();
        crate::transfer_diagnostics::round(
            id,
            crate::transfer_diagnostics::RoundSummary {
                round: self.number,
                received,
                max_index,
                final_index,
                missing,
                confirmed,
                idle,
                active_wait_seconds: wait,
            },
        );
    }
    fn complete(&self, wanted: &[String]) -> bool {
        self.last_index.is_some_and(|last| {
            if self.seen.is_empty() || !(0..=last).all(|idx| self.seen.contains_key(&idx)) {
                return false;
            }
            // For the initial conversation metadata round (round 0, wanted empty),
            // ensure at least one metadata batch (batchType=2) was received.
            // Message rounds (number > 0 or non-empty wanted) receive batchType=1 (is_meta=false).
            if self.number == 0 && wanted.is_empty() {
                self.seen.values().any(|(is_meta, _, _)| *is_meta)
            } else {
                true
            }
        })
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
                    partition_tags: data
                        .get("scopes")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|scope| scope.get("partition").and_then(Value::as_str))
                        .take(8)
                        .map(crate::transfer_diagnostics::tag)
                        .collect(),
                },
            );
            match act {
                "transfer_status" => {
                    self.activity += 1;
                    match status {
                        1 => {
                            self.resumed |= self.idle;
                            self.idle = false;
                        }
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
                        // A re-delivered control may carry a re-signed URL; only a different
                        // batch kind for the same index is a real conflict.
                        ensure!(old.0 == metadata, "Conflicting duplicate phone batch");
                    } else {
                        self.seen.insert(index, (metadata, url.clone(), error));
                        self.activity += 1;
                        self.resumed = false;
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
fn request(id: &str, crypto: &Crypto, wanted: &[plan::Query], limit: u32) -> Value {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let queries = request_queries(wanted, now, limit);
    json!({"data":{"syncId":id,"syncType":0,"ek":B64.encode(&crypto.ephemeral.public),"ik":B64.encode(&crypto.identity.public),
        "toDevice":0,"tempKey":"","deviceName":"ctdozlite Windows ARM64","req":{"type":if wanted.is_empty(){"conversation"}else{"message"},
        "priority":if wanted.is_empty(){0}else{2},"batchSize":2000,"queries":queries},"ver":1,"ussidx":0},"reqId":format!("req_sync_{id}")})
}
/// Unlimited per-partition query, as Zalo sends for complete transfers.
const UNLIMITED: u32 = 2_147_483_647;
fn request_queries(wanted: &[plan::Query], now: u64, limit: u32) -> Vec<Value> {
    if wanted.is_empty() {
        vec![json!({"partition":"","from":0,"to":now,"limit":UNLIMITED})]
    } else {
        wanted
            .iter()
            .map(|p| json!({"partition":p.partition,"from":p.from,"to":p.to,"limit":limit}))
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
        timeouts: crate::server_config::SyncTimeoutConfig,
        transport: &SyncTransport,
        epoch: u64,
        cancel: Arc<AtomicBool>,
        crypto: &Crypto,
        wanted: &[plan::Query],
        progress: Arc<std::sync::Mutex<F>>,
        number: usize,
        limit: u32,
        state_sink: crate::sync_coordinator::SyncStateSink,
    ) -> Result<Self> {
        let id = sync_id()?;
        let packet = request(&id, crypto, wanted, limit);
        ensure!(
            serde_json::to_vec(&packet)?.len() <= policy::PAYLOAD_BUDGET,
            "Yêu cầu đồng bộ vượt giới hạn payload cục bộ"
        );
        journal(session, Some(&id), None).await?;
        let mut events = transport.subscribe();
        // Keep the journal if delivery is uncertain; the next explicit sync disposes it.
        if let Err(error) = transport.request(epoch, packet).await {
            if transport.dispose_reason(epoch, &id, 2).await.is_ok() {
                journal(session, None, Some(&id)).await?;
            }
            if transport.epoch() != Some(epoch) {
                return Err(Disconnected.into());
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
            state_sink.clone(),
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
            let mut confirmed_at: Option<std::time::Instant> = None;
            let mut idle_since: Option<std::time::Instant> = None;
            let mut active_wait = Duration::ZERO;
            let await_confirm = timeouts.awaiting_mobile_timeout();
            let wake_grace_dur = timeouts.wakeup_timeout();
            let mut previous = std::time::Instant::now();
            let mut tick = tokio::time::interval(Duration::from_millis(200));
            let mut last_display = std::time::Instant::now();
            let mut wake_seq = WAKE_SEQ.load(Ordering::Acquire);
            let mut last_wake: Option<std::time::Instant> = None;
            let mut auto_woken = false;
            let mut wake_grace: Option<std::time::Instant> = None;
            let mut idle_woken: Option<std::time::Instant> = None;
            let wake_reply = crate::socket::wake_request_id(&sync);
            loop {
                tokio::select! {
                    _=stopped.changed()=>anyhow::bail!("Đã dừng nhận lịch sử"),
                    _=tick.tick()=>{
                        ensure!(account.is_active() && !cancel.load(Ordering::Acquire),"Đã dừng đồng bộ điện thoại");
                        if transfer.epoch()!=Some(epoch) {return Err(Disconnected.into());}
                        let now=std::time::Instant::now();
                        if !state.idle {active_wait+=now.duration_since(previous);}
                        previous=now;
                        let engaged=state.confirmed || !state.seen.is_empty();
                        if engaged && confirmed_at.is_none() {confirmed_at=Some(now);}
                        match confirmed_at {
                            Some(at)=>ensure!(now.duration_since(at)<ROUND_CAP,"Lượt đồng bộ quá 2 giờ; đã dừng và giữ dữ liệu đã nhập"),
                            None=>ensure!(active_wait<await_confirm,"Điện thoại chưa xác nhận đồng bộ; mở Zalo trên điện thoại rồi thử lại"),
                        }
                        if idle_since.is_some_and(|since|now.duration_since(since)>=IDLE_LIMIT) {return Err(Stalled.into());}
                        if let Some(since)=idle_since.filter(|_|state.idle) {
                            match idle_action(now.duration_since(since),peer_served_since(number,since),idle_woken.map(|at|now.duration_since(at))) {
                                IdleAction::Wait=>{}
                                IdleAction::Wake=>{
                                    let written=transfer.wake(epoch,&sync).await.is_ok();
                                    crate::transfer_diagnostics::wake(&sync,true,written,None);
                                    idle_woken=Some(now);
                                    last_wake=Some(now);
                                    if let Ok(mut p)=progress.lock(){p(format!("Tải về: lượt {number} • điện thoại đang gửi lượt khác; đã đánh thức lượt này"));}
                                    last_display=now;
                                }
                                IdleAction::Retry=>return Err(Stalled.into()),
                            }
                        }
                        let seq=WAKE_SEQ.load(Ordering::Acquire);
                        let manual=seq!=wake_seq;
                        wake_seq=seq;
                        let automatic=engaged && !state.idle && !auto_woken && active_wait>=AUTO_WAKE_AFTER;
                        if (manual || automatic) && engaged && last_wake.is_none_or(|at|now.duration_since(at)>=WAKE_SPACING) {
                            auto_woken|=automatic;
                            let written=transfer.wake(epoch,&sync).await.is_ok();
                            crate::transfer_diagnostics::wake(&sync,automatic && !manual,written,None);
                            last_wake=Some(now);
                            if let Ok(mut p)=progress.lock(){p(format!("Tải về: lượt {number} • đã gửi lệnh đánh thức; giữ Zalo mở trên điện thoại"));}
                            last_display=now;
                        }
                        if engaged && active_wait>=STALL_AFTER && wake_grace.is_none_or(|until|now>=until) {return Err(Stalled.into());}
                        if last_display.elapsed()>=Duration::from_secs(1){state.trace(&sync,active_wait.as_secs(),false);if let Ok(mut p)=progress.lock(){p(state.progress(active_wait.as_secs()));}last_display=now;}
                    },
                    event=events.recv()=>{
                        let event=event.map_err(|error|match error {tokio::sync::broadcast::error::RecvError::Lagged(n)=>anyhow::anyhow!("Luồng control bỏ lỡ {n} sự kiện; không thể xác nhận lịch sử đầy đủ"),_=>anyhow::anyhow!("Luồng nhận lịch sử gián đoạn")})?;
                        if event.cmd==crate::socket::CMD_SYNC_WAKE {
                            let reply=event.payload.get("reqId").or_else(||event.payload.pointer("/data/reqId")).and_then(Value::as_str);
                            if reply==Some(wake_reply.as_str()) {
                                let code=event.payload.get("error_code").and_then(integer).unwrap_or(0);
                                crate::transfer_diagnostics::wake(&sync,false,true,Some(code));
                                if code==0 {wake_grace=Some(std::time::Instant::now()+wake_grace_dur);}
                            }
                            continue;
                        }
                        let before=state.activity;
                        let was_idle=state.idle;
                        let batches_before=state.seen.len();
                        let waiting=state.apply(&event,&sync)?;
                        // Only a newly received batch index proves the phone is serving; status
                        // controls also change `activity` but must not count as progress here.
                        if state.seen.len()>batches_before {idle_woken=None;note_peer_batch(number);}
                        if state.idle && !was_idle {idle_since=Some(std::time::Instant::now());}
                        if !state.idle {idle_since=None;}
                        if state.activity!=before{active_wait=Duration::ZERO;previous=std::time::Instant::now();}
                        if waiting{
                            if let Ok(mut p)=progress.lock(){p(format!("Tải về: lượt {number} • chờ xác nhận trên điện thoại…"));}
                            if let Some((_, cb)) = &state_sink {
                                cb(crate::sync_coordinator::SyncState::AwaitingPhoneConfirmation {
                                    timeout_at: std::time::Instant::now() + await_confirm,
                                });
                            }
                        }
                        let mut received_descriptor = false;
                        for batch in state.batches.drain(..){
                            received_descriptor = true;
                            batch_tx.try_send(batch).map_err(|_|anyhow::anyhow!("Hàng đợi gói đồng bộ vượt giới hạn"))?;
                        }
                        if received_descriptor {
                            if let Some((tracker, cb)) = &state_sink {
                                cb(tracker.record_descriptor_received());
                            }
                        }
                        if state.activity!=before {if let Ok(mut p)=progress.lock(){p(state.progress(0));}last_display=std::time::Instant::now();}
                        if state.complete(&[]){state.trace(&sync,0,true);return Ok(state.failed);}
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
            journal(session, None, Some(&self.id)).await?;
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
            let account = session.clone();
            tokio::task::spawn_blocking(move || -> Result<()> {
                let _write_permit = account.begin_account_write_blocking()?;
                let mut limits = policy::Limits::load(&path);
                limits.cooldown_until = (crate::login::unix_now() as u64).saturating_add(delay);
                crate::storage::atomic_write(&path, &serde_json::to_vec(&limits)?)
            })
            .await??;
        }
        received
    }
}
fn journal_path(session: &Session) -> std::path::PathBuf {
    session.history_path().with_extension("sync-session.json")
}
/// Read journaled session IDs; accepts the legacy single-ID and null formats.
fn read_journal(path: &std::path::Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_slice(&crate::limits::read_file(path, 16 * 1024)?)?;
    let ids: Vec<String> = match value {
        Value::Null => Vec::new(),
        Value::String(id) => vec![id],
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => anyhow::bail!("Invalid saved sync identity"),
    };
    for id in &ids {
        ensure!(
            id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Invalid saved sync identity"
        );
    }
    Ok(ids)
}
/// Add or remove one session ID; the journal never stores keys or URLs.
async fn journal(session: &Session, add: Option<&str>, remove: Option<&str>) -> Result<()> {
    let _guard = JOURNAL.lock().await;
    let path = journal_path(session);
    let account = session.clone();
    let (add, remove) = (add.map(str::to_owned), remove.map(str::to_owned));
    tokio::task::spawn_blocking(move || -> Result<()> {
        let _write_permit = account.begin_account_write_blocking()?;
        let mut ids = read_journal(&path).unwrap_or_default();
        if let Some(id) = remove {
            ids.retain(|known| *known != id);
        }
        if let Some(id) = add {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        // Oldest entries beyond the bound are abandoned; the phone times them out itself.
        if ids.len() > JOURNAL_LIMIT {
            ids.drain(..ids.len() - JOURNAL_LIMIT);
        }
        crate::storage::atomic_write(&path, &serde_json::to_vec(&ids)?)
    })
    .await?
}
async fn clean_old(session: &Session, transport: &SyncTransport, epoch: u64) -> Result<()> {
    let path = journal_path(session);
    let old = tokio::task::spawn_blocking(move || read_journal(&path)).await??;
    let mut failed = false;
    for id in old {
        if transport.dispose_reason(epoch, &id, 3).await.is_ok() {
            journal(session, None, Some(&id)).await?;
        } else {
            failed = true;
        }
    }
    ensure!(!failed, "Stale phone sessions could not be released yet");
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
/// Request phone approval once, emitting typed 5-milestone `SyncState` updates and holding
/// `pauseIdx` (`IndexingPauseGuard`) without ever auto-falling back to Legacy V1 on failure.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn download_with_state<
    F: FnMut(String) + Send + 'static,
    G: FnMut(Thread) + Send,
    S: Fn(crate::sync_coordinator::SyncState) + Send + Sync + 'static,
>(
    session: &Session,
    transport: SyncTransport,
    cancel: Arc<AtomicBool>,
    full_rescan: bool,
    recent_only: bool,
    range_days: u32,
    source: crate::sync_coordinator::SyncSource,
    origin: crate::sync_coordinator::SyncOrigin,
    tracker: crate::sync_coordinator::SyncProgressTracker,
    state_cb: S,
    mut progress: F,
    page: G,
) -> Result<crate::history_sync::SyncReport> {
    let timeouts = session.server_config.sync_2_pc.timeouts_for_origin(origin);
    let _owner = SYNC_OWNER.lock().await;
    // Pause heavy FTS backfill (`pauseIdx`) for the full lifetime of Sync2 (RAII ensures resume on return/cancel/error).
    let _indexing_pause = crate::sync_coordinator::pause_indexing();
    let state_cb: Arc<dyn Fn(crate::sync_coordinator::SyncState) + Send + Sync> =
        Arc::new(state_cb);
    state_cb(crate::sync_coordinator::SyncState::Preparing { source });
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
    state_cb(
        crate::sync_coordinator::SyncState::AwaitingPhoneConfirmation {
            timeout_at: std::time::Instant::now() + timeouts.awaiting_mobile_timeout(),
        },
    );
    clean_old(session, &transport, epoch).await?;
    let progress = Arc::new(std::sync::Mutex::new(progress));
    let mut run = RunningRound::open(
        session,
        timeouts,
        &transport,
        epoch,
        cancel.clone(),
        &crypto,
        &[],
        progress.clone(),
        0,
        UNLIMITED,
        Some((tracker.clone(), state_cb.clone())),
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
            let (store, _write_permit) =
                crate::store::MessageStore::open_for_account_write(&account)?;
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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let floor = range_floor(range_days, now);
    let limit_path = session.history_path().with_extension("sync-limits.json");
    let read_path = limit_path.clone();
    let mut limits = tokio::task::spawn_blocking(move || policy::Limits::load(&read_path)).await?;
    let per_round = limits.count.unwrap_or(30).min(30);
    let deferred_path = session.history_path().with_extension("sync-deferred.json");
    let read_deferred = deferred_path.clone();
    let mut deferred = tokio::task::spawn_blocking(move || {
        if full_rescan {
            plan::Deferred::default()
        } else {
            plan::Deferred::load(&read_deferred)
        }
    })
    .await?;
    deferred.prune(&owners, now);
    let planned = plan::Update {
        coverage: &coverage,
        owners: &owners,
        deferred: &deferred,
        now,
        floor,
        full_rescan,
    }
    .plan(&ordered, recent_only);
    let unmapped_checks = planned
        .queries
        .iter()
        .filter(|q| !owners.contains_key(&q.partition))
        .count();
    emit_plan(&progress, &planned, unmapped_checks);
    let plan_summary = plan_line(&planned, unmapped_checks);
    crate::transfer_diagnostics::phone_plan(
        planned.queries.len(),
        planned.unchanged,
        planned.deferred,
        unmapped_checks,
    );
    let activity: HashMap<String, i64> = ordered
        .iter()
        .map(|c| (c.partition.clone(), c.activity))
        .collect();
    let jobs = planned.queries;
    // Most recent conversations without durable coverage get a short page first, like
    // Zalo PC's recent-chat phase; the full rounds later fetch them completely.
    let priority: Vec<_> = if recent_only {
        Vec::new()
    } else {
        jobs.iter()
            .take(PRIORITY_CONVERSATIONS)
            .filter(|q| !coverage.contains_key(&q.partition))
            .cloned()
            .collect()
    };
    let mut raw_queue = std::collections::VecDeque::new();
    let mut number = 0;
    if !priority.is_empty() {
        number += 1;
        raw_queue.push_back(Job {
            wanted: priority,
            number,
            attempt: 0,
            priority: true,
            user_prioritized: false,
        });
    }
    for wanted in policy::split(jobs, Some(per_round))? {
        number += 1;
        raw_queue.push_back(Job {
            wanted,
            number,
            attempt: 0,
            priority: false,
            user_prioritized: false,
        });
    }
    let transfer_queue = Arc::new(std::sync::Mutex::new(TransferQueue::new(
        raw_queue, &owners, number,
    )));
    let _priority_registration = ActivePriorityRegistration::register(
        session.account_key().to_owned(),
        session.runtime_id(),
        transfer_queue.clone(),
    );
    let identity = (
        crypto.identity.public.clone(),
        crypto.identity.private.clone(),
    );
    drop(crypto);
    let writer = super::pipeline::Writer::start(session.clone(), threads.clone());
    let shared = std::sync::Mutex::new(Shared {
        senders: Arc::new(senders),
        attempted: HashSet::new(),
        read: 0,
        inserted: 0,
        existing: 0,
        deleted: 0,
        unmapped: 0,
    });
    let page = std::sync::Mutex::new(page);
    let board = Arc::new(std::sync::Mutex::new(Board {
        sink: progress.clone(),
        lines: Default::default(),
        done: 0,
        total: number,
    }));
    let ctx = RoundCtx {
        session,
        timeouts,
        transport: &transport,
        cancel: &cancel,
        codec_bytes: &codec_bytes,
        identity: &identity,
        writer: &writer,
        shared: &shared,
        page: &page,
        board: &board,
        next_start: std::sync::Mutex::new(tokio::time::Instant::now()),
        state_sink: Some((tracker.clone(), state_cb.clone())),
    };
    let emit = |text: String| {
        if let Ok(mut p) = progress.lock() {
            p(text);
        }
    };
    let mut epoch = epoch;
    let mut running = futures_util::stream::FuturesUnordered::new();
    let mut reconnects = 0u8;
    let mut consecutive = 0usize;
    let mut failures = 0usize;
    let mut skipped = 0usize;
    let mut adjustments = 0usize;
    let mut disconnected = false;
    let mut fatal: Option<anyhow::Error> = None;
    let parallel_rounds = session
        .server_config
        .sync_2_pc
        .max_concurrent_sync_items
        .max(1);
    loop {
        while fatal.is_none()
            && !disconnected
            && running.len() < parallel_rounds
            && !transfer_queue
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
            && session.is_active()
            && !cancel.load(Ordering::Acquire)
        {
            if transport.epoch() != Some(epoch) {
                disconnected = true;
                break;
            }
            let next_job = transfer_queue
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop_next();
            if let Some(job) = next_job {
                running.push(run_round(&ctx, job, epoch));
            }
        }
        let Some((job, result)) = futures_util::StreamExt::next(&mut running).await else {
            if fatal.is_some() || !session.is_active() || cancel.load(Ordering::Acquire) {
                break;
            }
            if disconnected {
                if reconnects >= MAX_RECONNECTS {
                    fatal = Some(Disconnected.into());
                    break;
                }
                reconnects += 1;
                emit("Tải về: mất kết nối; đang chờ kết nối lại để tiếp tục…".into());
                epoch = match wait_for_epoch(&transport, epoch, session, &cancel).await {
                    Ok(next) => next,
                    Err(error) => {
                        fatal = Some(error);
                        break;
                    }
                };
                if clean_old(session, &transport, epoch).await.is_err() {
                    crate::logger::log_error("phone cleanup", "Deferred stale-session cleanup");
                }
                crate::transfer_diagnostics::resume(reconnects, epoch);
                emit(
                    "Tải về: đã kết nối lại; tiếp tục đồng bộ (điện thoại có thể hỏi xác nhận lại)"
                        .into(),
                );
                disconnected = false;
                continue;
            }
            break;
        };
        let partitions = job.wanted.len();
        let outcome = classify(&result, &transport, epoch);
        crate::transfer_diagnostics::round_result(job.number, job.attempt, partitions, outcome);
        if let Ok(mut b) = board.lock() {
            b.remove(
                job.number,
                outcome == crate::transfer_diagnostics::RoundOutcome::Complete,
            );
        }
        match (outcome, result) {
            (crate::transfer_diagnostics::RoundOutcome::Complete, Ok(failed)) => {
                consecutive = 0;
                failures += failed.len();
                let mut postponed = false;
                for query in job.wanted.iter().filter(|q| {
                    !job.priority
                        && !failed.contains(&q.partition)
                        && !owners.contains_key(&q.partition)
                }) {
                    let last = activity.get(&query.partition).copied().unwrap_or(0);
                    deferred.postpone(&query.partition, last, now);
                    postponed = true;
                }
                if postponed {
                    let snapshot = deferred.clone();
                    let path = deferred_path.clone();
                    let account = session.clone();
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        ensure!(account.is_active(), "Đã đăng xuất");
                        let _write_permit = account.begin_account_write_blocking()?;
                        snapshot.save(&path)
                    })
                    .await??;
                }
                transfer_queue
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .finish_job(&job.wanted);
                if !job.priority {
                    let mut completed: Vec<_> = job
                        .wanted
                        .into_iter()
                        .filter(|q| {
                            !failed.contains(&q.partition) && owners.contains_key(&q.partition)
                        })
                        .collect();
                    plan::checkpoint_completed(&mut coverage, &mut completed, &owners)?;
                    let ranges: Vec<_> = coverage
                        .iter()
                        .map(|(p, r)| (p.clone(), r.clone()))
                        .collect();
                    let account = session.clone();
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        ensure!(account.is_active(), "Đã đăng xuất");
                        let (store, _write_permit) =
                            crate::store::MessageStore::open_for_account_write(&account)?;
                        store.save_phone_sync_ranges(&ranges)
                    })
                    .await??;
                    state_cb(tracker.record_partition_complete());
                }
                let inserted = shared.lock().map(|s| s.inserted).unwrap_or(0);
                emit(format!(
                    "Nhập CSDL: {inserted} tin mới • hoàn tất lượt {}",
                    job.number
                ));
            }
            (crate::transfer_diagnostics::RoundOutcome::Disconnected, _) => {
                disconnected = true;
                transfer_queue
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push_front_requeue(job);
            }
            (crate::transfer_diagnostics::RoundOutcome::Stalled, _) => {
                if job.attempt == 0 {
                    emit(format!(
                        "Tải về: lượt {} chưa đầy đủ; sẽ xin lại một lần",
                        job.number
                    ));
                    transfer_queue
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push_back_retry(Job { attempt: 1, ..job });
                } else {
                    transfer_queue
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .finish_job(&job.wanted);
                    consecutive += 1;
                    skipped += 1;
                    failures += partitions;
                    crate::transfer_diagnostics::round_result(
                        job.number,
                        job.attempt,
                        partitions,
                        crate::transfer_diagnostics::RoundOutcome::Skipped,
                    );
                    if consecutive >= MAX_CONSECUTIVE_FAILURES {
                        fatal = Some(anyhow::anyhow!(
                            "Điện thoại không hoàn tất {MAX_CONSECUTIVE_FAILURES} lượt liên tiếp; đã dừng và giữ dữ liệu đã nhập. Mở Zalo trên điện thoại rồi đồng bộ lại để lấy bù."
                        ));
                    }
                }
            }
            (_, Err(error)) => {
                if let Some(limit) = error.downcast_ref::<policy::LimitRejection>() {
                    if adjustments < 4 && partitions > 1 {
                        adjustments += 1;
                        let maximum = limit
                            .0
                            .unwrap_or(partitions.div_ceil(2))
                            .min(partitions - 1)
                            .max(1);
                        limits.count = Some(maximum);
                        let user_prioritized = job.user_prioritized;
                        let priority = job.priority;
                        let attempt = job.attempt;
                        let job_number = job.number;
                        let mut pieces = policy::split(job.wanted, Some(maximum))?.into_iter();
                        let mut replanned = Vec::new();
                        if let Some(first) = pieces.next() {
                            replanned.push(Job {
                                wanted: first,
                                number: job_number,
                                attempt,
                                priority,
                                user_prioritized,
                            });
                        }
                        for wanted in pieces {
                            let next_num = transfer_queue
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .allocate_number();
                            replanned.push(Job {
                                wanted,
                                number: next_num,
                                attempt,
                                priority,
                                user_prioritized,
                            });
                        }
                        if let Ok(mut b) = board.lock() {
                            b.total = b
                                .total
                                .max(replanned.iter().map(|j| j.number).max().unwrap_or(b.total));
                        }
                        {
                            let mut q = transfer_queue.lock().unwrap_or_else(|e| e.into_inner());
                            for j in replanned.into_iter().rev() {
                                q.push_front_requeue(j);
                            }
                        }
                        let path = limit_path.clone();
                        let data = serde_json::to_vec(&limits)?;
                        let account = session.clone();
                        tokio::task::spawn_blocking(move || -> Result<()> {
                            let _write_permit = account.begin_account_write_blocking()?;
                            crate::storage::atomic_write(&path, &data)
                        })
                        .await??;
                        continue;
                    }
                }
                transfer_queue
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .finish_job(&job.wanted);
                fatal = Some(error);
            }
            (_, Ok(_)) => {
                transfer_queue
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .finish_job(&job.wanted);
            }
        }
    }
    drop(running);
    if let Some(error) = fatal {
        return Err(error);
    }
    ensure!(
        session.is_active() && !cancel.load(Ordering::Acquire),
        "Đã dừng đồng bộ điện thoại; giữ dữ liệu đã lưu"
    );
    diagnostic.finish();
    let (read, inserted, existing, deleted, unmapped) = shared
        .lock()
        .map(|s| (s.read, s.inserted, s.existing, s.deleted, s.unmapped))
        .unwrap_or_default();
    crate::transfer_diagnostics::phone_totals(crate::transfer_diagnostics::PhoneTotals {
        read,
        inserted,
        existing,
        deleted,
        unmapped,
        skipped_rounds: skipped,
        failed_conversations: failures,
    });
    let tail = if skipped > 0 || failures > 0 {
        format!(" {skipped} lượt chưa hoàn tất, {failures} hội thoại cần đồng bộ lại để lấy bù.")
    } else {
        String::new()
    };
    let deleted_note = if deleted > 0 {
        format!(", {deleted} tin bạn đã xóa trên máy này được bỏ qua")
    } else {
        String::new()
    };
    Ok(crate::history_sync::SyncReport { complete: skipped == 0 && failures == 0 && unmapped == 0, message: format!("Đã hoàn tất tải và nhập lịch sử: đọc {read} tin, {inserted} tin mới, {existing} tin đã có{deleted_note}; {unmapped} tin chưa xác định hội thoại được giữ riêng.{tail} {plan_summary}.") })
}

/// Vietnamese summary of what an update run will check.
fn plan_line(planned: &plan::Planned, unmapped_checks: usize) -> String {
    let mut line = format!("Kế hoạch: {} hội thoại cần kiểm tra", planned.queries.len());
    if planned.unchanged > 0 {
        line.push_str(&format!(" • {} hội thoại không đổi", planned.unchanged));
    }
    if planned.uncheckpointed > 0 {
        line.push_str(&format!(
            " • {} hội thoại chưa có checkpoint hoàn tất, cần kiểm tra lịch sử",
            planned.uncheckpointed
        ));
    }
    if unmapped_checks > 0 {
        line.push_str(&format!(" • {unmapped_checks} chưa nhận diện (thử lại)"));
    }
    if planned.deferred > 0 {
        line.push_str(&format!(
            " • {} chưa nhận diện đang tạm hoãn (Quét lại toàn bộ để xin lại)",
            planned.deferred
        ));
    }
    line
}
fn emit_plan<F: FnMut(String)>(
    progress: &std::sync::Mutex<F>,
    planned: &plan::Planned,
    unmapped_checks: usize,
) {
    if let Ok(mut p) = progress.lock() {
        p(plan_line(planned, unmapped_checks));
    }
}

/// One message round to run, retried at most once when the phone stalls.
struct Job {
    wanted: Vec<plan::Query>,
    number: usize,
    attempt: u8,
    priority: bool,
    user_prioritized: bool,
}

/// Import state shared by parallel rounds; locks are never held across an await.
struct Shared {
    senders: Arc<HashMap<String, String>>,
    attempted: HashSet<String>,
    read: usize,
    inserted: usize,
    existing: usize,
    deleted: usize,
    unmapped: usize,
}

/// Combines per-round progress into one line so parallel rounds do not flicker.
struct Board<F> {
    sink: Arc<std::sync::Mutex<F>>,
    lines: std::collections::BTreeMap<usize, String>,
    done: usize,
    total: usize,
}
impl<F: FnMut(String)> Board<F> {
    fn update(&mut self, number: usize, text: String) {
        let Some(detail) = text.strip_prefix("Tải về:") else {
            if let Ok(mut sink) = self.sink.lock() {
                sink(text);
            }
            return;
        };
        let detail = detail.trim();
        let detail = detail
            .strip_prefix(&format!("lượt {number} • "))
            .unwrap_or(detail)
            .to_owned();
        self.lines.insert(number, detail);
        self.emit();
    }
    fn remove(&mut self, number: usize, done: bool) {
        self.lines.remove(&number);
        self.done += usize::from(done);
        self.emit();
    }
    fn emit(&self) {
        let current = self
            .lines
            .iter()
            .next()
            .map(|(n, l)| format!(" • lượt {n}: {l}"))
            .unwrap_or_default();
        if let Ok(mut sink) = self.sink.lock() {
            sink(format!(
                "Tải về: xong {}/{} lượt • {} lượt đang chạy{current}",
                self.done,
                self.total,
                self.lines.len()
            ));
        }
    }
}

struct RoundCtx<'a, F, G> {
    session: &'a Session,
    timeouts: crate::server_config::SyncTimeoutConfig,
    transport: &'a SyncTransport,
    cancel: &'a Arc<AtomicBool>,
    codec_bytes: &'a [u8],
    identity: &'a (Vec<u8>, Zeroizing<Vec<u8>>),
    writer: &'a super::pipeline::Writer,
    shared: &'a std::sync::Mutex<Shared>,
    page: &'a std::sync::Mutex<G>,
    board: &'a Arc<std::sync::Mutex<Board<F>>>,
    next_start: std::sync::Mutex<tokio::time::Instant>,
    state_sink: crate::sync_coordinator::SyncStateSink,
}

fn classify(
    result: &Result<HashSet<String>>,
    transport: &SyncTransport,
    epoch: u64,
) -> crate::transfer_diagnostics::RoundOutcome {
    use crate::transfer_diagnostics::RoundOutcome;
    match result {
        Ok(_) => RoundOutcome::Complete,
        Err(error) if error.is::<Disconnected>() || transport.epoch() != Some(epoch) => {
            RoundOutcome::Disconnected
        }
        Err(error) if error.is::<Stalled>() => RoundOutcome::Stalled,
        Err(error) if error.is::<policy::LimitRejection>() => RoundOutcome::Rejected,
        Err(_) => RoundOutcome::Failed,
    }
}

/// Floor of the requested history window; `0` days means the whole history.
fn range_floor(range_days: u32, now: i64) -> i64 {
    if range_days == 0 {
        0
    } else {
        now.saturating_sub(i64::from(range_days) * 86_400_000)
            .max(0)
    }
}

/// Wait for a socket generation newer than `old`.
async fn wait_for_epoch(
    transport: &SyncTransport,
    old: u64,
    session: &Session,
    cancel: &AtomicBool,
) -> Result<u64> {
    let mut epochs = transport.epochs();
    let deadline = tokio::time::Instant::now() + RECONNECT_WAIT;
    loop {
        if let Some(epoch) = (*epochs.borrow_and_update()).filter(|e| *e != old) {
            return Ok(epoch);
        }
        ensure!(
            session.is_active() && !cancel.load(Ordering::Acquire),
            "Đã dừng đồng bộ điện thoại; giữ dữ liệu đã lưu"
        );
        match tokio::time::timeout_at(deadline, epochs.changed()).await {
            Ok(Ok(())) => {}
            _ => return Err(Disconnected.into()),
        }
    }
}

fn round_progress<F: FnMut(String) + Send + 'static>(
    board: Arc<std::sync::Mutex<Board<F>>>,
    number: usize,
) -> impl FnMut(String) + Send + 'static {
    move |text: String| {
        if let Ok(mut b) = board.lock() {
            b.update(number, text);
        }
    }
}

/// Reserve one launch slot under a short lock; sleep happens after releasing it.
fn reserve_round_start(
    next: &std::sync::Mutex<tokio::time::Instant>,
    now: tokio::time::Instant,
) -> tokio::time::Instant {
    let mut next = next.lock().unwrap_or_else(|e| e.into_inner());
    let at = (*next).max(now);
    *next = at + ROUND_START_GAP;
    at
}

async fn run_round<F, G>(
    ctx: &RoundCtx<'_, F, G>,
    job: Job,
    epoch: u64,
) -> (Job, Result<HashSet<String>>)
where
    F: FnMut(String) + Send + 'static,
    G: FnMut(Thread) + Send,
{
    let result = async {
        let bytes = ctx.codec_bytes.to_vec();
        let public = ctx.identity.0.clone();
        let private = ctx.identity.1.clone();
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
        let progress = Arc::new(std::sync::Mutex::new(round_progress(ctx.board.clone(), job.number)));
        let limit = if job.priority { PRIORITY_MESSAGES } else { UNLIMITED };
        // FuturesUnordered is lazy: pacing insertion does not pace actual requests.
        let start = reserve_round_start(&ctx.next_start, tokio::time::Instant::now());
        tokio::time::sleep_until(start).await;
        ensure!(ctx.session.is_active() && !ctx.cancel.load(Ordering::Acquire), "Đã dừng đồng bộ điện thoại");
        let mut run = RunningRound::open(
            ctx.session,
            ctx.timeouts,
            ctx.transport,
            epoch,
            ctx.cancel.clone(),
            &crypto,
            &job.wanted,
            progress,
            job.number,
            limit,
            ctx.state_sink.clone(),
        )
        .await?;
        let emit = |text: String| {
            if let Ok(mut b) = ctx.board.lock() {
                b.update(job.number, text);
            }
        };
        let batch_timeout = ctx.timeouts.batch_timeout();
        let consumed = async {
            let mut waiting = std::collections::VecDeque::new();
            loop {
                let staged = match tokio::time::timeout(batch_timeout, run.files.recv()).await {
                    Ok(Some(staged)) => staged,
                    Ok(None) => break,
                    Err(_) => {
                        anyhow::bail!(
                            "Quá thời gian chờ nhận batch file đồng bộ ({:?})",
                            batch_timeout
                        );
                    }
                };
                let staged = staged?;
                if !staged.batch.metadata && crypto.record.is_none() {
                    ensure!(waiting.len() < 8, "Chưa nhận khóa metadata; đã đạt giới hạn chờ");
                    waiting.push_back(staged);
                    continue;
                }
                waiting.push_front(staged);
                while let Some(staged) = waiting.pop_front() {
                    let batch_index = staged.batch.index;
                    let (next, objects) = decoded_file(
                        crypto,
                        staged,
                        false,
                        ctx.cancel.clone(),
                        ctx.session.clone(),
                        run.id.clone(),
                        job.number,
                    )
                    .await?;
                    if let Some((tracker, cb)) = &ctx.state_sink {
                        cb(tracker.record_decoded());
                    }
                    crypto = next;
                    if crypto.record.is_some() {
                        let _ = run.decoder_ready.send(true);
                    }
                    for object in objects {
                        ensure!(
                            ctx.session.is_active() && !ctx.cancel.load(Ordering::Acquire),
                            "Đã dừng nhập lịch sử; dữ liệu đã lưu được giữ"
                        );
                        let unknown: Vec<String> = {
                            let shared = ctx.shared.lock().unwrap_or_else(|e| e.into_inner());
                            object
                                .get("partitions")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                                .flat_map(|p| p.get("messages").and_then(Value::as_array).into_iter().flatten())
                                .map(|m| import::number(m, "senderId"))
                                .filter(|id| {
                                    !id.is_empty()
                                        && !shared.senders.contains_key(id)
                                        && !shared.attempted.contains(id)
                                })
                                .collect::<HashSet<_>>()
                                .into_iter()
                                .collect()
                        };
                        if !unknown.is_empty() {
                            let mut trace = crate::transfer_diagnostics::BatchWork::start(&run.id, job.number, batch_index, crate::transfer_diagnostics::BatchStage::ResolveSenders);
                            for ids in unknown.chunks(50) {
                                // A transport failure is retried by a later batch instead of
                                // permanently leaving the sender unresolved for this run.
                                if let Ok(resolved) = crate::chat::resolve_sync_ids(ctx.session, ids).await {
                                    let mut shared = ctx.shared.lock().unwrap_or_else(|e| e.into_inner());
                                    shared.attempted.extend(ids.iter().cloned());
                                    Arc::make_mut(&mut shared.senders).extend(resolved);
                                }
                            }
                            trace.finish(unknown.len());
                        }
                        let senders = ctx.shared.lock().unwrap_or_else(|e| e.into_inner()).senders.clone();
                        let mut trace = crate::transfer_diagnostics::BatchWork::start(&run.id, job.number, batch_index, crate::transfer_diagnostics::BatchStage::Import);
                        let imported = ctx.writer.save(object, senders).await?;
                        trace.finish(imported.inserted);
                        if let Some((tracker, cb)) = &ctx.state_sink {
                            cb(tracker.record_db_committed(imported.inserted as u64));
                        }
                        let inserted = {
                            let mut shared = ctx.shared.lock().unwrap_or_else(|e| e.into_inner());
                            shared.read += imported.read;
                            shared.inserted += imported.inserted;
                            shared.existing += imported.existing;
                            shared.deleted += imported.deleted;
                            shared.unmapped += imported.unmapped;
                            shared.inserted
                        };
                        for preview in imported.previews {
                            if let Ok(mut page) = ctx.page.lock() {
                                page(preview);
                            }
                            tokio::task::yield_now().await;
                        }
                        emit(format!("Nhập CSDL: {inserted} tin mới • đã nhập gói {batch_index} • chờ gói tiếp theo"));
                    }
                }
            }
            ensure!(waiting.is_empty(), "Không nhận được metadata giải mã");
            Ok::<_, anyhow::Error>(())
        }
        .await;
        let received = run
            .dispose(ctx.session, ctx.transport, epoch, consumed.is_ok(), ctx.cancel)
            .await;
        consumed?;
        received
    }
    .await;
    (job, result)
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
        let values = request_queries(&wanted, 999, UNLIMITED);
        assert_eq!(
            values[0],
            json!({"partition":"group/recent", "from":100, "to":500, "limit":2147483647})
        );
        assert_eq!(values[1]["partition"], "oneone/older");
        assert_eq!(values[1]["to"], 250);
        assert_eq!(
            request_queries(&[], 999, UNLIMITED)[0],
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
/// Dispose journaled phone sessions on every new socket generation.
///
/// A running sync owns its sessions and disposes or resumes them itself; cleanup waits until it
/// releases ownership, so a session interrupted by a disconnect is still released afterwards.
pub(crate) async fn clean_after_connect(session: Session, transport: SyncTransport) {
    let mut epochs = transport.epochs();
    let mut cleaned: Option<u64> = None;
    loop {
        if !session.is_active() {
            return;
        }
        let current = *epochs.borrow_and_update();
        let mut pending = false;
        if let Some(epoch) = current.filter(|e| cleaned != Some(*e)) {
            match SYNC_OWNER.try_lock() {
                Ok(_owner) => {
                    if clean_old(&session, &transport, epoch).await.is_err() {
                        crate::logger::log_error("phone cleanup", "Deferred stale-session cleanup");
                    }
                    cleaned = Some(epoch);
                }
                Err(_) => pending = true,
            }
        }
        if pending {
            tokio::select! {
                changed = epochs.changed() => if changed.is_err() { return; },
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            }
        } else if epochs.changed().await.is_err() {
            return;
        }
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
        queues.register(key, "initial".into(), now, None);
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

#[cfg(test)]
mod resilience_tests {
    #[tokio::test]
    async fn lazily_polled_rounds_reserve_separate_launch_slots() {
        use futures_util::{stream::FuturesUnordered, StreamExt};
        let now = tokio::time::Instant::now();
        let next = std::sync::Mutex::new(now);
        let mut pending = FuturesUnordered::new();
        for _ in 0..PARALLEL_ROUNDS {
            let next = &next;
            pending.push(async move { reserve_round_start(next, now) });
        }
        let mut starts = Vec::new();
        while let Some(at) = pending.next().await {
            starts.push(at);
        }
        starts.sort();
        for pair in starts.windows(2) {
            assert_eq!(pair[1] - pair[0], ROUND_START_GAP);
        }
        // A new wave after an idle period starts immediately, without replaying old slots.
        let later = now + Duration::from_secs(10);
        assert_eq!(reserve_round_start(&next, later), later);
    }

    use super::*;

    fn control(data: Value) -> WsEvent {
        WsEvent {
            cmd: 601,
            sub_cmd: 0,
            version: 1,
            payload: json!({"controls":[{"content":{"act_type":"transfer_sync2","act":data["act"].clone(),"data":data["data"].clone()}}]}),
        }
    }
    fn batch(idx: u64, url: &str, metadata: bool) -> WsEvent {
        control(
            json!({"act":"upload_batch","data":{"syncId":"s","idx":idx,"batchType":if metadata{2}else{1},"msgUrl":url,"scopes":[{"partition":"oneone/a"}]}}),
        )
    }
    fn status(code: u64) -> WsEvent {
        control(json!({"act":"transfer_status","data":{"syncId":"s","status":code}}))
    }

    #[test]
    fn resigned_duplicate_is_ignored_but_changed_batch_kind_is_rejected() {
        let mut round = Round::default();
        round
            .apply(&batch(1, "https://a.zdn.vn/one", false), "s")
            .unwrap();
        round
            .apply(&batch(1, "https://a.zdn.vn/one-resigned", false), "s")
            .unwrap();
        assert_eq!(round.seen.len(), 1);
        assert_eq!(round.batches.len(), 1);
        assert!(round
            .apply(&batch(1, "https://a.zdn.vn/one", true), "s")
            .is_err());
    }

    #[test]
    fn only_new_batch_indices_count_as_progress() {
        let mut round = Round::default();
        round.apply(&status(5), "s").unwrap();
        round.apply(&status(2), "s").unwrap();
        assert!(round.seen.is_empty(), "status controls are not batches");
        round
            .apply(&batch(0, "https://a.zdn.vn/m", false), "s")
            .unwrap();
        round
            .apply(&batch(0, "https://a.zdn.vn/m", false), "s")
            .unwrap();
        assert_eq!(round.seen.len(), 1, "a repeated index is not new");
    }

    #[test]
    fn idle_round_is_woken_once_while_peers_are_served_then_retried() {
        let s = Duration::from_secs;
        assert_eq!(idle_action(s(120), false, None), IdleAction::Wait);
        assert_eq!(idle_action(s(59), true, None), IdleAction::Wait);
        assert_eq!(idle_action(s(60), true, None), IdleAction::Wake);
        assert_eq!(idle_action(s(90), true, Some(s(30))), IdleAction::Wait);
        assert_eq!(idle_action(s(120), true, Some(s(60))), IdleAction::Retry);
        assert_eq!(idle_action(s(120), false, Some(s(60))), IdleAction::Retry);
    }

    #[test]
    fn peer_activity_ignores_the_round_itself_and_older_batches() {
        let before = std::time::Instant::now();
        note_peer_batch(7);
        assert!(!peer_served_since(7, before));
        assert!(peer_served_since(8, before));
        assert!(!peer_served_since(
            8,
            std::time::Instant::now() + Duration::from_secs(1)
        ));
    }

    #[test]
    fn round_summaries_are_written_on_change_or_every_ten_seconds() {
        let mut round = Round::default();
        let start = std::time::Instant::now();
        assert!(round.trace_due(start, false));
        assert!(!round.trace_due(start + Duration::from_secs(1), false));
        assert!(!round.trace_due(start + Duration::from_secs(9), false));
        assert!(round.trace_due(start + TRACE_EVERY, false));
        round.apply(&status(5), "s").unwrap();
        assert!(round.trace_due(start + TRACE_EVERY + Duration::from_secs(1), false));
        assert!(round.trace_due(start + TRACE_EVERY + Duration::from_secs(1), true));
    }

    #[test]
    fn progress_hints_stall_idle_and_resume_when_phone_state_changes() {
        let mut round = Round {
            number: 3,
            ..Default::default()
        };
        round.apply(&status(5), "s").unwrap();
        round
            .apply(&batch(0, "https://a.zdn.vn/m", true), "s")
            .unwrap();
        assert!(!round.progress(5).contains("khóa màn hình"));
        assert!(round
            .progress(STALL_HINT.as_secs())
            .contains("khóa màn hình"));
        round.apply(&status(2), "s").unwrap();
        assert!(round.progress(0).contains("tạm nghỉ"));
        round.apply(&status(1), "s").unwrap();
        assert!(round.progress(0).contains("hoạt động lại"));
        round
            .apply(&batch(1, "https://a.zdn.vn/n", false), "s")
            .unwrap();
        assert!(!round.progress(0).contains("hoạt động lại"));
    }

    #[test]
    fn journal_reads_legacy_formats_and_rejects_unsafe_ids() {
        let dir = std::env::temp_dir().join(format!(
            "ctdoz-journal-{}",
            crate::param_cipher::random_hex(8)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("j.json");
        assert!(read_journal(&path).unwrap().is_empty());
        std::fs::write(&path, b"null").unwrap();
        assert!(read_journal(&path).unwrap().is_empty());
        std::fs::write(&path, br#""abc-1""#).unwrap();
        assert_eq!(read_journal(&path).unwrap(), vec!["abc-1".to_string()]);
        std::fs::write(&path, br#"["a-1","b-2"]"#).unwrap();
        assert_eq!(read_journal(&path).unwrap().len(), 2);
        std::fs::write(&path, br#"["../x"]"#).unwrap();
        assert!(read_journal(&path).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn range_floor_is_zero_for_whole_history_and_days_back_otherwise() {
        let now = 1_800_000_000_000;
        assert_eq!(range_floor(0, now), 0);
        assert_eq!(range_floor(30, now), now - 30 * 86_400_000);
        assert_eq!(range_floor(u32::MAX, 1000), 0);
    }

    #[test]
    fn board_combines_parallel_round_lines_and_passes_import_lines_through() {
        let lines = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink_lines = lines.clone();
        let sink = Arc::new(std::sync::Mutex::new(move |text: String| {
            sink_lines.lock().unwrap().push(text)
        }));
        let mut board = Board {
            sink,
            lines: Default::default(),
            done: 0,
            total: 5,
        };
        board.update(2, "Tải về: lượt 2 • 10 mô tả gói".into());
        board.update(3, "Tải về: lượt 3 • chờ".into());
        board.update(2, "Nhập CSDL: 5 tin mới".into());
        board.remove(2, true);
        let out = lines.lock().unwrap().clone();
        assert_eq!(
            out[0],
            "Tải về: xong 0/5 lượt • 1 lượt đang chạy • lượt 2: 10 mô tả gói"
        );
        assert_eq!(
            out[1],
            "Tải về: xong 0/5 lượt • 2 lượt đang chạy • lượt 2: 10 mô tả gói"
        );
        assert_eq!(out[2], "Nhập CSDL: 5 tin mới");
        assert_eq!(
            out[3],
            "Tải về: xong 1/5 lượt • 1 lượt đang chạy • lượt 3: chờ"
        );
    }

    #[test]
    fn priority_requests_carry_short_page_limit() {
        let wanted = vec![plan::Query {
            partition: "oneone/a".into(),
            from: 0,
            to: 10,
        }];
        assert_eq!(
            request_queries(&wanted, 99, PRIORITY_MESSAGES)[0]["limit"],
            json!(100)
        );
        assert_eq!(
            request_queries(&wanted, 99, UNLIMITED)[0]["limit"],
            json!(2_147_483_647)
        );
    }

    #[test]
    fn test_round_complete_metadata_and_message_rounds() {
        // Round 0 (metadata round, wanted is empty)
        let mut r0 = Round::new(0);
        r0.last_index = Some(1);
        // Chunks without is_meta=true should NOT complete r0
        r0.seen.insert(0, (false, "url0".to_string(), 10));
        r0.seen.insert(1, (false, "url1".to_string(), 10));
        assert!(!r0.complete(&[]));

        // When a metadata chunk is received, r0 completes
        r0.seen.insert(0, (true, "url0".to_string(), 10));
        assert!(r0.complete(&[]));

        // Round 1 (message round, wanted is not empty)
        let mut r1 = Round::new(1);
        r1.last_index = Some(1);
        r1.seen.insert(0, (false, "url0".to_string(), 20));
        r1.seen.insert(1, (false, "url1".to_string(), 20));
        let wanted = vec!["oneone/user1".to_string()];
        // Should complete successfully even though all chunks have is_meta == false
        assert!(r1.complete(&wanted));

        // If a chunk is missing in r1, it must not complete
        r1.seen.remove(&1);
        assert!(!r1.complete(&wanted));
    }

    #[test]
    fn priority_queue_moves_open_thread_ranges_to_front_with_dedup_and_anti_starvation() {
        let mut owners = HashMap::new();
        owners.insert("oneone/bg1".into(), "t-bg1".into());
        owners.insert("oneone/bg2".into(), "t-bg2".into());
        owners.insert("oneone/p1".into(), "t-p1".into());
        owners.insert("oneone/p2".into(), "t-p2".into());
        owners.insert("oneone/p3".into(), "t-p3".into());

        let q = |partition: &str| plan::Query {
            partition: partition.into(),
            from: 0,
            to: 1000,
        };
        let mut raw = std::collections::VecDeque::new();
        raw.push_back(Job {
            wanted: vec![q("oneone/bg1"), q("oneone/p1")],
            number: 1,
            attempt: 0,
            priority: false,
            user_prioritized: false,
        });
        raw.push_back(Job {
            wanted: vec![q("oneone/bg2"), q("oneone/p2"), q("oneone/p3")],
            number: 2,
            attempt: 0,
            priority: false,
            user_prioritized: false,
        });

        let mut tq = TransferQueue::new(raw, &owners, 2);

        // Promote t-p1 -> moves to front, removed from job 1 without duplicating.
        assert!(tq.add_ranges_to_front_in_queue("t-p1"));
        // Repeated click while front-promoted is deduplicated (returns false).
        assert!(!tq.add_ranges_to_front_in_queue("t-p1"));

        // Promote t-p2 and t-p3 to test anti-starvation fairness.
        assert!(tq.add_ranges_to_front_in_queue("t-p2"));
        assert!(tq.add_ranges_to_front_in_queue("t-p3"));

        // Pop 1 & Pop 2: priority jobs p3, p2
        let j1 = tq.pop_next().unwrap();
        assert_eq!(j1.wanted[0].partition, "oneone/p3");
        assert!(j1.user_prioritized);
        // Repeated click while in-flight is rejected.
        assert!(!tq.add_ranges_to_front_in_queue("t-p3"));

        let j2 = tq.pop_next().unwrap();
        assert_eq!(j2.wanted[0].partition, "oneone/p2");
        assert!(j2.user_prioritized);

        // Pop 3: after MAX_CONSECUTIVE_PRIORITY_POPS (2), background job MUST run to prevent starvation!
        let j3 = tq.pop_next().unwrap();
        assert!(!j3.user_prioritized);
        assert_eq!(j3.wanted.len(), 1);
        assert_eq!(j3.wanted[0].partition, "oneone/bg1");

        // Pop 4: remaining promoted job p1 can now run.
        let j4 = tq.pop_next().unwrap();
        assert_eq!(j4.wanted[0].partition, "oneone/p1");
    }

    #[test]
    fn active_priority_registration_is_scoped_to_owner_and_runtime() {
        let mut owners = HashMap::new();
        owners.insert("oneone/g1".into(), "thread-1".into());
        let mut raw = std::collections::VecDeque::new();
        raw.push_back(Job {
            wanted: vec![plan::Query {
                partition: "oneone/g-other".into(),
                from: 0,
                to: 100,
            }],
            number: 1,
            attempt: 0,
            priority: false,
            user_prioritized: false,
        });
        raw.push_back(Job {
            wanted: vec![plan::Query {
                partition: "oneone/g1".into(),
                from: 0,
                to: 100,
            }],
            number: 2,
            attempt: 0,
            priority: false,
            user_prioritized: false,
        });
        let tq = Arc::new(std::sync::Mutex::new(TransferQueue::new(raw, &owners, 2)));
        let _reg = ActivePriorityRegistration::register("acct-a".into(), 7, tq.clone());

        // Foreign account/runtime cannot promote.
        assert!(!prioritize_conversation("acct-b", 7, "thread-1"));
        assert!(!prioritize_conversation("acct-a", 8, "thread-1"));
        // Matching account & runtime succeeds.
        assert!(prioritize_conversation("acct-a", 7, "thread-1"));
    }
}
