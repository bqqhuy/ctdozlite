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
use std::collections::{BTreeMap, HashMap, HashSet};
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

impl Saved {
    fn validated(mut self) -> Self {
        self.socket.retain(|key, (last, ids)| {
            let known = key.split_once('_').is_some_and(|(cmd, sub)| {
                cmd.parse::<u16>()
                    .is_ok_and(|cmd| MESSAGE_QUEUES.contains(&cmd))
                    && sub.parse::<u8>().is_ok()
            });
            known
                && id_number(last).is_some_and(|n| n > 1)
                && ids
                    .as_array()
                    .is_some_and(|ids| ids.len() <= 10 && ids.iter().all(valid_boundary_id))
        });
        if self.poll.as_ref().is_some_and(|poll| {
            !decimal_id(&poll.last_action_id)
                || !decimal_id(&poll.last_action_id_other)
                || poll.queues.iter().any(|(key, cursor)| {
                    key.parse::<u16>().is_err()
                        || !decimal_id(&cursor.last_id)
                        || cursor.ids.len() > 10
                        || cursor.ids.iter().any(|id| id.is_empty() || id.len() > 128)
                })
        }) {
            self.poll = None;
        }
        self
    }
}
fn decimal_id(id: &str) -> bool {
    id_number(&Value::String(id.to_owned())).is_some_and(|n| n > 0)
}
fn valid_boundary_id(id: &Value) -> bool {
    match id {
        Value::String(s) => !s.is_empty() && s.len() <= 128,
        Value::Number(n) => n.as_u64().is_some(),
        _ => false,
    }
}

struct Entry {
    saved: Saved,
    attempted: Option<Instant>,
    dirty: bool,
    // A failed page creates a gap; don't acknowledge later pages over it this process.
    blocked_socket: HashSet<String>,
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
            .and_then(|b| serde_json::from_slice::<Saved>(&b).ok())
            .map(Saved::validated)
            .unwrap_or_default(),
        attempted: None,
        dirty: false,
        blocked_socket: HashSet::new(),
    });
    Some((f(entry), path))
}

/// Write the entry when it changed and either `force` or `WRITE_EVERY` passed.
fn flush(entry: &mut Entry, path: &std::path::Path, force: bool) {
    if !entry.dirty || !(force || entry.attempted.is_none_or(|at| at.elapsed() >= WRITE_EVERY)) {
        return;
    }
    entry.attempted = Some(Instant::now());
    if let Ok(bytes) = serde_json::to_vec(&entry.saved) {
        if crate::storage::atomic_write(path, &bytes).is_ok() {
            entry.dirty = false;
        } else {
            crate::logger::log_error(
                "queue cursors",
                "Cannot save cursors; keeping pending state for retry",
            );
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
fn socket_start_inner(s: &Session, cmd: u16, sub: u8) -> Option<(Value, Value)> {
    if !MESSAGE_QUEUES.contains(&cmd) {
        return None;
    }
    with(s, |e| e.saved.socket.get(&key(cmd, sub)).cloned())?.0
}

/// Record a socket queue page as processed. Call only after its messages were stored.
fn commit_socket_inner(s: &Session, cmd: u16, sub: u8, payload: &Value, stored: bool) {
    if !MESSAGE_QUEUES.contains(&cmd) {
        return;
    }
    if !successful(payload) {
        return;
    }
    let qkey = key(cmd, sub);
    if !stored {
        let _ = with(s, |entry| {
            entry.blocked_socket.insert(qkey);
        });
        crate::logger::log_error(
            "queue cursors",
            "Message persistence failed; retaining last durable socket cursor until restart",
        );
        return;
    }
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
        record_socket(entry, &qkey, last, ids);
        done
    })
    .map(|(done, path)| {
        let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = guard.as_mut().and_then(|c| c.get_mut(&path)) {
            flush(entry, &path, done);
        }
    });
}

fn successful(payload: &Value) -> bool {
    [
        payload.get("error_code"),
        payload.pointer("/data/error_code"),
    ]
    .into_iter()
    .flatten()
    .all(|code| code.as_i64().or_else(|| code.as_str()?.parse().ok()) == Some(0))
}

fn record_socket(entry: &mut Entry, qkey: &str, last: Value, ids: Value) {
    if entry.blocked_socket.contains(qkey) {
        return;
    }
    let newer = entry
        .saved
        .socket
        .get(qkey)
        .is_none_or(
            |(saved, saved_ids)| match (id_number(&last), id_number(saved)) {
                (Some(next), Some(previous)) => {
                    next > previous || (next == previous && ids != *saved_ids)
                }
                (Some(_), None) => true,
                _ => false,
            },
        );
    if newer {
        entry.saved.socket.insert(qkey.into(), (last, ids));
        entry.dirty = true;
    }
}

/// Saved `lp_chat` cursors, when this account has any.
fn poll_start_inner(s: &Session) -> Option<crate::chat::PollState> {
    with(s, |e| e.saved.poll.clone())?.0
}

/// Record `lp_chat` cursors after the polled messages were stored.
fn commit_poll_inner(s: &Session, state: &crate::chat::PollState) {
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
fn flush_all_inner() {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    for (path, entry) in guard.iter_mut().flat_map(|c| c.iter_mut()) {
        flush(entry, path, true);
    }
}

/// Load socket progress off the Tokio reader thread.
pub async fn socket_start(s: &Session, cmd: u16, sub: u8) -> Option<(Value, Value)> {
    if !MESSAGE_QUEUES.contains(&cmd) {
        return None;
    }
    let s = s.clone();
    tokio::task::spawn_blocking(move || socket_start_inner(&s, cmd, sub))
        .await
        .ok()
        .flatten()
}

/// Commit only the small queue envelope, never clone message bodies for disk work.
pub async fn commit_socket(s: &Session, cmd: u16, sub: u8, payload: &Value, stored: bool) {
    if !MESSAGE_QUEUES.contains(&cmd) {
        return;
    }
    let mut envelope = serde_json::Map::new();
    if let Some(code) = payload.get("error_code") {
        envelope.insert("error_code".into(), code.clone());
    }
    let mut data = serde_json::Map::new();
    for field in ["error_code", "lastActionId", "more", "queueStatus"] {
        if let Some(value) = payload.get("data").and_then(|data| data.get(field)) {
            data.insert(field.into(), value.clone());
        }
    }
    envelope.insert("data".into(), Value::Object(data));
    let payload = Value::Object(envelope);
    let s = s.clone();
    if tokio::task::spawn_blocking(move || commit_socket_inner(&s, cmd, sub, &payload, stored))
        .await
        .is_err()
    {
        crate::logger::log_error("queue cursors", "Socket cursor task failed");
    }
}

/// Load persisted HTTP progress without blocking the async executor.
pub async fn poll_start(s: &Session) -> Option<crate::chat::PollState> {
    let s = s.clone();
    tokio::task::spawn_blocking(move || poll_start_inner(&s))
        .await
        .ok()
        .flatten()
}

/// Store the snapshot belonging to the page just committed to SQLite.
pub async fn commit_poll(s: &Session, state: &crate::chat::PollState) {
    let s = s.clone();
    let state = state.clone();
    if tokio::task::spawn_blocking(move || commit_poll_inner(&s, &state))
        .await
        .is_err()
    {
        crate::logger::log_error("queue cursors", "HTTP cursor task failed");
    }
}

/// Flush pending writes without performing filesystem I/O on the worker executor.
pub async fn flush_all() {
    if tokio::task::spawn_blocking(flush_all_inner).await.is_err() {
        crate::logger::log_error("queue cursors", "Cursor flush task failed");
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
            attempted: None,
            dirty: false,
            blocked_socket: HashSet::new(),
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
    fn entry() -> Entry {
        Entry {
            saved: Saved::default(),
            attempted: None,
            dirty: false,
            blocked_socket: HashSet::new(),
        }
    }
    #[test]
    fn boundary_ids_advance_even_when_last_id_is_equal() {
        let mut entry = entry();
        record_socket(&mut entry, "510_0", json!(100), json!([1]));
        entry.dirty = false;
        record_socket(&mut entry, "510_0", json!("100"), json!([1, 2]));
        assert_eq!(entry.saved.socket["510_0"].1, json!([1, 2]));
        assert!(entry.dirty);
        entry.dirty = false;
        record_socket(&mut entry, "510_0", json!(99), json!([3]));
        assert!(!entry.dirty);
        assert_eq!(entry.saved.socket["510_0"].1, json!([1, 2]));
    }
    #[test]
    fn a_failed_page_prevents_later_pages_from_acknowledging_over_the_gap() {
        let mut entry = entry();
        record_socket(&mut entry, "510_0", json!(100), json!([]));
        entry.blocked_socket.insert("510_0".into());
        record_socket(&mut entry, "510_0", json!(300), json!([]));
        assert_eq!(entry.saved.socket["510_0"].0, json!(100));
        record_socket(&mut entry, "511_0", json!(400), json!([]));
        assert_eq!(entry.saved.socket["511_0"].0, json!(400));
        let persisted = serde_json::to_vec(&entry.saved).unwrap();
        let reloaded: Saved = serde_json::from_slice(&persisted).unwrap();
        assert_eq!(reloaded.socket["510_0"].0, json!(100));
    }
    #[test]
    fn error_responses_never_confirm_a_cursor() {
        assert!(!successful(
            &json!({"error_code":"114","data":{"lastActionId":"500"}})
        ));
        assert!(!successful(
            &json!({"error_code":0,"data":{"error_code":-102}})
        ));
        assert!(successful(&json!({"data":{"lastActionId":"500"}})));
    }
    #[test]
    fn malformed_saved_cursors_do_not_poison_the_next_login() {
        let mut saved = Saved::default();
        saved
            .socket
            .insert("510_0".into(), (json!(100), json!(["99"])));
        saved
            .socket
            .insert("511_0".into(), (json!("invalid"), json!([])));
        saved.socket.insert("603_0".into(), (json!(500), json!([])));
        let mut poll = crate::chat::PollState::default();
        poll.last_action_id = "not-a-cursor".into();
        saved.poll = Some(poll);
        let valid = saved.validated();
        assert_eq!(valid.socket.len(), 1);
        assert!(valid.socket.contains_key("510_0"));
        assert!(valid.poll.is_none());
    }
}
