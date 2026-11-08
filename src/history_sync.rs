//! Bounded, resumable history download for the verified group/Cloud API.
use crate::{chat::Thread, session::Session, store::MessageStore};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// Newest-page sentinel used by Zalo Web's CM history reader.
pub const NEWEST_CURSOR: &str = "9223372036854775808";

/// Server pagination metadata; message IDs are not pagination cursors.
#[derive(Debug, PartialEq, Eq)]
pub struct Page {
    pub cursor: Option<String>,
    pub more: bool,
    pub old: bool,
    pub filtered: bool,
}

/// Decode the CM envelope, retaining server filtering and endpoint selection.
///
/// Evidence: Zalo Web CM loop reproduced in ardennguyen/zalo-agent-cli,
/// f867f0e, src/core/group-history.js. No phone-only history is inferred.
/// # Errors
/// Rejects malformed envelopes and non-decimal/non-positive continuation cursors.
pub fn page_metadata(response: &Value) -> Result<Page> {
    let mut data = response.get("data").unwrap_or(response).clone();
    for _ in 0..3 {
        if let Some(text) = data.as_str() {
            data = serde_json::from_str(text)?;
        } else if let Some(inner) = data.get("data") {
            data = inner.clone();
        } else {
            break;
        }
    }
    anyhow::ensure!(data.is_object(), "Phản hồi lịch sử không hợp lệ");
    let flag = |name: &str| {
        data.get(name)
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
            .unwrap_or(0)
            > 0
    };
    let filtered = flag("isFiltered");
    let more = flag("hasMore");
    let cursor = if more && !filtered {
        let id = data
            .get("lastMsgId")
            .context("Thiếu lastMsgId để tải trang tiếp theo")?;
        let text = match id {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => anyhow::bail!("lastMsgId không hợp lệ"),
        };
        anyhow::ensure!(
            text.bytes().all(|c| c.is_ascii_digit()) && text.parse::<u64>().unwrap_or(0) > 0,
            "lastMsgId không hợp lệ"
        );
        Some(text)
    } else {
        None
    };
    Ok(Page {
        cursor,
        more,
        old: flag("isOld"),
        filtered,
    })
}

/// How long a conversation the server answered with `isFiltered` is left out.
const FILTERED_KEEP_MS: i64 = 24 * 60 * 60 * 1000;
const FILTERED_LIMIT: usize = 1024 * 1024;

/// Conversations the server recently limited (`isFiltered`) for one login session.
///
/// This only avoids repeating calls that just returned nothing; it never marks history
/// complete. A different login session, expiry or a manual retry ignores it, and a missing
/// or damaged file reads as empty.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FilteredMemo {
    /// Irreversible tag of the login session that observed the limits.
    session: u64,
    until: std::collections::HashMap<String, i64>,
}
impl FilteredMemo {
    pub fn load(path: &std::path::Path, session: u64) -> Self {
        crate::limits::read_file(path, FILTERED_LIMIT)
            .ok()
            .and_then(|b| serde_json::from_slice::<Self>(&b).ok())
            .filter(|memo| memo.session == session)
            .unwrap_or(Self {
                session,
                until: Default::default(),
            })
    }
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        crate::storage::atomic_write(path, &serde_json::to_vec(self)?)
    }
    pub fn holds(&self, thread: &str, now: i64) -> bool {
        self.until.get(thread).is_some_and(|until| *until > now)
    }
    pub fn remember(&mut self, thread: &str, now: i64) {
        self.until
            .insert(thread.to_owned(), now.saturating_add(FILTERED_KEEP_MS));
    }
    pub fn forget(&mut self, thread: &str) -> bool {
        self.until.remove(thread).is_some()
    }
    fn prune(&mut self, now: i64) {
        self.until.retain(|_, until| *until > now);
    }
}
/// Login-session tag for `FilteredMemo`; the key itself is never stored.
fn session_tag(session: &Session) -> u64 {
    let key = session.session_key_bytes();
    let mut identity = session.account_key().as_bytes().to_vec();
    identity.push(0);
    identity.extend_from_slice(key);
    crate::transfer_diagnostics::tag(&String::from_utf8_lossy(&identity))
}
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Backfill position and the newest message observed in that server traversal.
/// Message identity is a stopping boundary, never a pagination cursor.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct HistoryCheckpoint {
    next: Option<String>,
    old: bool,
    head: String,
    stop: String,
    complete: bool,
}
impl HistoryCheckpoint {
    fn decode(raw: Option<&str>) -> Self {
        let valid_cursor = |id: &str| {
            !id.is_empty()
                && id.bytes().all(|b| b.is_ascii_digit())
                && id.parse::<u64>().is_ok_and(|id| id > 0)
        };
        if let Some(raw) = raw {
            if let Ok(saved) = serde_json::from_str::<Self>(raw) {
                if saved.next.as_deref().is_none_or(valid_cursor) {
                    return saved;
                }
            }
            if let Some((kind, id)) = raw.split_once(':') {
                if matches!(kind, "old" | "recent") && valid_cursor(id) {
                    return Self {
                        next: Some(id.into()),
                        old: kind == "old",
                        ..Self::default()
                    };
                }
            }
        }
        Self::default()
    }
    fn advance(&self, page: &Page, messages: &[crate::chat::ChatMessage]) -> Option<Self> {
        if page.filtered {
            return None;
        }
        let stop = if self.complete {
            self.head.clone()
        } else {
            self.stop.clone()
        };
        let reached = !stop.is_empty() && messages.iter().any(|m| m.id == stop);
        let mut head = if self.complete {
            String::new()
        } else {
            self.head.clone()
        };
        if head.is_empty() {
            head = messages
                .iter()
                .max_by_key(|m| (m.ts, m.id.parse::<u64>().unwrap_or(0)))
                .map(|m| m.id.clone())
                .unwrap_or_default();
        }
        let next = if reached { None } else { page.cursor.clone() };
        Some(Self {
            complete: next.is_none(),
            stop: if next.is_some() { stop } else { String::new() },
            next,
            old: page.old,
            head,
        })
    }
    fn start(&self) -> (String, bool) {
        if self.complete {
            (NEWEST_CURSOR.into(), false)
        } else {
            (
                self.next.clone().unwrap_or_else(|| NEWEST_CURSOR.into()),
                self.old,
            )
        }
    }
}

/// Download the group and Cloud history the server still provides.
/// `retry_filtered` ignores the memo of conversations the server recently limited.
pub async fn download<F: FnMut(String), G: FnMut(Thread)>(
    session: &Session,
    threads: Vec<Thread>,
    canceled: Arc<AtomicBool>,
    retry_filtered: bool,
    mut progress: F,
    mut page_ready: G,
) -> Result<String> {
    let memo_path = session
        .history_path()
        .with_extension("server-filtered.json");
    let now = now_ms();
    let mut memo = if retry_filtered {
        FilteredMemo {
            session: session_tag(session),
            until: Default::default(),
        }
    } else {
        FilteredMemo::load(&memo_path, session_tag(session))
    };
    memo.prune(now);
    let mut memo_changed = retry_filtered;
    let mut remembered = 0;
    let cloud = session
        .login
        .as_ref()
        .map(|l| l.send2me_id.as_str())
        .unwrap_or("");
    let total = threads
        .iter()
        .filter(|t| t.is_group || (!cloud.is_empty() && t.id == cloud))
        .count();
    let skipped = threads.len() - total;
    let mut store = MessageStore::open(session.history_path())?;
    let mut inserted = 0;
    let mut downloaded_pages = 0;
    let mut failed = 0;
    let mut filtered = 0;
    for (index, mut thread) in threads
        .into_iter()
        .filter(|t| t.is_group || (!cloud.is_empty() && t.id == cloud))
        .enumerate()
    {
        if memo.holds(&thread.id, now) {
            remembered += 1;
            continue;
        }
        thread.messages.clear();
        let checkpoint: Option<String> = store
            .conn
            .query_row(
                "SELECT cursor FROM history_sync WHERE thread_id=?1",
                [&thread.id],
                |row| row.get(0),
            )
            .ok();
        let mut checkpoint = HistoryCheckpoint::decode(checkpoint.as_deref());
        let (mut cursor, mut old) = checkpoint.start();
        for page in 0..200 {
            if canceled.load(Ordering::Relaxed) || !session.is_active() {
                if memo_changed && session.is_active() {
                    let _ = memo.save(&memo_path);
                }
                return Ok(format!(
                    "Đã dừng; lưu {inserted} tin mới và vị trí tiếp tục"
                ));
            }
            progress(format!(
                "Lịch sử {}/{total}: {} — trang {}, {inserted} tin mới",
                index + 1,
                thread.name,
                page + 1
            ));
            let result = async {
                let response = crate::chat::get_cloud_messages(session, &thread.id,
                    &cursor, old, 50).await?;
                if let Some(code) = response.get("error_code").and_then(Value::as_i64).filter(|c| *c != 0) {
                    anyhow::bail!("server error {code}");
                }
                let metadata = page_metadata(&response)?;
                downloaded_pages += 1;
                if let Some(next) = &metadata.cursor {
                    anyhow::ensure!(next.parse::<u64>()? < cursor.parse::<u64>()?,
                        "Máy chủ không trả trang cũ hơn; đã dừng để tránh lặp");
                }
                thread.messages.clear();
                crate::chat::merge_group_history_with_profile(&mut thread, &response, Some(session.account_key()));
                inserted += crate::store::persist_threads(&mut store, std::slice::from_ref(&thread))?;
                let updated = checkpoint.advance(&metadata, &thread.messages);
                let next = updated.as_ref().and_then(|state| state.next.clone());
                // Persist data before progress. Completed history only needs the new prefix;
                // unfinished history keeps both its cursor and stopping boundary on restart.
                if let Some(updated) = updated {
                    store.conn.execute("INSERT INTO history_sync(thread_id,cursor) VALUES (?1,?2) ON CONFLICT(thread_id) DO UPDATE SET cursor=excluded.cursor",
                        rusqlite::params![thread.id, serde_json::to_string(&updated)?])?;
                    checkpoint = updated;
                }
                if !metadata.filtered && memo.forget(&thread.id) {
                    memo_changed = true;
                }
                store.filter_deleted(std::slice::from_mut(&mut thread))?;
                page_ready(thread.clone());
                if metadata.filtered {
                    filtered += 1;
                    memo.remember(&thread.id, now);
                    memo_changed = true;
                    progress(format!("{}: máy chủ giới hạn lịch sử cho phiên này; dữ liệu cũ chưa được tải đầy đủ", thread.name));
                }
                Ok::<_,anyhow::Error>(next.map(|next| (next, metadata.old)))
            }.await;
            match result {
                Ok(Some((next, next_old))) => {
                    cursor = next;
                    old = next_old;
                }
                Ok(None) => break,
                Err(e) => {
                    if e.to_string().contains("429") {
                        if memo_changed {
                            let _ = memo.save(&memo_path);
                        }
                        return Ok(format!("Máy chủ giới hạn tốc độ; đã lưu {inserted} tin mới. Thử đồng bộ lại sau; vị trí tiếp tục được giữ."));
                    }
                    failed += 1;
                    progress(format!(
                        "{}: {e}; giữ dữ liệu và vị trí tiếp tục",
                        thread.name
                    ));
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
            if page == 199 {
                progress(format!(
                    "{}: đạt giới hạn 200 trang/lượt, bấm tiếp tục để tải thêm",
                    thread.name
                ));
            }
        }
    }
    if memo_changed && session.is_active() {
        // Best effort: losing the memo only means asking the server again next time.
        let _ = memo.save(&memo_path);
    }
    let remembered = if remembered > 0 {
        format!(" {remembered} hội thoại vừa bị giới hạn trong 24 giờ qua được bỏ qua (bấm \"Thử lại hội thoại bị giới hạn\" để hỏi lại).")
    } else {
        String::new()
    };
    Ok(format!("Đã kiểm tra {downloaded_pages} trang, lưu {inserted} tin mới; {failed} hội thoại lỗi; {filtered} hội thoại bị máy chủ giới hạn lịch sử cho thiết bị này; {skipped} chat cá nhân không có lịch sử trên máy chủ (dùng Đồng bộ từ điện thoại).{remembered} Đây là dữ liệu máy chủ còn cung cấp, chưa chắc là toàn bộ lịch sử."))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str) -> crate::chat::ChatMessage {
        crate::chat::ChatMessage {
            id: id.into(),
            ts: id.parse().unwrap(),
            ..Default::default()
        }
    }
    fn history_page(cursor: Option<&str>, filtered: bool) -> Page {
        Page {
            cursor: cursor.map(str::to_owned),
            more: cursor.is_some(),
            old: false,
            filtered,
        }
    }
    #[test]
    fn completed_history_checks_only_new_prefix_after_restart() {
        let first = HistoryCheckpoint::default()
            .advance(
                &history_page(Some("900"), false),
                &[message("100"), message("99")],
            )
            .unwrap();
        let complete = first
            .advance(&history_page(None, false), &[message("1")])
            .unwrap();
        let persisted = serde_json::to_string(&complete).unwrap();
        let restored = HistoryCheckpoint::decode(Some(&persisted));
        assert!(restored.complete);
        assert_eq!(restored.start(), (NEWEST_CURSOR.into(), false));
        // The server still has older pages, but they were already traversed successfully.
        let next = restored
            .advance(
                &history_page(Some("800"), false),
                &[message("101"), message("100")],
            )
            .unwrap();
        assert!(next.complete && next.next.is_none());
        assert_eq!(next.head, "101");
    }
    #[test]
    fn interrupted_update_keeps_its_cursor_and_prior_stopping_boundary() {
        let completed = HistoryCheckpoint {
            complete: true,
            head: "100".into(),
            ..Default::default()
        };
        let partial = completed
            .advance(&history_page(Some("500"), false), &[message("200")])
            .unwrap();
        let restored = HistoryCheckpoint::decode(Some(&serde_json::to_string(&partial).unwrap()));
        assert_eq!(restored.start(), ("500".into(), false));
        assert_eq!(restored.stop, "100");
        let next = restored
            .advance(&history_page(Some("400"), false), &[message("100")])
            .unwrap();
        assert!(next.complete && next.next.is_none());
        assert!(next.stop.is_empty());
        assert_eq!(next.head, "200");
        assert!(completed.advance(&history_page(None, true), &[]).is_none());
    }
    #[test]
    fn legacy_cursors_resume_without_claiming_complete_history() {
        let saved = HistoryCheckpoint::decode(Some("old:123"));
        assert_eq!(saved.start(), ("123".into(), true));
        assert!(!saved.complete);
        for raw in ["bad:123", "recent:0", "old:bad", "not-json"] {
            assert_eq!(
                HistoryCheckpoint::decode(Some(raw)).start().0,
                NEWEST_CURSOR
            );
        }
    }
    #[test]
    fn filtered_memo_is_per_session_expires_and_survives_damage() {
        let root = std::env::temp_dir().join(format!(
            "filtered-memo-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("a.server-filtered.json");
        std::fs::write(&path, b"[broken").unwrap();
        assert!(FilteredMemo::load(&path, 1).until.is_empty());
        let mut memo = FilteredMemo::load(&path, 1);
        memo.remember("g1", 1_000);
        memo.save(&path).unwrap();
        let loaded = FilteredMemo::load(&path, 1);
        assert!(loaded.holds("g1", 1_000 + FILTERED_KEEP_MS - 1));
        assert!(!loaded.holds("g1", 1_000 + FILTERED_KEEP_MS));
        // Another login session never reuses the memo.
        assert!(!FilteredMemo::load(&path, 2).holds("g1", 1_001));
        let mut loaded = loaded;
        assert!(loaded.forget("g1"));
        assert!(!loaded.holds("g1", 1_001));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn follows_server_cursor_instead_of_message_ids() {
        let page = page_metadata(&serde_json::json!({"data":{"groupMsgs":[{"msgId":"1"}], "lastMsgId":"9007199254740993", "hasMore":1,"isOld":0}})).unwrap();
        assert_eq!(page.cursor.as_deref(), Some("9007199254740993"));
        assert!(!page.old);
        assert!(page.more);
    }
    #[test]
    fn stops_filtered_pages_even_when_server_reports_more() {
        let page = page_metadata(
            &serde_json::json!({"data":"{\"groupMsgs\":[],\"hasMore\":1,\"isFiltered\":1}"}),
        )
        .unwrap();
        assert!(page.filtered);
        assert!(page.cursor.is_none());
    }
    #[test]
    fn old_endpoint_and_terminal_pages_follow_response_flags() {
        let page =
            page_metadata(&serde_json::json!({"data":{"hasMore":1,"isOld":1,"lastMsgId":123}}))
                .unwrap();
        assert!(page.old);
        assert_eq!(page.cursor.as_deref(), Some("123"));
        assert!(page_metadata(&serde_json::json!({"data":{"hasMore":1}})).is_err());
        assert!(page_metadata(&serde_json::json!({"data":{"hasMore":1,"lastMsgId":"0"}})).is_err());
        assert!(page_metadata(&serde_json::json!({"data":{"hasMore":0}}))
            .unwrap()
            .cursor
            .is_none());
        assert!(page_metadata(&serde_json::json!({"data":null})).is_err());
    }
}
