//! Zalo server reminders and pinned-board reads, using the discovered GROUP_BOARD service.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

/// A server reminder or pinned item, retaining its original fields for safe edits.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub creator: String,
    pub raw: Value,
    pub message_id: String,
    pub repeating: bool,
}
/// One bounded server page; malformed payloads are errors, not empty successful lists.
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub items: Vec<Item>,
    pub next: Option<String>,
    pub version: Option<u64>,
}
/// Explicit server intent. Mutations are not automatically replayed.
#[derive(Clone, Debug)]
pub enum Action {
    PinMessage {
        message: Box<crate::chat::ChatMessage>,
    },
    Unpin {
        item: Value,
        version: u64,
    },
    List {
        page: u32,
        last: String,
        pins: bool,
    },
    Detail {
        event: String,
    },
    Save {
        id: Option<String>,
        title: String,
        start: i64,
        original: Option<Value>,
    },
    Remove {
        id: String,
        original: Value,
    },
}
fn clean(thread: &str, group: bool) -> Result<String> {
    let wire = crate::server_pins::identity(thread, group)?;
    Ok(wire[1..].into())
}
fn id(v: Option<&Value>) -> String {
    v.and_then(|v| {
        v.as_str()
            .map(str::to_owned)
            .or_else(|| v.as_u64().map(|n| n.to_string()))
    })
    .unwrap_or_default()
}
fn object(v: &Value) -> Result<Value> {
    if let Some(s) = v.as_str() {
        ensure!(s.len() <= 256 * 1024, "Dữ liệu bảng tin quá lớn");
        Ok(serde_json::from_str(s)?)
    } else {
        Ok(v.clone())
    }
}
/// Peel only response envelopes; never classify missing fields as an empty list.
pub fn data(value: Value) -> Result<Value> {
    crate::api_response::data(value, 256 * 1024, false)
}
/// Parse list/detail fields used by the PC reminder adapters.
pub fn parse_item(value: Value) -> Result<Item> {
    let value = object(&value)?;
    let raw = if value.get("boardType").is_some() {
        object(value.get("data").context("Thiếu nội dung mục bảng tin")?)?
    } else {
        value
    };
    let params = object(raw.get("params").unwrap_or(&Value::Null))?;
    let identifier = ["topicId", "reminderId", "id", "eventId"]
        .iter()
        .map(|key| id(raw.get(key)))
        .find(|v| !v.is_empty())
        .context("Thiếu ID mục bảng tin")?;
    ensure!(identifier.len() <= 128, "ID bảng tin quá dài");
    let title = params
        .get("title")
        .or_else(|| raw.get("title"))
        .and_then(Value::as_str)
        .unwrap_or("Nội dung chưa được nhận diện")
        .chars()
        .take(4000)
        .collect();
    let start = raw
        .get("startTime")
        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
        .unwrap_or(0);
    let creator = id(raw
        .get("creatorUid")
        .or_else(|| raw.get("creatorId"))
        .or_else(|| raw.get("creator")));
    let message_id = id(params.get("msgId").or_else(|| raw.get("msgId")));
    let repeating = raw.get("repeat").and_then(Value::as_i64).unwrap_or(0) != 0
        || raw
            .get("repeatRule")
            .is_some_and(|v| !v.is_null() && v != &json!("") && v != &json!({}));
    Ok(Item {
        id: identifier,
        title,
        start,
        creator,
        raw,
        message_id,
        repeating,
    })
}
/// Preserve server continuation IDs even if some items cannot yet be rendered.
pub fn parse_page(value: Value, pins: bool) -> Result<Page> {
    let value = data(value)?;
    let list = value
        .as_array()
        .or_else(|| value.get("items").and_then(Value::as_array))
        .or_else(|| {
            if pins {
                value.get("topics").and_then(Value::as_array)
            } else {
                None
            }
        })
        .context("Danh sách bảng tin chưa hỗ trợ định dạng này")?;
    ensure!(list.len() <= 1000, "Danh sách bảng tin quá lớn");
    let mut items = Vec::new();
    for value in list {
        if !pins
            && value
                .get("boardType")
                .and_then(Value::as_i64)
                .is_some_and(|kind| kind != 1)
        {
            continue;
        }
        let item = parse_item(value.clone())?;
        if pins || item.start > 0 {
            items.push(item);
        }
    }
    let next = if !pins && list.len() >= 50 {
        list.last()
            .and_then(|v| parse_item(v.clone()).ok())
            .map(|i| i.id)
    } else {
        None
    };
    let version = value
        .get("boardVersion")
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()));
    Ok(Page {
        items,
        next,
        version,
    })
}
/// Build verified legacy one-shot reminder params; edits preserve other original fields.
pub fn save_params(
    s: &Session,
    thread: &str,
    group: bool,
    id: Option<&str>,
    title: &str,
    start: i64,
    original: Option<&Value>,
) -> Result<Value> {
    ensure!(
        !title.trim().is_empty() && title.len() <= 4000,
        "Nội dung nhắc hẹn phải có từ 1 đến 4000 byte"
    );
    ensure!(
        start > crate::login::unix_now().saturating_mul(1000),
        "Thời gian nhắc hẹn phải ở tương lai"
    );
    let target = clean(thread, group)?;
    let mut body = json!({"type":0,"color":-16777216i64,"emoji":"⏰","duration":-1,"needPin":false,"repeat":0});
    if let Some(original) = original {
        for field in ["color", "emoji", "duration", "needPin", "params"] {
            if let Some(value) = original.get(field) {
                body[field] = value.clone();
            }
        }
    }
    let mut params = object(body.get("params").unwrap_or(&json!({})))?;
    if !params.is_object() {
        params = json!({});
    }
    params["title"] = json!(title.trim());
    body["params"] = params;
    body["startTime"] = json!(start);
    if group {
        body["grid"] = json!(target);
        body["imei"] = json!(s.imei);
        body["params"] = json!(body["params"].to_string());
        if let Some(id) = id {
            body["topicId"] = json!(id);
        }
    } else {
        body["toUid"] = json!(target);
        body["creatorUid"] = json!(s.account_key());
        if let Some(id) = id {
            body["reminderId"] = json!(id);
        }
        let object = body.to_string();
        body = json!({"objectData":object});
        if id.is_none() {
            body["imei"] = json!(s.imei);
        }
    }
    Ok(body)
}
/// Execute against the session's service map; unknown service routing fails closed.
pub async fn execute(s: &Session, thread: &str, group: bool, action: &Action) -> Result<Page> {
    ensure!(s.is_active() && s.login.is_some(), "Phiên làm việc đã đóng");
    let host = &s.domains.group_board;
    ensure!(
        !host.is_empty(),
        "Máy chủ chưa cấp dịch vụ bảng tin; hãy đăng nhập lại để cập nhật cấu hình"
    );
    let target = clean(thread, group)?;
    let (path, body, mutation, pins) = match action {
        Action::PinMessage { message } => {
            ensure!(group, "Ghim tin server hiện hỗ trợ nhóm");
            (
                "/api/board/topic/create",
                pin_params(s, &target, message)?,
                true,
                true,
            )
        }
        Action::Unpin { item, version } => {
            ensure!(group, "Bỏ ghim server hiện hỗ trợ nhóm");
            let item = parse_item(item.clone())?;
            let kind = item
                .raw
                .get("type")
                .and_then(Value::as_i64)
                .context("Thiếu loại mục ghim; tải lại bảng tin")?;
            (
                "/api/board/unpinv2",
                json!({"grid":target,"imei":s.imei,"topic":{"topicId":item.id,"topicType":kind},"boardVersion":version}),
                true,
                true,
            )
        }

        Action::List { page, last, pins } => {
            ensure!(
                *page < 10000 && last.len() <= 128,
                "Vị trí bảng tin không hợp lệ"
            );
            if *pins {
                ensure!(group, "Danh sách ghim server hiện hỗ trợ hội thoại nhóm");
                (
                    "/api/board/pin/list",
                    json!({"groupId":target,"boardVersion":0,"imei":s.imei}),
                    false,
                    true,
                )
            } else {
                let mut body = json!({"objectData":json!({if group{"group_id"}else{"uid"}:target,"board_type":1,"page":page,"count":50,"last_id":last,"last_type":0}).to_string()});
                if group {
                    body["imei"] = json!(s.imei);
                }
                (
                    if group {
                        "/api/board/listReminder"
                    } else {
                        "/api/board/oneone/list"
                    },
                    body,
                    false,
                    false,
                )
            }
        }
        Action::Detail { event } => {
            ensure!(
                !event.is_empty() && event.len() <= 128,
                "ID nhắc hẹn không hợp lệ"
            );
            (
                "/api/board/topic/getReminder",
                json!({"eventId":event,"imei":s.imei}),
                false,
                false,
            )
        }
        Action::Save {
            id,
            title,
            start,
            original,
        } => {
            if let Some(original) = original {
                let item = parse_item(original.clone())?;
                ensure!(
                    item.creator == s.account_key() && !item.repeating,
                    "Chỉ sửa nhắc hẹn một lần do tài khoản này tạo"
                );
            }
            ensure!(
                id.is_none() || original.is_some(),
                "Thiếu dữ liệu gốc; tải lại nhắc hẹn trước khi sửa"
            );
            (
                match (group, id.is_some()) {
                    (true, false) => "/api/board/topic/create",
                    (true, true) => "/api/board/topic/update",
                    (false, false) => "/api/board/oneone/create",
                    (false, true) => "/api/board/oneone/update",
                },
                save_params(
                    s,
                    thread,
                    group,
                    id.as_deref(),
                    title,
                    *start,
                    original.as_ref(),
                )?,
                true,
                false,
            )
        }
        Action::Remove { id, original } => {
            let item = parse_item(original.clone())?;
            ensure!(
                item.id == *id && item.creator == s.account_key(),
                "Chỉ xóa nhắc hẹn do tài khoản này tạo"
            );
            ensure!(
                !id.is_empty() && id.len() <= 128,
                "ID nhắc hẹn không hợp lệ"
            );
            (
                if group {
                    "/api/board/topic/remove"
                } else {
                    "/api/board/oneone/remove"
                },
                if group {
                    json!({"grid":target,"topicId":id,"imei":s.imei})
                } else {
                    json!({"uid":target,"reminderId":id})
                },
                true,
                false,
            )
        }
    };
    let params = s.encoded_params(&body)?;
    let url = format!("{}{path}?{}", host.trim_end_matches('/'), s.common_params());
    let raw = if mutation {
        s.post_form(&url, format!("params={params}")).await?
    } else {
        s.get(&format!("{url}&params={params}")).await?
    };
    let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    if mutation {
        crate::chat::confirmed_mutation(raw)?;
        return Ok(Page::default());
    }
    if matches!(action, Action::Detail { .. }) {
        return Ok(Page {
            items: vec![parse_item(data(raw)?)?],
            next: None,
            version: None,
        });
    }
    parse_page(raw, pins)
}
/// PC PinMessageMenuItem uses MSG_TOPIC=2 and CLI message codes, not reaction codes.
pub fn pin_params(s: &Session, thread: &str, message: &crate::chat::ChatMessage) -> Result<Value> {
    use crate::message_model::MessageKind as K;
    let msg_type = match message.kind {
        K::Text => 1,
        K::Photo => 32,
        K::File => 46,
        K::Video => 44,
        K::Sticker => 36,
        _ => anyhow::bail!("Loại tin này chưa hỗ trợ ghim server"),
    };
    let global = crate::chat::server_message_id(message).context("Thiếu ID server để ghim tin")?;
    let client = crate::chat::client_message_id(message).context("Thiếu client ID để ghim tin")?;
    let media = crate::message_view::media(message);
    let sender = message
        .wire_ref
        .sender
        .clone()
        .filter(|v| v != &json!("0") && v != &json!(0))
        .or_else(|| message.is_outgoing.then(|| json!(s.account_key())))
        .context("Thiếu UID người gửi để ghim tin")?;
    let params = json!({"client_msg_id":client,"global_msg_id":global,"senderUid":sender,"senderName":message.sender,"title":message.text,"msg_type":msg_type,"thumb":media.thumbnail.unwrap_or_default()});
    Ok(
        json!({"grid":thread,"type":2,"color":-16777216i64,"emoji":"","startTime":-1,"duration":-1,"params":params.to_string(),"needPin":true,"repeat":0,"imei":s.imei}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_json_params_and_rejects_ambiguous_empty_results() {
        let p=parse_page(json!({"error_code":0,"data":{"items":[{"boardType":1,"data":{"id":"12","params":"{\"title\":\"Test\"}","startTime":100,"creatorUid":"user-1"}}]}}),false).unwrap();
        assert_eq!(p.items[0].title, "Test");
        assert!(parse_page(json!({"error_code":0,"data":{}}), false).is_err());
        assert!(parse_page(json!({"error_code":166,"data":[]}), false).is_err());
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;
    fn session() -> Session {
        let mut s = Session::new(false, false, false).unwrap();
        s.login = Some(crate::session::LoginInfo {
            uin: "100".into(),
            ..Default::default()
        });
        s
    }
    #[test]
    fn one_shot_params_use_server_contract_and_reject_past_times() {
        let s = session();
        let start = (crate::login::unix_now() + 3600) * 1000;
        let group = save_params(&s, "g200", true, None, "Reminder", start, None).unwrap();
        assert_eq!(group["grid"], "200");
        assert_eq!(group["type"], 0);
        assert_eq!(group["repeat"], 0);
        assert!(group["params"].is_string());
        let peer = save_params(&s, "200", false, None, "Reminder", start, None).unwrap();
        let obj: Value = serde_json::from_str(peer["objectData"].as_str().unwrap()).unwrap();
        assert_eq!(obj["toUid"], "200");
        assert_eq!(obj["creatorUid"], "100");
        assert!(save_params(
            &s,
            "200",
            false,
            None,
            "Reminder",
            (crate::login::unix_now() - 60) * 1000,
            None
        )
        .is_err());
    }
    #[test]
    fn pin_params_keep_cli_codes_distinct_from_reaction_codes() {
        let s = session();
        let message = crate::chat::ChatMessage {
            id: "500".into(),
            client_message_id: "600".into(),
            is_outgoing: true,
            kind: crate::message_model::MessageKind::Photo,
            text: "caption".into(),
            ..Default::default()
        };
        let body = pin_params(&s, "200", &message).unwrap();
        let params: Value = serde_json::from_str(body["params"].as_str().unwrap()).unwrap();
        assert_eq!(body["type"], 2);
        assert_eq!(params["msg_type"], 32);
        assert_eq!(params["senderUid"], "100");
        assert_eq!(params["global_msg_id"], "500");
    }
}
