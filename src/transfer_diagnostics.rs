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
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Started,
    Finished,
    Failed,
    Cancelled,
}
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Event {
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
fn control_event(id: &str, control: PhoneControl) -> Event {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(id.as_bytes());
    let mut tag = [0; 8];
    tag.copy_from_slice(&digest[..8]);
    Event::PhoneControl {
        session_tag: u64::from_le_bytes(tag),
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
                    // Diagnostics are best effort and may never fail a chat/transfer operation.
                    let _ = append(&root, &event);
                }
            })
            .ok()
            .map(|_| tx)
    }) {
        let _ = sender.try_send(event);
    }
}
#[cfg(test)]
fn record(_event: Event) {}

fn append(root: &Path, event: &Event) -> std::io::Result<()> {
    use std::io::Write;
    let path = root.join("sync-media.jsonl");
    let previous = root.join("sync-media.previous.jsonl");
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink())
        || std::fs::symlink_metadata(&previous).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(std::io::Error::other("Diagnostic path is a symlink"));
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= 1024 * 1024) {
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
            },
        );
        let text = serde_json::to_string(&event).unwrap();
        assert!(!text.contains(id));
        assert!(!text.contains("url"));
        assert!(!text.contains("key"));
        assert!(text.contains("session_tag"));
        let root = std::env::temp_dir().join(format!(
            "safe-diagnostics-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("sync-media.jsonl"), vec![b' '; 1024 * 1024]).unwrap();
        append(&root, &event).unwrap();
        assert!(root.join("sync-media.previous.jsonl").exists());
        let current = std::fs::read_to_string(root.join("sync-media.jsonl")).unwrap();
        assert_eq!(current.lines().count(), 1);
        assert!(!current.contains(id));
        std::fs::remove_dir_all(root).unwrap();
    }
}
