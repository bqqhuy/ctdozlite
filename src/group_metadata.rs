//! Server-versioned group display metadata; unchanged groups do not need zero-version refetches.
use crate::{chat::Thread, session::Session};
use anyhow::Result;
use serde_json::{Map, Value};
use std::collections::HashMap;
fn version(value: &Value) -> Option<String> {
    let value = value.as_u64().or_else(|| value.as_str()?.parse().ok())?;
    (value > 0).then(|| value.to_string())
}
/// Reuse only metadata obtained at exactly the membership version now reported by the server.
pub(crate) async fn refresh(
    session: &Session,
    versions: &Map<String, Value>,
    groups: &mut [Thread],
) -> Result<std::collections::HashSet<String>> {
    let account = session.clone();
    let cache =
        tokio::task::spawn_blocking(move || -> Result<HashMap<String, (String, Thread)>> {
            let store = crate::store::MessageStore::open(account.history_path())?;
            let mut query = store
                .conn
                .prepare("SELECT thread_id,version,data FROM group_metadata LIMIT 100001")?;
            let rows = query.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            let mut result = HashMap::new();
            for row in rows {
                let (id, version, raw) = row?;
                if raw.len() <= 1024 * 1024 {
                    if let Ok(mut thread) = serde_json::from_str::<Thread>(&raw) {
                        thread.messages.clear();
                        result.insert(id, (version, thread));
                    }
                }
            }
            Ok(result)
        })
        .await??;
    let mut missing = Vec::new();
    for group in groups.iter_mut() {
        let cached = versions
            .get(&group.id)
            .and_then(version)
            .and_then(|version| cache.get(&group.id).filter(|(old, _)| *old == version));
        if let Some((_, cached)) = cached {
            group.name = cached.name.clone();
            group.avatar = cached.avatar.clone();
            group.presentation.members = cached.presentation.members.clone();
            group.presentation.member_count = cached.presentation.member_count;
            group.presentation.global_id = cached.presentation.global_id.clone();
            // Membership versions authorize display caching, not current send permissions.
            group.presentation.send_policy = None;
        } else {
            missing.push(group.clone());
        }
    }
    let removed = crate::chat::resolve_group_metadata(session, &mut missing).await?;
    let mut saved = Vec::new();
    for mut updated in missing {
        if updated.presentation.metadata_verified {
            if let Some(version) = versions.get(&updated.id).and_then(version) {
                let mut metadata = updated.clone();
                metadata.messages.clear();
                metadata.unread = Default::default();
                metadata.refresh_after_removal();
                saved.push((
                    updated.id.clone(),
                    version,
                    serde_json::to_string(&metadata)?,
                ));
            }
        }
        if let Some(group) = groups.iter_mut().find(|group| group.id == updated.id) {
            std::mem::swap(group, &mut updated);
        }
    }
    let account = session.clone();
    tokio::task::spawn_blocking(move||->Result<()>{let store=crate::store::MessageStore::open(account.history_path())?;let tx=store.conn.unchecked_transaction()?;
        for (id,version,data) in saved {tx.execute("INSERT INTO group_metadata VALUES(?1,?2,?3) ON CONFLICT(thread_id) DO UPDATE SET version=excluded.version,data=excluded.data",rusqlite::params![id,version,data])?;}tx.commit()?;Ok(())}).await??;
    Ok(removed)
}
/// Forget derived display metadata after a group mutation, even when its membership version is unchanged.
/// # Errors
/// Returns local cache errors; no history, messages or membership state is deleted.
pub(crate) async fn invalidate(session: &Session, group: &str) -> Result<()> {
    let identity = crate::server_pins::identity(group, true)?;
    let bare = identity[1..].to_owned();
    let account = session.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let store = crate::store::MessageStore::open(account.history_path())?;
        store.conn.execute(
            "DELETE FROM group_metadata WHERE thread_id IN (?1,?2,?3)",
            rusqlite::params![bare, identity, format!("group_{bare}")],
        )?;
        Ok(())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn zero_or_malformed_server_versions_are_never_trusted() {
        assert_eq!(version(&json!(7)), Some("7".into()));
        assert_eq!(version(&json!("7")), Some("7".into()));
        assert_eq!(version(&json!(0)), None);
        assert_eq!(version(&json!("unknown")), None);
    }
}
