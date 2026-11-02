//! Account-wide conversation mute state shared with official Zalo clients.
//!
//! Wire contract (`getmute`, `setmute`, `act_type: "mute"` controls) is documented in
//! `re-notes/mute-sync.md`; only the mute flag is synchronized, pin/hide stay local.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::session::Session;

/// One server-side mute change for a conversation identified by its raw thread ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuteChange {
    /// Raw numeric thread ID as stored in `thread_preferences` (no `g` prefix).
    pub thread_id: String,
    /// `true` when notifications are currently suppressed by the server state.
    pub muted: bool,
}

fn number(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str()?.trim().parse().ok())
}

fn identity(v: &Value) -> Option<String> {
    let raw = v
        .as_str()
        .map(str::to_owned)
        .or_else(|| v.as_u64().map(|n| n.to_string()))
        .or_else(|| v.as_i64().map(|n| n.to_string()))?;
    let raw = raw.trim();
    let clean = raw
        .strip_prefix("group_")
        .or_else(|| raw.strip_prefix('g'))
        .or_else(|| raw.strip_prefix('G'))
        .unwrap_or(raw);
    // Thread identities are numeric server IDs; anything else is ignored, never guessed.
    (!clean.is_empty() && clean != "0" && clean.bytes().all(|b| b.is_ascii_digit()))
        .then(|| clean.to_owned())
}

/// Whether one mute entry is active at `now_unix`.
///
/// `duration == -1` means "until turned back on"; a positive duration expires at
/// `startTime + duration`; `muteMode != 1` or `duration == 0` is not muted.
fn entry_is_muted(entry: &Value, now_unix: i64) -> bool {
    let mode = entry.get("muteMode").and_then(number).unwrap_or(1);
    let duration = entry.get("duration").and_then(number).unwrap_or(-1);
    if mode != 1 || duration == 0 {
        return false;
    }
    if duration < 0 {
        return true;
    }
    let start = entry
        .get("startTime")
        .or_else(|| entry.get("systemTime"))
        .and_then(number)
        .unwrap_or(now_unix);
    // Some payloads carry milliseconds; normalize before comparing with Unix seconds.
    let start = if start > 100_000_000_000 {
        start / 1000
    } else {
        start
    };
    start.saturating_add(duration) > now_unix
}

fn decode_layers(mut value: Value) -> Result<Value> {
    for _ in 0..4 {
        if let Some(code) = value.get("error_code").and_then(number) {
            anyhow::ensure!(code == 0, "server error {code}: mute snapshot unavailable");
        }
        if let Some(text) = value.as_str() {
            value = serde_json::from_str(text).context("Invalid mute snapshot JSON")?;
        } else if value.get("chatEntries").is_some() {
            return Ok(value);
        } else if let Some(data) = value.get("data").cloned() {
            value = data;
        } else {
            return Ok(value);
        }
    }
    Ok(value)
}

/// Parse a complete `getmute` response into the set of currently muted thread IDs.
///
/// # Errors
/// Returns an error for a non-zero `error_code` or when either entry list is missing, so a
/// partial or malformed response is never mistaken for "nothing is muted".
pub fn parse_mute_snapshot(
    response: Value,
    now_unix: i64,
) -> Result<std::collections::HashSet<String>> {
    let payload = decode_layers(response)?;
    let peers = payload
        .get("chatEntries")
        .and_then(Value::as_array)
        .context("Mute snapshot lacks chatEntries")?;
    let groups = payload
        .get("groupChatEntries")
        .and_then(Value::as_array)
        .context("Mute snapshot lacks groupChatEntries")?;
    Ok(peers
        .iter()
        .chain(groups)
        .filter(|entry| entry_is_muted(entry, now_unix))
        .filter_map(|entry| identity(entry.get("id")?))
        .collect())
}

/// Extract `act_type: "mute"` changes from a decrypted control payload (WebSocket `601`/`603`
/// or `lp_control`). Unknown or malformed items are skipped.
pub fn parse_mute_controls(data_node: &Value, now_unix: i64) -> Vec<MuteChange> {
    let mut candidates: Vec<&Value> = Vec::new();
    match data_node {
        Value::Array(items) => {
            for item in items {
                match item.get("controls").and_then(Value::as_array) {
                    Some(sub) => candidates.extend(sub),
                    None => candidates.push(item),
                }
            }
        }
        _ => match data_node.get("controls").and_then(Value::as_array) {
            Some(items) => candidates.extend(items),
            None => candidates.push(data_node),
        },
    }
    let mut out = Vec::new();
    for item in candidates {
        let content = item.get("content").unwrap_or(item);
        if content.get("act_type").and_then(Value::as_str) != Some("mute") {
            continue;
        }
        let raw = content.get("data").unwrap_or(content);
        let decoded;
        let data = match raw.as_str() {
            Some(text) => match serde_json::from_str::<Value>(text) {
                Ok(value) => {
                    decoded = value;
                    &decoded
                }
                Err(_) => continue,
            },
            None => raw,
        };
        let Some(thread_id) = ["toId", "toid", "id"]
            .iter()
            .find_map(|key| identity(data.get(*key)?))
        else {
            continue;
        };
        let muted = match content.get("act").and_then(Value::as_str) {
            Some("unset") => false,
            Some("set") => entry_is_muted(data, now_unix),
            _ => {
                data.get("action").and_then(number).unwrap_or(1) == 1
                    && entry_is_muted(data, now_unix)
            }
        };
        out.push(MuteChange { thread_id, muted });
    }
    out
}

/// Read the account-wide mute snapshot shared with official Zalo clients.
///
/// # Errors
/// Returns transport, protocol or malformed-response failures; callers keep local state.
pub async fn fetch_mute_snapshot(s: &Session) -> Result<std::collections::HashSet<String>> {
    anyhow::ensure!(s.is_active() && s.login.is_some(), "Not logged in");
    let params = s.encoded_params(&json!({ "imei": s.imei }))?;
    let url = format!(
        "{}/api/social/profile/getmute?{}&params={params}",
        s.domains.profile,
        s.common_params()
    );
    let response = s
        .maybe_decrypt(s.get(&url).await?, s.session_key_bytes())
        .await;
    parse_mute_snapshot(response, crate::login::unix_now())
}

/// Build the `setmute` body; `muted = true` mutes until turned back on (`duration = -1`).
fn set_mute_body(thread_id: &str, is_group: bool, muted: bool, imei: &str, now: i64) -> Value {
    json!({
        "toid": thread_id,
        "duration": -1,
        "action": if muted { 1 } else { 2 },
        "startTime": now,
        "muteType": if is_group { 2 } else { 1 },
        "imei": imei,
    })
}

/// Change the account-wide mute state of one conversation.
///
/// # Errors
/// Fails closed: any transport error or unconfirmed response is an error and the caller must
/// leave the local preference unchanged.
pub async fn set_remote_mute(
    s: &Session,
    thread_id: &str,
    is_group: bool,
    muted: bool,
) -> Result<()> {
    anyhow::ensure!(s.is_active() && s.login.is_some(), "Not logged in");
    let id = identity(&json!(thread_id)).context("Invalid thread identity")?;
    let body = set_mute_body(&id, is_group, muted, &s.imei, crate::login::unix_now());
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/social/profile/setmute?{}",
        s.domains.profile,
        s.common_params()
    );
    let response = s.post_form(&url, format!("params={params}")).await?;
    crate::chat::confirmed_mutation(s.maybe_decrypt(response, s.session_key_bytes()).await)?;
    Ok(())
}

/// Merge a complete server snapshot into local preferences without touching pin/hide.
///
/// Returns the thread IDs whose mute flag changed.
pub fn reconcile(
    local: &HashMap<String, crate::store::ConversationPreference>,
    server_muted: &std::collections::HashSet<String>,
) -> Vec<MuteChange> {
    let mut changes: Vec<MuteChange> = local
        .iter()
        .filter(|(id, value)| value.muted && !server_muted.contains(*id))
        .map(|(id, _)| MuteChange {
            thread_id: id.clone(),
            muted: false,
        })
        .collect();
    changes.extend(
        server_muted
            .iter()
            .filter(|id| !local.get(*id).is_some_and(|value| value.muted))
            .map(|id| MuteChange {
                thread_id: id.clone(),
                muted: true,
            }),
    );
    changes.sort_by(|a, b| a.thread_id.cmp(&b.thread_id));
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn snapshot_includes_indefinite_and_unexpired_mutes_when_complete() {
        let response = json!({"error_code":0,"data":{
            "chatEntries":[
                {"id":"1001","duration":-1,"muteMode":1},
                {"id":"1002","duration":3600,"startTime":NOW-60,"muteMode":1},
                {"id":"1003","duration":3600,"startTime":NOW-7200,"muteMode":1},
                {"id":"1004","duration":-1,"muteMode":0}
            ],
            "groupChatEntries":[{"id":"g2001","duration":-1,"muteMode":1}]
        }});
        let muted = parse_mute_snapshot(response, NOW).unwrap();
        let mut ids: Vec<_> = muted.into_iter().collect();
        ids.sort();
        assert_eq!(ids, ["1001", "1002", "2001"]);
    }

    #[test]
    fn snapshot_accepts_millisecond_start_time_when_server_sends_it() {
        let response = json!({"chatEntries":[{"id":"1","duration":60,"startTime":(NOW-30)*1000}],"groupChatEntries":[]});
        assert!(parse_mute_snapshot(response, NOW).unwrap().contains("1"));
    }

    #[test]
    fn snapshot_rejects_partial_or_failed_response_when_lists_missing() {
        assert!(parse_mute_snapshot(json!({"data":{"chatEntries":[]}}), NOW).is_err());
        assert!(parse_mute_snapshot(json!({"error_code":-102}), NOW).is_err());
        assert!(parse_mute_snapshot(json!({"data":"not json"}), NOW).is_err());
    }

    #[test]
    fn snapshot_decodes_string_encoded_data_when_double_wrapped() {
        let inner = json!({"chatEntries":[{"id":7,"duration":-1}],"groupChatEntries":[]});
        let response = json!({"error_code":0,"data":inner.to_string()});
        assert!(parse_mute_snapshot(response, NOW).unwrap().contains("7"));
    }

    #[test]
    fn controls_parse_set_unset_and_timed_events_for_peers_and_groups() {
        let payload = json!({"controls":[
            {"content":{"act_type":"mute","act":"set","data":{"toId":"111","muteType":1,"duration":-1,"muteMode":1}}},
            {"content":{"act_type":"mute","act":"set","data":"{\"toId\":\"222\",\"muteType\":2,\"duration\":3600,\"startTime\":1799999000,\"muteMode\":1}"}},
            {"content":{"act_type":"mute","act":"unset","data":{"toId":"333"}}},
            {"content":{"act_type":"voip","act":"set","data":{"toId":"444"}}},
            {"content":{"act_type":"mute","act":"set","data":{"toId":"not-a-number"}}}
        ]});
        assert_eq!(
            parse_mute_controls(&payload, NOW),
            vec![
                MuteChange {
                    thread_id: "111".into(),
                    muted: true
                },
                MuteChange {
                    thread_id: "222".into(),
                    muted: true
                },
                MuteChange {
                    thread_id: "333".into(),
                    muted: false
                },
            ]
        );
    }

    #[test]
    fn controls_treat_expired_set_as_unmuted_when_duration_elapsed() {
        let payload = json!([{"controls":[{"content":{"act_type":"mute","act":"set",
            "data":{"toId":"5","duration":10,"startTime":NOW-100,"muteMode":1}}}]}]);
        assert_eq!(
            parse_mute_controls(&payload, NOW),
            vec![MuteChange {
                thread_id: "5".into(),
                muted: false
            }]
        );
    }

    #[test]
    fn set_mute_body_encodes_action_and_type_when_muting_group() {
        let body = set_mute_body("42", true, true, "imei-1", NOW);
        assert_eq!(body["action"], 1);
        assert_eq!(body["muteType"], 2);
        assert_eq!(body["duration"], -1);
        assert_eq!(body["toid"], "42");
        assert_eq!(
            set_mute_body("42", false, false, "imei-1", NOW)["action"],
            2
        );
    }

    #[test]
    fn reconcile_reports_only_flag_differences_when_snapshot_complete() {
        use crate::store::ConversationPreference as P;
        let local = HashMap::from([
            (
                "a".to_string(),
                P {
                    muted: true,
                    pinned: true,
                    hidden: false,
                },
            ),
            (
                "b".to_string(),
                P {
                    muted: false,
                    pinned: false,
                    hidden: true,
                },
            ),
            (
                "c".to_string(),
                P {
                    muted: true,
                    ..P::default()
                },
            ),
        ]);
        let server =
            std::collections::HashSet::from(["b".to_string(), "c".to_string(), "d".to_string()]);
        assert_eq!(
            reconcile(&local, &server),
            vec![
                MuteChange {
                    thread_id: "a".into(),
                    muted: false
                },
                MuteChange {
                    thread_id: "b".into(),
                    muted: true
                },
                MuteChange {
                    thread_id: "d".into(),
                    muted: true
                },
            ]
        );
    }
}
