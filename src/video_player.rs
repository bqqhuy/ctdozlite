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
#[cfg(windows)]
#[path = "video_player_windows.rs"]
mod native;

/// An app-owned temporary video file; active viewers are excluded from cleanup.
pub(crate) struct CachedVideo(std::path::PathBuf, std::sync::atomic::AtomicU64);
static CACHE_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ACTIVE_CACHE: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());
impl CachedVideo {
    /// Remove stale private video files and reserve the new playback path.
    /// # Errors
    /// Returns private directory, cache enumeration or lock failures.
    pub(crate) fn reserve(root: &Path, name: &str) -> Result<Self> {
        crate::storage::private_dir(root)?;
        let path = root.join(name);
        ACTIVE_CACHE
            .lock()
            .map_err(|_| anyhow::anyhow!("Video cache unavailable"))?
            .push(path.clone());
        let lease = Self(path, std::sync::atomic::AtomicU64::new(0));
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;
            let in_use = ACTIVE_CACHE
                .lock()
                .map_err(|_| anyhow::anyhow!("Video cache unavailable"))?
                .contains(&path);
            if !in_use && metadata.is_file() && !entry.file_type()?.is_symlink() {
                // Files in this directory are disposable playback copies, never user saves.
                if std::fs::remove_file(path).is_err() {
                    crate::logger::log_error("video cache", "Cannot remove stale playback file");
                }
            }
        }
        Ok(lease)
    }
    /// Admit bytes against the shared 512 MiB playback budget before writing.
    pub(crate) fn admit(&self, bytes: usize) -> Result<()> {
        use std::sync::atomic::Ordering;
        let bytes = bytes as u64;
        CACHE_BYTES
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                n.checked_add(bytes).filter(|n| *n <= 512 * 1024 * 1024)
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
}
impl Drop for CachedVideo {
    fn drop(&mut self) {
        CACHE_BYTES.fetch_sub(
            self.1.load(std::sync::atomic::Ordering::Acquire),
            std::sync::atomic::Ordering::AcqRel,
        );
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
    #[test]
    fn active_videos_survive_pruning_and_budget_is_released_on_close() {
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
}
