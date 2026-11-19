//! Unverified account recovery API models and fail-closed network gates.
//!
//! Native PC evidence does not establish the previous request contract; see
//! `re-notes/55-unverified-feature-gates.md`. Parsers remain offline research helpers.

use crate::session::Session;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;

/// Status of Two-Factor Authentication (2FA) on the account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TwoFactorStatus {
    pub is_enabled: bool,
    pub phone_masked: String,
    pub has_authenticator_app: bool,
}

/// Parse 2FA status from Zalo API response.
pub fn parse_two_factor_status(val: &Value) -> Result<TwoFactorStatus> {
    let data = crate::api_response::checked_data(val.clone())?;
    let is_enabled = data
        .get("enabled")
        .or_else(|| data.get("is_2fa"))
        .or_else(|| data.get("status"))
        .and_then(boolean)
        .context("Thiếu trạng thái xác thực hai bước")?;

    let phone_masked = data
        .get("phone")
        .or_else(|| data.get("masked_phone"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let has_authenticator_app = data
        .get("has_app")
        .or_else(|| data.get("otp_app"))
        .and_then(boolean)
        .context("Thiếu thông tin ứng dụng xác thực")?;

    Ok(TwoFactorStatus {
        is_enabled,
        phone_masked,
        has_authenticator_app,
    })
}

fn boolean(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| match crate::api_response::integer(value)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        })
}

/// Return an unavailable error until this request contract has native evidence.
pub async fn check_two_factor_status(_s: &Session) -> Result<TwoFactorStatus> {
    // Fail closed until native protocol evidence establishes this contract.
    anyhow::bail!("Trạng thái 2FA chưa xác định: chưa xác minh giao thức Zalo PC.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unverified_contracts_fail_before_using_session_credentials() {
        let session = Session::new(false, false, false).unwrap();
        assert!(check_two_factor_status(&session).await.is_err());
    }

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

        let status = parse_two_factor_status(&val).unwrap();
        assert!(status.is_enabled);
        assert_eq!(status.phone_masked, "+84 *** *** 888");
        assert!(!status.has_authenticator_app);
    }

    #[test]
    fn missing_status_and_server_refusals_are_not_disabled_2fa() {
        for value in [
            json!({"error_code":600,"data":{}}),
            json!({"error_code":0,"data":{}}),
            json!({}),
        ] {
            assert!(parse_two_factor_status(&value).is_err());
        }
    }
}
