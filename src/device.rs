//! Device Management and Remote Kick (Zalo Parity C02).
//!
//! Provides models and API helpers for listing logged-in device sessions and
//! kicking unauthorized sessions remotely (`/api/login/getdevicelist`, `/api/login/kickdevice`).

use crate::session::Session;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

/// Call `/api/login/getdevicelist` to retrieve active devices.
pub async fn get_logged_in_devices(s: &Session) -> Result<Vec<DeviceSession>> {
    let url = format!("{}/api/login/getdevicelist", s.domains.chat);
    let payload = json!({
        "imei": s.imei,
    });
    let params = s
        .encoded_params(&payload)
        .context("Mã hoá tham số lấy danh sách thiết bị thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    parse_device_list(&decrypted, Some(&s.imei))
}

/// Call `/api/login/kickdevice` to remotely log out a device.
pub async fn kick_remote_device(s: &Session, target_device_id: &str) -> Result<Value> {
    let url = format!("{}/api/login/kickdevice", s.domains.chat);
    let payload = json!({
        "target_device_id": target_device_id,
        "imei": s.imei,
    });
    let params = s
        .encoded_params(&payload)
        .context("Mã hoá tham số đăng xuất thiết bị thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    crate::chat::confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

#[cfg(test)]
mod tests {
    use super::*;

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
