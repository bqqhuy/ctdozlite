//! Bounded invalidation hints from official control types; never patch permissions from guessed fields.
use serde_json::Value;
use std::collections::HashSet;
#[derive(Default, Debug)]
pub struct Impact {
    pub activities: Vec<(String, crate::last_activity::Status)>,
    pub profiles: bool,
    pub privacy: bool,
    pub relations: bool,
    pub account: bool,
    pub hidden: bool,
    pub groups: HashSet<String>,
    pub all_groups: bool,
}
/// Only native control queues may invalidate account or group state. Chat text is never control.
pub fn from_frame(event: &crate::socket::WsEvent) -> Impact {
    if event.is_voip_ish() {
        parse(&event.payload)
    } else {
        Impact::default()
    }
}
/// Parse content envelopes and bounded JSON-string data on authenticated control pages.
pub fn parse(payload: &Value) -> Impact {
    fn visit(value: &Value, depth: usize, out: &mut Impact) {
        if depth > 8 {
            return;
        }
        if let Some(text) = value.as_str() {
            if text.len() <= 1_048_576 {
                if let Ok(value) = serde_json::from_str::<Value>(text) {
                    visit(&value, depth + 1, out);
                }
            }
            return;
        }
        if let Some(rows) = value.as_array() {
            for row in rows.iter().take(4096) {
                visit(row, depth + 1, out);
            }
            return;
        }
        if let Some(rows) = value.get("controls") {
            visit(rows, depth + 1, out);
            return;
        }
        if let Some(content) = value.get("content") {
            visit(content, depth + 1, out);
            return;
        }
        let Some(kind) = value.get("act_type").and_then(Value::as_str) else {
            if let Some(data) = value.get("data") {
                visit(data, depth + 1, out);
            }
            return;
        };
        match kind {
            "hidden_chat" => {
                out.hidden = true;
                out.account = true;
            }
            "my_settings" => {
                out.privacy = true;
                out.account = true;
            }
            "fr" => {
                out.relations = true;
                out.account = true;
            }
            "alias" | "label_convers" | "pinned_convers" | "mark_unread" | "archived_convers" => {
                out.account = true
            }
            "profile" if value.get("act").and_then(Value::as_str) == Some("online_status") => {
                if let Some(data) = value.get("data") {
                    if let Some(uid) = data
                        .get("uid")
                        .and_then(|value| {
                            value
                                .as_str()
                                .map(str::to_owned)
                                .or_else(|| value.as_u64().map(|id| id.to_string()))
                        })
                        .filter(|uid| {
                            !uid.is_empty()
                                && uid.len() <= 32
                                && uid.bytes().all(|b| b.is_ascii_digit())
                        })
                    {
                        let status = crate::last_activity::parse(
                            data,
                            chrono::Utc::now().timestamp_millis(),
                        );
                        if status != crate::last_activity::Status::Unavailable {
                            out.activities.push((uid, status));
                        }
                    }
                }
            }
            "profile" => {
                out.profiles = true;
                out.account = true;
            }
            "group" => {
                let raw = value.get("data").unwrap_or(&Value::Null);
                let owned;
                let data = if let Some(text) = raw.as_str() {
                    if text.len() > 1_048_576 {
                        out.all_groups = true;
                        return;
                    }
                    match serde_json::from_str::<Value>(text) {
                        Ok(value) => {
                            owned = value;
                            &owned
                        }
                        Err(_) => {
                            out.all_groups = true;
                            return;
                        }
                    }
                } else {
                    raw
                };
                let id = data
                    .get("groupId")
                    .or_else(|| data.get("group_id"))
                    .and_then(|id| {
                        id.as_str()
                            .map(str::to_owned)
                            .or_else(|| id.as_u64().map(|id| id.to_string()))
                    });
                if let Some(id) = id.and_then(|id| crate::server_pins::identity(&id, true).ok()) {
                    out.groups.insert(id);
                } else {
                    out.all_groups = true;
                }
            }
            _ => {}
        }
    }
    let mut impact = Impact::default();
    visit(payload, 0, &mut impact);
    impact
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn control_hints_invalidate_without_accepting_guessed_permissions() {
        let hint = parse(
            &json!({"data":{"controls":[{"content":{"act_type":"group","act":"update_setting","data":"{\"groupId\":12,\"groupSetting\":{\"lockSendMsg\":0}}"}},{"content":{"act_type":"hidden_chat","act":"change_pin"}}]}}),
        );
        assert!(hint.groups.contains("g12"));
        assert!(hint.hidden && hint.account);
        assert!(!parse(&json!({"act_type":"typing"})).account);
        assert!(parse(&json!({"act_type":"group","data":{}})).all_groups);
    }
}

#[cfg(test)]
mod activity_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn activity_controls_do_not_trigger_account_wide_sync() {
        let now = chrono::Utc::now().timestamp_millis();
        let impact = parse(
            &json!({"controls":[{"content":{"act_type":"profile","act":"online_status","data":{"uid":"12","lastOnline":now}}}]}),
        );
        assert!(!impact.account);
        assert_eq!(impact.activities.len(), 1);
        assert_eq!(impact.activities[0].0, "12");
        assert!(parse(&json!({"act_type":"profile","act":"update"})).account);
    }
}

#[cfg(test)]
mod frame_boundary_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn a_chat_payload_cannot_impersonate_a_hidden_or_group_control() {
        let payload = json!({"data":{"controls":[{"content":{"act_type":"hidden_chat","act":"change_pin"}}]}});
        let chat = crate::socket::WsEvent {
            cmd: 501,
            sub_cmd: 0,
            version: 1,
            payload: payload.clone(),
        };
        assert!(!from_frame(&chat).hidden);
        let control = crate::socket::WsEvent {
            cmd: 603,
            sub_cmd: 0,
            version: 1,
            payload,
        };
        assert!(from_frame(&control).hidden);
    }
    #[test]
    fn nested_array_control_pages_keep_their_invalidation_hints() {
        assert!(parse(&json!({"data":[{"controls":[{"content":{"act_type":"hidden_chat","act":"add_remove"}}]}]})).hidden);
    }
}
