//! Complete account metadata snapshots, independent from history synchronization.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
#[derive(Default)]
pub struct Snapshot {
    pub from_cache: bool,
    pub cloud_id: String,
    pub inbox: Option<Vec<(String, bool, crate::inbox::Metadata)>>,
    pub privacy: Option<crate::privacy::Snapshot>,
    pub labels: Option<crate::labels::Snapshot>,
    pub archive: Option<crate::archive::Snapshot>,
    pub aliases: Option<HashMap<String, String>>,
    pub friends: Option<HashSet<String>>,
    pub business: Option<bool>,
    pub profile: Option<crate::profile::Snapshot>,
    pub warnings: Vec<&'static str>,
}
async fn get(s: &Session, host: &str, path: &str, body: Value) -> Result<Value> {
    let params = s.encoded_params(&body)?;
    let raw = s
        .get(&format!(
            "{host}{path}?{}&params={params}",
            s.common_params()
        ))
        .await?;
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    crate::api_response::checked_data(s.maybe_decrypt(raw, s.session_key_bytes()).await)
}
/// Fetch all alias pages; malformed/looping/partial lists never replace a complete old snapshot.
pub async fn aliases(s: &Session) -> Result<HashMap<String, String>> {
    let host = crate::endpoints::service("alias").context("Thiếu dịch vụ biệt danh")?;
    let mut result = HashMap::new();
    let mut pages = HashSet::new();
    for page in 1..=1000 {
        let data = get(
            s,
            host,
            "/api/alias/list",
            json!({"page":page,"count":100,"imei":s.imei}),
        )
        .await?;
        let rows = data
            .get("items")
            .and_then(Value::as_array)
            .context("Thiếu danh sách biệt danh")?;
        ensure!(rows.len() <= 100, "Trang biệt danh quá lớn");
        let signature = rows
            .iter()
            .map(|row| row.get("userId").cloned().unwrap_or(Value::Null))
            .collect::<Vec<_>>();
        ensure!(
            rows.is_empty() || pages.insert(serde_json::to_string(&signature)?),
            "Trang biệt danh bị lặp"
        );
        for row in rows {
            let id = id(row.get("userId"))?;
            let alias = row
                .get("alias")
                .and_then(Value::as_str)
                .context("Thiếu tên biệt danh")?;
            ensure!(alias.len() <= 1024, "Biệt danh quá dài");
            if !alias.is_empty() {
                ensure!(
                    result.insert(id, alias.to_owned()).is_none(),
                    "UID biệt danh bị trùng"
                );
            }
        }
        let more = data
            .get("hasMore")
            .and_then(|value| {
                value
                    .as_bool()
                    .or_else(|| crate::api_response::integer(value).map(|value| value != 0))
            })
            .unwrap_or(rows.len() == 100);
        if !more {
            return Ok(result);
        }
    }
    anyhow::bail!("Danh sách biệt danh vượt giới hạn")
}
fn id(value: Option<&Value>) -> Result<String> {
    let id = value
        .and_then(|id| {
            id.as_str()
                .map(str::to_owned)
                .or_else(|| id.as_u64().map(|id| id.to_string()))
        })
        .context("Thiếu UID")?;
    ensure!(
        !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_digit()),
        "UID không hợp lệ"
    );
    Ok(id)
}
/// Friend membership comes from a complete list, never from successfully resolving a name.
pub async fn friends(s: &Session) -> Result<HashSet<String>> {
    let mut result = HashSet::new();
    let mut signatures = HashSet::new();
    for page in 1..=100 {
        let data=get(s,&s.domains.friend,"/api/social/friend/getfriends",json!({"incInvalid":0,"page":page,"count":1000,"avatar_size":160,"actiontime":0,"imei":s.imei})).await?;
        let rows = data
            .as_array()
            .or_else(|| data.get("friends").and_then(Value::as_array))
            .or_else(|| data.get("data").and_then(Value::as_array))
            .context("Thiếu danh sách bạn bè")?;
        ensure!(rows.len() <= 1000, "Trang bạn bè quá lớn");
        let mut ids = Vec::new();
        for row in rows {
            ids.push(id(row
                .get("userId")
                .or_else(|| row.get("uid"))
                .or_else(|| row.get("id")))?);
        }
        ensure!(
            ids.is_empty() || signatures.insert(ids.clone()),
            "Trang bạn bè bị lặp"
        );
        for id in ids {
            ensure!(result.insert(id), "UID bạn bè bị trùng");
        }
        let more = data
            .get("hasMore")
            .or_else(|| data.get("has_more"))
            .and_then(|value| {
                value
                    .as_bool()
                    .or_else(|| crate::api_response::integer(value).map(|value| value != 0))
            })
            .unwrap_or(rows.len() == 1000);
        if !more {
            return Ok(result);
        }
    }
    anyhow::bail!("Danh sách bạn bè vượt giới hạn")
}
/// Read independent snapshots concurrently. One unavailable capability preserves that old UI state.
/// Conversation metadata only: previews are not imported again as history messages.
async fn inbox(s: &Session) -> Result<Vec<(String, bool, crate::inbox::Metadata)>> {
    let raw = get(
        s,
        &s.domains.convers_api,
        "/api/preloadconvers/get-last-msgs",
        json!({"threadIdLocalMsgId":"{}","imei":s.imei}),
    )
    .await?;
    let mut threads = Vec::new();
    crate::chat::merge_previews(&mut threads, &raw);
    ensure!(threads.len() <= 20_000, "Danh sách hội thoại quá lớn");
    let cached = s
        .inbox_policy
        .read()
        .map(|policy| policy.profile_types.clone())
        .unwrap_or_default();
    let missing = threads
        .iter()
        .filter(|thread| !thread.is_group && !cached.contains_key(&thread.id))
        .map(|thread| thread.id.clone())
        .collect::<Vec<_>>();
    let mut types = cached;
    for chunk in missing.chunks(50) {
        let profiles = crate::chat::get_mini_profiles(s, chunk).await?;
        for (id, profile) in profiles {
            if !chunk.contains(&id) {
                continue;
            }
            let mut metadata = crate::inbox::Metadata::default();
            metadata.apply_profile(&profile);
            if let Some(kind) = metadata.official_account {
                types.insert(id, kind);
            }
        }
    }
    let present = threads
        .iter()
        .map(|thread| thread.id.as_str())
        .collect::<HashSet<_>>();
    types.retain(|id, _| present.contains(id.as_str()));
    if let Ok(mut policy) = s.inbox_policy.write() {
        policy.profile_types = types.clone();
    }
    for thread in &mut threads {
        if !thread.is_group {
            thread.presentation.inbox.official_account = types.get(&thread.id).copied();
        }
    }
    Ok(threads
        .into_iter()
        .map(|thread| (thread.id, thread.is_group, thread.presentation.inbox))
        .collect())
}
async fn bounded<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(std::time::Duration::from_secs(45), future)
        .await
        .context("Hết giờ tải metadata")?
}
pub async fn read(s: &Session) -> Snapshot {
    let (privacy, labels, archive, aliases, friends, profile, inbox) = tokio::join!(
        bounded(crate::privacy::execute(s, &crate::privacy::Action::Read)),
        bounded(crate::labels::execute(s, &crate::labels::Action::Read)),
        bounded(crate::archive::execute(s, "", false, None)),
        bounded(aliases(s)),
        bounded(friends(s)),
        bounded(crate::profile::execute(s, &crate::profile::Action::Read)),
        bounded(inbox(s))
    );
    let mut warnings = Vec::new();
    if privacy.is_err() {
        warnings.push("quyền riêng tư");
    }
    if labels.is_err() {
        warnings.push("nhãn");
    }
    if archive.is_err() {
        warnings.push("lưu trữ");
    }
    if aliases.is_err() {
        warnings.push("biệt danh");
    }
    if friends.is_err() {
        warnings.push("bạn bè");
    }
    if inbox.is_err() {
        warnings.push("hộp người lạ");
    }
    if profile.is_err() {
        warnings.push("loại tài khoản");
    }
    Snapshot {
        from_cache: false,
        cloud_id: s
            .login
            .as_ref()
            .map(|login| login.send2me_id.clone())
            .unwrap_or_default(),
        inbox: inbox.ok(),
        privacy: privacy.ok(),
        labels: labels.ok(),
        archive: archive.ok(),
        aliases: aliases.ok(),
        friends: friends.ok(),
        business: profile.as_ref().ok().map(|profile| profile.business),
        profile: profile.ok(),
        warnings,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Cache {
    version: u8,
    owner: String,
    policy: crate::inbox::Policy,
    aliases: Option<HashMap<String, String>>,
}
fn decode_cache(bytes: &[u8], owner: &str) -> Result<Cache> {
    let cache: Cache = serde_json::from_slice(bytes)?;
    ensure!(
        cache.version == 1 && cache.owner == owner && !owner.is_empty(),
        "Metadata lưu thuộc tài khoản khác"
    );
    if let Some(friends) = &cache.policy.friends {
        ensure!(friends.len() <= 100_000, "Danh bạ lưu quá lớn");
        for key in friends {
            ensure!(id(Some(&json!(key)))? == *key, "UID lưu không hợp lệ");
        }
    }
    ensure!(
        cache.policy.profile_types.len() <= 20_000,
        "Cache phân loại quá lớn"
    );
    if let Some(aliases) = &cache.aliases {
        ensure!(aliases.len() <= 100_000, "Danh sách biệt danh lưu quá lớn");
        for (key, name) in aliases {
            id(Some(&json!(key)))?;
            ensure!(name.len() <= 1024, "Biệt danh lưu quá dài");
        }
    }
    Ok(cache)
}
/// Restore the last complete account snapshot before network refresh; no messages or secrets are cached.
/// # Errors
/// Invalid/foreign files are rejected without replacing runtime state.
pub async fn restore_cached(s: &Session) -> Result<Option<Snapshot>> {
    let account = s.clone();
    let path = s.cache_path().with_file_name("account_metadata_cache.json");
    let cache = tokio::task::spawn_blocking(move || -> Result<Option<Cache>> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = crate::limits::read_file(&path, 8 * 1024 * 1024)?;
        Ok(Some(decode_cache(&bytes, account.account_key())?))
    })
    .await??;
    let Some(cache) = cache else {
        return Ok(None);
    };
    ensure!(s.is_active(), "Phiên đã đóng");
    if let Ok(mut policy) = s.inbox_policy.write() {
        *policy = cache.policy.clone();
    }
    let mut privacy = crate::privacy::Snapshot::default();
    if let Some(direct) = cache.policy.direct {
        privacy
            .values
            .insert(crate::privacy::Setting::StrangerDirect, i64::from(direct));
    }
    if let Some(online) = cache.policy.show_online {
        privacy
            .values
            .insert(crate::privacy::Setting::ShowOnline, i64::from(online));
    }
    Ok(Some(Snapshot {
        from_cache: true,
        privacy: Some(privacy),
        friends: cache.policy.friends,
        business: cache.policy.business,
        aliases: cache.aliases,
        cloud_id: s
            .login
            .as_ref()
            .map(|login| login.send2me_id.clone())
            .unwrap_or_default(),
        ..Default::default()
    }))
}
/// Persist only successful policy/alias capabilities, retaining older complete values for unavailable ones.
pub async fn save_cached(s: &Session, snapshot: &Snapshot) {
    let account = s.clone();
    let path = s.cache_path().with_file_name("account_metadata_cache.json");
    let aliases = snapshot.aliases.clone();
    let policy = s
        .inbox_policy
        .read()
        .map(|policy| policy.clone())
        .unwrap_or_default();
    let result = tokio::task::spawn_blocking(move || -> Result<()> {
        ensure!(account.is_active(), "Phiên đã đóng");
        let old = crate::limits::read_file(&path, 8 * 1024 * 1024)
            .ok()
            .and_then(|bytes| decode_cache(&bytes, account.account_key()).ok());
        let cache = Cache {
            version: 1,
            owner: account.account_key().into(),
            policy,
            aliases: aliases.or_else(|| old.and_then(|old| old.aliases)),
        };
        let bytes = serde_json::to_vec(&cache)?;
        ensure!(bytes.len() <= 8 * 1024 * 1024, "Metadata lưu quá lớn");
        if crate::limits::read_file(&path, 8 * 1024 * 1024).is_ok_and(|old| old == bytes) {
            return Ok(());
        }
        crate::storage::atomic_write(&path, &bytes)
    })
    .await;
    if !matches!(result, Ok(Ok(()))) {
        crate::logger::log_error("account metadata", "Cannot save account metadata cache");
    }
}
#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn account_policy_cache_rejects_foreign_and_malformed_membership() {
        let bytes = serde_json::to_vec(&Cache {
            version: 1,
            owner: "owner".into(),
            policy: crate::inbox::Policy {
                friends: Some(HashSet::from(["12".into()])),
                direct: Some(false),
                ..Default::default()
            },
            aliases: Some(HashMap::from([("12".into(), "Tên 🎉".into())])),
        })
        .unwrap();
        let cache = decode_cache(&bytes, "owner").unwrap();
        assert_eq!(cache.policy.direct, Some(false));
        assert!(decode_cache(&bytes, "other").is_err());
        assert!(decode_cache(
            br#"{"version":1,"owner":"owner","policy":{"friends":["invalid"]},"aliases":{}}"#,
            "owner"
        )
        .is_err());
    }
}
