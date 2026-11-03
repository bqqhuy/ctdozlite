//! Typed media failures and bounded retries; transport errors never expose signed URLs.
use serde_json::Value;
use std::time::{Duration, Instant};

/// Failure categories used by media UI and recovery decisions.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Failure {
    Network,
    Timeout,
    Http(u16),
    Json(i64),
    Content,
    Decode,
    Missing,
    Local,
    Recovery,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}
impl std::error::Error for Failure {}
impl Failure {
    /// Display text does not claim the remote object was deleted.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Network => "Không kết nối được nguồn media",
            Self::Timeout => "Tải media hết thời gian chờ",
            Self::Http(_) => "Máy chủ chưa cho phép tải tệp",
            Self::Json(_) => "Máy chủ trả lỗi thay vì dữ liệu media",
            Self::Content => "Nguồn trả về dữ liệu không phải media",
            Self::Decode => "Không đọc được định dạng ảnh",
            Self::Recovery => "Chưa thể phục hồi đường dẫn media; tệp chưa được xác định là đã mất",
            Self::Missing => "Thiếu thông tin tải ảnh/video",
            Self::Local => "Không đọc được dữ liệu media cục bộ",
        }
    }
    /// Only temporary transport failures are automatically retried.
    pub fn transient(&self) -> bool {
        matches!(
            self,
            Self::Network | Self::Timeout | Self::Http(429 | 500..=599)
        )
    }
    /// A single renewal may recover inaccessible URLs; this is not proof of expiration.
    pub fn renew_candidate(&self) -> bool {
        matches!(self, Self::Http(403 | 404 | 410) | Self::Json(_))
    }
}
/// Reject bounded JSON/HTML responses before handing bytes to the image decoder.
/// # Errors
/// Returns a typed remote error or unexpected content classification.
pub fn validate_image_body(bytes: &[u8]) -> Result<(), Failure> {
    let head = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map(|i| &bytes[i..])
        .unwrap_or_default();
    if head.starts_with(b"{") || head.starts_with(b"[") {
        if bytes.len() <= 65536 {
            if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
                for key in ["error_code", "err_code", "errorCode"] {
                    if let Some(code) = value
                        .get(key)
                        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
                        .filter(|c| *c != 0)
                    {
                        return Err(Failure::Json(code));
                    }
                }
            }
        }
        return Err(Failure::Content);
    }
    if head.starts_with(b"<") {
        return Err(Failure::Content);
    }
    Ok(())
}
/// A retry ticket survives rendering; terminal failures remain visible until a user retries.
#[derive(Clone)]
pub struct Retry {
    pub failure: Failure,
    pub attempts: u8,
    pub at: Option<Instant>,
}
impl Retry {
    /// Three automatic retries maximum, with increasing delay.
    pub fn next(failure: Failure, previous: Option<&Self>) -> Self {
        let attempts = previous.map_or(1, |r| r.attempts.saturating_add(1));
        let at = (failure.transient() && attempts <= 3)
            .then(|| Instant::now() + Duration::from_secs(1 << attempts));
        Self {
            failure,
            attempts,
            at,
        }
    }
}
/// Stable identity excludes expiring URLs and includes the attachment and quality.
pub fn image_key(thread: &str, message: &str, attachment: usize, preview: bool) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for v in [thread, message] {
        h.update((v.len() as u64).to_le_bytes());
        h.update(v.as_bytes());
    }
    h.update((attachment as u64).to_le_bytes());
    h.update([u8::from(preview)]);
    format!(
        "{}_{:x}",
        if preview { "full" } else { "thumb" },
        h.finalize()
    )
}
/// Apply the official nonencrypted JXL-to-JPEG URL projection, preserving other query bytes.
/// # Errors
/// Rejects malformed URLs and device-encrypted media requiring a separate adapter.
pub fn compatible_image_url(raw: &str, jpeg: bool) -> Result<String, Failure> {
    let mut url = reqwest::Url::parse(raw).map_err(|_| Failure::Missing)?;
    if url
        .query()
        .is_some_and(|q| q.split('&').any(|p| p.starts_with("e2esession")))
    {
        return Err(Failure::Content);
    }
    if url.path().contains("/jxl/") && url.path().contains(".jxl") {
        let path = url.path().to_owned();
        let query = url.query().map(|q| {
            q.split('&')
                .filter(|p| p.split('=').next() != Some("jxlstatus"))
                .collect::<Vec<_>>()
                .join("&")
        });
        url.set_query(query.as_deref().filter(|q| !q.is_empty()));
        if jpeg {
            url.set_path(
                &path
                    .replacen("/jxl/", "/jpg/", 1)
                    .replacen(".jxl", ".jpg", 1),
            );
        }
    }
    Ok(url.to_string())
}
#[cfg(test)]
mod jxl_tests {
    use super::*;
    #[test]
    fn compatible_url_preserves_signed_query_and_does_not_rewrite_encrypted_media() {
        let input = "https://photo.zdn.vn/jxl/abc.jxl?token=a%2Fb%3D&jxlstatus=1&x=%252F";
        assert_eq!(
            compatible_image_url(input, true).unwrap(),
            "https://photo.zdn.vn/jpg/abc.jpg?token=a%2Fb%3D&x=%252F"
        );
        assert_eq!(
            compatible_image_url(input, false).unwrap(),
            "https://photo.zdn.vn/jxl/abc.jxl?token=a%2Fb%3D&x=%252F"
        );
        assert!(
            compatible_image_url("https://photo.zdn.vn/jxl/a.jxl?e2esession=key", true).is_err()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_error_is_structural_and_never_a_fixed_signature_string() {
        assert_eq!(
            validate_image_body(br#"{"err_code":"17","message":"anything"}"#),
            Err(Failure::Json(17))
        );
        assert_eq!(
            validate_image_body(br#"{"message":"Invalid signature"}"#),
            Err(Failure::Content)
        );
    }
    #[test]
    fn retry_is_finite_and_terminal_errors_never_retry() {
        let mut r = Retry::next(Failure::Timeout, None);
        for _ in 0..3 {
            r = Retry::next(Failure::Timeout, Some(&r));
        }
        assert!(r.at.is_none());
        assert!(Retry::next(Failure::Decode, None).at.is_none());
    }
    #[test]
    fn stable_key_separates_thread_attachment_and_quality() {
        let a = image_key("a", "m", 0, false);
        assert_ne!(a, image_key("b", "m", 0, false));
        assert_ne!(a, image_key("a", "m", 1, false));
        assert_ne!(a, image_key("a", "m", 0, true));
    }
}
