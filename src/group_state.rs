//! Account/session-scoped group permission snapshots shared by outgoing operations.
use crate::{chat::GroupSendPolicy, session::Session};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

/// Server metadata and the exact membership removal result from getmg-v2.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Original metadata, retained for feature-specific permissions.
    pub info: Value,
    /// A removed membership must not be inferred to be an open group.
    pub removed: bool,
}
impl Snapshot {
    /// Resolve send permission without inferring roles from shortened rosters.
    pub fn can_send(&self, uid: &str) -> Result<bool> {
        if self.removed {
            return Ok(false);
        }
        let policy =
            GroupSendPolicy::parse(&self.info).context("Chưa xác minh được quyền gửi của nhóm")?;
        Ok(policy.allows(uid))
    }
    /// Creator identity, never a display name.
    pub fn creator(&self) -> Option<&str> {
        self.info.get("creatorId")?.as_str()
    }
    /// Whether a user has a management role in the current snapshot.
    pub fn manager(&self, uid: &str) -> bool {
        self.creator() == Some(uid)
            || self
                .info
                .get("adminIds")
                .and_then(Value::as_array)
                .is_some_and(|ids| {
                    ids.iter().any(|id| {
                        id.as_str() == Some(uid)
                            || id.as_u64().is_some_and(|id| id.to_string() == uid)
                    })
                })
    }
}
type Cell = Arc<tokio::sync::OnceCell<Result<Snapshot, String>>>;
struct Entry {
    at: Instant,
    value: Cell,
}
type Key = (usize, String, String);
static CACHE: OnceLock<Mutex<HashMap<Key, Entry>>> = OnceLock::new();
const FRESH: Duration = Duration::from_secs(15);

/// Invalidate permissions after a group mutation or a relevant server control.
pub fn invalidate(session: &Session, group: Option<&str>) {
    if let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() {
        cache.retain(|(runtime, owner, id), _| {
            *runtime != session.runtime_id()
                || owner != session.account_key()
                || group.is_some_and(|group| id != group)
        });
    }
}
/// Fetch once for concurrent sends; no cache lock is held while awaiting network I/O.
/// # Errors
/// Propagates metadata/authentication errors; absent permissions never become success.
pub async fn snapshot(session: &Session, group: &str, refresh: bool) -> Result<Snapshot> {
    ensure!(
        session.is_active() && !session.account_key().is_empty(),
        "Phiên làm việc đã đóng"
    );
    let key = (
        session.runtime_id(),
        session.account_key().to_owned(),
        group.to_owned(),
    );
    let cell = {
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| anyhow::anyhow!("Không đọc được quyền nhóm"))?;
        cache.retain(|_, entry| entry.at.elapsed() < FRESH);
        if refresh {
            cache.remove(&key);
        }
        if !cache.contains_key(&key) {
            if cache.len() >= 256 {
                if let Some(oldest) = cache
                    .iter()
                    .min_by_key(|(_, entry)| entry.at)
                    .map(|(key, _)| key.clone())
                {
                    cache.remove(&oldest);
                }
            }
            cache.insert(
                key.clone(),
                Entry {
                    at: Instant::now(),
                    value: Arc::new(tokio::sync::OnceCell::new()),
                },
            );
        }
        cache
            .get(&key)
            .context("Thiếu snapshot quyền nhóm")?
            .value
            .clone()
    };
    let result = cell
        .get_or_init(|| async {
            let result = async {
                let (profiles, removed) =
                    crate::chat::fetch_group_profiles(session, &[group.to_owned()]).await?;
                ensure!(session.is_active(), "Phiên làm việc đã đóng");
                if removed.contains(group) {
                    return Ok(Snapshot {
                        info: Value::Null,
                        removed: true,
                    });
                }
                let info = profiles
                    .get(group)
                    .cloned()
                    .context("Không xác minh được nhóm")?;
                Ok(Snapshot {
                    info,
                    removed: false,
                })
            }
            .await;
            result.map_err(|error: anyhow::Error| error.to_string())
        })
        .await;
    ensure!(session.is_active(), "Phiên làm việc đã đóng");
    result.clone().map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn missing_and_removed_group_never_grant_send_permission() {
        let absent = Snapshot {
            info: json!({}),
            removed: false,
        };
        assert!(absent.can_send("member").is_err());
        let removed = Snapshot {
            info: json!({"setting":{"lockSendMsg":0}}),
            removed: true,
        };
        assert!(!removed.can_send("owner").unwrap());
    }
    #[test]
    fn manager_can_send_locked_group_but_roster_order_does_not_grant_role() {
        let value = Snapshot {
            info: json!({"creatorId":"owner","adminIds":["admin"],"setting":{"lockSendMsg":1},"topMember":[{"id":"member"}]}),
            removed: false,
        };
        assert!(value.can_send("admin").unwrap());
        assert!(!value.can_send("member").unwrap());
    }
}
