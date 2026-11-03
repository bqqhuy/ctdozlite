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

pub async fn download<F: FnMut(String), G: FnMut(Thread)>(
    session: &Session,
    threads: Vec<Thread>,
    canceled: Arc<AtomicBool>,
    mut progress: F,
    mut page_ready: G,
) -> Result<String> {
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
    let mut failed = 0;
    let mut filtered = 0;
    for (index, mut thread) in threads
        .into_iter()
        .filter(|t| t.is_group || (!cloud.is_empty() && t.id == cloud))
        .enumerate()
    {
        thread.messages.clear();
        let checkpoint: Option<String> = store
            .conn
            .query_row(
                "SELECT cursor FROM history_sync WHERE thread_id=?1",
                [&thread.id],
                |row| row.get(0),
            )
            .ok();
        // Legacy cursors were derived from individual rows and cannot be trusted.
        let saved = checkpoint.as_deref().and_then(|s| s.split_once(':'));
        let mut cursor = saved
            .map(|(_, id)| id.to_owned())
            .unwrap_or_else(|| NEWEST_CURSOR.into());
        let mut old = saved.is_some_and(|(kind, _)| kind == "old");
        for page in 0..200 {
            if canceled.load(Ordering::Relaxed) || !session.is_active() {
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
                if let Some(next) = &metadata.cursor {
                    anyhow::ensure!(next.parse::<u64>()? < cursor.parse::<u64>()?,
                        "Máy chủ không trả trang cũ hơn; đã dừng để tránh lặp");
                }
                thread.messages.clear();
                crate::chat::merge_group_history(&mut thread, &response);
                inserted += crate::store::persist_threads(&mut store, std::slice::from_ref(&thread))?;
                // Persist messages before advancing. An interrupted checkpoint can
                // replay one page safely because message IDs are unique in SQLite.
                if let Some(next) = &metadata.cursor {
                    let checkpoint = format!("{}:{next}", if metadata.old { "old" } else { "recent" });
                    store.conn.execute("INSERT INTO history_sync(thread_id,cursor) VALUES (?1,?2) ON CONFLICT(thread_id) DO UPDATE SET cursor=excluded.cursor",
                        rusqlite::params![thread.id, checkpoint])?;
                }
                store.filter_deleted(std::slice::from_mut(&mut thread))?;
                page_ready(thread.clone());
                if metadata.filtered {
                    filtered += 1;
                    progress(format!("{}: máy chủ giới hạn lịch sử cho phiên này; dữ liệu cũ chưa được tải đầy đủ", thread.name));
                }
                Ok::<_,anyhow::Error>(metadata.cursor.map(|next| (next, metadata.old)))
            }.await;
            match result {
                Ok(Some((next, next_old))) => {
                    cursor = next;
                    old = next_old;
                }
                Ok(None) => break,
                Err(e) => {
                    if e.to_string().contains("429") {
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
    Ok(format!("Đã lưu {inserted} tin mới; {failed} hội thoại lỗi; {filtered} hội thoại bị server giới hạn; {skipped} chat cá nhân chỉ có lịch sử cục bộ. Chỉ tải được dữ liệu server còn lưu."))
}
#[cfg(test)]
mod tests {
    use super::*;
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
