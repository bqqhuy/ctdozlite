//! Reusable, bounded account-owned attachment copies for opening in desktop apps.
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime},
};

const BUDGET: u64 = 512 * 1024 * 1024;
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
#[derive(Default)]
struct Registry {
    active: HashSet<PathBuf>,
    pruning: HashSet<PathBuf>,
}
static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);
struct Pruning(PathBuf);
impl Drop for Pruning {
    fn drop(&mut self) {
        if let Ok(mut state) = REGISTRY.lock() {
            if let Some(state) = state.as_mut() {
                state.pruning.remove(&self.0);
            }
        }
    }
}

/// Derive the same account-local destination used by completed attachment downloads.
pub(crate) fn path_for(root: &Path, thread: &str, message: &str, name: &str) -> PathBuf {
    // Length-delimited fields cannot collide across conversation/message boundaries.
    let key = format!("{}:{thread}{}:{message}", thread.len(), message.len());
    let digest = format!("{:x}", Sha256::digest(key.as_bytes()));
    root.join(format!(
        "{digest}-{}",
        crate::media_download::safe_name(name)
    ))
}

/// An exclusive download reservation; completed files remain reusable after release.
pub(crate) struct Entry {
    path: PathBuf,
}
impl Entry {
    /// Reserve a stable attachment destination and prune old inactive copies.
    /// Returns `None` when another request owns this attachment or eviction is in progress.
    pub(crate) fn reserve(
        root: &Path,
        thread: &str,
        message: &str,
        name: &str,
    ) -> Result<Option<Self>> {
        crate::storage::private_dir(root)?;
        ensure!(
            !std::fs::symlink_metadata(root)?.file_type().is_symlink(),
            "Thư mục cache không hợp lệ"
        );
        let path = path_for(root, thread, message, name);
        {
            let mut registry = REGISTRY
                .lock()
                .map_err(|_| anyhow::anyhow!("Cache đang bận"))?;
            let registry = registry.get_or_insert_default();
            if registry.pruning.contains(root) || !registry.active.insert(path.clone()) {
                return Ok(None);
            }
        }
        let entry = Self { path };
        if let Ok(metadata) = std::fs::symlink_metadata(entry.path()) {
            ensure!(metadata.is_file(), "Tệp cache không hợp lệ");
            if metadata.len() == 0 || metadata.len() > BUDGET {
                std::fs::remove_file(entry.path())?;
            }
        }
        entry.prune()?;
        Ok(Some(entry))
    }
    /// Destination reserved for this request only.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    /// Only nonempty regular completed files can be opened; partial files are separate.
    pub(crate) fn ready(&self) -> bool {
        std::fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.is_file() && m.len() > 0 && m.len() <= BUDGET)
    }
    /// Evict oldest inactive files, never a file being downloaded by another request.
    pub(crate) fn prune(&self) -> Result<()> {
        let root = self.path.parent().context("Cache không hợp lệ")?;
        let mut files = Vec::new();
        // Block new reservations during eviction without holding a mutex over I/O.
        let active = {
            let mut registry = REGISTRY
                .lock()
                .map_err(|_| anyhow::anyhow!("Cache đang bận"))?;
            let registry = registry.get_or_insert_default();
            if !registry.pruning.insert(root.to_owned()) {
                return Ok(());
            }
            registry.active.clone()
        };
        let _pruning = Pruning(root.to_owned());
        let now = SystemTime::now();
        let mut total = 0u64;
        for item in std::fs::read_dir(root)? {
            let item = item?;
            let metadata = item.path().symlink_metadata()?;
            if !metadata.is_file() {
                continue;
            }
            if item.path().extension().is_some_and(|ext| ext == "partial")
                && now
                    .duration_since(metadata.modified().unwrap_or(now))
                    .unwrap_or_default()
                    < Duration::from_secs(24 * 3600)
            {
                continue;
            }
            total = total.saturating_add(metadata.len());
            if active.contains(&item.path()) {
                continue;
            }
            files.push((
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                metadata.len(),
                item.path(),
            ));
        }
        files.sort_by_key(|f| f.0);
        for (modified, bytes, path) in files {
            if total > BUDGET || now.duration_since(modified).unwrap_or_default() > MAX_AGE {
                match std::fs::remove_file(path) {
                    Ok(()) => total = total.saturating_sub(bytes),
                    Err(_) => {
                        crate::logger::log_error("attachment cache", "Cannot remove cached file")
                    }
                }
            }
        }
        Ok(())
    }
}
impl Drop for Entry {
    fn drop(&mut self) {
        if let Ok(mut registry) = REGISTRY.lock() {
            if let Some(registry) = registry.as_mut() {
                registry.active.remove(&self.path);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reuse_is_stable_and_concurrent_downloads_are_coalesced() {
        let root = std::env::temp_dir().join(format!(
            "attachment-cache-{}",
            crate::chat::next_client_message_id()
        ));
        let first = Entry::reserve(&root, "thread-a", "msg-a", "report.xlsx")
            .unwrap()
            .unwrap();
        assert!(!first.ready());
        assert!(Entry::reserve(&root, "thread-a", "msg-a", "report.xlsx")
            .unwrap()
            .is_none());
        std::fs::write(first.path(), b"complete").unwrap();
        let path = first.path().to_owned();
        drop(first);
        let again = Entry::reserve(&root, "thread-a", "msg-a", "report.xlsx")
            .unwrap()
            .unwrap();
        assert!(again.ready());
        assert_eq!(again.path(), path);
        let other = Entry::reserve(&root, "thread-b", "msg-a", "report.xlsx")
            .unwrap()
            .unwrap();
        assert_ne!(again.path(), other.path());
        drop((again, other));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod eviction_tests {
    use super::*;
    #[test]
    fn pruning_never_removes_an_in_progress_partial_and_reclaims_old_copies() {
        let root = std::env::temp_dir().join(format!(
            "attachment-prune-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".ctdoz-active.partial"), b"downloading").unwrap();
        let old = std::fs::File::create(root.join("old.bin")).unwrap();
        old.set_len(BUDGET + 1).unwrap();
        drop(old);
        let entry = Entry::reserve(&root, "thread", "message", "report.xlsx")
            .unwrap()
            .unwrap();
        assert!(root.join(".ctdoz-active.partial").exists());
        assert!(!root.join("old.bin").exists());
        drop(entry);
        std::fs::remove_dir_all(root).unwrap();
    }
}
