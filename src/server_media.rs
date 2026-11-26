//! Server Media Library (Zalo Parity B05).
//!
//! Provides data structures and API helpers to retrieve photos, videos, files, and links
//! stored on the Zalo server for groups and 1-1 conversations
//! (`/api/mediastore/list` commands 11841..11843, `/api/media/oneone/list` commands 12220..12222).

use crate::session::Session;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Target consumer for server media requests, separating tools panel from right drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMediaTarget {
    ToolsPanel,
    RightDrawer,
}

/// Category of media stored on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMediaType {
    Photo = 1,
    Video = 2,
    File = 3,
    Link = 4,
}

impl ServerMediaType {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Photo | Self::Video => "media",
            Self::File => "file",
            Self::Link => "link",
        }
    }
}

impl ServerMediaType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Photo => "Hình ảnh",
            Self::Video => "Video",
            Self::File => "Tệp tin",
            Self::Link => "Liên kết",
        }
    }
}

/// An item in the server media library.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerMediaItem {
    pub id: String,
    pub sender_id: String,
    pub group_id: String,
    pub global_message_id: String,
    pub client_message_id: String,
    pub created_ts: i64,
    pub sub_type: Option<u8>,
    pub media_type: ServerMediaType,
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub file_name: String,
    pub file_size: u64,
    pub is_expired: bool,
}

/// A paginated page of server media items.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerMediaPage {
    pub items: Vec<ServerMediaItem>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// Strict parser for server media response from `/api/mediastore/list` or `/api/media/oneone/list`.
///
/// Returns `Err` when the response contains a server error code or when the payload schema
/// is malformed or missing the items list. Returns `Ok(ServerMediaPage)` on valid responses,
/// including valid empty lists (`items: []`).
pub fn try_parse_server_media_list(
    val: &Value,
    expected_kind: ServerMediaType,
) -> Result<ServerMediaPage> {
    if let Some(code) = val.get("error_code").and_then(crate::api_response::integer) {
        if code != 0 {
            let msg = val
                .get("error_message")
                .or_else(|| val.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Máy chủ trả về mã lỗi");
            anyhow::bail!("Máy chủ từ chối lấy kho media (mã {code}): {msg}");
        }
    }

    let data = val.get("data").unwrap_or(val);
    if data.is_null() {
        anyhow::bail!("Phản hồi kho media rỗng hoặc lỗi (data null)");
    }

    if let Some(inner_code) = data
        .get("error_code")
        .and_then(crate::api_response::integer)
    {
        if inner_code != 0 {
            let msg = data
                .get("error_message")
                .or_else(|| data.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Máy chủ trả về mã lỗi nội bộ");
            anyhow::bail!("Máy chủ từ chối lấy kho media (mã {inner_code}): {msg}");
        }
    }

    let items_val = data
        .get("items")
        .and_then(Value::as_array)
        .context("Thiếu mảng items trong phản hồi kho media")?;
    let has_more = match data.get("has_more") {
        Some(Value::Bool(value)) => *value,
        Some(value) => match crate::api_response::integer(value) {
            Some(0) => false,
            Some(1) => true,
            _ => anyhow::bail!("has_more trong phản hồi kho media không hợp lệ"),
        },
        None => anyhow::bail!("Thiếu has_more trong phản hồi kho media"),
    };
    let next_cursor = data
        .get("last_id")
        .filter(|value| !value.is_null())
        .map(scalar_id)
        .transpose()?;
    anyhow::ensure!(
        !has_more
            || next_cursor
                .as_deref()
                .is_some_and(|cursor| !cursor.is_empty()),
        "Kho media yêu cầu trang tiếp theo nhưng thiếu last_id"
    );

    let items = items_val
        .iter()
        .map(|item| parse_item(item, expected_kind))
        .collect::<Result<Vec<_>>>()?;

    Ok(ServerMediaPage {
        items,
        has_more,
        next_cursor,
    })
}

fn scalar_id(value: &Value) -> Result<String> {
    match value {
        Value::String(value) if !value.is_empty() => Ok(value.clone()),
        Value::Number(value) if value.is_u64() => Ok(value.to_string()),
        _ => anyhow::bail!("ID trong phản hồi kho media không hợp lệ"),
    }
}

fn optional_id(item: &Value, key: &str) -> Result<String> {
    match item.get(key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(value) => scalar_id(value),
    }
}

fn parse_item(item: &Value, requested: ServerMediaType) -> Result<ServerMediaItem> {
    anyhow::ensure!(item.is_object(), "Mục kho media không hợp lệ");
    let wire_type = item
        .get("type")
        .and_then(Value::as_str)
        .context("Thiếu type trong mục kho media")?;
    let sub_type = item
        .get("sub_type")
        .map(|value| {
            let value = crate::api_response::integer(value)
                .context("sub_type trong mục kho media không hợp lệ")?;
            u8::try_from(value).context("sub_type ngoài phạm vi")
        })
        .transpose()?;
    let media_type = match wire_type {
        "media" => match sub_type {
            Some(1) => ServerMediaType::Photo,
            Some(2) => ServerMediaType::Video,
            _ => anyhow::bail!("sub_type media không được hỗ trợ"),
        },
        "file" => ServerMediaType::File,
        "link" => ServerMediaType::Link,
        _ => anyhow::bail!("type kho media không được hỗ trợ"),
    };
    anyhow::ensure!(
        media_type.wire_name() == requested.wire_name(),
        "Máy chủ trả về sai loại mục kho media"
    );
    let file_size = match item.get("file_size") {
        None | Some(Value::Null) => 0,
        Some(value) => u64::try_from(
            crate::api_response::integer(value).context("size trong mục kho media không hợp lệ")?,
        )
        .context("size trong mục kho media âm")?,
    };
    Ok(ServerMediaItem {
        id: scalar_id(item.get("id").context("Thiếu id trong mục kho media")?)?,
        sender_id: optional_id(item, "owner_id")?,
        group_id: optional_id(item, "group_id")?,
        global_message_id: optional_id(item, "global_msg_id")?,
        client_message_id: optional_id(item, "client_msg_id")?,
        created_ts: item
            .get("created_time")
            .and_then(crate::api_response::integer)
            .context("created_time trong mục kho media không hợp lệ")?,
        sub_type,
        media_type,
        url: item
            .get("src_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        thumbnail_url: item
            .get("thumb_url")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        file_name: item
            .get("file_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        file_size,
        is_expired: false,
    })
}

/// Parse server media response from `/api/mediastore/list` or `/api/media/oneone/list`.
pub fn parse_server_media_list(
    val: &Value,
    expected_kind: ServerMediaType,
) -> Result<ServerMediaPage> {
    try_parse_server_media_list(val, expected_kind)
}

/// Call `/api/mediastore/list` to retrieve group media items.
pub async fn fetch_group_media_store(
    s: &Session,
    grid: &str,
    media_type: ServerMediaType,
    page: u32,
    count: u32,
) -> Result<ServerMediaPage> {
    fetch_group_media_store_paged(s, grid, media_type, page, count, None).await
}

/// Call `/api/mediastore/list` with pagination and optional cursor.
pub async fn fetch_group_media_store_paged(
    s: &Session,
    grid: &str,
    media_type: ServerMediaType,
    page: u32,
    count: u32,
    cursor: Option<&str>,
) -> Result<ServerMediaPage> {
    let _ = page;
    fetch_conversation_media_store(s, grid, media_type, count, cursor).await
}

/// Retrieve conversation media, using the send-to-me route only for its exact id.
pub async fn fetch_direct_media_store(
    s: &Session,
    fid: &str,
    media_type: ServerMediaType,
    page: u32,
    count: u32,
) -> Result<ServerMediaPage> {
    fetch_direct_media_store_paged(s, fid, media_type, page, count, None).await
}

/// Retrieve conversation media with an explicit server cursor.
pub async fn fetch_direct_media_store_paged(
    s: &Session,
    fid: &str,
    media_type: ServerMediaType,
    page: u32,
    count: u32,
    cursor: Option<&str>,
) -> Result<ServerMediaPage> {
    let _ = page;
    fetch_conversation_media_store(s, fid, media_type, count, cursor).await
}

/// Read the same conversation-scoped server media pages as the installed PC client.
pub async fn fetch_conversation_media_store(
    s: &Session,
    conversation_id: &str,
    media_type: ServerMediaType,
    limit: u32,
    last_id: Option<&str>,
) -> Result<ServerMediaPage> {
    anyhow::ensure!(
        (1..=100).contains(&limit),
        "Số mục kho media phải từ 1 đến 100"
    );
    anyhow::ensure!(
        last_id.is_none_or(|value| value.len() <= 512),
        "Con trỏ kho media vượt giới hạn"
    );
    let (url, _payload) = media_request(s, conversation_id, media_type, limit, last_id)?;
    let response = s.get(&url).await?;
    let decoded = s.maybe_decrypt(response, s.session_key_bytes()).await;
    try_parse_server_media_list(&decoded, media_type)
}

fn media_request(
    s: &Session,
    conversation_id: &str,
    media_type: ServerMediaType,
    limit: u32,
    last_id: Option<&str>,
) -> Result<(String, Value)> {
    anyhow::ensure!(
        (1..=100).contains(&limit),
        "media-store limit ngoài phạm vi 1..=100"
    );
    anyhow::ensure!(
        last_id.is_none_or(|value| value.len() <= 512 && !value.chars().any(char::is_control)),
        "Con trỏ kho media không hợp lệ"
    );
    let send_to_me_id = s
        .login
        .as_ref()
        .map(|login| login.send2me_id.as_str())
        .filter(|id| !id.is_empty())
        .context("Thiếu định danh Tin nhắn của tôi từ phiên đăng nhập")?;
    let is_send_to_me = conversation_id == send_to_me_id;
    let (service, path) = if is_send_to_me {
        ("media_store_send2me", "/api/media/oneone/list")
    } else {
        ("media_store", "/api/mediastore/list")
    };
    let mut payload = json!({
        "media_type": media_type.wire_name(),
        "limit": limit,
    });
    if let Some(last_id) = last_id.filter(|value| !value.is_empty()) {
        payload["last_id"] = json!(last_id);
    }
    if !is_send_to_me {
        payload["group_id"] = json!(group_id_from_conversation_id(conversation_id)?);
        payload["imei"] = json!(s.imei);
    }
    let encoded = s
        .encoded_params(&payload)
        .context("Mã hoá tham số lấy kho media thất bại")?;
    let base = s.resolve_discovered_service_url(service, path)?;
    let url = format!("{base}?{}&params={encoded}", s.common_params());
    Ok((url, payload))
}

fn group_id_from_conversation_id(value: &str) -> Result<&str> {
    let group_id = match value.strip_prefix('g') {
        Some(rest)
            if rest.chars().next().is_some_and(|c| c.is_ascii_digit())
                || (rest.starts_with('g')
                    && rest[1..].chars().next().is_some_and(|c| c.is_ascii_digit())) =>
        {
            rest
        }
        _ => value,
    };
    anyhow::ensure!(
        !group_id.trim().is_empty()
            && group_id.len() <= 128
            && !group_id.chars().any(char::is_control),
        "ID hội thoại không hợp lệ cho kho media"
    );
    Ok(group_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_server_media_list_photos() {
        let val = json!({
            "error_code": 0,
            "data": {
                "has_more": true,
                "last_id": 900,
                "items": [
                    {
                        "id": 77,
                        "global_msg_id": 900,
                        "client_msg_id": "client-01",
                        "owner_id": 123,
                        "group_id": 456,
                        "created_time": 1794980000,
                        "type": "media",
                        "sub_type": 1,
                        "src_url": "https://photo-cdn.zalo.me/p1.jpg",
                        "thumb_url": "https://photo-cdn.zalo.me/p1_thumb.jpg",
                        "file_name": "IMG_001.JPG",
                        "size": 204800
                    }
                ]
            }
        });

        let page = parse_server_media_list(&val, ServerMediaType::Photo).unwrap();
        assert!(page.has_more);
        assert_eq!(page.next_cursor.as_deref(), Some("900"));
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "77");
        assert_eq!(page.items[0].global_message_id, "900");
        assert_eq!(page.items[0].sender_id, "123");
        assert_eq!(page.items[0].media_type, ServerMediaType::Photo);
        assert_eq!(page.items[0].file_name, "IMG_001.JPG");
        assert_eq!(page.items[0].sub_type, Some(1));
    }

    #[test]
    fn test_parse_server_media_list_files() {
        let val = json!({
            "error_code": 0,
            "data": {
                "has_more": false,
                "items": [
                    {
                        "id": "file-1",
                        "owner_id": "456",
                        "group_id": "456",
                        "created_time": 1794970000,
                        "type": "file",
                        "src_url": "https://tt-files-wpa.zaloapp.com/d/report.pdf",
                        "file_name": "Bao_Cao_v0010.pdf",
                        "file_size": 1048576
                    }
                ]
            }
        });

        let page = parse_server_media_list(&val, ServerMediaType::File).unwrap();
        assert!(!page.has_more);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].file_name, "Bao_Cao_v0010.pdf");
        assert_eq!(page.items[0].file_size, 1048576);
    }

    #[test]
    fn test_parse_server_media_list_links() {
        let val = json!({
            "error_code": 0,
            "data": {
                "has_more": false,
                "items": [
                    {
                        "id": "link-1",
                        "owner_id": "789",
                        "group_id": "456",
                        "created_time": 1794960000,
                        "type": "link",
                        "src_url": "https://github.com/bqqhuy/ctdozlite",
                        "file_name": "ctdozlite GitHub Repository"
                    }
                ]
            }
        });

        let page = parse_server_media_list(&val, ServerMediaType::Link).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].url, "https://github.com/bqqhuy/ctdozlite");
        assert_eq!(page.items[0].file_name, "ctdozlite GitHub Repository");
    }

    #[test]
    fn test_parse_server_media_list_business_error() {
        // (a) Outer/inner business error
        let outer_err = json!({
            "error_code": 114,
            "error_message": "Session expired or permission denied",
            "data": null
        });
        assert!(try_parse_server_media_list(&outer_err, ServerMediaType::Photo).is_err());

        let inner_err = json!({
            "error_code": 0,
            "data": {
                "error_code": 5001,
                "error_message": "Thread media access restricted",
                "items": null
            }
        });
        assert!(try_parse_server_media_list(&inner_err, ServerMediaType::Photo).is_err());
    }

    #[test]
    fn test_parse_server_media_list_malformed_schema() {
        // (b) Malformed schema (missing list array)
        let missing_list = json!({
            "error_code": 0,
            "data": {
                "has_more": false,
                "cursor": "abc"
            }
        });
        assert!(try_parse_server_media_list(&missing_list, ServerMediaType::File).is_err());

        let invalid_items_type = json!({
            "error_code": 0,
            "data": {
                "items": "not-an-array"
            }
        });
        assert!(try_parse_server_media_list(&invalid_items_type, ServerMediaType::File).is_err());
    }

    #[test]
    fn test_parse_server_media_list_valid_empty_list() {
        // (c) Valid empty list
        let empty_val = json!({
            "error_code": 0,
            "data": {
                "items": [],
                "has_more": false,
                "last_id": null
            }
        });
        let res = try_parse_server_media_list(&empty_val, ServerMediaType::Photo);
        assert!(res.is_ok());
        let page = res.unwrap();
        assert!(page.items.is_empty());
        assert!(!page.has_more);
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn request_matches_pc_dynamic_service_and_cursor_contract() {
        let mut session = Session::new(false, false, false).unwrap();
        session.secret_key = Some(vec![b'f'; 32]);
        session.login = Some(crate::session::LoginInfo {
            uin: "self".into(),
            send2me_id: "my-documents".into(),
            ..Default::default()
        });
        session.apply_server_info(&json!({"zpw_service_map_v3": {
            "media_store": ["https://media.dynamic.zaloapp.com"],
            "media_store_send2me": ["https://cloud.dynamic.zalo.me"]
        }}));

        let (url, payload) =
            media_request(&session, "g456", ServerMediaType::Video, 30, Some("900")).unwrap();
        assert!(url.starts_with("https://media.dynamic.zaloapp.com/api/mediastore/list?"));
        assert!(url.contains("zpw_ver="));
        assert!(url.contains("&params="));
        assert_eq!(payload["media_type"], "media");
        assert_eq!(payload["group_id"], "456");
        assert_eq!(payload["imei"], session.imei);
        assert_eq!(payload["limit"], 30);
        assert_eq!(payload["last_id"], "900");
        assert!(payload.get("page").is_none());

        let (url, payload) =
            media_request(&session, "my-documents", ServerMediaType::File, 30, None).unwrap();
        assert!(url.starts_with("https://cloud.dynamic.zalo.me/api/media/oneone/list?"));
        assert_eq!(payload["media_type"], "file");
        assert!(payload.get("group_id").is_none());
        assert!(payload.get("imei").is_none());
        assert!(payload.get("last_id").is_none());

        let (url, payload) =
            media_request(&session, "123456", ServerMediaType::Link, 30, None).unwrap();
        assert!(url.starts_with("https://media.dynamic.zaloapp.com/api/mediastore/list?"));
        assert_eq!(payload["group_id"], "123456");
        let (url, payload) =
            media_request(&session, "g456", ServerMediaType::Photo, 30, None).unwrap();
        assert!(url.starts_with("https://media.dynamic.zaloapp.com/api/mediastore/list?"));
        assert_eq!(payload["group_id"], "456");
        assert!(media_request(&session, "", ServerMediaType::Link, 30, None).is_err());
        let error = media_request(&session, "g456", ServerMediaType::Photo, 0, None).unwrap_err();
        assert!(error.to_string().contains("limit"));
    }

    #[test]
    fn group_id_helper_strips_exactly_one_pc_prefix() {
        assert_eq!(group_id_from_conversation_id("g123").unwrap(), "123");
        assert_eq!(group_id_from_conversation_id("123").unwrap(), "123");
        assert_eq!(group_id_from_conversation_id("gg123").unwrap(), "g123");
        assert_eq!(
            group_id_from_conversation_id("group_123").unwrap(),
            "group_123"
        );
    }
}
