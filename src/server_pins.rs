//! Versioned server pin snapshots, kept separate from local conversation preferences.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Ordered wire identities retain `g`/`u` prefixes and server version.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub version: u64,
    pub conversations: Vec<String>,
}
impl Snapshot {
    /// Resolve rank without conflating group and user identities.
    pub fn rank(&self, thread: &str, group: bool) -> Option<usize> {
        let key = identity(thread, group).ok()?;
        self.conversations.iter().position(|id| id == &key)
    }
}
/// Create a typed wire identity without stripping arbitrary leading letters.
pub fn identity(thread: &str, group: bool) -> Result<String> {
    let id = if group {
        thread
            .strip_prefix("group_")
            .or_else(|| thread.strip_prefix('g'))
            .unwrap_or(thread)
    } else {
        thread.strip_prefix('u').unwrap_or(thread)
    };
    ensure!(
        !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_digit()),
        "Định danh hội thoại không hợp lệ"
    );
    Ok(format!("{}{id}", if group { 'g' } else { 'u' }))
}
/// Decode only documented snapshot fields; arbitrary numeric metadata is never an ID.
pub fn parse(mut value: Value) -> Result<Snapshot> {
    for _ in 0..4 {
        if let Some(text) = value.as_str() {
            ensure!(text.len() <= 256 * 1024, "Danh sách ghim quá lớn");
            value = serde_json::from_str(text)?;
            continue;
        }
        if let Some(code) = value.get("error_code") {
            let code = code
                .as_i64()
                .or_else(|| code.as_str()?.parse().ok())
                .context("Mã trả về ghim không hợp lệ")?;
            ensure!(code == 0, "Máy chủ từ chối ghim ({code})");
        }
        if value.get("conversations").is_some() {
            break;
        }
        value = value
            .get("data")
            .cloned()
            .context("Thiếu danh sách ghim từ máy chủ")?;
    }
    let list = value
        .get("conversations")
        .and_then(Value::as_array)
        .context("Danh sách ghim không đúng định dạng")?;
    ensure!(list.len() <= 1000, "Danh sách ghim quá lớn");
    let version = value
        .get("version")
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
        .context("Thiếu phiên bản danh sách ghim")?;
    let mut conversations = Vec::new();
    for item in list {
        let id = item.as_str().context("ID ghim không đúng định dạng")?;
        if id == "m1" {
            continue;
        }
        let (prefix, tail) = id.split_at_checked(1).context("ID ghim trống")?;
        ensure!(matches!(prefix, "g" | "u"), "Thiếu loại hội thoại ghim");
        let canonical = identity(tail, prefix == "g")?;
        if !conversations.contains(&canonical) {
            conversations.push(canonical);
        }
    }
    Ok(Snapshot {
        version,
        conversations,
    })
}
/// Fetch an authoritative snapshot using the PC encoded IMEI parameters.
pub async fn fetch(s: &Session) -> Result<Snapshot> {
    parse(crate::chat::get_pinned(s).await?)
}
/// Change one pin against a fresh server version; never retry a possibly applied POST.
pub async fn set(s: &Session, thread: &str, group: bool, pinned: bool) -> Result<Snapshot> {
    let mut snapshot = fetch(s).await?;
    let key = identity(thread, group)?;
    snapshot.conversations.retain(|id| id != &key);
    if pinned {
        snapshot.conversations.insert(0, key);
    }
    let body = json!({"version":snapshot.version,"conversations":snapshot.conversations});
    let enc = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/pinconvers/update?{}",
        s.domains.conversation.trim_end_matches('/'),
        s.common_params()
    );
    let result = s.post_form(&url, format!("params={enc}")).await?;
    crate::chat::confirmed_mutation(s.maybe_decrypt(result, s.session_key_bytes()).await)?;
    fetch(s).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authoritative_empty_snapshot_is_distinct_from_failure() {
        assert!(parse(json!({"version":2,"conversations":[]}))
            .unwrap()
            .conversations
            .is_empty());
        assert!(parse(json!({"version":2,"other":123})).is_err());
        assert!(parse(json!({"error_code":230,"data":{"version":2,"conversations":[]}})).is_err());
    }
    #[test]
    fn preserves_order_type_and_removals() {
        let s = parse(json!({"data":"{\"version\":3,\"conversations\":[\"g12\",\"u12\",\"m1\"]}"}))
            .unwrap();
        assert_eq!(s.rank("12", true), Some(0));
        assert_eq!(s.rank("12", false), Some(1));
        assert_eq!(s.rank("group_12", true), Some(0));
        assert!(parse(json!({"version":4,"conversations":[12]})).is_err());
    }
}
