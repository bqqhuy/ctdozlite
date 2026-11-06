//! Server privacy settings from the PC/Web privacy controller, with readback after mutation.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
/// Only settings whose names and value domains appear in the official controller/UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Setting {
    StrangerDirect,
    ReceiveMessages,
    ReceiveCalls,
    ShowOnline,
    DisplaySeen,
}
impl Setting {
    /// Exact server property; numeric registration IDs are not request field names.
    pub fn key(self) -> &'static str {
        match self {
            Self::StrangerDirect => "setting_show_stranger_msg_directly",
            Self::ReceiveMessages => "receive_message",
            Self::ReceiveCalls => "accept_stranger_call",
            Self::ShowOnline => "show_online_status",
            Self::DisplaySeen => "display_seen_status",
        }
    }
    /// Reject unsupported values instead of silently turning them into a default permission.
    pub fn valid(self, value: i64) -> bool {
        match self {
            Self::ReceiveMessages => matches!(value, 1 | 2),
            Self::ReceiveCalls => matches!(value, 2..=4),
            _ => matches!(value, 0 | 1),
        }
    }
}
/// Missing keys remain unsupported/unknown for this account.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub values: BTreeMap<Setting, i64>,
}
impl Snapshot {
    /// Decode only explicit fields and retain unknown as absence.
    /// # Errors
    /// Rejects malformed setting maps or recognized fields outside their official value domain.
    pub fn parse(value: Value) -> Result<Self> {
        ensure!(
            value.is_object(),
            "Phản hồi quyền riêng tư không phải danh sách thiết lập"
        );
        let mut values = BTreeMap::new();
        for setting in [
            Setting::StrangerDirect,
            Setting::ReceiveMessages,
            Setting::ReceiveCalls,
            Setting::ShowOnline,
            Setting::DisplaySeen,
        ] {
            if let Some(raw) = value.get(setting.key()) {
                let number = crate::api_response::integer(raw)
                    .or_else(|| raw.as_bool().map(i64::from))
                    .context("Giá trị quyền riêng tư không hợp lệ")?;
                ensure!(
                    setting.valid(number),
                    "Máy chủ trả giá trị quyền riêng tư chưa được hỗ trợ"
                );
                values.insert(setting, number);
            }
        }
        Ok(Self { values })
    }
}
/// One user intent, never an automatic mutation/retry.
#[derive(Clone, Debug)]
pub enum Action {
    Read,
    Set { setting: Setting, value: i64 },
}
/// Construct only the documented key and value, without leaking unrelated preferences.
/// # Errors
/// Rejects unsupported values before I/O.
pub fn update_params(setting: Setting, value: i64) -> Result<Value> {
    ensure!(
        setting.valid(value),
        "Giá trị quyền riêng tư không được hỗ trợ"
    );
    let mut body = json!({});
    body[setting.key()] = json!(value);
    Ok(body)
}
async fn call(s: &Session, path: &str, body: Option<Value>) -> Result<Value> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    let mut url = format!("{}{path}?{}", s.domains.auth, s.common_params());
    if let Some(body) = body {
        url.push_str("&params=");
        url.push_str(&s.encoded_params(&body)?);
    }
    let raw = s.get(&url).await?;
    let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    ensure!(
        raw.get("error_code").is_some(),
        "Thiếu xác nhận quyền riêng tư từ máy chủ"
    );
    crate::api_response::checked_data(raw)
}
async fn read(s: &Session) -> Result<Snapshot> {
    Snapshot::parse(call(s, "/api/setting/me", None).await?)
}
/// Read fresh capabilities, perform one mutation, then require the requested value in readback.
/// # Errors
/// Missing keys, refusal, transport errors and mismatched readback never produce optimistic success.
pub async fn execute(s: &Session, action: &Action) -> Result<Snapshot> {
    let before = read(s).await?;
    let Action::Set { setting, value } = action else {
        return Ok(before);
    };
    ensure!(
        before.values.contains_key(setting),
        "Tài khoản chưa cung cấp thiết lập này"
    );
    call(
        s,
        "/api/setting/update",
        Some(update_params(*setting, *value)?),
    )
    .await?;
    let after = read(s).await?;
    ensure!(
        after.values.get(setting) == Some(value),
        "Chưa xác nhận máy chủ đã áp dụng; tải lại trước khi thử tiếp"
    );
    Ok(after)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_setting_is_distinct_from_receive_permission_and_unknown_stays_unknown() {
        let snapshot =
            Snapshot::parse(json!({"setting_show_stranger_msg_directly":0,"receive_message":"2"}))
                .unwrap();
        assert_eq!(snapshot.values[&Setting::StrangerDirect], 0);
        assert_eq!(snapshot.values[&Setting::ReceiveMessages], 2);
        assert!(!snapshot.values.contains_key(&Setting::ShowOnline));
        assert_eq!(
            update_params(Setting::ReceiveMessages, 1).unwrap(),
            json!({"receive_message":1})
        );
        assert!(update_params(Setting::ReceiveMessages, 0).is_err());
        assert!(Snapshot::parse(json!({"display_seen_status":7})).is_err());
        assert!(Snapshot::parse(Value::Null).is_err());
    }
}
