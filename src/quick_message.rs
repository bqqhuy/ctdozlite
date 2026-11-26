//! Quick Messages & Shortcut Expansions (Zalo Parity B07).
//!
//! Allows users to define predefined quick messages (e.g., `/dc` -> address, `/stk` -> bank details)
//! that expand automatically when typed in the composer.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::session::Session;

/// A predefined quick message item.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickMessage {
    #[serde(default)]
    pub id: String,
    pub shortcut: String,
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub media: Option<String>,
    #[serde(default)]
    pub msg_type: i32,
}

/// Account-local user-defined quick messages.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickMessageStore {
    pub items: Vec<QuickMessage>,
}

impl QuickMessageStore {
    /// Search for quick messages matching prefix (e.g. "/" or "/d").
    pub fn search(&self, query: &str) -> Vec<&QuickMessage> {
        let q = query.to_lowercase();
        self.items
            .iter()
            .filter(|it| {
                it.shortcut.to_lowercase().contains(&q)
                    || it.title.to_lowercase().contains(&q)
                    || it.text.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Match an exact shortcut and expand it.
    pub fn expand(&self, text: &str) -> Option<&str> {
        let trimmed = text.trim();
        self.items
            .iter()
            .find(|it| it.shortcut.eq_ignore_ascii_case(trimmed))
            .map(|it| it.text.as_str())
    }

    /// Add or update a quick message retaining all fields (id, media, msg_type).
    pub fn upsert_item(&mut self, item: QuickMessage) {
        let pos = if !item.id.is_empty() {
            self.items
                .iter()
                .position(|it| it.id == item.id)
                .or_else(|| {
                    self.items.iter().position(|it| {
                        it.id.is_empty() && it.shortcut.eq_ignore_ascii_case(&item.shortcut)
                    })
                })
        } else {
            self.items
                .iter()
                .position(|it| it.id.is_empty() && it.shortcut.eq_ignore_ascii_case(&item.shortcut))
        };

        if let Some(p) = pos {
            self.items[p] = item;
        } else {
            self.items.push(item);
        }
    }

    /// Add or update a quick message.
    pub fn upsert(&mut self, shortcut: String, title: String, text: String) {
        if let Some(pos) = self
            .items
            .iter()
            .position(|it| it.shortcut.eq_ignore_ascii_case(&shortcut))
        {
            self.items[pos].title = title;
            self.items[pos].text = text;
        } else {
            self.items.push(QuickMessage {
                id: String::new(),
                shortcut,
                title,
                text,
                media: None,
                msg_type: 0,
            });
        }
    }

    /// Merge server items without destroying local un-migrated items.
    /// If an item exists locally without an ID, only consider it matched if its content
    /// is identical to a server item. If shortcut matches but content differs, retain
    /// the local item as unmigrated/conflict (with a distinct title suffix if needed).
    pub fn merge_server_items(&mut self, server_items: Vec<QuickMessage>) {
        let mut unmigrated: Vec<QuickMessage> = self
            .items
            .drain(..)
            .filter_map(|mut local| {
                if !local.id.is_empty() {
                    None
                } else {
                    let matching_server = server_items
                        .iter()
                        .find(|srv| srv.shortcut.eq_ignore_ascii_case(&local.shortcut));
                    match matching_server {
                        Some(srv)
                            if srv.text == local.text
                                && srv.media == local.media
                                && srv.msg_type == local.msg_type =>
                        {
                            None
                        }
                        Some(_) => {
                            if !local.title.contains("(cục bộ)") {
                                local.title.push_str(" (cục bộ)");
                            }
                            Some(local)
                        }
                        None => Some(local),
                    }
                }
            })
            .collect();
        self.items = server_items;
        self.items.append(&mut unmigrated);
    }

    /// Items that are only local and ready to be migrated to server.
    pub fn unmigrated_items(&self) -> Vec<&QuickMessage> {
        self.items.iter().filter(|it| it.id.is_empty()).collect()
    }

    /// Remove a quick message by shortcut.
    pub fn remove(&mut self, shortcut: &str) -> bool {
        if let Some(pos) = self
            .items
            .iter()
            .position(|it| it.shortcut.eq_ignore_ascii_case(shortcut))
        {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }
}

/// GET /api/quickmessage/list (CmdId 12025)
pub async fn fetch_quick_messages(s: &Session, version: u32) -> Result<(Vec<QuickMessage>, u32)> {
    let body = serde_json::json!({
        "version": version,
        "lang": "vi",
        "imei": s.imei,
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/quickmessage/list?{}&params={params}",
        crate::endpoints::QUICK_MESSAGE,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let data_obj = crate::api_response::checked_data(data)?;
    let new_version = data_obj
        .get("version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let arr = data_obj
        .get("items")
        .and_then(|it| it.as_array())
        .ok_or_else(|| {
            anyhow::anyhow!("Phản hồi tin nhắn nhanh từ máy chủ thiếu danh sách 'items'")
        })?;
    let mut result = Vec::with_capacity(arr.len());
    for item in arr {
        let id = item
            .get("id")
            .or_else(|| item.get("itemId"))
            .and_then(crate::api_response::string_or_integer)
            .unwrap_or_default();
        let kw = item
            .get("keyword")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let msg = item
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let med = item
            .get("media")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let t = item.get("type").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let shortcut = if kw.starts_with('/') {
            kw.clone()
        } else {
            format!("/{kw}")
        };
        result.push(QuickMessage {
            id,
            shortcut,
            title: kw,
            text: msg,
            media: med,
            msg_type: t,
        });
    }
    Ok((result, new_version))
}

/// GET /api/quickmessage/create (CmdId 12029)
pub async fn create_quick_message(
    s: &Session,
    keyword: &str,
    message: &str,
    media: Option<&str>,
    msg_type: i32,
) -> Result<QuickMessage> {
    let clean_kw = keyword.trim_start_matches('/');
    let body = serde_json::json!({
        "keyword": clean_kw,
        "message": message,
        "media": media.unwrap_or(""),
        "type": msg_type,
        "imei": s.imei,
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/quickmessage/create?{}&params={params}",
        crate::endpoints::QUICK_MESSAGE,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let data_obj = crate::api_response::checked_data(data)?;
    let item_val = data_obj.get("item").cloned().unwrap_or(data_obj.clone());
    let id = item_val
        .get("id")
        .or_else(|| item_val.get("itemId"))
        .and_then(crate::api_response::string_or_integer)
        .ok_or_else(|| anyhow::anyhow!("Phản hồi tạo tin nhắn nhanh thiếu ID hợp lệ"))?;
    let shortcut = if clean_kw.starts_with('/') {
        clean_kw.to_string()
    } else {
        format!("/{clean_kw}")
    };
    Ok(QuickMessage {
        id,
        shortcut,
        title: clean_kw.to_string(),
        text: message.to_string(),
        media: media.map(str::to_string),
        msg_type,
    })
}

/// GET /api/quickmessage/update (CmdId 12026)
pub async fn update_quick_message(
    s: &Session,
    item_id: &str,
    keyword: &str,
    message: &str,
    media: Option<&str>,
    msg_type: i32,
) -> Result<()> {
    let clean_kw = keyword.trim_start_matches('/');
    let body = serde_json::json!({
        "itemId": item_id,
        "keyword": clean_kw,
        "message": message,
        "media": media.unwrap_or(""),
        "type": msg_type,
        "imei": s.imei,
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/quickmessage/update?{}&params={params}",
        crate::endpoints::QUICK_MESSAGE,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    crate::api_response::checked_data(data)?;
    Ok(())
}

/// GET /api/quickmessage/delete (CmdId 12032)
pub async fn delete_quick_messages(s: &Session, item_ids: &[String]) -> Result<()> {
    let body = serde_json::json!({
        "itemIds": item_ids,
        "imei": s.imei,
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/quickmessage/delete?{}&params={params}",
        crate::endpoints::QUICK_MESSAGE,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    crate::api_response::checked_data(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_message_expansion() {
        let store = QuickMessageStore {
            items: vec![
                QuickMessage {
                    id: String::new(),
                    shortcut: "/dc".into(),
                    title: "Địa chỉ".into(),
                    text: "Địa chỉ nhận hàng: Hà Nội, Việt Nam".into(),
                    media: None,
                    msg_type: 0,
                },
                QuickMessage {
                    id: String::new(),
                    shortcut: "/chao".into(),
                    title: "Chào hỏi".into(),
                    text: "Xin chào".into(),
                    media: None,
                    msg_type: 0,
                },
            ],
        };
        assert_eq!(
            store.expand("/dc"),
            Some("Địa chỉ nhận hàng: Hà Nội, Việt Nam")
        );
        assert_eq!(store.expand("/none"), None);

        let matches = store.search("chao");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].shortcut, "/chao");
    }

    #[test]
    fn test_quick_message_upsert_and_remove() {
        let mut store = QuickMessageStore::default();
        store.upsert(
            "/test".into(),
            "Thử nghiệm".into(),
            "Nội dung thử nghiệm".into(),
        );
        assert_eq!(store.expand("/test"), Some("Nội dung thử nghiệm"));

        assert!(store.remove("/test"));
        assert_eq!(store.expand("/test"), None);
    }

    #[test]
    fn test_quick_message_upsert_item_and_merge_conflict() {
        let mut store = QuickMessageStore::default();
        store.upsert_item(QuickMessage {
            id: String::new(),
            shortcut: "/dc".into(),
            title: "Địa chỉ nhà".into(),
            text: "Số 10 Tràng Thi".into(),
            media: Some("file://path/to/img.png".into()),
            msg_type: 1,
        });

        // Server has /dc but with different text and server ID
        let server_items = vec![QuickMessage {
            id: "srv_123".into(),
            shortcut: "/dc".into(),
            title: "Địa chỉ văn phòng".into(),
            text: "Số 20 Lý Thường Kiệt".into(),
            media: None,
            msg_type: 0,
        }];

        store.merge_server_items(server_items);

        // Server item must be present
        let srv = store
            .items
            .iter()
            .find(|i| i.id == "srv_123")
            .expect("Server item must exist");
        assert_eq!(srv.text, "Số 20 Lý Thường Kiệt");

        // Local unmigrated item must NOT be dropped, but kept as conflict with title updated
        let local = store
            .items
            .iter()
            .find(|i| i.id.is_empty())
            .expect("Local conflict item must be preserved");
        assert_eq!(local.text, "Số 10 Tràng Thi");
        assert!(local.title.contains("(cục bộ)"));
        assert_eq!(local.media.as_deref(), Some("file://path/to/img.png"));
    }
}
