//! Unverified device API models and fail-closed network gates.
//!
//! Native PC evidence does not establish the previous request contract; see
//! `re-notes/55-unverified-feature-gates.md`. Parsers remain offline research helpers.

use crate::session::Session;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;

/// An active logged-in device session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceSession {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub last_active_ts: i64,
    pub location: String,
    pub is_current: bool,
}

/// Parse device list from Zalo API response.
pub fn parse_device_list(val: &Value, current_imei: Option<&str>) -> Result<Vec<DeviceSession>> {
    let envelope = crate::chat::confirmed_mutation(val.clone())?;
    let val = &envelope;
    let items = val
        .get("data")
        .and_then(|d| d.get("items").or_else(|| d.get("devices")))
        .or_else(|| val.get("items"))
        .and_then(|v| v.as_array());

    let arr = items.context("Thiếu danh sách thiết bị trong phản hồi")?;
    anyhow::ensure!(arr.len() <= 1000, "Danh sách thiết bị quá lớn");

    arr.iter()
        .map(|it| {
            let device_id = it
                .get("device_id")
                .or_else(|| it.get("imei"))
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty() && id.len() <= 256)
                .context("Thiết bị thiếu định danh hợp lệ")?
                .to_string();
            let name = it
                .get("name")
                .or_else(|| it.get("device_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Thiết bị không tên")
                .to_string();
            let platform = it
                .get("platform")
                .or_else(|| it.get("os"))
                .and_then(|v| v.as_str())
                .unwrap_or("Desktop")
                .to_string();
            let last_active_ts = it
                .get("last_active")
                .or_else(|| it.get("last_active_time"))
                .and_then(crate::api_response::integer)
                .unwrap_or(0);
            let location = it
                .get("location")
                .or_else(|| it.get("ip_location"))
                .and_then(|v| v.as_str())
                .unwrap_or("Việt Nam")
                .to_string();
            let is_current = it
                .get("is_current")
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| current_imei.is_some_and(|imei| imei == device_id));

            Ok(DeviceSession {
                device_id,
                name,
                platform,
                last_active_ts,
                location,
                is_current,
            })
        })
        .collect()
}

/// Return an unavailable error until this request contract has native evidence.
pub async fn get_logged_in_devices(_s: &Session) -> Result<Vec<DeviceSession>> {
    // Fail closed until native protocol evidence establishes this contract.
    anyhow::bail!("Danh sách thiết bị chưa khả dụng: chưa xác minh giao thức Zalo PC.")
}

/// Return an unavailable error until this request contract has native evidence.
pub async fn kick_remote_device(_s: &Session, _target_device_id: &str) -> Result<Value> {
    // Fail closed until native protocol evidence establishes this contract.
    anyhow::bail!("Đăng xuất từ xa chưa khả dụng: chưa xác minh giao thức Zalo PC.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unverified_contracts_fail_before_using_session_credentials() {
        let session = Session::new(false, false, false).unwrap();
        assert!(get_logged_in_devices(&session).await.is_err());
        assert!(kick_remote_device(&session, "target").await.is_err());
    }

    #[test]
    fn test_parse_device_list() {
        let val = json!({
            "error_code": 0,
            "data": {
                "items": [
                    {
                        "device_id": "arm64-win11-surface",
                        "device_name": "Surface Pro (Windows 11 ARM64)",
                        "platform": "Windows ARM64",
                        "last_active": 1793988000,
                        "location": "Hà Nội, Việt Nam",
                        "is_current": true
                    },
                    {
                        "device_id": "iphone-15-pro",
                        "device_name": "iPhone 15 Pro",
                        "platform": "iOS",
                        "last_active": 1793985000,
                        "location": "Đà Nẵng, Việt Nam",
                        "is_current": false
                    }
                ]
            }
        });

        let devices = parse_device_list(&val, Some("arm64-win11-surface")).unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Surface Pro (Windows 11 ARM64)");
        assert!(devices[0].is_current);

        assert_eq!(devices[1].name, "iPhone 15 Pro");
        assert!(!devices[1].is_current);
    }
}
