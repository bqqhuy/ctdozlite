//! Explicit contact mutations from the PC 26.11.10 adapter; never automatically retried.
use crate::session::Session;
use anyhow::{ensure, Result};
use serde_json::{json, Value};

/// Contact operations are distinct from conversation deletion and request rejection.
#[derive(Clone, Debug)]
pub enum Action {
    /// Fetch current friendship before offering a destructive action.
    Inspect,
    /// Account-authorized QR profile URL, keyed by the exact recipient UID.
    Qr,
    LookupUsername {
        username: String,
    },
    CloseFriends,
    /// Explicit activity lookup, gated by the account privacy snapshot.
    Presence {
        official: bool,
    },
    /// Read one page of account-wide blocked contacts.
    Blocked {
        page: u32,
    },
    /// Block or unblock direct messages from this identity.
    Block(bool),
    /// Submit an explicit free-form Other report; blocking/deleting remain separate intents.
    ReportOther {
        description: String,
    },
    /// Report a group without implicitly leaving, blocking users or deleting history.
    ReportGroupOther {
        description: String,
    },
    /// Remove an existing friend without deleting local history.
    Remove,
    /// Set or remove a server alias.
    Alias(String),
}
impl Action {
    /// Read intents never set mutation-uncertainty or report a server change.
    pub fn mutation(&self) -> bool {
        !matches!(
            self,
            Self::Inspect
                | Self::Qr
                | Self::LookupUsername { .. }
                | Self::CloseFriends
                | Self::Blocked { .. }
                | Self::Presence { .. }
        )
    }
}
/// Build the exact service, path, method and parameters for a single operation.
/// # Errors
/// Rejects invalid recipient IDs and aliases before network access.
pub fn request(
    s: &Session,
    target: &str,
    action: &Action,
) -> Result<(String, &'static str, bool, Value)> {
    if matches!(action, Action::ReportGroupOther { .. }) {
        crate::server_pins::identity(target, true)?;
    } else if !matches!(
        action,
        Action::Blocked { .. } | Action::LookupUsername { .. } | Action::CloseFriends
    ) {
        ensure!(
            !target.is_empty()
                && target.len() <= 32
                && target.bytes().all(|b| b.is_ascii_digit())
                && target != s.account_key(),
            "Liên hệ không hợp lệ"
        );
    }
    let (host, path, get, body) = match action {
        Action::LookupUsername { username } => {
            let username = username.trim();
            ensure!(
                !username.is_empty()
                    && username.len() <= 128
                    && !username.chars().any(char::is_control),
                "Tên đăng nhập không hợp lệ"
            );
            (
                s.domains.friend.clone(),
                "/api/friend/search/by-user-name",
                true,
                json!({"user_name":username,"avatar_size":240}),
            )
        }
        Action::CloseFriends => (
            s.domains.profile.clone(),
            "/api/social/friend/getclosedfriends",
            true,
            json!({}),
        ),
        Action::Blocked { page } => {
            ensure!(
                (1..=400).contains(page),
                "Trang danh sách chặn không hợp lệ"
            );
            (
                s.domains.friend.clone(),
                "/api/friend/blocklistv2",
                true,
                json!({"page":page,"count":50,"avatar":75}),
            )
        }
        Action::Presence { .. } => {
            anyhow::bail!("Trạng thái hoạt động phải kiểm tra quyền riêng tư trước")
        }
        Action::Qr => (
            s.domains.friend.clone(),
            "/api/friend/mget-qr",
            false,
            json!({"fids":[target]}),
        ),
        Action::Inspect => (
            s.domains.friend.clone(),
            "/api/friend/reqstatus",
            true,
            json!({"fid":target,"imei":s.imei}),
        ),
        Action::Block(block) => (
            s.domains.friend.clone(),
            if *block {
                "/api/friend/block"
            } else {
                "/api/friend/unblock"
            },
            false,
            json!({"fid":target,"imei":s.imei}),
        ),
        Action::ReportOther { description } | Action::ReportGroupOther { description } => {
            let description = description.trim();
            ensure!(
                !description.is_empty()
                    && description.encode_utf16().count() <= 1000
                    && !description.chars().any(
                        |character| character.is_control() && !matches!(character, '\n' | '\t')
                    ),
                "Nội dung báo cáo phải có từ 1 đến 1000 ký tự UTF-16"
            );
            (
                s.domains.profile.clone(),
                "/api/social/profile/reportabuse",
                false,
                json!({"uidTo":if matches!(action,Action::ReportGroupOther {..}) {crate::server_pins::identity(target,true)?[1..].to_owned()}else{target.to_owned()},"type":if matches!(action,Action::ReportGroupOther {..}) {14}else{2},"reason":0,"content":description,"imei":s.imei}),
            )
        }
        Action::Remove => (
            s.domains.friend.clone(),
            "/api/friend/remove",
            false,
            json!({"fid":target,"imei":s.imei}),
        ),
        Action::Alias(alias) => {
            ensure!(
                alias.chars().count() <= 100 && !alias.chars().any(char::is_control),
                "Biệt danh không hợp lệ"
            );
            let host = crate::endpoints::service("alias")
                .ok_or_else(|| anyhow::anyhow!("Thiếu dịch vụ biệt danh"))?
                .to_owned();
            if alias.trim().is_empty() {
                (host, "/api/alias/remove", true, json!({"friendId":target}))
            } else {
                (
                    host,
                    "/api/alias/update",
                    true,
                    json!({"friendId":target,"alias":alias.trim(),"imei":s.imei}),
                )
            }
        }
    };
    Ok((host, path, get, body))
}
/// Account-wide page of blocked contacts, not limited to current friends.
#[derive(Clone, Debug)]
pub struct BlockPage {
    pub page: u32,
    pub more: bool,
    pub users: Vec<(String, String)>,
}
/// Public profile lookup result; this does not establish friendship or create a conversation.
#[derive(Clone, Debug)]
pub struct Lookup {
    pub uid: String,
    pub name: String,
}
fn parse_lookup(data: &Value) -> Result<Lookup> {
    let uid = data
        .get("uid")
        .or_else(|| data.get("userId"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_u64().map(|id| id.to_string()))
        })
        .ok_or_else(|| anyhow::anyhow!("Thiếu UID trong hồ sơ tra cứu"))?;
    ensure!(
        !uid.is_empty() && uid.len() <= 32 && uid.bytes().all(|byte| byte.is_ascii_digit()),
        "UID tra cứu không hợp lệ"
    );
    let name = ["display_name", "displayName", "zalo_name", "zaloName"]
        .iter()
        .find_map(|key| {
            data.get(*key)
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty() && name.len() <= 1024)
        })
        .ok_or_else(|| anyhow::anyhow!("Thiếu tên hồ sơ tra cứu"))?
        .to_owned();
    Ok(Lookup { uid, name })
}
/// A read may carry relation or block-list metadata; mutation ACKs do not invent either.
#[derive(Clone, Debug, Default)]
pub struct Reply {
    pub lookup: Option<Lookup>,
    pub close_friends: Option<Vec<Lookup>>,
    pub friendship: Option<crate::chat::FriendshipStatus>,
    pub presence: Option<crate::last_activity::Status>,
    pub qr: Option<String>,
    pub aliases: Option<std::collections::HashMap<String, String>>,
    pub blocked: Option<BlockPage>,
}
/// Parse the official blocklistv2 profiles/has_more response.
/// # Errors
/// Rejects missing lists, malformed cursors and invalid profile IDs.
pub fn parse_blocked(data: Value, page: u32) -> Result<BlockPage> {
    use anyhow::Context;
    let profiles = data
        .get("profiles")
        .and_then(Value::as_array)
        .context("Thiếu danh sách chặn")?;
    ensure!(profiles.len() <= 50, "Trang danh sách chặn quá lớn");
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
        .context("Thiếu trạng thái trang danh sách chặn")?;
    let mut users = Vec::new();
    let mut unique = std::collections::HashSet::new();
    for profile in profiles {
        let id = profile
            .get("id")
            .or_else(|| profile.get("userId"))
            .and_then(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| value.as_u64().map(|id| id.to_string()))
            })
            .context("Thiếu ID người bị chặn")?;
        ensure!(
            !id.is_empty() && id.len() <= 32 && id.bytes().all(|byte| byte.is_ascii_digit()),
            "ID người bị chặn không hợp lệ"
        );
        let name = profile
            .get("displayName")
            .or_else(|| profile.get("zaloName"))
            .and_then(Value::as_str)
            .filter(|name| name.len() <= 1024)
            .unwrap_or("Chưa có tên")
            .to_owned();
        if unique.insert(id.clone()) {
            users.push((id, name));
        }
    }
    Ok(BlockPage { page, more, users })
}
/// Execute once. An uncertain acknowledgement must be reconciled before another mutation.
/// # Errors
/// Returns validation, account, transport and explicit server errors.
pub async fn execute(s: &Session, target: &str, action: &Action) -> Result<Reply> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    if let Action::Presence { official } = action {
        return Ok(Reply {
            presence: Some(crate::last_activity::read(s, target, *official).await?),
            ..Default::default()
        });
    }
    let (host, path, get, body) = request(s, target, action)?;
    if matches!(action, Action::Remove) {
        ensure!(
            crate::chat::check_friendship_status(s, target).await?
                == crate::chat::FriendshipStatus::Friend,
            "Chưa xác minh đây là bạn bè"
        );
    }
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
        "Thiếu xác nhận máy chủ; không tự gửi lại"
    );
    let data = crate::api_response::checked_data(raw.clone())?;
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    if let Action::Alias(alias) = action {
        let aliases = tokio::time::timeout(
            std::time::Duration::from_secs(45),
            crate::account_metadata::aliases(s),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Hết giờ đọc lại biệt danh"))??;
        let expected = (!alias.trim().is_empty()).then(|| alias.trim().to_owned());
        ensure!(
            aliases.get(target) == expected.as_ref(),
            "Chưa xác nhận biệt danh mới; tải lại trước khi thử tiếp"
        );
        return Ok(Reply {
            aliases: Some(aliases),
            ..Default::default()
        });
    }
    if matches!(action, Action::Remove) {
        if let Ok(mut policy) = s.inbox_policy.write() {
            if let Some(friends) = policy.friends.as_mut() {
                friends.remove(target);
            }
        }
    }
    Ok(match action {
        Action::LookupUsername { .. } => Reply {
            lookup: Some(parse_lookup(&data)?),
            ..Default::default()
        },
        Action::CloseFriends => {
            let rows = data
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Danh sách bạn thân sai định dạng"))?;
            ensure!(rows.len() <= 20_000, "Danh sách bạn thân quá lớn");
            Reply {
                close_friends: Some(rows.iter().map(parse_lookup).collect::<Result<Vec<_>>>()?),
                ..Default::default()
            }
        }
        Action::Qr => {
            let url = data
                .get(target)
                .and_then(Value::as_str)
                .filter(|url| url.len() <= 8192)
                .ok_or_else(|| anyhow::anyhow!("Máy chủ chưa cung cấp mã QR cho liên hệ"))?;
            let parsed = reqwest::Url::parse(url)
                .map_err(|_| anyhow::anyhow!("Đường dẫn QR không hợp lệ"))?;
            ensure!(
                crate::media_download::allowed(&parsed),
                "Nguồn ảnh QR chưa được hỗ trợ"
            );
            Reply {
                qr: Some(url.to_owned()),
                ..Default::default()
            }
        }
        Action::Inspect => Reply {
            friendship: Some(crate::chat::parse_friendship_status_response(&raw)),
            ..Default::default()
        },
        Action::Remove => Reply {
            friendship: Some(crate::chat::FriendshipStatus::Unknown),
            ..Default::default()
        },
        Action::Blocked { page } => Reply {
            blocked: Some(parse_blocked(data, *page)?),
            ..Default::default()
        },
        _ => Reply::default(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activity_and_relation_reads_are_not_mutations() {
        for action in [
            Action::Inspect,
            Action::Blocked { page: 1 },
            Action::Presence { official: false },
            Action::Presence { official: true },
        ] {
            assert!(!action.mutation());
        }
        assert!(Action::Block(false).mutation());
    }
    #[test]
    fn block_remove_and_alias_have_distinct_pc_contracts() {
        let s = Session::new(false, false, false).unwrap();
        let (_, path, get, body) = request(&s, "123", &Action::Block(false)).unwrap();
        assert_eq!(path, "/api/friend/unblock");
        assert!(!get);
        assert_eq!(body["fid"], "123");
        assert_eq!(
            request(&s, "123", &Action::Remove).unwrap().1,
            "/api/friend/remove"
        );
        let (_, path, get, body) = request(&s, "123", &Action::Alias("Tên 😀".into())).unwrap();
        assert_eq!(path, "/api/alias/update");
        assert!(get);
        assert_eq!(body["alias"], "Tên 😀");
        assert!(request(&s, "g123", &Action::Block(true)).is_err());
        assert!(request(&s, "123", &Action::Alias("bad\nname".into())).is_err());
    }
}

#[cfg(test)]
mod blocklist_report_tests {
    use super::*;
    #[test]
    fn blocked_pages_require_explicit_pagination_and_reports_do_not_bundle_other_mutations() {
        let page = parse_blocked(
            json!({"profiles":[{"id":"123","zaloName":"Người dùng"}],"has_more":1}),
            2,
        )
        .unwrap();
        assert_eq!(page.page, 2);
        assert!(page.more);
        assert_eq!(page.users[0].0, "123");
        assert!(parse_blocked(json!({"profiles":[]}), 1).is_err());
        let s = Session::new(false, false, false).unwrap();
        let (host, path, get, body) = request(
            &s,
            "123",
            &Action::ReportOther {
                description: "Mô tả báo cáo".into(),
            },
        )
        .unwrap();
        assert_eq!(host, s.domains.profile);
        assert_eq!(path, "/api/social/profile/reportabuse");
        assert!(!get);
        assert_eq!(body["uidTo"], "123");
        assert_eq!(body["type"], 2);
        assert_eq!(body["reason"], 0);
        assert!(body.get("block").is_none());
        assert!(body.get("delete").is_none());
        assert!(request(
            &s,
            "123",
            &Action::ReportOther {
                description: "😀".repeat(501)
            }
        )
        .is_err());
    }
}

#[cfg(test)]
mod qr_group_report_tests {
    use super::*;
    #[test]
    fn qr_and_group_reports_have_distinct_read_and_mutation_intents() {
        let session = Session::new(false, false, false).unwrap();
        let (_, path, get, body) = request(&session, "123", &Action::Qr).unwrap();
        assert_eq!(path, "/api/friend/mget-qr");
        assert!(!get);
        assert_eq!(body, json!({"fids":["123"]}));
        assert!(!Action::Qr.mutation());
        let (_, path, get, body) = request(
            &session,
            "g456",
            &Action::ReportGroupOther {
                description: "Nội dung do tôi nhập".into(),
            },
        )
        .unwrap();
        assert_eq!(path, "/api/social/profile/reportabuse");
        assert!(!get);
        assert_eq!(body["uidTo"], "456");
        assert_eq!(body["type"], 14);
        assert_eq!(body["reason"], 0);
        assert!(body.get("block").is_none());
        assert!(body.get("delete").is_none());
    }
}

#[cfg(test)]
mod username_tests {
    use super::*;
    #[test]
    fn username_lookup_is_a_bounded_read_and_does_not_add_a_friend() {
        let session = Session::new(false, false, false).unwrap();
        let (_, path, get, body) = request(
            &session,
            "",
            &Action::LookupUsername {
                username: "  ten.dangnhap  ".into(),
            },
        )
        .unwrap();
        assert_eq!(path, "/api/friend/search/by-user-name");
        assert!(get);
        assert_eq!(body, json!({"user_name":"ten.dangnhap","avatar_size":240}));
        assert!(!Action::LookupUsername {
            username: "abc".into()
        }
        .mutation());
        let profile = parse_lookup(&json!({"uid":"12","display_name":"Tên 🎉"})).unwrap();
        assert_eq!(profile.uid, "12");
        assert!(parse_lookup(&json!({"uid":"invalid","display_name":"Tên"})).is_err());
        assert!(request(
            &session,
            "",
            &Action::LookupUsername {
                username: "bad\nname".into()
            }
        )
        .is_err());
    }
}
