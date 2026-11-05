//! Server conversation TTL and block-sharing snapshots, independent of local preferences.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
/// Confirmed server snapshot; a failed refresh must not replace it with an empty policy.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub from_cache: bool,
    /// Canonical `u`/`g` identities whose content is protected by the server.
    pub protected: BTreeSet<String>,
    /// TTL durations in milliseconds, not per-message deadlines.
    pub ttls: BTreeMap<String, i64>,
}
impl Snapshot {
    /// Test a typed conversation identity.
    pub fn protects(&self, thread: &str, group: bool) -> bool {
        crate::server_pins::identity(thread, group).is_ok_and(|id| self.protected.contains(&id))
    }
    /// Missing entries in a complete server snapshot have no TTL setting.
    pub fn ttl(&self, thread: &str, group: bool) -> i64 {
        crate::server_pins::identity(thread, group)
            .ok()
            .and_then(|id| self.ttls.get(&id).copied())
            .unwrap_or(0)
    }
}
/// Explicit account-bound policy intents, never optimistically committed.
#[derive(Clone, Debug)]
pub enum Action {
    Read,
    SetTtl {
        thread: String,
        group: bool,
        ttl: i64,
    },
    SetProtection {
        thread: String,
        group: bool,
        enabled: bool,
    },
}
fn number(v: Option<&Value>) -> Option<i64> {
    v.and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
}
fn boolean(v: Option<&Value>) -> Option<bool> {
    match v? {
        Value::Bool(v) => Some(*v),
        v => match number(Some(v))? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
    }
}
fn identity(value: &Value) -> Result<String> {
    let id = value
        .get("destId")
        .context("Thiếu destId trong thiết lập hội thoại")?;
    let id = id
        .as_str()
        .map(str::to_owned)
        .or_else(|| id.as_u64().map(|id| id.to_string()))
        .context("destId không hợp lệ")?;
    crate::server_pins::identity(
        &id,
        boolean(value.get("isGroup")).context("Thiếu loại hội thoại")?,
    )
}
/// Decode the PC `convers` format; a malformed list is not an empty success.
pub fn parse(ttl: Value, sharing: Value) -> Result<Snapshot> {
    let ttl = crate::board::data(ttl)?;
    let sharing = crate::board::data(sharing)?;
    let ttl = ttl
        .get("convers")
        .and_then(Value::as_array)
        .context("Thiếu danh sách tự xóa")?;
    let sharing = sharing
        .get("convers")
        .and_then(Value::as_array)
        .context("Thiếu danh sách bảo vệ nội dung")?;
    ensure!(
        ttl.len() <= 100000 && sharing.len() <= 100000,
        "Thiết lập hội thoại quá lớn"
    );
    let mut result = Snapshot::default();
    for row in ttl {
        let value = number(row.get("ttl"))
            .filter(|ttl| *ttl >= 0)
            .context("TTL không hợp lệ")?;
        result.ttls.insert(identity(row)?, value);
    }
    for row in sharing {
        result.protected.insert(identity(row)?);
    }
    Ok(result)
}
type Key = (usize, String);
static NETWORK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
static CACHE: OnceLock<Mutex<HashMap<Key, (Instant, Snapshot)>>> = OnceLock::new();
async fn persist(s: &Session, snapshot: &Snapshot) -> Result<()> {
    let s = s.clone();
    let value = snapshot.clone();
    tokio::task::spawn_blocking(move||->Result<()>{
        ensure!(s.is_active(),"Phiên đã đóng");let store=crate::store::MessageStore::open(s.history_path())?;
        let tx=store.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM conversation_policies",[])?;
        let keys=value.ttls.keys().chain(value.protected.iter()).collect::<BTreeSet<_>>();
        for id in keys {tx.execute("INSERT INTO conversation_policies(thread_id,is_group,is_protected,ttl) VALUES(?1,?2,?3,?4)",rusqlite::params![&id[1..],id.starts_with('g'),value.protected.contains(id),value.ttls.get(id).copied().unwrap_or(0)])?;}
        tx.execute("INSERT INTO tool_config(key,value) VALUES('conversation-policy',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(&value)?])?;
        tx.commit()?;Ok(())
    }).await?
}
/// Fetch the complete server policy at most once per short freshness window.
/// # Errors
/// Keeps the previous snapshot on transport/format/authentication failures.
async fn fetch_snapshot(s: &Session, refresh: bool) -> Result<Snapshot> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Chưa đăng nhập"
    );
    let key = (s.runtime_id(), s.account_key().to_owned());
    if !refresh {
        if let Ok(cache) = CACHE.get_or_init(Default::default).lock() {
            if let Some((at, value)) = cache.get(&key) {
                if at.elapsed() < Duration::from_secs(30) {
                    return Ok(value.clone());
                }
            }
        }
    }
    let _network = NETWORK.acquire().await?;
    let params = s.encoded_params(&json!({}))?;
    let mut result = Vec::new();
    for feature in ["autodelete", "blocksharing"] {
        let url = format!(
            "{}/api/conv/{feature}/getConvers?{}&params={params}",
            s.domains.conversation,
            s.common_params()
        );
        let response = s.get(&url).await?;
        result.push(s.maybe_decrypt(response, s.session_key_bytes()).await);
    }
    let value = parse(result.remove(0), result.remove(0))?;
    ensure!(s.is_active(), "Phiên đã đóng");
    persist(s, &value).await?;
    if let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() {
        if cache.len() >= 16 {
            cache.clear();
        }
        cache.insert(key, (Instant::now(), value.clone()));
    }
    Ok(value)
}
/// Read only a previously confirmed, account-local policy snapshot.
pub(crate) async fn cached(s: &Session) -> Result<Option<Snapshot>> {
    let account = s.clone();
    tokio::task::spawn_blocking(move || -> Result<_> {
        use rusqlite::OptionalExtension;
        let store = crate::store::MessageStore::open(account.history_path())?;
        let raw = store
            .conn
            .query_row(
                "SELECT value FROM tool_config WHERE key='conversation-policy'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(raw
            .filter(|raw| raw.len() <= 2 * 1024 * 1024)
            .and_then(|raw| serde_json::from_str::<Snapshot>(&raw).ok())
            .map(|mut snapshot| {
                snapshot.from_cache = true;
                snapshot
            }))
    })
    .await?
}
/// Refresh normally; an offline failure retains the last confirmed account-local protection.
pub async fn snapshot(s: &Session, refresh: bool) -> Result<Snapshot> {
    match fetch_snapshot(s, refresh).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => match cached(s).await? {
            Some(snapshot) => {
                if let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() {
                    cache.insert(
                        (s.runtime_id(), s.account_key().into()),
                        (Instant::now(), snapshot.clone()),
                    );
                }
                Ok(snapshot)
            }
            None => Err(error),
        },
    }
}
/// Apply one server mutation then refetch; an uncertain mutation is never replayed.
pub async fn execute(s: &Session, action: &Action) -> Result<Snapshot> {
    if matches!(action, Action::Read) {
        return snapshot(s, true).await;
    }
    let (thread, group, body, path, query) = match action {
        Action::SetTtl { thread, group, ttl } => {
            ensure!(
                [0, 3600000, 86400000, 604800000, 1209600000, 2592000000].contains(ttl),
                "Thời lượng tự xóa chưa được hỗ trợ"
            );
            (
                thread,
                *group,
                json!({"ttl":ttl}),
                "/api/conv/autodelete/updateConvers",
                true,
            )
        }
        Action::SetProtection {
            thread,
            group,
            enabled,
        } => (
            thread,
            *group,
            json!({"isEnabled":enabled}),
            "/api/conv/blocksharing/updateConvers",
            false,
        ),
        Action::Read => unreachable!(),
    };
    let id = crate::server_pins::identity(thread, group)?;
    let mut body = body;
    body["threadId"] = json!(&id[1..]);
    body["isGroup"] = json!(if group { 1 } else { 0 });
    body["clientLang"] = json!("vi");
    let params = s.encoded_params(&body)?;
    let url = format!("{}{path}?{}", s.domains.conversation, s.common_params());
    let network = NETWORK.acquire().await?;
    let response = if query {
        s.post_form(&format!("{url}&params={params}"), String::new())
            .await?
    } else {
        s.post_form(&url, format!("params={params}")).await?
    };
    let response = s.maybe_decrypt(response, s.session_key_bytes()).await;
    ensure!(
        response.get("error_code").is_some(),
        "Thiếu xác nhận đổi thiết lập"
    );
    crate::board::data(response)?;
    drop(network);
    snapshot(s, true).await
}
/// Worker export boundary; preview/viewing is independent of copying/saving.
pub async fn require_shareable(s: &Session, thread: &str, group: bool) -> Result<()> {
    ensure!(
        !snapshot(s, false).await?.protects(thread, group),
        "Hội thoại đang chặn chia sẻ nội dung"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_snapshot_cannot_clear_existing_protection() {
        assert!(parse(json!({}), json!({"convers":[]})).is_err());
        let policy = parse(
            json!({"convers":[{"destId":"123","isGroup":1,"ttl":86400000}]}),
            json!({"convers":[{"destId":"123","isGroup":1,"isActor":0}]}),
        )
        .unwrap();
        assert!(policy.protects("123", true));
        assert!(!policy.protects("123", false));
        assert_eq!(policy.ttl("123", true), 86400000);
    }
}
