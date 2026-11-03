//! Typing indicators and delivery/read receipts from the realtime transports.
//!
//! Evidence (Zalo PC 26.11.10 `compact-app-pc`):
//! - `_processRawMessages`: `actions[]` entries with `act_type:"typing"` and `act` `typing`
//!   (1-1) or `gtyping` (group); `data` is a JSON object body without braces
//!   (`"{" + data + "}"`) holding `uid`, `gid` (groups), `ts` and `isPC`. Typing from the
//!   account itself is ignored. The indicator is shown for `userTypingTime` (8 s).
//! - `getDeliveredsFromData`: `delivereds[]` with `msgId`, `seen`, `seenUids`,
//!   `deliveredUids` and `groupId` / `threadId`; `seens[]` and `groupSeens[]` sit beside the
//!   messages of the same payload.
//! - `sendIsTyping`: 1-1 → chat domain `/api/message/typing` `{toid, destType, imei}`;
//!   group → group domain `/api/group/typing` `{grid, imei}`; sent at most every 10 s.
use serde_json::Value;

/// How long a typing indicator stays visible without a new event (`userTypingTime`).
pub const TYPING_VISIBLE: std::time::Duration = std::time::Duration::from_secs(8);
/// Minimum spacing between outgoing typing signals for one conversation (`functionTyping`).
pub const TYPING_SEND_EVERY: std::time::Duration = std::time::Duration::from_secs(10);
/// `DEST_TYPING.USER` in Zalo PC.
const DEST_TYPING_USER: i64 = 3;

/// Someone is typing in a conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Typing {
    /// Conversation ID without the group prefix.
    pub thread_id: String,
    pub is_group: bool,
    pub uid: String,
}

/// How far an outgoing message has progressed at the recipient(s).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    /// Conversation ID without the group prefix; empty when only the message ID is known.
    pub thread_id: String,
    pub is_group: bool,
    pub msg_ids: Vec<String>,
    pub read: bool,
}

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() && s.len() <= 64 => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
fn field(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| v.get(*k).and_then(text))
}
fn clean_id(id: String) -> String {
    id.trim_start_matches("group_")
        .trim_start_matches('g')
        .to_owned()
}
/// Find the payload level that carries the lists (`data` may be nested or a JSON string).
fn body(payload: &Value) -> Value {
    let mut node = payload.clone();
    for _ in 0..3 {
        let next = match node.get("data") {
            Some(Value::String(raw)) => serde_json::from_str(raw).ok(),
            Some(inner @ Value::Object(_)) => Some(inner.clone()),
            _ => None,
        };
        match next {
            Some(inner) if !has_lists(&node) => node = inner,
            _ => break,
        }
    }
    node
}
fn has_lists(v: &Value) -> bool {
    ["actions", "delivereds", "seens", "groupSeens"]
        .iter()
        .any(|k| v.get(*k).is_some())
}

/// Parse typing actions; `self_id` filters the account's own devices.
pub fn typing(payload: &Value, self_id: &str) -> Vec<Typing> {
    let body = body(payload);
    let mut out = Vec::new();
    for action in body
        .get("actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if action.get("act_type").and_then(Value::as_str) != Some("typing") {
            continue;
        }
        let group = match action.get("act").and_then(Value::as_str) {
            Some("typing") => false,
            Some("gtyping") => true,
            _ => continue,
        };
        let data = match action.get("data") {
            Some(Value::String(raw)) => {
                let raw = raw.trim();
                if raw.len() > 4096 {
                    continue;
                }
                let wrapped = if raw.starts_with('{') {
                    raw.to_owned()
                } else {
                    format!("{{{raw}}}")
                };
                serde_json::from_str::<Value>(&wrapped).unwrap_or(Value::Null)
            }
            Some(value) => value.clone(),
            None => Value::Null,
        };
        let Some(uid) = field(&data, &["uid"]) else {
            continue;
        };
        if uid == self_id {
            continue;
        }
        let thread_id = if group {
            match field(&data, &["gid", "grid"]) {
                Some(gid) => clean_id(gid),
                None => continue,
            }
        } else {
            uid.clone()
        };
        out.push(Typing {
            thread_id,
            is_group: group,
            uid,
        });
    }
    out
}

/// Parse delivery and read receipts carried beside messages.
pub fn receipts(payload: &Value) -> Vec<Receipt> {
    let body = body(payload);
    let mut out = Vec::new();
    let ids = |v: &Value| -> Vec<String> {
        let mut ids: Vec<String> = ["msgId", "realMsgId"]
            .iter()
            .filter_map(|k| v.get(*k).and_then(text))
            .filter(|id| id != "0")
            .collect();
        ids.dedup();
        ids
    };
    let list = |key: &str| {
        body.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    for item in list("delivereds") {
        let msg_ids = ids(&item);
        if msg_ids.is_empty() {
            continue;
        }
        let (thread_id, is_group) = match field(&item, &["groupId"]) {
            Some(group) => (clean_id(group), true),
            None => (
                field(&item, &["threadId", "idTo"])
                    .or_else(|| {
                        item.get("deliveredUids")
                            .and_then(Value::as_array)
                            .and_then(|a| a.first())
                            .and_then(text)
                    })
                    .unwrap_or_default(),
                false,
            ),
        };
        let read = item
            .get("seen")
            .is_some_and(|v| v == true || v.as_i64().is_some_and(|n| n > 0));
        out.push(Receipt {
            thread_id,
            is_group,
            msg_ids,
            read,
        });
    }
    for (key, is_group) in [("seens", false), ("groupSeens", true)] {
        for item in list(key) {
            let msg_ids = ids(&item);
            if msg_ids.is_empty() {
                continue;
            }
            let thread_id = if is_group {
                field(&item, &["groupId", "grid"]).map(clean_id)
            } else {
                field(&item, &["idTo", "threadId", "uidFrom"])
            };
            out.push(Receipt {
                thread_id: thread_id.unwrap_or_default(),
                is_group,
                msg_ids,
                read: true,
            });
        }
    }
    out
}

/// Request body and path for a typing signal, as `sendIsTyping` builds it.
pub(crate) fn typing_request(thread_id: &str, is_group: bool, imei: &str) -> (&'static str, Value) {
    let id = thread_id.trim_start_matches("group_");
    if is_group {
        (
            "/api/group/typing",
            serde_json::json!({"grid": id.trim_start_matches('g'), "imei": imei}),
        )
    } else {
        (
            "/api/message/typing",
            serde_json::json!({"toid": id, "destType": DEST_TYPING_USER, "imei": imei}),
        )
    }
}

/// Send one typing signal through the service map (chat or group domain).
/// # Errors
/// Returns transport and session errors; callers treat typing as best effort.
pub async fn send_typing(
    s: &crate::session::Session,
    thread_id: &str,
    is_group: bool,
) -> anyhow::Result<()> {
    anyhow::ensure!(s.is_active() && s.login.is_some(), "Chưa đăng nhập");
    let (path, body) = typing_request(thread_id, is_group, &s.imei);
    let host = if is_group {
        &s.domains.group
    } else {
        &s.domains.chat
    };
    anyhow::ensure!(!host.is_empty(), "Thiếu máy chủ dịch vụ");
    let params = s.encoded_params(&body)?;
    let url = format!("{}{path}?{}", host.trim_end_matches('/'), s.common_params());
    s.post_form(&url, format!("params={params}")).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn typing_actions_follow_zalo_pc_wrapping_and_skip_self() {
        let payload = json!({"data":{"actions":[
            {"act_type":"typing","act":"typing","data":"\"uid\":\"111\",\"ts\":\"1700000000000000\",\"isPC\":0"},
            {"act_type":"typing","act":"gtyping","data":"\"uid\":\"222\",\"gid\":\"999\",\"ts\":\"1\",\"isPC\":1"},
            {"act_type":"typing","act":"typing","data":"\"uid\":\"me\""},
            {"act_type":"fsending","act":"sending","data":"\"uid\":\"333\""},
            {"act_type":"typing","act":"gtyping","data":"\"uid\":\"444\""}
        ]}});
        assert_eq!(
            typing(&payload, "me"),
            vec![
                Typing {
                    thread_id: "111".into(),
                    is_group: false,
                    uid: "111".into()
                },
                Typing {
                    thread_id: "999".into(),
                    is_group: true,
                    uid: "222".into()
                },
            ]
        );
    }

    #[test]
    fn receipts_cover_delivered_seen_and_group_seen_lists() {
        let payload = json!({"data":"{\"delivereds\":[{\"msgId\":\"10\",\"seen\":0,\"deliveredUids\":[\"u1\"]},{\"msgId\":\"11\",\"seen\":1,\"groupId\":\"g5\",\"seenUids\":[\"u2\"]}],\"seens\":[{\"idTo\":\"u3\",\"msgId\":\"12\",\"realMsgId\":\"13\"}],\"groupSeens\":[{\"msgId\":\"14\",\"groupId\":\"6\",\"seenUids\":[\"u4\"]}]}"});
        let got = receipts(&payload);
        assert_eq!(got.len(), 4);
        assert_eq!(
            got[0],
            Receipt {
                thread_id: "u1".into(),
                is_group: false,
                msg_ids: vec!["10".into()],
                read: false
            }
        );
        assert_eq!(
            got[1],
            Receipt {
                thread_id: "5".into(),
                is_group: true,
                msg_ids: vec!["11".into()],
                read: true
            }
        );
        assert_eq!(got[2].msg_ids, vec!["12".to_string(), "13".to_string()]);
        assert!(got[2].read && !got[2].is_group);
        assert_eq!(got[3].thread_id, "6");
        assert!(receipts(&json!({"data":{"msgs":[]}})).is_empty());
    }

    #[test]
    fn typing_request_matches_send_is_typing() {
        let (path, body) = typing_request("123", false, "imei-x");
        assert_eq!(path, "/api/message/typing");
        assert_eq!(body, json!({"toid":"123","destType":3,"imei":"imei-x"}));
        let (path, body) = typing_request("g456", true, "imei-x");
        assert_eq!(path, "/api/group/typing");
        assert_eq!(body, json!({"grid":"456","imei":"imei-x"}));
    }
}
