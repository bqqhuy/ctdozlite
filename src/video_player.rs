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
pub static CACHE_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static PENDING_RESERVATIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
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

    /// Reserve an upfront capacity budget before starting to download.
    /// Atomically ensures CACHE_BYTES + PENDING_RESERVATIONS + bytes <= MAX_CACHE_BYTES.
    pub(crate) fn reserve_capacity(&self, bytes: u64) -> Result<()> {
        use std::sync::atomic::Ordering;
        if bytes == 0 {
            return Ok(());
        }
        PENDING_RESERVATIONS
            .try_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                let current = CACHE_BYTES.load(Ordering::Acquire);
                let total = current.checked_add(pending)?.checked_add(bytes)?;
                if total <= MAX_CACHE_BYTES {
                    Some(pending + bytes)
                } else {
                    None
                }
            })
            .map_err(|_| {
                anyhow::anyhow!(
                    "Cache video đã đạt 512 MiB; hãy đóng viewer khác hoặc lưu tệp về máy"
                )
            })?;
        self.2.fetch_add(bytes, Ordering::AcqRel);
        Ok(())
    }

    /// Admit bytes against the shared 512 MiB playback budget before writing.
    /// If this lease has reserved capacity, transfers bytes from pending to admitted.
    /// Otherwise, atomically checks that CACHE_BYTES + PENDING_RESERVATIONS + bytes <= MAX_CACHE_BYTES.
    pub(crate) fn admit(&self, bytes: usize) -> Result<()> {
        use std::sync::atomic::Ordering;
        let bytes = bytes as u64;
        if bytes == 0 {
            return Ok(());
        }

        // Deduct from this lease's pending reservation if available
        let mut reserved = self.2.load(Ordering::Acquire);
        while reserved > 0 {
            let to_transfer = reserved.min(bytes);
            match self.2.compare_exchange_weak(
                reserved,
                reserved - to_transfer,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    PENDING_RESERVATIONS.fetch_sub(to_transfer, Ordering::AcqRel);
                    CACHE_BYTES.fetch_add(to_transfer, Ordering::AcqRel);
                    self.1.fetch_add(to_transfer, Ordering::AcqRel);
                    let remainder = bytes - to_transfer;
                    if remainder == 0 {
                        return Ok(());
                    }
                    return self.admit_unreserved(remainder);
                }
                Err(actual) => reserved = actual,
            }
        }

        self.admit_unreserved(bytes)
    }

    fn admit_unreserved(&self, bytes: u64) -> Result<()> {
        use std::sync::atomic::Ordering;
        CACHE_BYTES
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                let pending = PENDING_RESERVATIONS.load(Ordering::Acquire);
                let total = n.checked_add(bytes)?.checked_add(pending)?;
                if total <= MAX_CACHE_BYTES {
                    Some(n + bytes)
                } else {
                    None
                }
            })
            .map_err(|_| {
                anyhow::anyhow!(
                    "Cache video đã đạt 512 MiB; hãy đóng viewer khác hoặc lưu tệp về máy"
                )
            })?;
        self.1.fetch_add(bytes, Ordering::AcqRel);
        Ok(())
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
        use std::sync::atomic::Ordering;
        CACHE_BYTES.fetch_sub(self.1.load(Ordering::Acquire), Ordering::AcqRel);
        let remaining_reserved = self.2.load(Ordering::Acquire);
        if remaining_reserved > 0 {
            PENDING_RESERVATIONS.fetch_sub(remaining_reserved, Ordering::AcqRel);
        }
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
        assert_eq!(CACHE_BYTES.load(std::sync::atomic::Ordering::Acquire), 0);
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
        assert_eq!(
            PENDING_RESERVATIONS.load(std::sync::atomic::Ordering::Acquire),
            300 * 1024 * 1024
        );

        // 300MB + 250MB = 550MB > 512MB MAX_CACHE_BYTES -> must fail
        assert!(second.reserve_capacity(250 * 1024 * 1024).is_err());
        // Unreserved admit must also be blocked by pending reservation
        assert!(second.admit(250 * 1024 * 1024).is_err());

        // Drop first -> pending reservation released
        drop(first);
        assert_eq!(
            PENDING_RESERVATIONS.load(std::sync::atomic::Ordering::Acquire),
            0
        );

        // Now second can reserve
        second.reserve_capacity(250 * 1024 * 1024).unwrap();
        drop(second);
        assert_eq!(
            PENDING_RESERVATIONS.load(std::sync::atomic::Ordering::Acquire),
            0
        );
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
        assert_eq!(
            PENDING_RESERVATIONS.load(std::sync::atomic::Ordering::Acquire),
            1000
        );
        assert_eq!(CACHE_BYTES.load(std::sync::atomic::Ordering::Acquire), 0);

        // Admit 400 bytes -> transferred from reserved to admitted
        lease.admit(400).unwrap();
        assert_eq!(lease.reserved_bytes(), 600);
        assert_eq!(lease.admitted_bytes(), 400);
        assert_eq!(
            PENDING_RESERVATIONS.load(std::sync::atomic::Ordering::Acquire),
            600
        );
        assert_eq!(CACHE_BYTES.load(std::sync::atomic::Ordering::Acquire), 400);

        // Admit 600 bytes -> remaining reservation consumed
        lease.admit(600).unwrap();
        assert_eq!(lease.reserved_bytes(), 0);
        assert_eq!(lease.admitted_bytes(), 1000);
        assert_eq!(
            PENDING_RESERVATIONS.load(std::sync::atomic::Ordering::Acquire),
            0
        );
        assert_eq!(CACHE_BYTES.load(std::sync::atomic::Ordering::Acquire), 1000);

        drop(lease);
        assert_eq!(
            PENDING_RESERVATIONS.load(std::sync::atomic::Ordering::Acquire),
            0
        );
        assert_eq!(CACHE_BYTES.load(std::sync::atomic::Ordering::Acquire), 0);
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

        assert_eq!(
            PENDING_RESERVATIONS.load(std::sync::atomic::Ordering::Acquire),
            0
        );
        assert_eq!(CACHE_BYTES.load(std::sync::atomic::Ordering::Acquire), 0);
        let _ = std::fs::remove_dir_all(&*root);
    }
}
