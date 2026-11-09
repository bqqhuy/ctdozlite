//! zCloud Media Storage & Cloud Recovery (Zalo Parity A20).
//!
//! Provides data structures and API helpers to retrieve viewer authentication keys,
//! verify media presence on zCloud, and obtain signed download URLs
//! (`/api/getCloudViewerKey`, `/cloudmedia/queue/pc/verify`, `/cloudmedia/downloadurls/pc/v1`).

use crate::session::Session;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudViewerKey {
    pub file_id: String,
    pub viewer_key: String,
    pub expires_at: i64,
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
