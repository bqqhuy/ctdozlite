//! Sync coordinator: data stream isolation, availability gating, typed state machine,
//! milestone-based progress (without fake percentages), and RAII `pauseIdx` FTS indexing gate.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

static NEXT_SYNC_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

/// Five independent synchronization/data streams in ctdozlite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataStream {
    StartupSession,
    Metadata,
    CloudPreview,
    PhoneHistory,
    Realtime,
}

/// Trigger or invocation origin for a synchronization operation.
/// Disentangles the entrypoint origin (Setting, AutoSync, Banner, Legacy)
/// from the data stream and transfer mode represented by `SyncSource`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SyncOrigin {
    Setting,
    AutoSync,
    Banner,
    Legacy,
}

/// Explicit phone history source selected by the user.
/// Legacy V1 is always an explicit choice; Sync2 failures never fall back to Legacy V1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncSource {
    Sync2Full,
    Sync2Recent,
    Sync2Rescan,
    LegacyV1,
    ManualBackup,
}

impl SyncSource {
    /// Sync2 failures must surface cleanly and never silently fall back to Legacy V1.
    pub fn auto_fallback_source(self) -> Option<Self> {
        None
    }

    pub fn is_legacy(self) -> bool {
        matches!(self, Self::LegacyV1)
    }
}

/// Explicit reason why manual phone sync cannot start even when the UI is usable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncUnavailableReason {
    DbMigrating,
    WorkerNotReady,
    AlreadySyncing,
    SourceDisabled,
    PermissionDenied,
}

impl SyncUnavailableReason {
    pub fn user_message(self) -> &'static str {
        match self {
            Self::DbMigrating => "Cơ sở dữ liệu cục bộ đang mở hoặc migrate; thử lại sau giây lát",
            Self::WorkerNotReady => {
                "WebSocket chưa kết nối hoặc chưa xác thực; chờ kết nối rồi thử lại"
            }
            Self::AlreadySyncing => {
                "Một phiên đồng bộ lịch sử đang chạy; chờ hoàn tất hoặc dừng phiên trước"
            }
            Self::SourceDisabled => "Nguồn đồng bộ này hiện đang tắt theo cấu hình",
            Self::PermissionDenied => "Phiên tài khoản chưa được cấp quyền hoặc đã đăng xuất",
        }
    }
}

/// Separates general application readiness from `can-start-sync`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncAvailability {
    Ready,
    Unavailable(SyncUnavailableReason),
}

impl SyncAvailability {
    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn reason(self) -> Option<SyncUnavailableReason> {
        match self {
            Self::Ready => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

/// Evaluate whether a phone-sync operation is allowed to start right now.
pub fn evaluate_sync_availability(
    has_permission: bool,
    db_ready: bool,
    ws_ready: bool,
    source_enabled: bool,
    already_syncing: bool,
) -> SyncAvailability {
    if !has_permission {
        SyncAvailability::Unavailable(SyncUnavailableReason::PermissionDenied)
    } else if !db_ready {
        SyncAvailability::Unavailable(SyncUnavailableReason::DbMigrating)
    } else if !source_enabled {
        SyncAvailability::Unavailable(SyncUnavailableReason::SourceDisabled)
    } else if already_syncing {
        SyncAvailability::Unavailable(SyncUnavailableReason::AlreadySyncing)
    } else if !ws_ready {
        SyncAvailability::Unavailable(SyncUnavailableReason::WorkerNotReady)
    } else {
        SyncAvailability::Ready
    }
}

/// Evaluate whether a phone-sync operation is allowed to start right now for a given origin.
pub fn evaluate_sync_availability_for_origin(
    has_permission: bool,
    db_ready: bool,
    ws_ready: bool,
    origin_enabled: bool,
    already_syncing: bool,
) -> SyncAvailability {
    evaluate_sync_availability(
        has_permission,
        db_ready,
        ws_ready,
        origin_enabled,
        already_syncing,
    )
}

/// Five distinct data milestones during phone-history transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncTransferStage {
    DescriptorReceived,
    Downloaded,
    Decoded,
    DbCommitted,
    PartitionComplete,
}

impl SyncTransferStage {
    pub fn label_vi(self) -> &'static str {
        match self {
            Self::DescriptorReceived => "Đã nhận descriptor",
            Self::Downloaded => "Đã tải gói dữ liệu",
            Self::Decoded => "Đã giải mã",
            Self::DbCommitted => "Đã lưu vào CSDL",
            Self::PartitionComplete => "Hoàn tất phân vùng",
        }
    }
}

/// Typed state machine for the phone-history sync coordinator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncState {
    Idle {
        availability: SyncAvailability,
    },
    Preparing {
        source: SyncSource,
    },
    AwaitingPhoneConfirmation {
        timeout_at: Instant,
    },
    Transferring {
        stage: SyncTransferStage,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        inserted_messages: u64,
        total_messages: Option<u64>,
    },
    Interrupted {
        reason: String,
        retryable: bool,
    },
    Failed {
        error: String,
        fatal: bool,
    },
    Completed {
        inserted_messages: u64,
        duration: Duration,
    },
}

impl Default for SyncState {
    fn default() -> Self {
        Self::Idle {
            availability: SyncAvailability::Unavailable(SyncUnavailableReason::WorkerNotReady),
        }
    }
}

impl SyncState {
    /// Returns a real completion ratio ONLY when an authoritative non-zero denominator is known.
    /// Never fabricates percentages when total bytes/messages are unknown (`None` or `0`).
    pub fn known_percentage(&self) -> Option<f32> {
        match self {
            Self::Transferring {
                downloaded_bytes,
                total_bytes: Some(total),
                ..
            } if *total > 0 => {
                Some(((*downloaded_bytes as f64 / *total as f64) * 100.0).clamp(0.0, 100.0) as f32)
            }
            Self::Transferring {
                inserted_messages,
                total_messages: Some(total),
                ..
            } if *total > 0 => {
                Some(((*inserted_messages as f64 / *total as f64) * 100.0).clamp(0.0, 100.0) as f32)
            }
            _ => None,
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self,
            Self::Preparing { .. }
                | Self::AwaitingPhoneConfirmation { .. }
                | Self::Transferring { .. }
        )
    }

    pub fn summary_vi(&self) -> String {
        match self {
            Self::Idle {
                availability: SyncAvailability::Ready,
            } => "Sẵn sàng đồng bộ lịch sử từ điện thoại".into(),
            Self::Idle {
                availability: SyncAvailability::Unavailable(reason),
            } => reason.user_message().into(),
            Self::Preparing { source } => match source {
                SyncSource::Sync2Recent => "Đang chuẩn bị đồng bộ tin gần đây…".into(),
                SyncSource::Sync2Rescan => "Đang chuẩn bị quét lại lịch sử…".into(),
                SyncSource::LegacyV1 => "Đang chuẩn bị đồng bộ V1 dự phòng…".into(),
                _ => "Đang chuẩn bị đồng bộ từ điện thoại…".into(),
            },
            Self::AwaitingPhoneConfirmation { .. } => {
                "Đang chờ bạn xác nhận đồng bộ trên điện thoại…".into()
            }
            Self::Transferring {
                stage,
                downloaded_bytes,
                total_bytes,
                inserted_messages,
                total_messages,
            } => {
                let bytes_part = match total_bytes {
                    Some(total) if *total > 0 => format!(
                        "{:.1}/{:.1} MiB",
                        *downloaded_bytes as f64 / (1024.0 * 1024.0),
                        *total as f64 / (1024.0 * 1024.0)
                    ),
                    _ => format!("{} KiB", downloaded_bytes / 1024),
                };
                let msgs_part = match total_messages {
                    Some(total) if *total > 0 => format!("{inserted_messages}/{total} tin mới"),
                    _ => format!("{inserted_messages} tin mới"),
                };
                format!("{} • {bytes_part} • {msgs_part}", stage.label_vi())
            }
            Self::Interrupted { reason, .. } => reason.clone(),
            Self::Failed { error, .. } => error.clone(),
            Self::Completed {
                inserted_messages,
                duration,
            } => format!(
                "Hoàn tất đồng bộ: {inserted_messages} tin mới ({:.1}s)",
                duration.as_secs_f32()
            ),
        }
    }
}

/// Allocates a unique monotonic operation identifier for each sync invocation.
pub fn next_operation_id() -> u64 {
    NEXT_SYNC_OPERATION_ID.fetch_add(1, Ordering::Relaxed)
}

/// Thread-safe progress aggregator tracking real bytes/messages across the 5 transfer stages
/// without fabricating unknown totals.
#[derive(Clone, Debug)]
pub struct SyncProgressTracker {
    inner: Arc<Mutex<TrackerInner>>,
}

#[derive(Debug)]
struct TrackerInner {
    stage: SyncTransferStage,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    inserted_messages: u64,
    total_messages: Option<u64>,
}

impl Default for SyncProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncProgressTracker {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TrackerInner {
                stage: SyncTransferStage::DescriptorReceived,
                downloaded_bytes: 0,
                total_bytes: None,
                inserted_messages: 0,
                total_messages: None,
            })),
        }
    }

    pub fn set_total_bytes(&self, total: Option<u64>) {
        if let Ok(mut g) = self.inner.lock() {
            g.total_bytes = total.filter(|n| *n > 0);
        }
    }

    pub fn set_total_messages(&self, total: Option<u64>) {
        if let Ok(mut g) = self.inner.lock() {
            g.total_messages = total.filter(|n| *n > 0);
        }
    }

    pub fn record_descriptor_received(&self) -> SyncState {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.stage = SyncTransferStage::DescriptorReceived;
        Self::to_state(&g)
    }

    pub fn record_downloaded_bytes(&self, delta_bytes: u64) -> SyncState {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.downloaded_bytes = g.downloaded_bytes.saturating_add(delta_bytes);
        g.stage = SyncTransferStage::Downloaded;
        Self::to_state(&g)
    }

    pub fn record_decoded(&self) -> SyncState {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.stage = SyncTransferStage::Decoded;
        Self::to_state(&g)
    }

    pub fn record_db_committed(&self, newly_inserted: u64) -> SyncState {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.inserted_messages = g.inserted_messages.saturating_add(newly_inserted);
        g.stage = SyncTransferStage::DbCommitted;
        Self::to_state(&g)
    }

    pub fn record_partition_complete(&self) -> SyncState {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.stage = SyncTransferStage::PartitionComplete;
        Self::to_state(&g)
    }

    pub fn snapshot(&self) -> SyncState {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        Self::to_state(&g)
    }

    fn to_state(g: &TrackerInner) -> SyncState {
        SyncState::Transferring {
            stage: g.stage,
            downloaded_bytes: g.downloaded_bytes,
            total_bytes: g.total_bytes,
            inserted_messages: g.inserted_messages,
            total_messages: g.total_messages,
        }
    }
}

pub type SyncStateSink = Option<(
    SyncProgressTracker,
    std::sync::Arc<dyn Fn(SyncState) + Send + Sync>,
)>;

/// Refcounted FTS/search indexing gate (`pauseIdx`).
#[derive(Clone, Debug)]
pub struct IndexingGate {
    depth: Arc<AtomicUsize>,
}

impl Default for IndexingGate {
    fn default() -> Self {
        Self {
            depth: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl IndexingGate {
    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<IndexingGate> = OnceLock::new();
        GLOBAL.get_or_init(Self::default)
    }

    pub fn pause(&self) -> IndexingPauseGuard {
        self.depth.fetch_add(1, Ordering::AcqRel);
        IndexingPauseGuard {
            depth: self.depth.clone(),
            released: false,
        }
    }

    pub fn is_paused(&self) -> bool {
        self.depth.load(Ordering::Acquire) > 0
    }

    pub fn depth(&self) -> usize {
        self.depth.load(Ordering::Acquire)
    }
}

/// RAII guard pausing heavy FTS/search indexing (`pauseIdx`) during bulk history sync.
/// Automatically restores indexing on normal completion, cancellation, error, or unwind.
#[must_use = "Dropping IndexingPauseGuard immediately resumes search indexing"]
pub struct IndexingPauseGuard {
    depth: Arc<AtomicUsize>,
    released: bool,
}

/// Pause FTS / search backfill indexing while returning an RAII guard that guarantees resume.
pub fn pause_indexing() -> IndexingPauseGuard {
    IndexingGate::global().pause()
}

/// Explicitly release an [`IndexingPauseGuard`] (equivalent to dropping it).
pub fn resume_indexing(mut guard: IndexingPauseGuard) {
    guard.release();
}

/// Returns `true` while any global [`IndexingPauseGuard`] is active.
pub fn is_indexing_paused() -> bool {
    IndexingGate::global().is_paused()
}

/// Active global pause depth (useful for diagnostics and tests).
pub fn indexing_pause_depth() -> usize {
    IndexingGate::global().depth()
}

impl IndexingPauseGuard {
    fn release(&mut self) {
        if !self.released {
            self.released = true;
            // Saturating decrement ensures a clean invariant even under unusual unwinds.
            let _ = self
                .depth
                .try_update(Ordering::AcqRel, Ordering::Acquire, |cur| {
                    cur.checked_sub(1)
                });
        }
    }
}

impl Drop for IndexingPauseGuard {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_availability_rejects_invalid_states_with_explicit_reason() {
        assert_eq!(
            evaluate_sync_availability(false, true, true, true, false),
            SyncAvailability::Unavailable(SyncUnavailableReason::PermissionDenied)
        );
        assert_eq!(
            evaluate_sync_availability(true, false, true, true, false),
            SyncAvailability::Unavailable(SyncUnavailableReason::DbMigrating)
        );
        assert_eq!(
            evaluate_sync_availability(true, true, true, false, false),
            SyncAvailability::Unavailable(SyncUnavailableReason::SourceDisabled)
        );
        assert_eq!(
            evaluate_sync_availability(true, true, true, true, true),
            SyncAvailability::Unavailable(SyncUnavailableReason::AlreadySyncing)
        );
        assert_eq!(
            evaluate_sync_availability(true, true, false, true, false),
            SyncAvailability::Unavailable(SyncUnavailableReason::WorkerNotReady)
        );
        assert_eq!(
            evaluate_sync_availability(true, true, true, true, false),
            SyncAvailability::Ready
        );
        assert_eq!(
            evaluate_sync_availability_for_origin(true, true, true, false, false),
            SyncAvailability::Unavailable(SyncUnavailableReason::SourceDisabled)
        );
        assert_eq!(
            evaluate_sync_availability_for_origin(true, true, true, true, false),
            SyncAvailability::Ready
        );
    }

    #[test]
    fn sync2_never_auto_falls_back_to_legacy_v1() {
        for source in [
            SyncSource::Sync2Full,
            SyncSource::Sync2Recent,
            SyncSource::Sync2Rescan,
            SyncSource::LegacyV1,
            SyncSource::ManualBackup,
        ] {
            assert_eq!(source.auto_fallback_source(), None);
        }
        assert!(!SyncSource::Sync2Full.is_legacy());
        assert!(SyncSource::LegacyV1.is_legacy());
    }

    #[test]
    fn sync_state_never_fabricates_percentage_without_known_denominator() {
        let tracker = SyncProgressTracker::new();
        let s1 = tracker.record_descriptor_received();
        assert_eq!(s1.known_percentage(), None);
        let s2 = tracker.record_downloaded_bytes(64 * 1024);
        assert_eq!(s2.known_percentage(), None);
        assert!(s2.summary_vi().contains("64 KiB"));
        assert!(!s2.summary_vi().contains('%'));

        // Zero denominator must also return None instead of dividing by zero or faking 100%.
        tracker.set_total_bytes(Some(0));
        assert_eq!(tracker.snapshot().known_percentage(), None);

        // Once a positive total is known, real percentage is reported accurately.
        tracker.set_total_bytes(Some(128 * 1024));
        assert_eq!(tracker.snapshot().known_percentage(), Some(50.0));
    }

    #[test]
    fn raii_pause_and_resume_indexing_recovers_on_drop_cancel_and_error() {
        let gate = IndexingGate::default();
        assert_eq!(gate.depth(), 0);
        // Normal completion via explicit resume_indexing
        let guard = gate.pause();
        assert!(gate.is_paused());
        assert_eq!(gate.depth(), 1);
        resume_indexing(guard);
        assert!(!gate.is_paused());
        assert_eq!(gate.depth(), 0);

        // Error/cancel early return drops guard cleanly
        fn early_cancel(gate: &IndexingGate) -> Result<(), &'static str> {
            let _g1 = gate.pause();
            let _g2 = gate.pause();
            assert_eq!(gate.depth(), 2);
            Err("cancelled")
        }
        let err_res = early_cancel(&gate);
        assert!(err_res.is_err());
        assert!(!gate.is_paused());
        assert_eq!(gate.depth(), 0);

        // Panic unwind drops guard cleanly
        let panic_gate = gate.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _g = panic_gate.pause();
            assert_eq!(panic_gate.depth(), 1);
            panic!("simulated panic during sync");
        }));
        assert!(!gate.is_paused());
        assert_eq!(gate.depth(), 0);
    }
}
