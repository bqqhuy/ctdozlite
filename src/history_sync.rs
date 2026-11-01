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

/// Read server global IDs, never client message IDs, for the pagination cursor.
pub fn page_cursor(response: &Value) -> Result<Option<String>> {
    let mut data = response.get("data").unwrap_or(response).clone();
    if let Some(text) = data.as_str() {
        data = serde_json::from_str(text)?;
    }
    if let Some(inner) = data.get("data") {
        data = inner.clone();
    }
    let items = data
        .as_array()
        .or_else(|| {
            ["msgs", "groupMsgs", "messages"]
                .iter()
                .find_map(|key| data.get(key).and_then(Value::as_array))
        })
        .context("Phản hồi lịch sử thiếu danh sách tin nhắn")?;
    if items.is_empty() {
        return Ok(None);
    }
    let mut oldest: Option<(u128, String)> = None;
    for item in items {
        let id = item
            .get("globalMsgId")
            .context("Thiếu globalMsgId; không thể phân trang an toàn")?;
        let text = match id {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => anyhow::bail!("globalMsgId không hợp lệ"),
        };
        let number: u128 = text.parse().context("globalMsgId không phải số")?;
        if oldest.as_ref().map_or(true, |(n, _)| number < *n) {
            oldest = Some((number, text));
        }
    }
    Ok(oldest.map(|(_, text)| text))
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
    for (index, mut thread) in threads
        .into_iter()
        .filter(|t| t.is_group || (!cloud.is_empty() && t.id == cloud))
        .enumerate()
    {
        thread.messages.clear();
        let mut cursor: Option<String> = store
            .conn
            .query_row(
                "SELECT cursor FROM history_sync WHERE thread_id=?1",
                [&thread.id],
                |row| row.get(0),
            )
            .ok();
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
                    cursor.as_deref().unwrap_or("0"), cursor.is_some(), 50).await?;
                if let Some(code) = response.get("error_code").and_then(Value::as_i64).filter(|c| *c != 0) {
                    anyhow::bail!("server error {code}");
                }
                let next = page_cursor(&response)?;
                let Some(next) = next else { return Ok(None); };
                anyhow::ensure!(cursor.as_ref().map_or(true, |old| next.parse::<u128>().ok() < old.parse::<u128>().ok()),
                    "Máy chủ không trả trang cũ hơn; đã dừng để tránh lặp");
                thread.messages.clear();
                crate::chat::merge_group_history(&mut thread, &response);
                inserted += crate::store::persist_threads(&mut store, std::slice::from_ref(&thread))?;
                // Persist messages before advancing. An interrupted checkpoint can
                // replay one page safely because message IDs are unique in SQLite.
                store.conn.execute("INSERT INTO history_sync(thread_id,cursor) VALUES (?1,?2) ON CONFLICT(thread_id) DO UPDATE SET cursor=excluded.cursor",
                    rusqlite::params![thread.id, next])?;
                page_ready(thread.clone());
                Ok::<_,anyhow::Error>(Some(next))
            }.await;
            match result {
                Ok(Some(next)) => cursor = Some(next),
                Ok(None) => break,
                Err(e) => {
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
    Ok(format!("Đã lưu {inserted} tin mới; {failed} hội thoại lỗi; {skipped} chat cá nhân chỉ có lịch sử cục bộ. Chỉ tải được dữ liệu server còn lưu."))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uses_global_not_client_id_and_handles_large_ids() {
        assert_eq!(
            page_cursor(&serde_json::json!({"data":{"msgs":[
            {"msgId":"1","globalMsgId":"100000000000000001"},
            {"msgId":"2","globalMsgId":"99999999999999999"}]}}))
            .unwrap(),
            Some("99999999999999999".into())
        );
        assert!(page_cursor(&serde_json::json!({"data":{"msgs":[{"msgId":"1"}]}})).is_err());
        assert_eq!(
            page_cursor(&serde_json::json!({"data":"{\"msgs\":[]}"})).unwrap(),
            None
        );
        assert!(page_cursor(&serde_json::json!({"data":null})).is_err());
    }
}
