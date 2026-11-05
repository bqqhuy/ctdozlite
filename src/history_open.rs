//! Deduplicated recent-head refreshes; this cache never claims complete history coverage.
use crate::session::Session;
use anyhow::Result;
use serde_json::Value;
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};
type Key = (usize, String, String, bool);
static ACTIVE: OnceLock<Mutex<HashSet<Key>>> = OnceLock::new();
struct Active(Key);
impl Drop for Active {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.get_or_init(Default::default).lock() {
            active.remove(&self.0);
        }
    }
}
/// A successful network head can be marked fresh only after its messages were persisted.
pub(crate) struct Head {
    pub response: Value,
    session: Session,
    thread: String,
    group: bool,
    _active: Active,
}
fn fresh(at: i64, now: i64) -> bool {
    at > 0 && now >= at && now - at < 300
}
impl Head {
    pub async fn commit(self) -> Result<()> {
        let account = self.session.clone();
        let thread = self.thread.clone();
        let group = self.group;
        tokio::task::spawn_blocking(move||->Result<()>{
            anyhow::ensure!(account.is_active(),"Phiên đã đóng");let store=crate::store::MessageStore::open(account.history_path())?;
            store.conn.execute("INSERT INTO history_heads(thread_id,is_group,fetched_at) VALUES(?1,?2,?3) ON CONFLICT(thread_id,is_group) DO UPDATE SET fetched_at=excluded.fetched_at",rusqlite::params![thread,group,crate::login::unix_now()])?;Ok(())
        }).await?
    }
}
/// Opening local history performs at most one confirmed cm/getrecentv2 request per freshness window.
pub(crate) async fn fetch(session: &Session, thread: &str, group: bool) -> Result<Option<Head>> {
    let key = (
        session.runtime_id(),
        session.account_key().to_owned(),
        thread.to_owned(),
        group,
    );
    {
        let mut active = ACTIVE
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| anyhow::anyhow!("Không đọc được trạng thái tải lịch sử"))?;
        if active.contains(&key) || active.len() >= 64 {
            return Ok(None);
        }
        active.insert(key.clone());
    }
    let guard = Active(key);
    let account = session.clone();
    let id = thread.to_owned();
    let at = tokio::task::spawn_blocking(move || -> Result<i64> {
        use rusqlite::OptionalExtension;
        let store = crate::store::MessageStore::open(account.history_path())?;
        Ok(store
            .conn
            .query_row(
                "SELECT fetched_at FROM history_heads WHERE thread_id=?1 AND is_group=?2",
                rusqlite::params![id, group],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0))
    })
    .await??;
    if fresh(at, crate::login::unix_now()) {
        return Ok(None);
    }
    let response = crate::chat::get_cloud_messages(session, thread, "0", false, 50).await?;
    crate::board::data(response.clone())?;
    Ok(Some(Head {
        response,
        session: session.clone(),
        thread: thread.into(),
        group,
        _active: guard,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn head_cache_survives_reopens_but_is_not_permanent_coverage() {
        assert!(fresh(1000, 1001));
        assert!(fresh(1000, 1299));
        assert!(!fresh(1000, 1300));
        assert!(!fresh(1000, 999));
        assert!(!fresh(0, 1));
    }
}
