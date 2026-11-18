//! zCloud Media Storage & Cloud Recovery (Zalo Parity A20).
//!
//! Provides data structures and API helpers to retrieve viewer authentication keys,
//! verify media presence on zCloud, and obtain signed download URLs
//! (`/api/getCloudViewerKey`, `/cloudmedia/queue/pc/verify`, `/cloudmedia/downloadurls/pc/v1`).

use crate::session::Session;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Server-reported usage for the account's My Documents storage, in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub usage: u64,
    pub quota: u64,
}

fn decode_usage_response(key: &[u8], encoded: &str) -> Result<String> {
    let mut bytes = zeroize::Zeroizing::new(
        B64.decode(encoded)
            .context("Mã dung lượng My Documents không hợp lệ")?,
    );
    anyhow::ensure!(
        bytes.len() >= 32 && bytes.len() % 16 == 0,
        "Dữ liệu dung lượng My Documents bị thiếu"
    );
    let (iv, ciphertext) = bytes.split_at_mut(16);
    let plaintext = match key.len() {
        16 => cbc::Decryptor::<aes::Aes128>::new_from_slices(key, iv)?
            .decrypt_padded_mut::<Pkcs7>(ciphertext),
        32 => cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv)?
            .decrypt_padded_mut::<Pkcs7>(ciphertext),
        _ => anyhow::bail!("Khóa dung lượng My Documents không hợp lệ"),
    }
    .map_err(|_| anyhow::anyhow!("Không giải mã được dung lượng My Documents"))?;
    Ok(std::str::from_utf8(plaintext)
        .context("Dữ liệu dung lượng My Documents không phải UTF-8")?
        .to_owned())
}

/// Parse only explicit nonnegative byte counts; unknown quota is never displayed as zero.
pub fn parse_my_documents_usage(value: &Value) -> Result<Usage> {
    let bytes = |field: &Value| {
        field
            .as_u64()
            .or_else(|| field.as_str()?.parse::<u64>().ok())
    };
    let quota = value
        .get("quota")
        .and_then(bytes)
        .context("Máy chủ chưa cung cấp hạn mức My Documents hợp lệ")?;
    anyhow::ensure!(
        quota > 0,
        "Máy chủ chưa cung cấp hạn mức My Documents hợp lệ"
    );
    let usage = value
        .pointer("/service_usage/my_cloud")
        .or_else(|| value.get("usage"))
        .and_then(bytes)
        .context("Máy chủ chưa cung cấp dung lượng đã dùng My Documents hợp lệ")?;
    Ok(Usage { usage, quota })
}

/// Read the PC My Documents quota using its dedicated viewer credential and encryption key.
/// Parity: PC 26.11.12 `tP1L.getUsageMyCloud`, evidence in re-notes/45-map19-21-29.md.
pub async fn fetch_my_documents_usage(s: &Session) -> Result<Usage> {
    anyhow::ensure!(
        s.is_active() && s.login.is_some(),
        "Cần đăng nhập để xem My Documents"
    );
    let auth_url = format!(
        "{}/api/getCloudViewerKey?{}",
        s.domains.auth,
        s.common_params()
    );
    let auth_response = s.get(&auth_url).await?;
    let auth_response = s.maybe_decrypt(auth_response, s.session_key_bytes()).await;
    let mut credentials = crate::api_response::checked_data(auth_response)?;
    let take_secret = |value: &mut Value, name: &str| -> Result<zeroize::Zeroizing<String>> {
        match value.as_object_mut().and_then(|fields| fields.remove(name)) {
            Some(Value::String(secret)) if !secret.is_empty() && secret.len() <= 4096 => {
                Ok(zeroize::Zeroizing::new(secret))
            }
            _ => anyhow::bail!("Máy chủ chưa cấp khóa dung lượng My Documents hợp lệ"),
        }
    };
    let viewer_key = take_secret(&mut credentials, "viewer_key")?;
    let enk = take_secret(&mut credentials, "enk")?;
    let key = zeroize::Zeroizing::new(
        B64.decode(enk.as_bytes())
            .context("Khóa dung lượng My Documents không hợp lệ")?,
    );
    anyhow::ensure!(
        matches!(key.len(), 16 | 32),
        "Khóa dung lượng My Documents không hợp lệ"
    );
    let mut usage_url = s.resolve_service_url("zcloud", "/cloudmedia/info/pc/v1/usage-mycloud");
    usage_url.push('?');
    usage_url.push_str(&s.common_params());
    let response = s.get_cloud_with_key(&usage_url, &viewer_key).await?;
    let code = response
        .get("error_code")
        .and_then(crate::api_response::integer)
        .context("Thiếu xác nhận dung lượng My Documents")?;
    anyhow::ensure!(
        code == 0,
        "Máy chủ từ chối xem dung lượng My Documents ({code})"
    );
    let data = response
        .get("data")
        .context("Thiếu dữ liệu dung lượng My Documents")?;
    // The PC decrypts this envelope with the cloud-specific `enk`, never the login key.
    let decoded = if let Some(ciphertext) = data.as_str() {
        anyhow::ensure!(
            ciphertext.len() <= 1024 * 1024,
            "Dữ liệu dung lượng My Documents vượt giới hạn"
        );
        let plaintext = zeroize::Zeroizing::new(
            decode_usage_response(&key, ciphertext)
                .context("Không giải mã được dung lượng My Documents")?,
        );
        serde_json::from_str::<Value>(&plaintext)
            .context("Không đọc được dung lượng My Documents")?
    } else {
        data.clone()
    };
    let data = crate::api_response::data(decoded, 1024 * 1024, false)?;
    anyhow::ensure!(s.is_active(), "Phiên làm việc đã đóng");
    parse_my_documents_usage(&data)
}

/// Status of a media file on zCloud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudFileStatus {
    /// File is available and ready for download/decryption on zCloud.
    Available,
    /// File has expired on the cloud server or was removed.
    Expired,
    /// File was never uploaded to or synchronized with zCloud.
    NotInCloud,
    /// File is currently downloading from zCloud.
    Downloading,
    /// An error occurred during retrieval.
    Failed(String),
}

impl CloudFileStatus {
    pub fn is_accessible(&self) -> bool {
        matches!(self, Self::Available)
    }

    pub fn display_text(&self) -> &'static str {
        match self {
            Self::Available => "Khả dụng trên zCloud",
            Self::Expired => "Tệp đã hết hạn trên đám mây",
            Self::NotInCloud => "Tệp chưa đồng bộ lên zCloud",
            Self::Downloading => "Đang tải từ zCloud…",
            Self::Failed(_) => "Không tải được tệp từ zCloud",
        }
    }
}

/// Authentication key for viewing a cloud media asset.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudViewerKey {
    pub file_id: String,
    pub viewer_key: String,
    pub expires_at: i64,
}

impl std::fmt::Debug for CloudViewerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudViewerKey")
            .field("file_id", &self.file_id)
            .field("viewer_key", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Signed download URL and integrity checksum for a zCloud asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudMediaDownloadUrl {
    pub file_id: String,
    pub url: String,
    pub checksum: Option<String>,
    pub size_bytes: u64,
}

/// Parse `CloudViewerKey` from `/api/getCloudViewerKey` JSON response.
pub fn parse_cloud_viewer_key(val: &Value) -> Result<CloudViewerKey> {
    let data = val.get("data").unwrap_or(val);
    let file_id = data
        .get("file_id")
        .or_else(|| data.get("fileId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let viewer_key = data
        .get("viewer_key")
        .or_else(|| data.get("key"))
        .or_else(|| data.get("viewerKey"))
        .and_then(|v| v.as_str())
        .context("Thiếu viewer_key trong phản hồi zCloud")?
        .to_string();
    let expires_at = data
        .get("expires_at")
        .or_else(|| data.get("expire_time"))
        .and_then(crate::api_response::integer)
        .unwrap_or(0);

    Ok(CloudViewerKey {
        file_id,
        viewer_key,
        expires_at,
    })
}

/// Parse a list of `CloudMediaDownloadUrl` from `/cloudmedia/downloadurls/pc/v1` response.
pub fn parse_cloud_download_urls(val: &Value) -> Vec<CloudMediaDownloadUrl> {
    let items = val
        .get("data")
        .and_then(|d| d.get("urls").or_else(|| d.get("items")))
        .or_else(|| val.get("urls"))
        .and_then(|v| v.as_array());

    let Some(arr) = items else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|it| {
            let file_id = it
                .get("file_id")
                .or_else(|| it.get("fileId"))?
                .as_str()?
                .to_string();
            let url = it
                .get("url")
                .or_else(|| it.get("download_url"))?
                .as_str()?
                .to_string();
            let checksum = it
                .get("checksum")
                .or_else(|| it.get("md5"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let size_bytes = it
                .get("size")
                .or_else(|| it.get("file_size"))
                .and_then(crate::api_response::integer)
                .unwrap_or(0) as u64;

            Some(CloudMediaDownloadUrl {
                file_id,
                url,
                checksum,
                size_bytes,
            })
        })
        .collect()
}

/// Call `/api/getCloudViewerKey` to retrieve an authorized decryption/viewing token.
pub async fn get_cloud_viewer_key(s: &Session, file_id: &str) -> Result<CloudViewerKey> {
    let host = crate::endpoints::service("zcloud").context("Thiếu cấu hình endpoint zcloud")?;
    let url = format!("{host}/api/getCloudViewerKey");
    let payload = json!({
        "file_id": file_id,
        "imei": s.imei,
    });
    let params = s
        .encoded_params(&payload)
        .context("Mã hoá tham số lấy viewer key zCloud thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    parse_cloud_viewer_key(&decrypted)
}

/// Call `/cloudmedia/queue/pc/verify` to verify asset state on zCloud.
pub async fn verify_cloud_media(s: &Session, file_id: &str, checksum: &str) -> Result<bool> {
    let host = crate::endpoints::service("zcloud").context("Thiếu cấu hình endpoint zcloud")?;
    let url = format!("{host}/cloudmedia/queue/pc/verify");
    let payload = json!({
        "file_id": file_id,
        "checksum": checksum,
        "imei": s.imei,
    });
    let params = s
        .encoded_params(&payload)
        .context("Mã hoá tham số kiểm tra tệp zCloud thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let ok = decrypted
        .get("error_code")
        .and_then(crate::api_response::integer)
        .unwrap_or(-1)
        == 0;
    Ok(ok)
}

/// Call `/cloudmedia/downloadurls/pc/v1` to obtain signed download links.
pub async fn get_cloud_download_urls(
    s: &Session,
    file_ids: &[&str],
) -> Result<Vec<CloudMediaDownloadUrl>> {
    let host = crate::endpoints::service("zcloud").context("Thiếu cấu hình endpoint zcloud")?;
    let url = format!("{host}/cloudmedia/downloadurls/pc/v1");
    let payload = json!({
        "file_ids": file_ids,
        "imei": s.imei,
    });
    let params = s
        .encoded_params(&payload)
        .context("Mã hoá tham số lấy download urls zCloud thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(parse_cloud_download_urls(&decrypted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_documents_usage_never_fabricates_missing_or_invalid_quota() {
        assert_eq!(
            parse_my_documents_usage(
                &json!({"quota":100,"usage":80,"service_usage":{"my_cloud":37}})
            )
            .unwrap(),
            Usage {
                usage: 37,
                quota: 100
            }
        );
        assert_eq!(
            parse_my_documents_usage(&json!({"quota":"100","usage":"0"})).unwrap(),
            Usage {
                usage: 0,
                quota: 100
            }
        );
        assert_eq!(
            parse_my_documents_usage(&json!({"quota":100,"usage":130}))
                .unwrap()
                .usage,
            130
        );
        for invalid in [
            json!({"usage":0}),
            json!({"quota":0,"usage":0}),
            json!({"quota":100}),
            json!({"quota":100,"usage":-1}),
            json!({"quota":100,"usage":1.5}),
            json!({"quota":100,"usage":80,"service_usage":{"my_cloud":"bad"}}),
        ] {
            assert!(parse_my_documents_usage(&invalid).is_err());
        }
    }

    #[test]
    fn cloud_usage_decrypts_iv_prefixed_cbc_with_cloud_key() {
        // Independent Windows .NET AES-CBC/PKCS7 vector: key 00..1f, IV 10..1f.
        let encoded = "EBESExQVFhcYGRobHB0eH+j923ZM45Jlv4kMkXXg49E5eckD6b2sc9FQE8FVW4ke";
        let key: Vec<u8> = (0..32).collect();
        assert_eq!(
            decode_usage_response(&key, encoded).unwrap(),
            r#"{"usage":37,"quota":100}"#
        );
        assert!(decode_usage_response(&[0; 32], encoded).is_err());
        assert!(decode_usage_response(&key, "AQID").is_err());
    }

    #[test]
    fn test_parse_cloud_viewer_key() {
        let val = json!({
            "error_code": 0,
            "data": {
                "file_id": "cld_file_9988",
                "viewer_key": "sec_k_abc123xyz789",
                "expires_at": 1794999999
            }
        });

        let key = parse_cloud_viewer_key(&val).expect("Failed to parse viewer key");
        assert_eq!(key.file_id, "cld_file_9988");
        assert_eq!(key.viewer_key, "sec_k_abc123xyz789");
        assert_eq!(key.expires_at, 1794999999);
    }

    #[test]
    fn test_cloud_file_status() {
        let available = CloudFileStatus::Available;
        assert!(available.is_accessible());
        assert_eq!(available.display_text(), "Khả dụng trên zCloud");

        let expired = CloudFileStatus::Expired;
        assert!(!expired.is_accessible());
        assert_eq!(expired.display_text(), "Tệp đã hết hạn trên đám mây");

        let not_in_cloud = CloudFileStatus::NotInCloud;
        assert!(!not_in_cloud.is_accessible());
        assert_eq!(not_in_cloud.display_text(), "Tệp chưa đồng bộ lên zCloud");
    }

    #[test]
    fn test_parse_cloud_download_urls() {
        let val = json!({
            "error_code": 0,
            "data": {
                "urls": [
                    {
                        "file_id": "f1",
                        "url": "https://zcld.chat.zalo.me/d/f1?sign=xyz",
                        "checksum": "d41d8cd98f00b204e9800998ecf8427e",
                        "size": 102400
                    }
                ]
            }
        });

        let urls = parse_cloud_download_urls(&val);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].file_id, "f1");
        assert_eq!(urls[0].url, "https://zcld.chat.zalo.me/d/f1?sign=xyz");
        assert_eq!(urls[0].size_bytes, 102400);
        assert_eq!(
            urls[0].checksum.as_deref(),
            Some("d41d8cd98f00b204e9800998ecf8427e")
        );
    }
}
