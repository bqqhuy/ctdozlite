//! On-demand last-activity lookup; missing timestamps never become an online indicator.
use crate::session::Session;
use anyhow::{ensure, Result};
use serde_json::{json, Value};
/// Unknown privacy or a hidden timestamp remains unavailable, not offline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    PrivacyUnavailable,
    Unavailable,
    LastActivity(i64),
}
/// Decode a positive server timestamp only, respecting an explicit peer privacy refusal.
pub fn parse(data: &Value, now: i64) -> Status {
    if data
        .pointer("/settings/show_online_status")
        .is_some_and(|value| {
            value == &json!(false) || crate::api_response::integer(value) == Some(0)
        })
    {
        return Status::Unavailable;
    }
    let Some(timestamp) = data
        .get("lastOnline")
        .and_then(crate::api_response::integer)
        .filter(|value| *value > 0)
        .map(crate::clock::normalize_ts_ms)
    else {
        return Status::Unavailable;
    };
    if timestamp > now.saturating_add(60_000) {
        return Status::Unavailable;
    }
    Status::LastActivity(timestamp)
}
/// Follow PC's own-privacy gate before querying a normal user (1) or OA (2).
/// # Errors
/// Invalid targets, account changes and transport/server refusals are explicit failures.
pub async fn read(s: &Session, target: &str, official: bool) -> Result<Status> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    ensure!(
        !target.is_empty()
            && target.len() <= 32
            && target.bytes().all(|byte| byte.is_ascii_digit())
            && target != s.account_key(),
        "Liên hệ không hợp lệ"
    );
    let privacy = crate::privacy::execute(s, &crate::privacy::Action::Read).await?;
    if privacy.values.get(&crate::privacy::Setting::ShowOnline) != Some(&1) {
        return Ok(Status::PrivacyUnavailable);
    }
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    let params =
        s.encoded_params(&json!({"uid":target,"conv_type":if official {2}else{1},"imei":s.imei}))?;
    let raw = s
        .get(&format!(
            "{}/api/social/profile/lastOnline?{}&params={params}",
            s.domains.profile,
            s.common_params()
        ))
        .await?;
    let data =
        crate::api_response::checked_data(s.maybe_decrypt(raw, s.session_key_bytes()).await)?;
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    Ok(parse(&data, chrono::Utc::now().timestamp_millis()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_valid_activity_times_are_shown() {
        let now = 1_800_000_000_000_i64;
        for value in [
            json!({}),
            json!({"lastOnline":0}),
            json!({"lastOnline":-1}),
            json!({"lastOnline":now+90_000}),
            json!({"lastOnline":now,"settings":{"show_online_status":false}}),
        ] {
            assert_eq!(parse(&value, now), Status::Unavailable);
        }
        assert_eq!(
            parse(&json!({"lastOnline":1_799_999_999}), now),
            Status::LastActivity(1_799_999_999_000)
        );
    }
}
