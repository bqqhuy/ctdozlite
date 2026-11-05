//! Explicit group management intents from PC 26.11.10; mutations are never replayed.
use crate::{chat::Thread, group_state, session::Session};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

/// One user intent, retained until a terminal acknowledgement or an uncertain failure.
#[derive(Clone, Debug)]
pub enum Action {
    /// Refresh permissions and display metadata.
    Inspect,
    /// Query server-reported shared groups with a peer.
    Related,
    /// Create a normal group from selected known contacts; client ID is stable.
    Create {
        name: String,
        members: Vec<String>,
        client_id: String,
    },
    /// Invite selected users; acknowledgement does not prove they have joined.
    Invite { members: Vec<String> },
    /// Remove a selected member.
    Kick { member: String },
    /// Assign or remove the deputy role.
    Admin { member: String, add: bool },
    /// Transfer ownership to a selected member.
    Transfer { member: String },
    /// Leave without deleting local history.
    Leave,
    /// Dissolve the group for all members.
    Disband,
}
impl Action {
    /// Reads do not require confirmation and may be refreshed safely.
    pub fn mutation(&self) -> bool {
        !matches!(self, Self::Inspect | Self::Related)
    }
}
/// Result values contain metadata, not session credentials.
#[derive(Clone, Debug, Default)]
pub struct Reply {
    /// Updated group metadata, when this was an inspection.
    pub info: Option<Value>,
    /// Hydrated common groups; a malformed response is an error rather than an empty list.
    pub related: Option<Vec<Thread>>,
    /// Explicit server success; invitation membership may still be pending.
    pub acknowledged: bool,
}
fn uid(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 128 && value.bytes().all(|b| b.is_ascii_digit()),
        "UID không hợp lệ"
    );
    Ok(())
}
fn grid(value: &str) -> Result<String> {
    let value = crate::server_pins::identity(value, true)?;
    Ok(value[1..].to_owned())
}
fn members(values: &[String]) -> Result<()> {
    ensure!(
        !values.is_empty() && values.len() <= 100,
        "Chọn từ 1 đến 100 người"
    );
    let mut unique = std::collections::HashSet::new();
    for value in values {
        uid(value)?;
        ensure!(unique.insert(value), "Thành viên bị trùng");
    }
    Ok(())
}
/// Build the PC adapter fields and HTTP method. No caller-supplied path is accepted.
/// # Errors
/// Rejects invalid identities or unsupported input before I/O.
pub fn request(s: &Session, target: &str, action: &Action) -> Result<(&'static str, Value, bool)> {
    let lang = "vi";
    let target = if matches!(action, Action::Create { .. }) {
        String::new()
    } else {
        grid(target)?
    };
    let (path, mut body, get) = match action {
        Action::Create {
            name,
            members: ids,
            client_id,
        } => {
            members(ids)?;
            ensure!(
                ids.len() >= 2 && !name.trim().is_empty() && name.chars().count() <= 100,
                "Nhóm cần tên và ít nhất hai người khác"
            );
            ensure!(
                !ids.iter().any(|id| id == s.account_key()),
                "Không chọn lại chính mình"
            );
            ensure!(
                !client_id.is_empty() && client_id.len() <= 64,
                "Thiếu ID thao tác tạo nhóm"
            );
            (
                "/api/group/create/v2",
                json!({"clientId":client_id,"gname":name.trim(),"gdesc":null,"members":ids,"memberTypes":vec![-1;ids.len()],"nameChanged":1,"createLink":1,"clientLang":lang,"groupType":null}),
                true,
            )
        }
        Action::Invite { members: ids } => {
            members(ids)?;
            (
                "/api/group/invite/v2",
                json!({"grid":target,"members":ids,"memberTypes":vec![-1;ids.len()],"clientLang":lang}),
                false,
            )
        }
        Action::Kick { member } => {
            uid(member)?;
            (
                "/api/group/kickout",
                json!({"grid":target,"members":[member]}),
                false,
            )
        }
        Action::Admin { member, add } => {
            uid(member)?;
            (
                if *add {
                    "/api/group/admins/add"
                } else {
                    "/api/group/admins/remove"
                },
                json!({"grid":target,"members":[member]}),
                true,
            )
        }
        Action::Transfer { member } => {
            uid(member)?;
            (
                "/api/group/change-owner",
                json!({"grid":target,"newAdminId":member,"language":lang}),
                false,
            )
        }
        Action::Leave => (
            "/api/group/leave",
            json!({"grids":[target],"silent":0,"language":lang}),
            false,
        ),
        Action::Disband => ("/api/group/disperse", json!({"grid":target}), false),
        _ => anyhow::bail!("Đây không phải thao tác thay đổi nhóm"),
    };
    body["imei"] = json!(s.imei);
    Ok((path, body, get))
}
/// Dispatch a typed intent after refreshing management permissions.
/// # Errors
/// Returns server/transport/permission failures. Uncertain mutations are not retried.
pub async fn execute(s: &Session, target: &str, action: &Action) -> Result<Reply> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Chưa đăng nhập"
    );
    if matches!(action, Action::Related) {
        uid(target)?;
        let body = json!({"friend_ids":serde_json::to_string(&[target])?,"imei":s.imei});
        let params = s.encoded_params(&body)?;
        let raw = s
            .post_form(
                &format!(
                    "{}/api/friend/group/related?{}",
                    s.domains.friend,
                    s.common_params()
                ),
                format!("params={params}"),
            )
            .await?;
        let data = crate::board::data(s.maybe_decrypt(raw, s.session_key_bytes()).await)?;
        let ids = data
            .get("groupRelateds")
            .and_then(|map| map.get(target))
            .and_then(Value::as_array)
            .context("Phản hồi nhóm chung chưa đúng định dạng")?;
        ensure!(ids.len() <= 2000, "Danh sách nhóm chung quá lớn");
        let mut result = Vec::new();
        for batch in ids.chunks(50) {
            let keys = batch
                .iter()
                .map(|id| {
                    id.as_str()
                        .map(str::to_owned)
                        .or_else(|| id.as_u64().map(|id| id.to_string()))
                        .context("ID nhóm chung không hợp lệ")
                        .and_then(|id| grid(&id))
                })
                .collect::<Result<Vec<_>>>()?;
            let (profiles, removed) = crate::chat::fetch_group_profiles(s, &keys).await?;
            for id in keys {
                if removed.contains(&id) {
                    continue;
                }
                let info = profiles.get(&id).context("Thiếu thông tin nhóm chung")?;
                let mut thread = Thread {
                    id,
                    is_group: true,
                    name: info
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("Nhóm")
                        .into(),
                    avatar: info
                        .get("avt")
                        .or_else(|| info.get("avatar"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    ..Default::default()
                };
                thread.apply_group_sidebar_metadata(info);
                result.push(thread);
            }
        }
        return Ok(Reply {
            related: Some(result),
            ..Default::default()
        });
    }
    if !matches!(action, Action::Create { .. }) {
        let state = group_state::snapshot(s, target, true).await?;
        ensure!(!state.removed, "Bạn không còn trong nhóm");
        if matches!(action, Action::Inspect) {
            return Ok(Reply {
                info: Some(state.info),
                ..Default::default()
            });
        }
        match action {
            Action::Disband | Action::Transfer { .. } | Action::Admin { .. } => ensure!(
                state.creator() == Some(s.account_key()),
                "Chỉ trưởng nhóm được thực hiện thao tác này"
            ),
            Action::Kick { member } => {
                ensure!(
                    state.manager(s.account_key())
                        && state.creator() != Some(member)
                        && member != s.account_key(),
                    "Không có quyền loại thành viên này"
                );
                ensure!(
                    !state.manager(member) || state.creator() == Some(s.account_key()),
                    "Phó nhóm không được loại quản trị viên khác"
                );
            }
            Action::Invite { .. } => {
                let locked = state
                    .info
                    .pointer("/setting/lockAddMember")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    != 0;
                ensure!(
                    !locked || state.manager(s.account_key()),
                    "Nhóm hạn chế thêm thành viên"
                );
            }
            _ => {}
        }
    }
    let (path, body, get) = request(s, target, action)?;
    let params = s.encoded_params(&body)?;
    let url = format!("{}{path}?{}", s.domains.group, s.common_params());
    let response = if get {
        s.get(&format!("{url}&params={params}")).await
    } else {
        s.post_form(&url, format!("params={params}")).await
    };
    group_state::invalidate(s, Some(target));
    let raw = s.maybe_decrypt(response?, s.session_key_bytes()).await;
    // An envelope must explicitly acknowledge success; a bare object cannot commit a mutation.
    ensure!(
        raw.get("error_code").is_some(),
        "Thiếu xác nhận server; tải lại nhóm trước khi thử tiếp"
    );
    crate::board::data(raw)?;
    ensure!(s.is_active(), "Phiên làm việc đã đóng");
    Ok(Reply {
        acknowledged: true,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destructive_paths_and_fields_are_separate_from_delete_conversation() {
        let s = Session::new(false, false, false).unwrap();
        let (path, body, get) = request(&s, "g123", &Action::Leave).unwrap();
        assert_eq!(path, "/api/group/leave");
        assert_eq!(body["grids"], json!(["123"]));
        assert!(!get);
        let (path, body, _) = request(
            &s,
            "123",
            &Action::Transfer {
                member: "456".into(),
            },
        )
        .unwrap();
        assert_eq!(path, "/api/group/change-owner");
        assert_eq!(body["newAdminId"], "456");
        assert!(request(
            &s,
            "123",
            &Action::Kick {
                member: "not-an-id".into()
            }
        )
        .is_err());
    }
}
