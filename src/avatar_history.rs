//! Own-avatar history adapters from PC main-startup. Reuse never publishes a social-feed post.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
/// Keep the server wire type when reusing an ID; deletion explicitly stringifies it as PC does.
#[derive(Clone, Debug)]
pub struct PhotoId {
    pub text: String,
    wire: Value,
}
impl PhotoId {
    fn parse(value: &Value) -> Result<Self> {
        let text = value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_u64().map(|id| id.to_string()))
            .context("ID ảnh đại diện không hợp lệ")?;
        ensure!(
            !text.is_empty() && text.len() <= 64 && text.bytes().all(|byte| byte.is_ascii_digit()),
            "ID ảnh đại diện không hợp lệ"
        );
        Ok(Self {
            text,
            wire: value.clone(),
        })
    }
}
/// Public CDN previews carry no account credentials.
#[derive(Clone, Debug)]
pub struct Photo {
    pub id: PhotoId,
    pub thumb: Option<String>,
    pub original: Option<String>,
}
/// A page is committed only after its cursor and list have been validated.
#[derive(Clone, Debug)]
pub struct Page {
    pub page: u32,
    pub album: String,
    pub next: String,
    pub more: bool,
    pub photos: Vec<Photo>,
}
/// Explicit user intent; reuse and delete are never automatically retried.
#[derive(Clone, Debug)]
pub enum Action {
    Read {
        page: u32,
        photo: String,
        album: String,
    },
    Reuse {
        id: PhotoId,
    },
    Delete {
        id: PhotoId,
    },
}
impl Action {
    pub fn mutation(&self) -> bool {
        !matches!(self, Self::Read { .. })
    }
}
/// Read result or explicit mutation acknowledgement, not proof of CDN file deletion.
#[derive(Clone, Debug, Default)]
pub struct Reply {
    pub page: Option<Page>,
    pub acknowledged: bool,
    pub profile: Option<crate::profile::Snapshot>,
}
fn cursor(value: &str, empty: bool) -> Result<()> {
    ensure!(
        (empty || !value.is_empty())
            && value.len() <= 64
            && value.bytes().all(|byte| byte.is_ascii_digit()),
        "Cursor lịch sử ảnh không hợp lệ"
    );
    Ok(())
}
fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|id| id.to_string()))
}
fn url(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str().filter(|value| value.len() <= 8192)?;
    let parsed = reqwest::Url::parse(value).ok()?;
    crate::media_download::allowed(&parsed).then(|| value.to_owned())
}
/// Build PC's GET adapters; no UID can be supplied to act on another account's history.
/// # Errors
/// Rejects invalid page/cursor values before I/O.
pub fn request(action: &Action) -> Result<(&'static str, Value)> {
    Ok(match action {
        Action::Read { page, photo, album } => {
            ensure!(
                (1..=400).contains(page),
                "Đã đạt giới hạn trang lịch sử ảnh"
            );
            cursor(photo, true)?;
            cursor(album, false)?;
            let mut body = json!({"page":page,"albumId":album,"count":50});
            if !photo.is_empty() {
                body["photoId"] = json!(photo);
            }
            ("/api/social/avatar-list", body)
        }
        Action::Reuse { id } => (
            "/api/social/reuse-avatar",
            json!({"photoId":id.wire,"isPostSocial":0}),
        ),
        Action::Delete { id } => (
            "/api/social/del-avatars",
            json!({"delPhotos":serde_json::to_string(&[json!({"photoId":id.text})])?}),
        ),
    })
}
/// Parse the documented albumId/hasMore/nextPhotoId/photos shape.
/// # Errors
/// Missing lists/cursors and duplicate/invalid identities never become an empty history.
pub fn parse_page(data: Value, page: u32, previous: &str) -> Result<Page> {
    let album = data
        .get("albumId")
        .and_then(text)
        .context("Thiếu album lịch sử ảnh")?;
    cursor(&album, false)?;
    let more = data
        .get("hasMore")
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| match crate::api_response::integer(value)? {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                })
        })
        .context("Thiếu trạng thái trang lịch sử ảnh")?;
    let next = data.get("nextPhotoId").and_then(text).unwrap_or_default();
    cursor(&next, !more)?;
    let rows = data
        .get("photos")
        .and_then(Value::as_array)
        .context("Thiếu danh sách lịch sử ảnh")?;
    ensure!(rows.len() <= 50, "Trang lịch sử ảnh quá lớn");
    ensure!(
        !more || (!rows.is_empty() && next != previous),
        "Cursor lịch sử ảnh không tiến; tải lại danh sách"
    );
    let mut photos = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for row in rows {
        let id = PhotoId::parse(row.get("photoId").context("Thiếu ID ảnh lịch sử")?)?;
        ensure!(ids.insert(id.text.clone()), "ID ảnh lịch sử bị trùng");
        photos.push(Photo {
            id,
            thumb: url(row.get("thumbnail")),
            original: url(row.get("url")),
        });
    }
    Ok(Page {
        page,
        album,
        next,
        more,
        photos,
    })
}
/// Perform one own-account request; errors and per-photo refusals retain the old UI data.
/// # Errors
/// Returns account, transport, malformed response or explicit server refusal without retries.
pub async fn execute(s: &Session, action: &Action) -> Result<Reply> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    let (path, body) = request(action)?;
    let params = s.encoded_params(&body)?;
    let raw = s
        .get(&format!(
            "{}{path}?{}&params={params}",
            s.domains.profile,
            s.common_params()
        ))
        .await?;
    let data =
        crate::api_response::checked_data(s.maybe_decrypt(raw, s.session_key_bytes()).await)?;
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    match action {
        Action::Read { page, photo, .. } => Ok(Reply {
            page: Some(parse_page(data, *page, photo)?),
            ..Default::default()
        }),
        Action::Delete { id } => {
            if let Some(code) = data.get("errMap").and_then(|map| map.get(&id.text)) {
                ensure!(
                    crate::api_response::integer(code) == Some(0),
                    "Máy chủ chưa xác nhận xóa ảnh đã chọn"
                );
            }
            Ok(Reply {
                acknowledged: true,
                ..Default::default()
            })
        }
        Action::Reuse { .. } => Ok(Reply {
            acknowledged: true,
            profile: Some(crate::profile::execute(s, &crate::profile::Action::Read).await?),
            ..Default::default()
        }),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn avatar_history_preserves_wire_ids_and_never_posts_a_reuse_to_the_feed() {
        let id = PhotoId::parse(&json!(123)).unwrap();
        let (path, body) = request(&Action::Reuse { id: id.clone() }).unwrap();
        assert_eq!(path, "/api/social/reuse-avatar");
        assert_eq!(body["photoId"], 123);
        assert_eq!(body["isPostSocial"], 0);
        let (_, body) = request(&Action::Delete { id }).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body["delPhotos"].as_str().unwrap()).unwrap(),
            json!([{"photoId":"123"}])
        );
        assert!(parse_page(
            json!({"albumId":"0","hasMore":true,"nextPhotoId":"123","photos":[{"photoId":123}]}),
            2,
            "123"
        )
        .is_err());
        assert!(parse_page(json!({"albumId":"0","hasMore":false}), 1, "").is_err());
    }
}
