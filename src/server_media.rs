//! Server Media Library (Zalo Parity B05).
//!
//! Provides data structures and API helpers to retrieve photos, videos, files, and links
//! stored on the Zalo server for groups and 1-1 conversations
//! (`/api/mediastore/list` commands 11841..11843, `/api/media/oneone/list` commands 12220..12222).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::session::Session;

/// Category of media stored on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMediaType {
    Photo = 1,
    Video = 2,
    File = 3,
    Link = 4,
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
    pub created_ts: i64,
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

/// Parse server media response from `/api/mediastore/list` or `/api/media/oneone/list`.
pub fn parse_server_media_list(val: &Value, expected_kind: ServerMediaType) -> ServerMediaPage {
    let data = val.get("data").unwrap_or(val);
    let items_array = data
        .get("items")
        .or_else(|| data.get("medias"))
        .or_else(|| data.get("list"))
        .and_then(|v| v.as_array());

    let has_more = data.get("has_more")
        .or_else(|| data.get("hasMore"))
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0)))
        .unwrap_or(false);

    let next_cursor = data.get("next_cursor")
        .or_else(|| data.get("nextCursor"))
        .or_else(|| data.get("cursor"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    let Some(arr) = items_array else {
        return ServerMediaPage {
            items: Vec::new(),
            has_more,
            next_cursor,
        };
    };

    let items = arr.iter()
        .filter_map(|it| {
            let id = it.get("id").or_else(|| it.get("msg_id"))?.as_str()?.to_string();
            let sender_id = it.get("sender_id").or_else(|| it.get("from_uid")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let created_ts = it.get("created_ts").or_else(|| it.get("time")).and_then(crate::api_response::integer).unwrap_or(0);
            let url = it.get("url").or_else(|| it.get("normal_url")).or_else(|| it.get("href")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let thumbnail_url = it.get("thumb_url").or_else(|| it.get("thumbnail")).and_then(|v| v.as_str()).map(ToString::to_string);
            let file_name = it.get("file_name").or_else(|| it.get("title")).or_else(|| it.get("name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let file_size = it.get("file_size").or_else(|| it.get("size")).and_then(crate::api_response::integer).unwrap_or(0) as u64;
            let is_expired = it.get("is_expired").or_else(|| it.get("expired")).and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0))).unwrap_or(false);

            Some(ServerMediaItem {
                id,
                sender_id,
                created_ts,
                media_type: expected_kind,
                url,
                thumbnail_url,
                file_name,
                file_size,
                is_expired,
            })
        })
        .collect();

    ServerMediaPage {
        items,
        has_more,
        next_cursor,
    }
}

/// Call `/api/mediastore/list` to retrieve group media items.
pub async fn fetch_group_media_store(
    s: &Session,
    grid: &str,
    media_type: ServerMediaType,
    page: u32,
    count: u32,
) -> Result<ServerMediaPage> {
    let url = format!("{}/api/mediastore/list", s.domains.group);
    let payload = json!({
        "grid": grid,
        "type": media_type as u8,
        "page": page,
        "count": count,
        "imei": s.imei,
    });
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số lấy kho media nhóm thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(parse_server_media_list(&decrypted, media_type))
}

/// Call `/api/media/oneone/list` to retrieve direct 1-1 media items.
pub async fn fetch_direct_media_store(
    s: &Session,
    fid: &str,
    media_type: ServerMediaType,
    page: u32,
    count: u32,
) -> Result<ServerMediaPage> {
    let url = format!("{}/api/media/oneone/list", s.domains.chat);
    let payload = json!({
        "fid": fid,
        "type": media_type as u8,
        "page": page,
        "count": count,
        "imei": s.imei,
    });
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số lấy kho media cá nhân thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(parse_server_media_list(&decrypted, media_type))
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
                "next_cursor": "cur_photo_100",
                "items": [
                    {
                        "id": "msg_p1",
                        "sender_id": "u123",
                        "time": 1794980000,
                        "url": "https://photo-cdn.zalo.me/p1.jpg",
                        "thumb_url": "https://photo-cdn.zalo.me/p1_thumb.jpg",
                        "file_name": "IMG_001.JPG",
                        "size": 204800,
                        "is_expired": false
                    }
                ]
            }
        });

        let page = parse_server_media_list(&val, ServerMediaType::Photo);
        assert!(page.has_more);
        assert_eq!(page.next_cursor.as_deref(), Some("cur_photo_100"));
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "msg_p1");
        assert_eq!(page.items[0].media_type, ServerMediaType::Photo);
        assert_eq!(page.items[0].file_name, "IMG_001.JPG");
        assert!(!page.items[0].is_expired);
    }

    #[test]
    fn test_parse_server_media_list_files() {
        let val = json!({
            "error_code": 0,
            "data": {
                "has_more": false,
                "items": [
                    {
                        "id": "msg_f1",
                        "sender_id": "u456",
                        "time": 1794970000,
                        "url": "https://tt-files-wpa.zaloapp.com/d/report.pdf",
                        "file_name": "Bao_Cao_v0010.pdf",
                        "size": 1048576,
                        "is_expired": false
                    }
                ]
            }
        });

        let page = parse_server_media_list(&val, ServerMediaType::File);
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
                        "id": "msg_l1",
                        "sender_id": "u789",
                        "time": 1794960000,
                        "href": "https://github.com/bqqhuy/ctdozlite",
                        "title": "ctdozlite GitHub Repository",
                        "is_expired": false
                    }
                ]
            }
        });

        let page = parse_server_media_list(&val, ServerMediaType::Link);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].url, "https://github.com/bqqhuy/ctdozlite");
        assert_eq!(page.items[0].file_name, "ctdozlite GitHub Repository");
    }
}
