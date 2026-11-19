//! Native playback entry point; the worker owns downloads and the player owns COM.
use anyhow::Result;
use std::path::Path;
pub fn play(path: &Path, title: &str, active: impl Fn() -> bool) -> Result<()> {
    #[cfg(windows)]
    {
        native::run(path, title, active)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, title, active);
        anyhow::bail!("Trình phát video yêu cầu Windows ARM64")
    }
}

/// Whether a native video playback window is currently active.
pub fn is_video_window_active() -> bool {
    #[cfg(windows)]
    {
        native::is_video_window_active()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Send play/pause toggle command to active video window.
pub fn control_play_pause() {
    #[cfg(windows)]
    {
        native::control_play_pause();
    }
}

/// Seek active video window to target position in seconds.
pub fn control_seek(target_secs: f32) {
    #[cfg(windows)]
    {
        native::control_seek(target_secs);
    }
    #[cfg(not(windows))]
    {
        let _ = target_secs;
    }
}

/// Set volume (0.0 to 1.0) on active video window.
pub fn control_set_volume(volume: f32) {
    #[cfg(windows)]
    {
        native::control_set_volume(volume);
    }
    #[cfg(not(windows))]
    {
        let _ = volume;
    }
}
#[cfg(windows)]
#[path = "video_player_windows.rs"]
mod native;

/// Maximum playback cache size in bytes (512 MiB).
pub const MAX_CACHE_BYTES: u64 = 512 * 1024 * 1024;

/// An app-owned temporary video file; active viewers are excluded from cleanup.
/// Fields: (path, admitted_bytes, reserved_pending_bytes)
pub(crate) struct CachedVideo(
    std::path::PathBuf,
    std::sync::atomic::AtomicU64,
    std::sync::atomic::AtomicU64,
);
#[derive(Default)]
struct BudgetState {
    admitted: u64,
    pending: u64,
}

#[derive(Default)]
struct PlaybackBudget(std::sync::Mutex<BudgetState>);

impl PlaybackBudget {
    fn reserve(&self, pending: &std::sync::atomic::AtomicU64, bytes: u64) -> Result<()> {
        use std::sync::atomic::Ordering;
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Cache video không khả dụng"))?;
        Self::check_capacity(&state, bytes)?;
        state.pending += bytes;
        pending.fetch_add(bytes, Ordering::Relaxed);
        Ok(())
    }

    fn admit(
        &self,
        admitted: &std::sync::atomic::AtomicU64,
        pending: &std::sync::atomic::AtomicU64,
        bytes: u64,
    ) -> Result<()> {
        use std::sync::atomic::Ordering;
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Cache video không khả dụng"))?;
        let transfer = pending.load(Ordering::Relaxed).min(bytes);
        // Validate the entire request before consuming any existing reservation.
        Self::check_capacity(&state, bytes - transfer)?;
        state.pending -= transfer;
        state.admitted += bytes;
        pending.fetch_sub(transfer, Ordering::Relaxed);
        admitted.fetch_add(bytes, Ordering::Relaxed);
        Ok(())
    }

    fn check_capacity(state: &BudgetState, extra: u64) -> Result<()> {
        anyhow::ensure!(
            state
                .admitted
                .checked_add(state.pending)
                .and_then(|total| total.checked_add(extra))
                .is_some_and(|total| total <= MAX_CACHE_BYTES),
            "Cache video đã đạt 512 MiB; hãy đóng viewer khác hoặc lưu tệp về máy"
        );
        Ok(())
    }

    fn release(
        &self,
        admitted: &std::sync::atomic::AtomicU64,
        pending: &std::sync::atomic::AtomicU64,
    ) {
        use std::sync::atomic::Ordering;
        // A dropped lease must release its units even if an earlier holder panicked.
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let next_admitted = state
            .admitted
            .checked_sub(admitted.swap(0, Ordering::Relaxed));
        let next_pending = state
            .pending
            .checked_sub(pending.swap(0, Ordering::Relaxed));
        if let (Some(admitted), Some(pending)) = (next_admitted, next_pending) {
            state.admitted = admitted;
            state.pending = pending;
        } else {
            drop(state);
            crate::logger::log_error("video cache", "Playback budget ownership mismatch");
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> (u64, u64) {
        let state = self.0.lock().unwrap();
        (state.admitted, state.pending)
    }
}

static PLAYBACK_BUDGET: PlaybackBudget = PlaybackBudget(std::sync::Mutex::new(BudgetState {
    admitted: 0,
    pending: 0,
}));
static ACTIVE_CACHE: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());
impl CachedVideo {
    /// Remove stale private video files and reserve the new playback path.
    /// # Errors
    /// Returns private directory, cache enumeration or lock failures.
    pub(crate) fn reserve(root: &Path, name: &str) -> Result<Self> {
        crate::storage::private_dir(root)?;
        let path = root.join(name);
        {
            let mut active = ACTIVE_CACHE
                .lock()
                .map_err(|_| anyhow::anyhow!("Video cache unavailable"))?;
            anyhow::ensure!(
                !active.contains(&path),
                "Video đang được tải hoặc đang được phát"
            );
            active.push(path.clone());
        }
        let lease = Self(
            path,
            std::sync::atomic::AtomicU64::new(0),
            std::sync::atomic::AtomicU64::new(0),
        );
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;
            let in_use = ACTIVE_CACHE
                .lock()
                .map_err(|_| anyhow::anyhow!("Video cache unavailable"))?
                .contains(&path);
            // A25 fix: Skip active partial files (.ctdoz-*.partial) from media_download
            let is_active_partial = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(".ctdoz-") && n.ends_with(".partial"));
            if !in_use
                && !is_active_partial
                && metadata.is_file()
                && !entry.file_type()?.is_symlink()
            {
                // Files in this directory are disposable playback copies, never user saves.
                if std::fs::remove_file(path).is_err() {
                    crate::logger::log_error("video cache", "Cannot remove stale playback file");
                }
            }
        }
        Ok(lease)
    }

    /// Reserve upfront capacity against the combined pending and admitted budget.
    pub(crate) fn reserve_capacity(&self, bytes: u64) -> Result<()> {
        PLAYBACK_BUDGET.reserve(&self.2, bytes)
    }

    /// Admit bytes before writing, consuming this lease's reserved capacity first.
    /// Pending-to-admitted transfers and other admissions share one budget lock.
    pub(crate) fn admit(&self, bytes: usize) -> Result<()> {
        PLAYBACK_BUDGET.admit(&self.1, &self.2, bytes as u64)
    }

    /// Playback destination reserved for this lease only.
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn admitted_bytes(&self) -> u64 {
        self.1.load(std::sync::atomic::Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn reserved_bytes(&self) -> u64 {
        self.2.load(std::sync::atomic::Ordering::Acquire)
    }
}
impl Drop for CachedVideo {
    fn drop(&mut self) {
        PLAYBACK_BUDGET.release(&self.1, &self.2);
        if let Ok(mut active) = ACTIVE_CACHE.lock() {
            active.retain(|p| p != &self.0);
        }
        if let Err(error) = std::fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                crate::logger::log_error("video cache", "Cannot remove playback file");
            }
        }
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn isolated_budget_serializes_competing_reserve_and_admit() {
        use std::sync::{atomic::AtomicU64, Arc, Barrier};
        let budget = Arc::new(PlaybackBudget::default());
        let reserved = Arc::new(AtomicU64::new(0));
        let admitted = Arc::new(AtomicU64::new(0));
        let pending = Arc::new(AtomicU64::new(0));
        let start = Arc::new(Barrier::new(3));
        let reserve = {
            let (budget, reserved, start) = (budget.clone(), reserved.clone(), start.clone());
            std::thread::spawn(move || {
                start.wait();
                budget.reserve(&reserved, 400 * 1024 * 1024).is_ok()
            })
        };
        let admit = {
            let (budget, admitted, pending, start) = (
                budget.clone(),
                admitted.clone(),
                pending.clone(),
                start.clone(),
            );
            std::thread::spawn(move || {
                start.wait();
                budget.admit(&admitted, &pending, 400 * 1024 * 1024).is_ok()
            })
        };
        start.wait();
        assert_ne!(reserve.join().unwrap(), admit.join().unwrap());
        let (cached, waiting) = budget.snapshot();
        assert_eq!(cached + waiting, 400 * 1024 * 1024);
        budget.release(&admitted, &pending);
        budget.release(&AtomicU64::new(0), &reserved);
        assert_eq!(budget.snapshot(), (0, 0));
    }

    #[test]
    fn reservation_transfer_never_exposes_capacity_or_partially_admits() {
        use std::sync::{atomic::AtomicU64, Arc, Barrier};
        let budget = Arc::new(PlaybackBudget::default());
        let pending = Arc::new(AtomicU64::new(0));
        let admitted = Arc::new(AtomicU64::new(0));
        budget.reserve(&pending, MAX_CACHE_BYTES).unwrap();
        assert!(budget
            .admit(&admitted, &pending, MAX_CACHE_BYTES + 1)
            .is_err());
        assert_eq!(budget.snapshot(), (0, MAX_CACHE_BYTES));
        let start = Arc::new(Barrier::new(2));
        let transfer = {
            let (budget, pending, admitted, start) = (
                budget.clone(),
                pending.clone(),
                admitted.clone(),
                start.clone(),
            );
            std::thread::spawn(move || {
                start.wait();
                budget.admit(&admitted, &pending, MAX_CACHE_BYTES).unwrap();
            })
        };
        start.wait();
        assert!(budget
            .admit(&AtomicU64::new(0), &AtomicU64::new(0), 1)
            .is_err());
        transfer.join().unwrap();
        assert_eq!(budget.snapshot(), (MAX_CACHE_BYTES, 0));
        budget.release(&admitted, &pending);
        assert_eq!(budget.snapshot(), (0, 0));
    }

    #[test]
    fn active_videos_survive_pruning_and_budget_is_released_on_close() {
        let _guard = TEST_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "video-lease-{}",
            crate::chat::next_client_message_id()
        ));
        let first = CachedVideo::reserve(&root, "first.mp4").unwrap();
        std::fs::write(first.path(), b"active").unwrap();
        std::fs::write(root.join("orphan.mp4"), b"old").unwrap();
        let second = CachedVideo::reserve(&root, "second.mp4").unwrap();
        assert!(first.path().exists());
        assert!(!root.join("orphan.mp4").exists());
        first.admit(512 * 1024 * 1024 - 1).unwrap();
        assert!(second.admit(2).is_err());
        drop(first);
        assert!(!root.join("first.mp4").exists());
        second.admit(2).unwrap();
        drop(second);
        assert_eq!(PLAYBACK_BUDGET.snapshot().0, 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_path_reservation_is_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "video-dup-{}",
            crate::chat::next_client_message_id()
        ));
        let first = CachedVideo::reserve(&root, "dup.mp4").unwrap();
        assert!(CachedVideo::reserve(&root, "dup.mp4").is_err());
        drop(first);
        assert!(CachedVideo::reserve(&root, "dup.mp4").is_ok());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn upfront_capacity_reservation_blocks_overflow() {
        let _guard = TEST_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "video-res-{}",
            crate::chat::next_client_message_id()
        ));
        let first = CachedVideo::reserve(&root, "res1.mp4").unwrap();
        let second = CachedVideo::reserve(&root, "res2.mp4").unwrap();

        first.reserve_capacity(300 * 1024 * 1024).unwrap();
        assert_eq!(first.reserved_bytes(), 300 * 1024 * 1024);
        assert_eq!(PLAYBACK_BUDGET.snapshot().1, 300 * 1024 * 1024);

        // 300MB + 250MB = 550MB > 512MB MAX_CACHE_BYTES -> must fail
        assert!(second.reserve_capacity(250 * 1024 * 1024).is_err());
        // Unreserved admit must also be blocked by pending reservation
        assert!(second.admit(250 * 1024 * 1024).is_err());

        // Drop first -> pending reservation released
        drop(first);
        assert_eq!(PLAYBACK_BUDGET.snapshot().1, 0);

        // Now second can reserve
        second.reserve_capacity(250 * 1024 * 1024).unwrap();
        drop(second);
        assert_eq!(PLAYBACK_BUDGET.snapshot().1, 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn capacity_reservation_transfers_to_admitted_bytes() {
        let _guard = TEST_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "video-trans-{}",
            crate::chat::next_client_message_id()
        ));
        let lease = CachedVideo::reserve(&root, "trans.mp4").unwrap();
        lease.reserve_capacity(1000).unwrap();
        assert_eq!(lease.reserved_bytes(), 1000);
        assert_eq!(PLAYBACK_BUDGET.snapshot().1, 1000);
        assert_eq!(PLAYBACK_BUDGET.snapshot().0, 0);

        // Admit 400 bytes -> transferred from reserved to admitted
        lease.admit(400).unwrap();
        assert_eq!(lease.reserved_bytes(), 600);
        assert_eq!(lease.admitted_bytes(), 400);
        assert_eq!(PLAYBACK_BUDGET.snapshot().1, 600);
        assert_eq!(PLAYBACK_BUDGET.snapshot().0, 400);

        // Admit 600 bytes -> remaining reservation consumed
        lease.admit(600).unwrap();
        assert_eq!(lease.reserved_bytes(), 0);
        assert_eq!(lease.admitted_bytes(), 1000);
        assert_eq!(PLAYBACK_BUDGET.snapshot().1, 0);
        assert_eq!(PLAYBACK_BUDGET.snapshot().0, 1000);

        drop(lease);
        assert_eq!(PLAYBACK_BUDGET.snapshot().1, 0);
        assert_eq!(PLAYBACK_BUDGET.snapshot().0, 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_admit_and_reserve_cannot_bypass_limit() {
        let _guard = TEST_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "video-conc-{}",
            crate::chat::next_client_message_id()
        ));
        let root = std::sync::Arc::new(root);
        let mut handles = Vec::new();

        for i in 0..16 {
            let root = root.clone();
            handles.push(std::thread::spawn(move || {
                let name = format!("conc_{i}.mp4");
                if let Ok(lease) = CachedVideo::reserve(&root, &name) {
                    // Try reserving 40 MiB each (16 * 40 MiB = 640 MiB > 512 MiB)
                    let chunk = 40 * 1024 * 1024;
                    let _ = lease.reserve_capacity(chunk);
                    let _ = lease.admit(chunk as usize);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(PLAYBACK_BUDGET.snapshot().1, 0);
        assert_eq!(PLAYBACK_BUDGET.snapshot().0, 0);
        let _ = std::fs::remove_dir_all(&*root);
    }
}
