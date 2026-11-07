//! Account Security & Recovery Management (Zalo Parity C02).
//!
//! Provides data structures and API helpers to inspect 2-step verification (2FA) status
//! and manage account authentication security (`/api/login/check2fa`, `/api/login/change_pwd`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::session::Session;

/// Status of Two-Factor Authentication (2FA) on the account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TwoFactorStatus {
    pub is_enabled: bool,
    pub phone_masked: String,
    pub has_authenticator_app: bool,
}

/// Parse 2FA status from Zalo API response.
pub fn parse_two_factor_status(val: &Value) -> TwoFactorStatus {
    let data = val.get("data").unwrap_or(val);
    let is_enabled = data
        .get("enabled")
        .or_else(|| data.get("is_2fa"))
        .or_else(|| data.get("status"))
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n == 1)))
        .unwrap_or(false);

    let phone_masked = data
        .get("phone")
        .or_else(|| data.get("masked_phone"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let has_authenticator_app = data
        .get("has_app")
        .or_else(|| data.get("otp_app"))
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n == 1)))
        .unwrap_or(false);

    TwoFactorStatus {
        is_enabled,
        phone_masked,
        has_authenticator_app,
    }
}

/// Call `/api/login/check2fa` to check whether 2FA is active on the account.
pub async fn check_two_factor_status(s: &Session) -> Result<TwoFactorStatus> {
    let url = format!("{}/api/login/check2fa", s.domains.chat);
    let payload = json!({
        "imei": s.imei,
    });
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số kiểm tra 2FA thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(parse_two_factor_status(&decrypted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_two_factor_status() {
        let val = json!({
            "error_code": 0,
            "data": {
                "enabled": true,
                "masked_phone": "+84 *** *** 888",
                "otp_app": false
            }
        });

        let status = parse_two_factor_status(&val);
        assert!(status.is_enabled);
        assert_eq!(status.phone_masked, "+84 *** *** 888");
        assert!(!status.has_authenticator_app);
    }
}
