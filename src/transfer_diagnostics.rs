//! Bounded production diagnostics with typed fields; no payload, URL or key inputs.
use serde::Serialize;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Safe fields extracted from a matching phone control.
#[derive(Serialize)]
pub(crate) struct PhoneControl {
    pub index: Option<u64>,
    pub last: bool,
    pub scopes: usize,
    pub error: i64,
    pub status: i64,
    pub batch_type: i64,
    pub command: u16,
    pub sub_command: u8,
    pub round: usize,
    /// Irreversible tags of the batch's partitions (at most eight) to locate missing ranges.
    pub partition_tags: Vec<u64>,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Started,
    Finished,
    Failed,
    Cancelled,
}
/// Pagination decisions contain no cursor or payload data.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QueueDecision {
    Initial,
    Continue,
    Complete,
    Repeated,
    Missing,
    Resume,
    Exhausted,
    Stale,
    /// A known pull queue first seen in a valid server page, not in the auth list.
    DiscoveredFromPage,
    /// A page with more data for a queue that is neither advertised nor eligible.
    Unadvertised,
}
/// Receiver counters let support distinguish actual descriptor gaps from dropped log events.
#[derive(Serialize)]
pub(crate) struct RoundSummary {
    pub round: usize,
    pub received: usize,
    pub max_index: Option<u64>,
    pub final_index: Option<u64>,
    pub missing: usize,
    pub confirmed: bool,
    pub idle: bool,
    pub active_wait_seconds: u64,
}
/// Totals for one phone sync run; counts only, no identities.
#[derive(Serialize)]
pub(crate) struct PhoneTotals {
    /// Messages read for conversations that could be mapped.
    pub read: usize,
    pub inserted: usize,
    /// Read but already stored (deduplicated).
    pub existing: usize,
    /// Kept in quarantine because the conversation was not recognised.
    pub unmapped: usize,
    pub skipped_rounds: usize,
    pub failed_conversations: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum QueueTransport {
    Http,
    WebSocket,
}
/// Independent stages distinguish network waiting from codec and database work.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BatchStage {
    Download,
    Decode,
    ResolveSenders,
    Import,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum WorkState {
    Started,
    Finished,
    Aborted,
}
/// Renewal phases distinguish local admission failures from actual server responses.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RenewPhase {
    Entered,
    PreparationFailed,
    RequestSent,
    NetworkFailed,
    HttpRejected,
    ServerReply,
    ServerRejected,
    ParseFailed,
    /// Server accepted the request but is still preparing the copy (`async`).
    Pending,
    /// The shared renewal deadline ran out while the server stayed `async`.
    PendingTimeout,
    Finished,
    CacheHitSuccess,
    CacheHitFailure,
}
/// Why a renewal reply could not be used; never carries URLs or server text.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RenewFailure {
    MissingData,
    WrongType,
    DecodeFailed,
    UrlBlocked,
    ServerRejected,
}
/// Why one socket generation ended; never carries the close reason text.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SocketEnd {
    Close,
    Eof,
    Error,
    ReceiverClosed,
    SessionRevoked,
}
/// Terminal result of one phone round attempt.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RoundOutcome {
    Complete,
    Stalled,
    Disconnected,
    Rejected,
    Failed,
    Skipped,
}
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Event {
    SocketClosed {
        epoch: u64,
        end: SocketEnd,
        code: Option<u16>,
    },
    SocketBackpressure {
        epoch: u64,
        waited_ms: u64,
    },
    PhoneWake {
        session_tag: u64,
        automatic: bool,
        written: bool,
        server_code: Option<i64>,
    },
    PhoneRoundResult {
        round: usize,
        attempt: u8,
        partitions: usize,
        outcome: RoundOutcome,
    },
    PhoneResume {
        reconnects: u8,
        epoch: u64,
    },
    PhoneTotals {
        totals: PhoneTotals,
    },
    DiagnosticDropped {
        count: u64,
    },
    PhoneRoundSummary {
        session_tag: u64,
        summary: RoundSummary,
    },
    QueuePage {
        transport: QueueTransport,
        epoch: u64,
        command: u16,
        sub_command: u8,
        decision: QueueDecision,
        retries: u8,
    },
    BatchWork {
        session_tag: u64,
        round: usize,
        index: u64,
        stage: BatchStage,
        state: WorkState,
        elapsed_ms: u64,
        rows: usize,
    },
    PhoneDispose {
        session_tag: u64,
        reason: u8,
        socket_write_ack: bool,
    },
    MediaRenewal {
        reference_tag: u64,
        phase: RenewPhase,
        server_code: Option<i64>,
        http_status: Option<u16>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<RenewFailure>,
    },
    PhoneControl {
        session_tag: u64,
        control: PhoneControl,
    },
    MediaFailure {
        failure: crate::media_recovery::Failure,
    },
    PhonePhase {
        phase: Phase,
    },
}
/// Correlate diagnostic events without retaining the underlying identifier.
pub(crate) fn tag(id: &str) -> u64 {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(id.as_bytes());
    let mut tag = [0; 8];
    tag.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(tag)
}
fn control_event(id: &str, control: PhoneControl) -> Event {
    Event::PhoneControl {
        session_tag: tag(id),
        control,
    }
}
/// Enqueue only numeric control fields and an irreversible session correlation tag.
pub(crate) fn phone(id: &str, control: PhoneControl) {
    record(control_event(id, control));
}
/// Persist the classified error, never its original network error or response body.
pub(crate) fn media(failure: &crate::media_recovery::Failure) {
    record(Event::MediaFailure {
        failure: failure.clone(),
    });
}

/// Record connection-scoped pagination decisions without raw cursor values.
pub(crate) fn queue(epoch: u64, key: (u16, u8), decision: QueueDecision, retries: u8) {
    record(Event::QueuePage {
        transport: if key == (0, 0) {
            QueueTransport::Http
        } else {
            QueueTransport::WebSocket
        },
        epoch,
        command: key.0,
        sub_command: key.1,
        decision,
        retries,
    });
}
/// Record the runtime's own counters, independent of individual diagnostic admission.
pub(crate) fn round(id: &str, summary: RoundSummary) {
    record(Event::PhoneRoundSummary {
        session_tag: tag(id),
        summary,
    });
}
/// Record the totals of one phone sync run.
pub(crate) fn phone_totals(totals: PhoneTotals) {
    record(Event::PhoneTotals { totals });
}
/// Record how a socket generation ended, with the numeric close code only.
pub(crate) fn socket_closed(epoch: u64, end: SocketEnd, code: Option<u16>) {
    record(Event::SocketClosed { epoch, end, code });
}
/// A slow consumer delayed the socket reader; long waits can starve keep-alive pings.
pub(crate) fn socket_backpressure(epoch: u64, waited_ms: u64) {
    record(Event::SocketBackpressure { epoch, waited_ms });
}
/// Record a wake-up request and, when known, the server acknowledgement code.
pub(crate) fn wake(id: &str, automatic: bool, written: bool, server_code: Option<i64>) {
    record(Event::PhoneWake {
        session_tag: tag(id),
        automatic,
        written,
        server_code,
    });
}
/// Record the terminal outcome of one round attempt.
pub(crate) fn round_result(round: usize, attempt: u8, partitions: usize, outcome: RoundOutcome) {
    record(Event::PhoneRoundResult {
        round,
        attempt,
        partitions,
        outcome,
    });
}
/// Record that a sync continued on a new socket generation.
pub(crate) fn resume(reconnects: u8, epoch: u64) {
    record(Event::PhoneResume { reconnects, epoch });
}
/// A socket write ACK is not an acknowledgement that the phone stopped its work.
pub(crate) fn dispose(id: &str, reason: u8, socket_write_ack: bool) {
    record(Event::PhoneDispose {
        session_tag: tag(id),
        reason,
        socket_write_ack,
    });
}
/// Record only a classified renewal phase and numeric server code.
pub(crate) fn renewal(reference_tag: u64, phase: RenewPhase, server_code: Option<i64>) {
    record(Event::MediaRenewal {
        reference_tag,
        phase,
        server_code,
        http_status: None,
        reason: None,
    });
}
/// Record why a successful-looking reply could not be used.
pub(crate) fn renewal_failed(reference_tag: u64, server_code: Option<i64>, reason: RenewFailure) {
    record(Event::MediaRenewal {
        reference_tag,
        phase: if reason == RenewFailure::ServerRejected {
            RenewPhase::ServerRejected
        } else {
            RenewPhase::ParseFailed
        },
        server_code,
        http_status: None,
        reason: Some(reason),
    });
}
/// Record typed HTTP failures separately from JSON API error codes.
pub(crate) fn renewal_http(reference_tag: u64, http_status: u16) {
    record(Event::MediaRenewal {
        reference_tag,
        phase: RenewPhase::HttpRejected,
        server_code: None,
        http_status: Some(http_status),
        reason: None,
    });
}
/// RAII stage tracing records aborted work even when a task exits through `?`.
pub(crate) struct BatchWork {
    session_tag: u64,
    round: usize,
    index: u64,
    stage: BatchStage,
    started: std::time::Instant,
    done: bool,
}
impl BatchWork {
    pub fn start(id: &str, round: usize, index: u64, stage: BatchStage) -> Self {
        let work = Self {
            session_tag: tag(id),
            round,
            index,
            stage,
            started: std::time::Instant::now(),
            done: false,
        };
        work.emit(WorkState::Started, 0);
        work
    }
    pub fn finish(&mut self, rows: usize) {
        self.done = true;
        self.emit(WorkState::Finished, rows);
    }
    fn emit(&self, state: WorkState, rows: usize) {
        record(Event::BatchWork {
            session_tag: self.session_tag,
            round: self.round,
            index: self.index,
            stage: self.stage,
            state,
            elapsed_ms: self.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            rows,
        });
    }
}
impl Drop for BatchWork {
    fn drop(&mut self) {
        if !self.done {
            self.emit(WorkState::Aborted, 0);
        }
    }
}

/// Record terminal outcomes even when the transfer returns early with an error.
pub(crate) struct PhoneRun {
    cancel: Arc<AtomicBool>,
    finished: bool,
}
impl PhoneRun {
    /// Start a diagnostic scope without blocking the transfer on filesystem work.
    pub(crate) fn start(cancel: Arc<AtomicBool>) -> Self {
        record(Event::PhonePhase {
            phase: Phase::Started,
        });
        Self {
            cancel,
            finished: false,
        }
    }
    /// Mark the complete result; failed scopes remain visible in their control events.
    pub(crate) fn finish(&mut self) {
        self.finished = true;
        record(Event::PhonePhase {
            phase: Phase::Finished,
        });
    }
}
impl Drop for PhoneRun {
    fn drop(&mut self) {
        if !self.finished {
            record(Event::PhonePhase {
                phase: if self.cancel.load(Ordering::Acquire) {
                    Phase::Cancelled
                } else {
                    Phase::Failed
                },
            });
        }
    }
}

#[cfg(not(test))]
static DROPPED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[cfg(not(test))]
fn record(event: Event) {
    use std::sync::{mpsc, OnceLock};
    static CHANNEL: OnceLock<Option<mpsc::SyncSender<Event>>> = OnceLock::new();
    if let Some(sender) = CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(256);
        std::thread::Builder::new()
            .name("safe-diagnostics".into())
            .spawn(move || {
                let root = crate::storage::root().join("diagnostics");
                if crate::storage::private_dir(&root).is_err() {
                    return;
                }
                while let Ok(event) = rx.recv() {
                    let count = DROPPED.swap(0, Ordering::AcqRel);
                    if count > 0 {
                        let _ = append(&root, &Event::DiagnosticDropped { count });
                    }
                    // Diagnostics are best effort and may never fail a chat/transfer operation.
                    let _ = append(&root, &event);
                }
            })
            .ok()
            .map(|_| tx)
    }) {
        if sender.try_send(event).is_err() {
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
    }
}
#[cfg(test)]
fn record(_event: Event) {}

/// Rotate the diagnostic log at this size, keeping one previous file (2 × 4 MiB).
const ROTATE_BYTES: u64 = 4 * 1024 * 1024;
fn append(root: &Path, event: &Event) -> std::io::Result<()> {
    use std::io::Write;
    let path = root.join("sync-media.jsonl");
    let previous = root.join("sync-media.previous.jsonl");
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink())
        || std::fs::symlink_metadata(&previous).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(std::io::Error::other("Diagnostic path is a symlink"));
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= ROTATE_BYTES) {
        match std::fs::remove_file(&previous) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        std::fs::rename(&path, &previous)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({"timestamp_ms":timestamp,"data":event}),
    )?;
    file.write_all(b"\n")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_log_contains_typed_counts_without_raw_session_or_keys_and_rotates() {
        assert!(
            serde_json::to_string(&Event::DiagnosticDropped { count: 7 })
                .unwrap()
                .contains("diagnostic_dropped")
        );
        let id = "private-session-do-not-record";
        let event = control_event(
            id,
            PhoneControl {
                index: Some(7),
                last: true,
                scopes: 3,
                error: 0,
                status: 4,
                batch_type: 2,
                command: 603,
                sub_command: 1,
                round: 2,
                partition_tags: vec![tag("oneone/private-partition")],
            },
        );
        let text = serde_json::to_string(&event).unwrap();
        assert!(!text.contains(id));
        assert!(!text.contains("private-partition"));
        assert!(!text.contains("url"));
        assert!(!text.contains("key"));
        assert!(text.contains("session_tag"));
        let root = std::env::temp_dir().join(format!(
            "safe-diagnostics-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("sync-media.jsonl"),
            vec![b' '; ROTATE_BYTES as usize - 1],
        )
        .unwrap();
        append(&root, &event).unwrap();
        assert!(!root.join("sync-media.previous.jsonl").exists());
        append(&root, &event).unwrap();
        assert!(root.join("sync-media.previous.jsonl").exists());
        let current = std::fs::read_to_string(root.join("sync-media.jsonl")).unwrap();
        assert_eq!(current.lines().count(), 1);
        assert!(!current.contains(id));
        std::fs::remove_dir_all(root).unwrap();
    }
}
