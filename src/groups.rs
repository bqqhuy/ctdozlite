//! Explicit group management intents from PC 26.11.10; mutations are never replayed.
use crate::{chat::Thread, group_state, session::Session};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

/// One user intent, retained until a terminal acknowledgement or an uncertain failure.
#[derive(Clone, Debug)]
pub enum Action {
    /// Refresh permissions and display metadata.
    Inspect,
    /// Query the authenticated account's upgrade capability for this group.
    CommunityQuota,
    /// Upgrade only after fresh owner and server capability checks.
    UpgradeCommunity,
    /// Rename the group after refreshing its current permissions.
    Rename { name: String },
    /// Change one documented server group setting.
    Setting { setting: Setting, enabled: bool },
    /// Read the current invitation link and its enabled state.
    Link,
    /// Rotate the invitation link, invalidating the previous link.
    RenewLink,
    /// Disable the current invitation link.
    DisableLink,
    /// Read users awaiting membership review.
    Pending,
    /// Read one bounded page of members blocked from rejoining.
    Blocked { page: u32 },
    /// Block or unblock explicitly selected members from rejoining.
    Ban { members: Vec<String>, blocked: bool },
    /// Review explicitly selected pending members.
    Review { members: Vec<String>, approve: bool },
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
/// Supported PC setting keys; arbitrary JSON fields cannot reach the server.
#[derive(Clone, Copy, Debug)]
pub enum Setting {
    LockName,
    LockPoll,
    LockPost,
    LockSend,
    ApproveMembers,
}
impl Setting {
    /// Return the exact PC adapter field name.
    pub fn key(self) -> &'static str {
        match self {
            Self::LockName => "blockName",
            Self::LockPoll => "lockCreatePoll",
            Self::LockPost => "lockCreatePost",
            Self::LockSend => "lockSendMsg",
            Self::ApproveMembers => "joinAppr",
        }
    }
}
impl Action {
    /// Reads do not require confirmation and may be refreshed safely.
    pub fn mutation(&self) -> bool {
        !matches!(
            self,
            Self::Inspect
                | Self::Related
                | Self::Link
                | Self::Pending
                | Self::Blocked { .. }
                | Self::CommunityQuota
        )
    }
}
/// Result values contain metadata, not session credentials.
#[derive(Clone, Debug, Default)]
pub struct Reply {
    /// Updated group metadata, when this was an inspection.
    pub info: Option<Value>,
    /// Explicitly requested auxiliary read result, kept separate from the group snapshot.
    pub detail: Option<(String, Value)>,
    /// One validated page; failure must not advance the UI cursor.
    pub blocked: Option<BlockedPage>,
    /// Fully validated pending members; malformed pages preserve the previous UI list.
    pub pending: Option<Vec<(String, String)>>,
    /// Server capability is separate from the group's role/membership snapshot.
    pub community: Option<CommunityQuota>,
    /// Hydrated common groups; a malformed response is an error rather than an empty list.
    pub related: Option<Vec<Thread>>,
    /// Explicit server success; invitation membership may still be pending.
    pub acknowledged: bool,
}
/// Return only a currently enabled, nonexpired official invitation URL.
/// # Errors
/// Unknown enabled/expiration fields never produce a shareable QR or link.
pub fn shareable_link(data: &Value, now: i64) -> Result<&str> {
    let enabled = data.get("enabled").and_then(|value| {
        value
            .as_bool()
            .or_else(|| match crate::api_response::integer(value)? {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            })
    });
    ensure!(enabled == Some(true), "Link đang tắt hoặc chưa xác minh");
    let expires = data
        .get("expiration_date")
        .and_then(crate::api_response::integer)
        .map(crate::clock::normalize_ts_ms)
        .context("Chưa xác minh hạn link mời")?;
    ensure!(expires > now, "Link mời đã hết hạn");
    crate::group_entry::validate_link(
        data.get("link")
            .and_then(Value::as_str)
            .context("Thiếu link mời")?,
    )
}

/// Explicit upgrade capability returned for the current account and group.
#[derive(Clone, Debug)]
pub struct CommunityQuota {
    pub can_upgrade: bool,
    pub message: Option<String>,
}
/// Parse a capability without assuming that an absent flag permits an upgrade.
/// # Errors
/// Rejects an unsupported or missing canUpgr value.
pub fn parse_community(data: &Value) -> Result<CommunityQuota> {
    let can_upgrade = data
        .get("canUpgr")
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| match crate::api_response::integer(value)? {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                })
        })
        .context("Chưa xác minh được điều kiện nâng cấp cộng đồng")?;
    let message = data
        .get("content")
        .and_then(Value::as_str)
        .filter(|text| text.len() <= 4000)
        .map(str::to_owned);
    Ok(CommunityQuota {
        can_upgrade,
        message,
    })
}
/// PC's member picker restricts direct additions when addMemberOnly or joinAppr is enabled.
/// Missing settings remain unknown for ordinary members; managers still pass the role gate.
pub fn can_invite(info: &Value, manager: bool) -> bool {
    manager
        || ["addMemberOnly", "joinAppr"].into_iter().all(|key| {
            info.get("setting")
                .and_then(|setting| setting.get(key))
                .and_then(crate::api_response::integer)
                == Some(0)
        })
}
/// Validate every pending member instead of silently truncating or dropping malformed rows.
/// # Errors
/// Rejects oversized lists, invalid or duplicate identities and oversized names.
pub fn parse_pending(data: &Value) -> Result<Vec<(String, String)>> {
    let users = data
        .get("users")
        .and_then(Value::as_array)
        .context("Thiếu danh sách chờ duyệt")?;
    ensure!(
        users.len() <= 10_000,
        "Danh sách chờ duyệt vượt giới hạn hỗ trợ"
    );
    let mut seen = std::collections::HashSet::new();
    users
        .iter()
        .map(|user| {
            let id = user
                .get("uid")
                .and_then(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| value.as_u64().map(|id| id.to_string()))
                })
                .context("Thiếu UID chờ duyệt")?;
            uid(&id)?;
            ensure!(seen.insert(id.clone()), "UID chờ duyệt bị trùng");
            let name = user
                .get("dName")
                .and_then(Value::as_str)
                .unwrap_or("Thành viên");
            ensure!(name.len() <= 1024, "Tên thành viên chờ duyệt quá dài");
            Ok((id, name.to_owned()))
        })
        .collect()
}
/// Validated blocked-member page, separate from the management snapshot.
#[derive(Clone, Debug)]
pub struct BlockedPage {
    pub page: u32,
    pub more: bool,
    pub members: Vec<String>,
}
/// Parse a page without confusing an incomplete response with an empty block list.
/// # Errors
/// Rejects invalid member identities, oversized pages and absent pagination status.
pub fn parse_blocked(data: &Value, page: u32) -> Result<BlockedPage> {
    let rows = data
        .get("blocked_members")
        .and_then(Value::as_array)
        .context("Thiếu danh sách chặn của nhóm")?;
    ensure!(rows.len() <= 50, "Trang danh sách chặn quá lớn");
    let more = data
        .get("has_more")
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| match crate::api_response::integer(value)? {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                })
        })
        .context("Thiếu trạng thái trang chặn của nhóm")?;
    let mut members = Vec::new();
    let mut unique = std::collections::HashSet::new();
    for row in rows {
        let value = row.get("id").or_else(|| row.get("uid")).unwrap_or(row);
        let id = value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_u64().map(|id| id.to_string()))
            .context("Thiếu UID bị chặn")?;
        uid(&id)?;
        if unique.insert(id.clone()) {
            members.push(id);
        }
    }
    Ok(BlockedPage {
        page,
        more,
        members,
    })
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
        Action::CommunityQuota => (
            "/api/group/quota/community",
            json!({"grId":target,"language":"vi"}),
            true,
        ),
        Action::UpgradeCommunity => (
            "/api/group/upgrade/community",
            json!({"grId":target,"language":"vi"}),
            true,
        ),
        Action::Rename { name } => {
            ensure!(
                !name.trim().is_empty() && name.chars().count() <= 100,
                "Tên nhóm phải có từ 1 đến 100 ký tự"
            );
            (
                "/api/group/updateinfo",
                json!({"grid":target,"gname":name.trim()}),
                false,
            )
        }
        Action::Setting { setting, enabled } => {
            let mut body = json!({"grid":target});
            body[setting.key()] = json!(i32::from(*enabled));
            ("/api/group/setting/update", body, true)
        }
        Action::Link => ("/api/group/link/detail", json!({"grid":target}), true),
        Action::RenewLink => ("/api/group/link/new", json!({"grid":target}), true),
        Action::DisableLink => ("/api/group/link/disable", json!({"grid":target}), true),
        Action::Pending => ("/api/group/pending-mems/list", json!({"grid":target}), true),
        Action::Blocked { page } => {
            ensure!(
                (1..=100).contains(page),
                "Trang danh sách chặn không hợp lệ"
            );
            (
                "/api/group/blockedmems/list",
                json!({"grid":target,"page":page,"count":50}),
                true,
            )
        }
        Action::Ban {
            members: ids,
            blocked,
        } => {
            members(ids)?;
            (
                if *blocked {
                    "/api/group/blockedmems/add"
                } else {
                    "/api/group/blockedmems/remove"
                },
                json!({"grid":target,"members":ids}),
                true,
            )
        }
        Action::Review {
            members: ids,
            approve,
        } => {
            members(ids)?;
            (
                "/api/group/pending-mems/review",
                json!({"grid":target,"members":ids,"isApprove":i32::from(*approve)}),
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
    if !matches!(
        action,
        Action::DisableLink
            | Action::Review { .. }
            | Action::Ban { .. }
            | Action::CommunityQuota
            | Action::UpgradeCommunity
    ) {
        body["imei"] = json!(s.imei);
    }
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
        let data =
            crate::api_response::checked_data(s.maybe_decrypt(raw, s.session_key_bytes()).await)?;
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
        if matches!(action, Action::UpgradeCommunity) {
            ensure!(
                state
                    .info
                    .get("type")
                    .and_then(crate::api_response::integer)
                    == Some(1),
                "Nhóm đã là cộng đồng hoặc chưa xác minh được loại nhóm"
            );
        }
        if let Action::Transfer { member } = action {
            ensure!(
                state.creator_is(s.account_key()),
                "Chỉ trưởng nhóm được chuyển quyền"
            );
            if state
                .info
                .get("type")
                .and_then(crate::api_response::integer)
                == Some(2)
            {
                uid(member)?;
                let params =
                    s.encoded_params(&json!({"language":"vi","grId":grid(target)?,"uid":member}))?;
                let raw = s
                    .get(&format!(
                        "{}/api/group/quota/members-comm?{}&params={params}",
                        s.domains.group,
                        s.common_params()
                    ))
                    .await?;
                let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
                ensure!(
                    raw.get("error_code").is_some(),
                    "Thiếu xác nhận điều kiện trưởng cộng đồng"
                );
                let data = crate::api_response::checked_data(raw)?;
                ensure!(
                    parse_community(&data)?.can_upgrade,
                    "Thành viên được chọn chưa đủ điều kiện làm trưởng cộng đồng"
                );
            }
        }
        match action {
            Action::Disband
            | Action::Transfer { .. }
            | Action::Admin { .. }
            | Action::UpgradeCommunity => ensure!(
                state.creator_is(s.account_key()),
                "Chỉ trưởng nhóm được thực hiện thao tác này"
            ),
            Action::Ban { members: ids, .. } => {
                ensure!(
                    state.manager(s.account_key()),
                    "Chỉ quản trị viên được chặn thành viên"
                );
                for id in ids {
                    ensure!(
                        id != s.account_key() && !state.manager(id),
                        "Không được chặn quản trị viên hoặc chính mình"
                    );
                }
            }
            Action::Blocked { .. }
            | Action::Setting { .. }
            | Action::RenewLink
            | Action::DisableLink
            | Action::Pending
            | Action::Review { .. } => {
                ensure!(
                    state.manager(s.account_key()),
                    "Chỉ quản trị viên được thực hiện thao tác này"
                );
            }
            Action::Rename { .. } => {
                let locked = state
                    .info
                    .pointer("/setting/blockName")
                    .and_then(Value::as_i64)
                    != Some(0);
                ensure!(
                    !locked || state.manager(s.account_key()),
                    "Nhóm hạn chế đổi tên"
                );
            }
            Action::Kick { member } => {
                ensure!(
                    state.manager(s.account_key())
                        && !state.creator_is(member)
                        && member != s.account_key(),
                    "Không có quyền loại thành viên này"
                );
                ensure!(
                    !state.manager(member) || state.creator_is(s.account_key()),
                    "Phó nhóm không được loại quản trị viên khác"
                );
            }
            Action::Invite { .. } => {
                ensure!(
                    can_invite(&state.info, state.manager(s.account_key())),
                    "Nhóm hạn chế thêm thành viên hoặc chưa xác minh quyền; nhờ quản trị viên mời"
                );
            }
            _ => {}
        }
    }
    if matches!(action, Action::UpgradeCommunity) {
        let (path, body, _) = request(s, target, &Action::CommunityQuota)?;
        let params = s.encoded_params(&body)?;
        let raw = s
            .get(&format!(
                "{}{path}?{}&params={params}",
                s.domains.group,
                s.common_params()
            ))
            .await?;
        let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
        ensure!(
            raw.get("error_code").is_some(),
            "Thiếu xác nhận điều kiện nâng cấp cộng đồng"
        );
        let data = crate::api_response::checked_data(raw)?;
        ensure!(
            parse_community(&data)?.can_upgrade,
            "Máy chủ chưa cho phép tài khoản nâng cấp nhóm này"
        );
    }
    let (path, body, get) = request(s, target, action)?;
    let params = s.encoded_params(&body)?;
    let url = format!("{}{path}?{}", s.domains.group, s.common_params());
    let response = if get {
        s.get(&format!("{url}&params={params}")).await
    } else {
        s.post_form(&url, format!("params={params}")).await
    };
    if action.mutation() {
        group_state::invalidate(s, Some(target));
        if let Err(error) = crate::group_metadata::invalidate(s, target).await {
            crate::logger::log_error("group display cache invalidation", &error.to_string());
        }
    }
    let raw = s.maybe_decrypt(response?, s.session_key_bytes()).await;
    // An envelope must explicitly acknowledge success; a bare object cannot commit a mutation.
    ensure!(
        raw.get("error_code").is_some(),
        "Thiếu xác nhận server; tải lại nhóm trước khi thử tiếp"
    );
    let data = crate::api_response::checked_data(raw)?;
    ensure!(s.is_active(), "Phiên làm việc đã đóng");
    if matches!(action, Action::UpgradeCommunity) {
        let after = group_state::snapshot(s, target, true).await?;
        ensure!(
            !after.removed
                && after
                    .info
                    .get("type")
                    .and_then(crate::api_response::integer)
                    == Some(2),
            "Chưa xác nhận nhóm đã chuyển thành cộng đồng; tải lại trước khi thử tiếp"
        );
    }
    let community = if matches!(action, Action::CommunityQuota) {
        Some(parse_community(&data)?)
    } else {
        None
    };
    let blocked = if let Action::Blocked { page } = action {
        Some(parse_blocked(&data, *page)?)
    } else {
        None
    };
    let pending = if let Action::Review { members, .. } = action {
        let (path, body, _) = request(s, target, &Action::Pending)?;
        let params = s.encoded_params(&body)?;
        let raw = s
            .get(&format!(
                "{}{path}?{}&params={params}",
                s.domains.group,
                s.common_params()
            ))
            .await?;
        let rows = parse_pending(&crate::api_response::checked_data(
            s.maybe_decrypt(raw, s.session_key_bytes()).await,
        )?)?;
        ensure!(
            members
                .iter()
                .all(|member| !rows.iter().any(|(id, _)| id == member)),
            "Chưa xác nhận mọi yêu cầu đã được xử lý; tải lại danh sách trước khi thử tiếp"
        );
        Some(rows)
    } else if matches!(action, Action::Pending) {
        Some(parse_pending(&data)?)
    } else {
        None
    };
    Ok(Reply {
        pending,
        community,
        blocked,
        detail: match action {
            Action::Link => Some(("link".into(), data)),

            _ => None,
        },
        acknowledged: action.mutation(),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invite_permission_uses_native_settings_and_does_not_default_missing_to_allow() {
        assert!(!can_invite(&json!({"setting":{"lockAddMember":0}}), false));
        assert!(can_invite(&json!({}), true));
        assert!(can_invite(
            &json!({"setting":{"addMemberOnly":0,"joinAppr":0}}),
            false
        ));
        for setting in [
            json!({"addMemberOnly":1,"joinAppr":0}),
            json!({"addMemberOnly":0,"joinAppr":1}),
        ] {
            assert!(!can_invite(&json!({"setting":setting}), false));
        }
    }
    #[test]
    fn pending_list_never_silently_drops_invalid_members() {
        assert!(parse_pending(&json!({})).is_err());
        assert!(parse_pending(&json!({"users":[{"uid":"12"},{}]})).is_err());
        assert!(parse_pending(&json!({"users":[{"uid":"12"},{"uid":12}]})).is_err());
        let rows = (1..=2100)
            .map(|id| json!({"uid":id,"dName":"Tên"}))
            .collect::<Vec<_>>();
        assert_eq!(parse_pending(&json!({"users":rows})).unwrap().len(), 2100);
        assert!(parse_pending(&json!({"users":[]})).unwrap().is_empty());
    }
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

#[cfg(test)]
mod pc_contract_tests {
    use super::*;
    #[test]
    fn requests_match_the_pc_adapter_contract_fixture() {
        let mut session = Session::new(false, false, false).unwrap();
        session.imei.clear();
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../re-notes/fixtures/v008-group-requests.json"
        ))
        .unwrap();
        for case in cases {
            let action = match case["case"].as_str().unwrap() {
                "rename" => Action::Rename {
                    name: "Nhóm 😀".into(),
                },
                "lock_name" => Action::Setting {
                    setting: Setting::LockName,
                    enabled: true,
                },
                "approval" => Action::Setting {
                    setting: Setting::ApproveMembers,
                    enabled: true,
                },
                "link" => Action::Link,
                "disable_link" => Action::DisableLink,
                "pending" => Action::Pending,
                "review" => Action::Review {
                    members: vec!["456".into()],
                    approve: true,
                },
                "blocked" => Action::Blocked { page: 1 },
                "ban" => Action::Ban {
                    members: vec!["456".into()],
                    blocked: true,
                },
                other => panic!("unknown fixture case {other}"),
            };
            let (path, body, get) = request(&session, "g123", &action).unwrap();
            assert_eq!(path, case["path"].as_str().unwrap(), "{}", case["case"]);
            assert_eq!(get, case["get"].as_bool().unwrap(), "{}", case["case"]);
            assert_eq!(body, case["body"], "{}", case["case"]);
        }
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;
    #[test]
    fn unknown_capability_or_pagination_does_not_grant_an_action_or_advance_a_page() {
        assert!(parse_community(&json!({})).is_err());
        assert!(
            !parse_community(&json!({"canUpgr":false}))
                .unwrap()
                .can_upgrade
        );
        assert!(parse_blocked(&json!({"blocked_members":[]}), 3).is_err());
        let s = Session::new(false, false, false).unwrap();
        let (path, body, get) = request(&s, "g123", &Action::UpgradeCommunity).unwrap();
        assert_eq!(path, "/api/group/upgrade/community");
        assert!(get);
        assert_eq!(body, json!({"grId":"123","language":"vi"}));
        assert!(shareable_link(
            &json!({"enabled":true,"link":"https://zalo.me/g/abc","expiration_date":1000}),
            2000000
        )
        .is_err());
        assert!(shareable_link(&json!({"enabled":true,"link":"https://attacker.example/g/abc","expiration_date":1700000000000_i64}),1000).is_err());
    }
}
