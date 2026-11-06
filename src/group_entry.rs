//! Joining and invitation-box operations do not require existing group membership.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
/// Invitation status remains distinct from established membership.
#[derive(Clone, Debug)]
pub struct Invitation {
    pub group: String,
    pub name: String,
    pub expires: i64,
    pub pending: bool,
}
/// One explicit page and cursor, committed only after successful parsing.
#[derive(Clone, Debug)]
pub struct Page {
    pub page: u32,
    pub more: bool,
    pub total: u64,
    pub last: Option<String>,
    pub invitations: Vec<Invitation>,
}
/// Link information is a preview, not evidence that the account belongs to the group.
#[derive(Clone, Debug)]
pub struct Preview {
    pub group: String,
    pub name: String,
    pub members: Option<u64>,
}
/// Entry-point reads/mutations are typed; no arbitrary endpoint or payload is accepted.
#[derive(Clone, Debug)]
pub enum Action {
    Invitations { page: u32, last: Option<String> },
    LinkInfo { link: String },
    JoinLink { link: String },
    JoinInvitation { group: String },
    RejectInvitation { group: String, block_group: bool },
}
impl Action {
    /// Mutations are never automatically replayed after an uncertain acknowledgement.
    pub fn mutation(&self) -> bool {
        !matches!(self, Self::Invitations { .. } | Self::LinkInfo { .. })
    }
}
/// The UI reports an acknowledged request or pending review, never optimistic membership.
#[derive(Clone, Debug, Default)]
pub struct Reply {
    pub page: Option<Page>,
    pub preview: Option<Preview>,
    pub acknowledged: bool,
    pub pending: bool,
}
fn bare(group: &str) -> Result<String> {
    Ok(crate::server_pins::identity(group, true)?[1..].to_owned())
}
fn id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|id| id.to_string()))
}
fn flag(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| match crate::api_response::integer(value)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        })
}
/// Validate public group invitations without fetching or opening an external URL.
/// # Errors
/// Rejects other hosts/schemes and unrecognized group-link paths.
pub(crate) fn validate_link(value: &str) -> Result<&str> {
    let parsed = reqwest::Url::parse(value.trim()).context("Link mời không hợp lệ")?;
    ensure!(
        parsed.scheme() == "https"
            && parsed.host_str() == Some(crate::endpoints::GROUP_INVITE_HOST)
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.port().is_none(),
        "Chỉ hỗ trợ link mời HTTPS của zalo.me"
    );
    let segments: Vec<_> = parsed
        .path_segments()
        .context("Link mời thiếu đường dẫn")?
        .collect();
    ensure!(
        segments.len() == 2
            && segments[0] == "g"
            && !segments[1].is_empty()
            && segments[1].len() <= 128
            && segments[1].bytes().all(|byte| byte.is_ascii_alphanumeric()),
        "Đường dẫn link mời chưa được hỗ trợ"
    );
    ensure!(value.len() <= 2048, "Link mời quá dài");
    Ok(value.trim())
}
/// Construct the official invitation-box/link adapter request.
/// # Errors
/// Rejects invalid IDs, pagination and external URLs before contacting any service.
pub fn request(action: &Action) -> Result<(&'static str, Value)> {
    Ok(match action {
        Action::Invitations { page, last } => {
            ensure!(*page <= 400, "Đã đạt giới hạn trang lời mời");
            (
                "/api/group/inv-box/list",
                json!({"mpage":1,"page":page,"invPerPage":10,"mcount":10,"lastGroupId":last.as_deref().map(bare).transpose()?,"avatar_size":160,"member_avatar_size":160}),
            )
        }
        Action::LinkInfo { link: input } => (
            "/api/group/link/ginfo",
            json!({"link":validate_link(input)?,"avatar_size":160,"member_avatar_size":160,"mpage":1}),
        ),
        Action::JoinLink { link: input } => (
            "/api/group/link/join",
            json!({"link":validate_link(input)?,"clientLang":"vi"}),
        ),
        Action::JoinInvitation { group } => (
            "/api/group/inv-box/join",
            json!({"grid":bare(group)?,"lang":"vi"}),
        ),
        Action::RejectInvitation { group, block_group } => (
            "/api/group/inv-box/mdel-inv",
            json!({"invitations":serde_json::to_string(&[json!({"grid":bare(group)?})])?,"block":i32::from(*block_group)}),
        ),
    })
}
fn invitation(row: &Value) -> Result<Invitation> {
    let info = row
        .get("groupInfo")
        .context("Thiếu thông tin nhóm trong lời mời")?;
    let group = id(info.get("groupId").context("Thiếu ID nhóm trong lời mời")?)
        .context("ID nhóm không hợp lệ")?;
    let group = crate::server_pins::identity(&group, true)?;
    let name = info
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| name.len() <= 1024)
        .context("Thiếu tên nhóm trong lời mời")?
        .to_owned();
    let expires = row
        .get("expiredTs")
        .and_then(crate::api_response::integer)
        .map(crate::clock::normalize_ts_ms)
        .filter(|ts| *ts > 0)
        .context("Thiếu hạn lời mời")?;
    // Official manager recognizes type 1 as pending; ordinary invitations may omit the field.
    let status = match row.get("type") {
        None => 0,
        Some(value) => crate::api_response::integer(value)
            .filter(|value| matches!(value, 0 | 1))
            .context("Trạng thái lời mời chưa được hỗ trợ")?,
    };
    Ok(Invitation {
        group,
        name,
        expires,
        pending: status == 1,
    })
}
/// Decode a page without consuming/advancing the cursor on an incomplete response.
/// # Errors
/// Rejects missing list/pagination, invalid entries and inconsistent cursors.
pub fn parse_page(data: Value, page: u32) -> Result<Page> {
    let rows = data
        .get("invitations")
        .and_then(Value::as_array)
        .context("Thiếu danh sách lời mời")?;
    ensure!(rows.len() <= 50, "Trang lời mời quá lớn");
    let more = data
        .get("hasMore")
        .and_then(flag)
        .context("Thiếu trạng thái phân trang lời mời")?;
    let total = data
        .get("total")
        .and_then(Value::as_u64)
        .context("Thiếu tổng lời mời")?;
    let invitations = rows.iter().map(invitation).collect::<Result<Vec<_>>>()?;
    ensure!(
        !more || !invitations.is_empty(),
        "Trang lời mời không tiến; tải lại danh sách"
    );
    let last = invitations
        .last()
        .map(|invitation| invitation.group.clone());
    Ok(Page {
        page,
        more,
        total,
        last,
        invitations,
    })
}
async fn call(s: &Session, path: &str, body: Value) -> Result<Value> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
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
        "Thiếu xác nhận máy chủ cho thao tác nhóm"
    );
    Ok(raw)
}
/// Execute once after revalidating an invitation's current status/expiry.
/// # Errors
/// Returns malformed metadata, refusal, expiry and transport failures without retrying mutations.
pub async fn execute(s: &Session, action: &Action) -> Result<Reply> {
    let (path, body) = request(action)?;
    if let Action::JoinInvitation { group } | Action::RejectInvitation { group, .. } = action {
        let data = crate::api_response::checked_data(
            call(
                s,
                "/api/group/inv-box/inv-info",
                json!({"grId":bare(group)?,"mcount":10,"mpage":1}),
            )
            .await?,
        )?;
        let current = invitation(&data)?;
        ensure!(
            bare(&current.group)? == bare(group)?,
            "Lời mời không khớp nhóm"
        );
        ensure!(!current.pending, "Yêu cầu tham gia đang chờ duyệt");
        ensure!(
            current.expires > crate::login::unix_now() * 1000,
            "Lời mời đã hết hạn"
        );
    }
    let raw = call(s, path, body).await?;
    let data = match crate::api_response::checked_data(raw) {
        Ok(data) => data,
        Err(error)
            if crate::api_response::server_code(&error) == Some(240)
                && matches!(
                    action,
                    Action::JoinLink { .. } | Action::JoinInvitation { .. }
                ) =>
        {
            ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
            return Ok(Reply {
                pending: true,
                ..Default::default()
            });
        }
        Err(error) => return Err(error),
    };
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    match action {
        Action::Invitations { page, last } => {
            let parsed = parse_page(data, *page)?;
            ensure!(
                !parsed.more || last.as_ref() != parsed.last.as_ref(),
                "Cursor lời mời không tiến; tải lại danh sách"
            );
            Ok(Reply {
                page: Some(parsed),
                ..Default::default()
            })
        }
        Action::LinkInfo { .. } => {
            let group = id(data
                .get("groupId")
                .context("Thiếu ID trong thông tin link")?)
            .context("ID nhóm không hợp lệ")?;
            let group = crate::server_pins::identity(&group, true)?;
            let name = data
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| name.len() <= 1024)
                .context("Thiếu tên nhóm từ link")?
                .to_owned();
            Ok(Reply {
                preview: Some(Preview {
                    group,
                    name,
                    members: data.get("totalMember").and_then(Value::as_u64),
                }),
                ..Default::default()
            })
        }
        _ => Ok(Reply {
            acknowledged: true,
            ..Default::default()
        }),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invitation_rejection_does_not_block_the_inviter_and_cursor_cannot_stall() {
        let (path, body) = request(&Action::RejectInvitation {
            group: "g123".into(),
            block_group: true,
        })
        .unwrap();
        assert_eq!(path, "/api/group/inv-box/mdel-inv");
        assert_eq!(body["block"], 1);
        assert_eq!(
            serde_json::from_str::<Value>(body["invitations"].as_str().unwrap()).unwrap(),
            json!([{"grid":"123"}])
        );
        assert!(request(&Action::JoinLink {
            link: "https://attacker.example/g/abc".into()
        })
        .is_err());
        assert!(parse_page(json!({"invitations":[],"hasMore":true,"total":2}), 0).is_err());
        let page=parse_page(json!({"invitations":[{"groupInfo":{"groupId":"123","name":"Nhóm"},"expiredTs":1700000000000_i64,"type":1}],"hasMore":false,"total":1}),0).unwrap();
        assert!(page.invitations[0].pending);
    }
}
