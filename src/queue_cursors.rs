//! Queue cursors kept across app starts, per account.
//!
//! Zalo PC keeps `lastActionId`, `lastActionIdOther` and each queue's `lastId` per user
//! (`KEY_ACTION`, `KEY_ACTION_OTHER`, `KEY_MSG`), reads them in `init` and sends them on the
//! first poll and on every socket pull (`getLastActionIdsForSocket`, `lastId: "1"` only for a
//! queue it has never seen). It saves them only after the page was processed. Without this
//! the client asked every queue from the beginning on each start, re-downloading pages that
//! were all already stored.
//!
//! A cursor is committed only after the page's messages were written to SQLite, so a crash
//! can only replay a page (deduplicated), never skip one. A missing or damaged file means
//! "start from the beginning", as before.
use crate::session::Session;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Socket message queues whose cursors are kept (control queues are left as they were).
pub const MESSAGE_QUEUES: [u16; 7] = [510, 511, 513, 514, 515, 517, 518];
/// Unchanged cursors are written at most this often while pages stream in.
const WRITE_EVERY: Duration = Duration::from_secs(1);
const FILE_LIMIT: usize = 256 * 1024;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
struct Saved {
    /// `"<cmd>_<subCmd>"` → `(lastId, ids)` of the last processed page.
    #[serde(default)]
    socket: BTreeMap<String, (Value, Value)>,
    /// `lp_chat` cursors (`PollState` without its runtime guard).
    #[serde(default)]
    poll: Option<crate::chat::PollState>,
}

struct Entry {
    saved: Saved,
    written: Option<Instant>,
    dirty: bool,
}

static CACHE: Mutex<Option<HashMap<PathBuf, Entry>>> = Mutex::new(None);

fn path(s: &Session) -> Option<PathBuf> {
    (!s.account_key().is_empty() && s.is_active())
        .then(|| s.history_path().with_extension("queue-cursors.json"))
}

fn with<R>(s: &Session, f: impl FnOnce(&mut Entry) -> R) -> Option<(R, PathBuf)> {
    let path = path(s)?;
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    let entry = cache.entry(path.clone()).or_insert_with(|| Entry {
        saved: crate::limits::read_file(&path, FILE_LIMIT)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default(),
        written: None,
        dirty: false,
    });
    Some((f(entry), path))
}

/// Write the entry when it changed and either `force` or `WRITE_EVERY` passed.
fn flush(entry: &mut Entry, path: &std::path::Path, force: bool) {
    if !entry.dirty || !(force || entry.written.is_none_or(|at| at.elapsed() >= WRITE_EVERY)) {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(&entry.saved) {
        if crate::storage::atomic_write(path, &bytes).is_ok() {
            entry.dirty = false;
            entry.written = Some(Instant::now());
        }
    }
}

fn key(cmd: u16, sub: u8) -> String {
    format!("{cmd}_{sub}")
}

/// Numeric ordering of Zalo IDs given as numbers or decimal strings.
fn id_number(v: &Value) -> Option<u128> {
    match v {
        Value::Number(n) => n.as_u64().map(u128::from),
        Value::String(s)
            if !s.is_empty() && s.len() <= 38 && s.bytes().all(|b| b.is_ascii_digit()) =>
        {
            s.parse().ok()
        }
        _ => None,
    }
}

/// Where a socket queue should resume: the saved `(lastId, preIds)`, if any.
pub fn socket_start(s: &Session, cmd: u16, sub: u8) -> Option<(Value, Value)> {
    if !MESSAGE_QUEUES.contains(&cmd) {
        return None;
    }
    with(s, |e| e.saved.socket.get(&key(cmd, sub)).cloned())?.0
}

/// Record a socket queue page as processed. Call only after its messages were stored.
pub fn commit_socket(s: &Session, cmd: u16, sub: u8, payload: &Value) {
    if !MESSAGE_QUEUES.contains(&cmd) {
        return;
    }
    let qkey = key(cmd, sub);
    let last = payload
        .pointer("/data/lastActionId")
        .or_else(|| payload.pointer(&format!("/data/queueStatus/{qkey}/lastId")))
        .cloned();
    let Some(last) = last.filter(|v| id_number(v).is_some_and(|n| n > 1)) else {
        return;
    };
    let ids = payload
        .pointer(&format!("/data/queueStatus/{qkey}/ids"))
        .and_then(Value::as_array)
        .map(|a| Value::Array(a.iter().take(10).cloned().collect()))
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let done = payload.pointer("/data/more").and_then(Value::as_i64) != Some(1);
    let _ = with(s, |entry| {
        let newer = entry
            .saved
            .socket
            .get(&qkey)
            .and_then(|(saved, _)| id_number(saved))
            .is_none_or(|saved| id_number(&last).is_some_and(|n| n > saved));
        if newer {
            entry
                .saved
                .socket
                .insert(qkey.clone(), (last.clone(), ids.clone()));
            entry.dirty = true;
        }
        done
    })
    .map(|(done, path)| {
        let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = guard.as_mut().and_then(|c| c.get_mut(&path)) {
            flush(entry, &path, done);
        }
    });
}

/// Saved `lp_chat` cursors, when this account has any.
pub fn poll_start(s: &Session) -> Option<crate::chat::PollState> {
    with(s, |e| e.saved.poll.clone())?.0
}

/// Record `lp_chat` cursors after the polled messages were stored.
pub fn commit_poll(s: &Session, state: &crate::chat::PollState) {
    let snapshot = state.clone();
    if let Some((_, path)) = with(s, |entry| {
        if entry.saved.poll.as_ref() != Some(&snapshot) {
            entry.saved.poll = Some(snapshot);
            entry.dirty = true;
        }
    }) {
        let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = guard.as_mut().and_then(|c| c.get_mut(&path)) {
            flush(entry, &path, false);
        }
    }
}

/// Write any pending cursors now (logout, shutdown).
pub fn flush_all() {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    for (path, entry) in guard.iter_mut().flat_map(|c| c.iter_mut()) {
        flush(entry, path, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn ids_compare_numerically_and_reject_junk() {
        assert_eq!(id_number(&json!("8399086511016")), Some(8_399_086_511_016));
        assert_eq!(id_number(&json!(12)), Some(12));
        assert_eq!(id_number(&json!("12a")), None);
        assert_eq!(id_number(&json!(null)), None);
    }
    #[test]
    fn flush_writes_only_changes_and_respects_the_interval() {
        let root = std::env::temp_dir().join(format!(
            "queue-cursors-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("a.queue-cursors.json");
        let mut entry = Entry {
            saved: Saved::default(),
            written: None,
            dirty: false,
        };
        flush(&mut entry, &path, true);
        assert!(!path.exists(), "nothing changed");
        entry
            .saved
            .socket
            .insert("510_0".into(), (json!("100"), json!([])));
        entry.dirty = true;
        flush(&mut entry, &path, false);
        let saved: Saved = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.socket["510_0"].0, json!("100"));
        entry
            .saved
            .socket
            .insert("510_0".into(), (json!("200"), json!([])));
        entry.dirty = true;
        flush(&mut entry, &path, false);
        let saved: Saved = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.socket["510_0"].0, json!("100"), "throttled");
        flush(&mut entry, &path, true);
        let saved: Saved = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.socket["510_0"].0, json!("200"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
