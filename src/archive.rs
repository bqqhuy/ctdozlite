//! Server-versioned archived conversations; local hiding and PIN-protected chats are separate.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
/// Server identity uses a bare ID plus explicit kind (0 direct, 1 group).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: u8,
}
/// A complete acknowledged list, not a speculative local mutation.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub version: i64,
    pub items: Vec<Item>,
}
/// Parse a full list; an absent list is not evidence that no chats are archived.
/// # Errors
/// Rejects incomplete, oversized or ambiguous identities.
pub fn parse(data: Value) -> Result<Snapshot> {
    let version = data
        .get("version")
        .and_then(crate::api_response::integer)
        .filter(|v| *v >= 0)
        .context("Thiếu phiên bản danh sách lưu trữ")?;
    let values = data
        .get("items")
        .and_then(Value::as_array)
        .context("Thiếu danh sách hội thoại lưu trữ")?;
    ensure!(values.len() <= 10000, "Danh sách lưu trữ quá lớn");
    let mut items = Vec::with_capacity(values.len());
    let mut unique = std::collections::HashSet::new();
    for value in values {
        let item: Item = serde_json::from_value(value.clone())?;
        ensure!(
            item.kind <= 1
                && !item.id.is_empty()
                && item.id.len() <= 32
                && item.id.bytes().all(|b| b.is_ascii_digit()),
            "ID lưu trữ không hợp lệ"
        );
        if unique.insert((item.id.clone(), item.kind)) {
            items.push(item);
        }
    }
    Ok(Snapshot { version, items })
}
async fn call(s: &Session, path: &str, body: Value, get: bool) -> Result<Value> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    let host = crate::endpoints::service("label").context("Thiếu dịch vụ lưu trữ")?;
    let params = s.encoded_params(&body)?;
    let url = format!("{host}{path}?{}", s.common_params());
    let raw = if get {
        s.get(&format!("{url}&params={params}")).await?
    } else {
        s.post_form(&url, format!("params={params}")).await?
    };
    let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    ensure!(
        raw.get("error_code").is_some(),
        "Thiếu xác nhận máy chủ lưu trữ"
    );
    crate::api_response::checked_data(raw)
}
async fn read(s: &Session) -> Result<Snapshot> {
    parse(
        call(
            s,
            "/api/archivedchat/list",
            json!({"version":0,"imei":s.imei}),
            true,
        )
        .await?,
    )
}
/// Build a versioned archive mutation from a freshly read list.
/// # Errors
/// Rejects invalid conversation identities or an unknown version.
pub fn update_params(
    s: &Session,
    target: &str,
    group: bool,
    archived: bool,
    version: i64,
) -> Result<Value> {
    ensure!(version >= 0, "Phiên bản lưu trữ không hợp lệ");
    let id = crate::server_pins::identity(target, group)?;
    let bare = &id[1..];
    Ok(
        json!({"ids":[{"id":bare,"type":u8::from(group)}],"version":version,"actionType":if archived {0}else{1},"imei":s.imei}),
    )
}
/// Read or mutate once, then reconcile from a fresh list instead of optimistic local state.
/// # Errors
/// An uncertain mutation is returned to the UI and is never retried here.
pub async fn execute(
    s: &Session,
    target: &str,
    group: bool,
    archived: Option<bool>,
) -> Result<Snapshot> {
    let snapshot = read(s).await?;
    if let Some(archived) = archived {
        let body = update_params(s, target, group, archived, snapshot.version)?;
        call(s, "/api/archivedchat/update", body, false).await?;
        let after = read(s).await?;
        let identity = crate::server_pins::identity(target, group)?;
        let bare = &identity[1..];
        let present = after
            .items
            .iter()
            .any(|item| item.kind == u8::from(group) && item.id == bare);
        ensure!(
            present == archived,
            "Chưa xác nhận máy chủ đã áp dụng lưu trữ; tải lại trước khi thử tiếp"
        );
        Ok(after)
    } else {
        Ok(snapshot)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_preserves_direct_and_group_identity_and_requires_a_complete_list() {
        let snapshot =
            parse(json!({"version":"8","items":[{"id":"123","type":0},{"id":"123","type":1}]}))
                .unwrap();
        assert_eq!(snapshot.items.len(), 2);
        assert!(parse(json!({"version":8})).is_err());
        assert!(parse(json!({"version":8,"items":[{"id":"g123","type":1}]})).is_err());
        let s = Session::new(false, false, false).unwrap();
        let body = update_params(&s, "g123", true, true, snapshot.version).unwrap();
        assert_eq!(body["ids"], json!([{"id":"123","type":1}]));
        assert_eq!(body["actionType"], 0);
        assert_eq!(
            update_params(&s, "123", false, false, 8).unwrap()["actionType"],
            1
        );
    }
}

#[cfg(test)]
mod direct_identity_tests {
    use super::*;
    #[test]
    fn direct_archive_requests_send_a_bare_uid_not_the_internal_u_prefix() {
        let session = Session::new(false, false, false).unwrap();
        for id in ["123", "u123"] {
            let body = update_params(&session, id, false, true, 7).unwrap();
            assert_eq!(body["ids"][0], json!({"id":"123","type":0}));
        }
    }
}
