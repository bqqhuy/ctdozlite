//! FILE renewlink adapter observed in Zalo PC 26.11.10, compact-app-pc static renewLink.
use crate::{
    chat::ChatMessage,
    message_view::{Media, MediaKind},
    session::Session,
    transfer_diagnostics::{self as diag, RenewPhase},
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

fn payload(s: &Session, thread: &str, group: bool, message: &ChatMessage) -> Result<Value> {
    let media = crate::message_view::media(message);
    let msg_type = match media.kind {
        MediaKind::Photo => 2,
        MediaKind::Video => 5,
        MediaKind::File => 1,
        _ => anyhow::bail!("Loại media chưa hỗ trợ phục hồi"),
    };
    // Forward/renew codes differ from archive and live message type codes.
    let client = message
        .wire_ref
        .client_id
        .as_ref()
        .and_then(|v| {
            v.as_str()
                .map(str::to_owned)
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        })
        .or_else(|| {
            (!message.client_message_id.is_empty()).then(|| message.client_message_id.clone())
        })
        .context("Thiếu client ID để phục hồi đường dẫn")?;
    ensure!(
        !client.is_empty() && client.len() <= 128,
        "Client ID không hợp lệ"
    );
    let clean = thread
        .strip_prefix("group_")
        .or_else(|| thread.strip_prefix('g'))
        .unwrap_or(thread);
    ensure!(
        !clean.is_empty() && clean.bytes().all(|b| b.is_ascii_digit()),
        "Định danh hội thoại không hợp lệ"
    );
    let mut info = json!({"thumbUrl":media.thumbnail.as_deref().unwrap_or_default(),"normalUrl":media.preview.as_deref().unwrap_or_default(),"hdUrl":media.original.as_deref().unwrap_or_default(),"oriUrl":media.original.as_deref().unwrap_or_default(),"url":media.original.as_deref().unwrap_or_default(),"videoUrl":media.original.as_deref().unwrap_or_default(),"isE2EE":0,"isOriginal":0});
    if let Some(w) = media.width {
        info["width"] = json!(w);
    }
    if let Some(h) = media.height {
        info["height"] = json!(h);
    }
    // Unknown encrypted objects require a separate verified adapter, never an invented key.
    let encrypted = message
        .wire_ref
        .fields
        .get("isE2EE")
        .or_else(|| message.wire_ref.fields.get("isE2ee"))
        .is_some_and(|v| v == true || v == 1);
    ensure!(!encrypted, "Tệp mã hóa cần cơ chế phục hồi riêng");
    for key in ["hdUrl", "thumbUrl", "normalUrl", "oriUrl", "url"] {
        if let Some(raw) = info
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            info[key] = json!(crate::media_recovery::compatible_image_url(raw, false)?);
        }
    }
    let self_cloud = s.login.as_ref().is_some_and(|l| l.send2me_id == thread);
    Ok(
        json!({"toId":clean,"msgType":msg_type,"msgInfo":info.to_string(),"clientId":client,"imei":s.imei,"isGroup":if group||self_cloud{1}else{0}}),
    )
}
/// Renew an inaccessible media reference once; the caller owns single-flight and retry policy.
/// # Errors
/// Returns sanitized protocol, account, unsupported encryption or response errors.
async fn renew_once(
    s: &Session,
    thread: &str,
    group: bool,
    message: &ChatMessage,
) -> Result<Media> {
    let tag = reference_tag(s, thread, message);
    let params = (|| -> Result<String> {
        ensure!(s.is_active(), "Đã đăng xuất");
        s.encoded_params(&payload(s, thread, group, message)?)
    })();
    let params = params.inspect_err(|_| {
        diag::renewal(tag, RenewPhase::PreparationFailed, None);
    })?;
    let endpoint = format!(
        "{}/api/message/renewlink?{}",
        s.domains.file,
        s.common_params()
    );
    diag::renewal(tag, RenewPhase::RequestSent, None);
    let response = s
        .post_form(&endpoint, format!("params={params}"))
        .await
        .map_err(|error| {
            if let Some(status) = crate::session::http_failure_status(&error) {
                diag::renewal_http(tag, status);
            } else {
                diag::renewal(tag, RenewPhase::NetworkFailed, None);
            }
            anyhow::anyhow!("Không thể yêu cầu phục hồi đường dẫn media")
        })?;
    let response = s.maybe_decrypt(response, s.session_key_bytes()).await;
    let code = reply_code(&response);
    diag::renewal(tag, RenewPhase::ServerReply, code);
    let result = parse(&response, crate::message_view::media(message));
    diag::renewal(
        tag,
        if result.is_ok() {
            RenewPhase::Finished
        } else if code.is_some_and(|code| code != 0) {
            RenewPhase::ServerRejected
        } else {
            RenewPhase::ParseFailed
        },
        code,
    );
    result
}
fn reply_code(response: &Value) -> Option<i64> {
    let number = |v: &Value| v.as_i64().or_else(|| v.as_str()?.parse().ok());
    let outer = response.get("error_code").and_then(number)?;
    if outer != 0 {
        return Some(outer);
    }
    response
        .pointer("/data/error_code")
        .and_then(number)
        .or(Some(outer))
}
fn reference_tag(s: &Session, thread: &str, message: &ChatMessage) -> u64 {
    // Length-delimited identities avoid cross-account correlation and concatenation collisions.
    diag::tag(&format!(
        "{}:{}{}:{}{}:{}",
        s.account_key().len(),
        s.account_key(),
        thread.len(),
        thread,
        message.id.len(),
        message.id
    ))
}
fn parse(response: &Value, mut media: Media) -> Result<Media> {
    ensure!(
        response
            .get("error_code")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
            == Some(0),
        "Máy chủ chưa cho phép phục hồi media"
    );
    let data = response
        .get("data")
        .context("Thiếu kết quả phục hồi media")?;
    let data = if data.get("error_code").is_some() {
        ensure!(
            data.get("error_code").and_then(Value::as_i64) == Some(0),
            "Không thể phục hồi media"
        );
        data.get("data").context("Thiếu đường dẫn phục hồi")?
    } else {
        data
    };
    let pick = |v: &Value, keys: &[&str]| -> Option<String> {
        keys.iter().find_map(|key| {
            v.get(*key)
                .and_then(Value::as_str)
                .filter(|s| {
                    reqwest::Url::parse(s)
                        .ok()
                        .is_some_and(|u| crate::media_download::allowed(&u))
                })
                .map(str::to_owned)
        })
    };
    if media.kind == MediaKind::Video {
        media.thumbnail = pick(
            data.get("thumb").unwrap_or(data),
            &["thumbUrl", "hdUrl", "oriUrl"],
        );
        media.original = pick(
            data.get("video").unwrap_or(data),
            &["url", "videoUrl", "fileUrl"],
        );
        media.preview = media.original.clone();
    } else {
        media.thumbnail = pick(data, &["thumbUrl", "thumb"]);
        media.original = pick(data, &["oriUrl", "hdUrl", "fileUrl", "url"]);
        media.preview = pick(data, &["normalUrl", "hdUrl", "oriUrl", "fileUrl", "url"]);
    }
    if media.kind == MediaKind::Photo && media.preview.is_none() {
        media.preview = media.thumbnail.clone();
    }
    ensure!(
        media.thumbnail.is_some() || media.original.is_some(),
        "Không nhận được đường dẫn media hợp lệ"
    );
    Ok(media)
}
type Renewal = std::sync::Arc<
    tokio::sync::Mutex<Option<(std::time::Instant, std::result::Result<Media, String>)>>,
>;
/// Coalesce simultaneous thumbnail/viewer renewals and briefly cache terminal results.
/// # Errors
/// Returns sanitized renewal errors without retrying the same inaccessible reference indefinitely.
pub async fn renew(s: &Session, thread: &str, group: bool, message: &ChatMessage) -> Result<Media> {
    static FLIGHTS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Renewal>>,
    > = std::sync::OnceLock::new();
    let tag = reference_tag(s, thread, message);
    diag::renewal(tag, RenewPhase::Entered, None);
    let identity = serde_json::to_string(&(s.account_key(), thread, &message.id))?;
    let cell = (|| -> Result<Renewal> {
        let mut map = FLIGHTS
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| anyhow::anyhow!("Hàng đợi phục hồi media không sẵn sàng"))?;
        if map.len() >= 128 {
            map.retain(|_, v| std::sync::Arc::strong_count(v) > 1);
        }
        ensure!(
            map.len() < 128 || map.contains_key(&identity),
            "Đang có nhiều yêu cầu phục hồi media"
        );
        Ok(map
            .entry(identity)
            .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(None)))
            .clone())
    })()
    .inspect_err(|_| {
        diag::renewal(tag, RenewPhase::PreparationFailed, None);
    })?;
    let mut state = cell.lock().await;
    if let Some((at, result)) = &*state {
        if at.elapsed() < std::time::Duration::from_secs(60) {
            diag::renewal(
                tag,
                if result.is_ok() {
                    RenewPhase::CacheHitSuccess
                } else {
                    RenewPhase::CacheHitFailure
                },
                None,
            );
            return result.clone().map_err(anyhow::Error::msg);
        }
    }
    let result = renew_once(s, thread, group, message)
        .await
        .map_err(|e| e.to_string());
    *state = Some((std::time::Instant::now(), result.clone()));
    result.map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renewed_photo_and_video_urls_are_validated() {
        let photo = Media {
            kind: MediaKind::Photo,
            ..Default::default()
        };
        let got=parse(&json!({"error_code":0,"data":{"thumbUrl":"https://photo.zdn.vn/a","hdUrl":"https://photo.zdn.vn/b"}}),photo.clone()).unwrap();
        assert!(got.thumbnail.is_some());
        assert!(parse(
            &json!({"error_code":0,"data":{"url":"https://evil.invalid/a"}}),
            photo
        )
        .is_err());
        let video = Media {
            kind: MediaKind::Video,
            ..Default::default()
        };
        assert!(parse(&json!({"error_code":0,"data":{"thumb":{"oriUrl":"https://photo.zdn.vn/a"},"video":{"url":"https://video.zdn.vn/a"}}}),video).unwrap().original.is_some());
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;
    #[test]
    fn optional_urls_are_strings_and_original_file_name_survives_renewal() {
        let s = Session::new(false, false, false).unwrap();
        let message = ChatMessage {
            kind: crate::message_model::MessageKind::Photo,
            client_message_id: "11".into(),
            link_url: Some("https://photo.zdn.vn/old.jpg".into()),
            ..Default::default()
        };
        let body = payload(&s, "123", false, &message).unwrap();
        let info: Value = serde_json::from_str(body["msgInfo"].as_str().unwrap()).unwrap();
        assert_eq!(info["thumbUrl"], "");
        assert_eq!(body["msgType"], 2);
        let old = Media {
            kind: MediaKind::File,
            name: "Báo cáo.doc".into(),
            ..Default::default()
        };
        let got=parse(&json!({"error_code":0,"data":{"fileUrl":"https://file-stal-3.dlfl.vn/file/renewMedia.doc"}}),old).unwrap();
        assert_eq!(got.name, "Báo cáo.doc");
    }
}

#[cfg(test)]
mod diagnostic_code_tests {
    use super::*;
    #[test]
    fn outer_and_nested_api_codes_remain_numeric_without_recording_response_text() {
        assert_eq!(
            reply_code(&json!({"error_code":"120","error_message":"private server text"})),
            Some(120)
        );
        assert_eq!(
            reply_code(&json!({"error_code":0,"data":{"error_code":"15"}})),
            Some(15)
        );
        assert_eq!(reply_code(&json!({"error_code":0,"data":{}})), Some(0));
        assert_eq!(
            reply_code(&json!({"error_message":"not an envelope"})),
            None
        );
    }
}
