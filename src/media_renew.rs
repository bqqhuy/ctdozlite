//! FILE renewlink adapter observed in Zalo PC 26.11.10, compact-app-pc static renewLink.
use crate::{
    chat::ChatMessage,
    message_view::{Media, MediaKind},
    session::Session,
    transfer_diagnostics::{self as diag, RenewFailure, RenewPhase},
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
/// Shared budget for one renewal including `async` re-polls and network time.
const RENEW_DEADLINE: std::time::Duration = std::time::Duration::from_secs(15);
/// Pauses between re-polls while the server reports `async` (Zalo PC `_awaitCtrlEvent`).
const PENDING_DELAYS: [std::time::Duration; 3] = [
    std::time::Duration::from_secs(2),
    std::time::Duration::from_secs(4),
    std::time::Duration::from_secs(8),
];
/// Renew an inaccessible media reference; the caller owns single-flight and caching.
///
/// Zalo PC re-sends `renewlink` while the decoded reply says `async`; this repeats that with a
/// bounded backoff inside one shared deadline that also covers network time.
/// # Errors
/// Returns sanitized protocol, account, unsupported encryption or response errors.
async fn renew_once(
    s: &Session,
    thread: &str,
    group: bool,
    message: &ChatMessage,
) -> Result<Media> {
    let tag = reference_tag(s, thread, message);
    let body = payload(s, thread, group, message).inspect_err(|_| {
        diag::renewal(tag, RenewPhase::PreparationFailed, None);
    })?;
    renew_body(s, tag, &body, crate::message_view::media(message)).await
}
/// Run `renewlink` for a prepared body, re-polling `async` replies within the shared budget.
async fn renew_body(s: &Session, tag: u64, body: &Value, base: Media) -> Result<Media> {
    let outcome = poll_pending(RENEW_DEADLINE, &PENDING_DELAYS, || async {
        let reply = renew_request(s, tag, body, base.clone()).await;
        if matches!(reply, Ok(Reply::Pending)) {
            diag::renewal(tag, RenewPhase::Pending, Some(0));
        }
        reply
    })
    .await;
    match outcome {
        Some(result) => result,
        None => {
            diag::renewal(tag, RenewPhase::PendingTimeout, None);
            anyhow::bail!("Máy chủ chưa chuẩn bị xong media, hãy thử lại sau")
        }
    }
}
/// `msgType` codes of `renewlink` / `forward` (Zalo PC `MSGTYPE_FORWARD_FILE/PHOTO/VIDEO`).
pub(crate) fn forward_type(kind: MediaKind) -> Option<u8> {
    match kind {
        MediaKind::File => Some(1),
        MediaKind::Photo => Some(2),
        MediaKind::Video => Some(5),
        MediaKind::None => None,
    }
}
/// Ask the server for a copy of media usable in `target` (Zalo PC `ForwardAsync.forward`:
/// `renewLink(targetConversation, msgType, msgInfo, newClientId)`). `info` is the forward
/// `msgInfo` object; the reply carries the target's URLs in the usual renewal shapes.
/// # Errors
/// Returns sanitized server, parse and timeout failures.
pub(crate) async fn clone_for(
    s: &Session,
    target: &str,
    target_group: bool,
    kind: MediaKind,
    info: &Value,
    client_id: u64,
    original: bool,
) -> Result<Media> {
    let msg_type = forward_type(kind).context("Loại media chưa hỗ trợ sao chép")?;
    // `renewLink` merges these two flags into msgInfo before encoding it.
    let mut info = info.clone();
    info["isE2EE"] = json!(0);
    info["isOriginal"] = json!(if original { 1 } else { 0 });
    let clean = target
        .strip_prefix("group_")
        .or_else(|| target_group.then(|| target.strip_prefix('g')).flatten())
        .unwrap_or(target);
    ensure!(
        !clean.is_empty() && clean.bytes().all(|b| b.is_ascii_digit()),
        "Định danh hội thoại không hợp lệ"
    );
    let self_cloud = s.login.as_ref().is_some_and(|l| l.send2me_id == target);
    let body = json!({"toId":clean,"msgType":msg_type,"msgInfo":info.to_string(),
        "clientId":client_id.to_string(),"imei":s.imei,"isGroup":if target_group||self_cloud{1}else{0}});
    let tag = diag::tag(&format!("clone:{}:{target}:{client_id}", s.account_key()));
    renew_body(
        s,
        tag,
        &body,
        Media {
            kind,
            ..Default::default()
        },
    )
    .await
}
/// Repeat `attempt` while it reports `Pending`, sleeping through `delays`, until one shared
/// `budget` (covering attempts and pauses) runs out. `None` means the budget ran out.
async fn poll_pending<F, Fut>(
    budget: std::time::Duration,
    delays: &[std::time::Duration],
    mut attempt: F,
) -> Option<Result<Media>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Reply>>,
{
    let deadline = tokio::time::Instant::now() + budget;
    let mut delays = delays.iter();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, attempt()).await {
            Ok(Ok(Reply::Ready(media))) => return Some(Ok(media)),
            Ok(Err(error)) => return Some(Err(error)),
            Ok(Ok(Reply::Pending)) => {
                let left = deadline.saturating_duration_since(tokio::time::Instant::now());
                match delays.next() {
                    Some(delay) if *delay < left => tokio::time::sleep(*delay).await,
                    _ => return None,
                }
            }
            Err(_) => return None,
        }
    }
}
/// One `renewlink` round trip.
async fn renew_request(s: &Session, tag: u64, body: &Value, base: Media) -> Result<Reply> {
    let params = (|| -> Result<String> {
        ensure!(s.is_active(), "Đã đăng xuất");
        s.encoded_params(body)
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
    // A session switch while the request was in flight must not leak into the new account.
    ensure!(s.is_active(), "Đã đăng xuất");
    match parse(&response, base) {
        Ok(reply) => {
            if matches!(reply, Reply::Ready(_)) {
                diag::renewal(tag, RenewPhase::Finished, code);
            }
            Ok(reply)
        }
        Err(reason) => {
            diag::renewal_failed(tag, code, reason);
            Err(anyhow::anyhow!(match reason {
                RenewFailure::ServerRejected => "Máy chủ chưa cho phép phục hồi media",
                RenewFailure::UrlBlocked => "Không nhận được đường dẫn media hợp lệ",
                _ => "Không đọc được kết quả phục hồi media",
            }))
        }
    }
}
fn number(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str()?.trim().parse().ok())
}
fn reply_code(response: &Value) -> Option<i64> {
    let outer = response.get("error_code").and_then(number)?;
    if outer != 0 {
        return Some(outer);
    }
    unwrap_layer(response.get("data"))
        .ok()
        .and_then(|data| data.get("error_code").and_then(number))
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
/// Usable result of one renewal reply.
#[derive(Debug)]
enum Reply {
    Ready(Media),
    /// Server reported `async`: the copy is still being prepared.
    Pending,
}
/// A `data` layer may arrive as an object or as a JSON string (after session decryption).
fn unwrap_layer(data: Option<&Value>) -> std::result::Result<Value, RenewFailure> {
    match data {
        None | Some(Value::Null) => Err(RenewFailure::MissingData),
        Some(Value::String(raw)) => {
            let raw = raw.trim();
            if raw.is_empty() {
                return Err(RenewFailure::MissingData);
            }
            match serde_json::from_str::<Value>(raw) {
                Ok(value @ Value::Object(_)) => Ok(value),
                Ok(_) => Err(RenewFailure::WrongType),
                Err(_) => Err(RenewFailure::DecodeFailed),
            }
        }
        Some(value @ Value::Object(_)) => Ok(value.clone()),
        Some(_) => Err(RenewFailure::WrongType),
    }
}
/// Decode the reply layers observed in Zalo PC `__callClone` / `_callClone`:
/// HTTP envelope `{error_code, data}` → decoded `{error_code, data:{...}}` → result fields
/// (`async`, `normal`/`hd`/`thumb` for photos and videos, `fileUrl` for files and videos).
/// Older `*Url` field names stay accepted.
fn parse(response: &Value, mut media: Media) -> std::result::Result<Reply, RenewFailure> {
    if response.get("error_code").and_then(number) != Some(0) {
        return Err(RenewFailure::ServerRejected);
    }
    let mut data = unwrap_layer(response.get("data"))?;
    if let Some(code) = data.get("error_code") {
        if number(code) != Some(0) {
            return Err(RenewFailure::ServerRejected);
        }
        data = unwrap_layer(data.get("data"))?;
    }
    if data
        .get("async")
        .is_some_and(|v| v == true || number(v) == Some(1))
    {
        return Ok(Reply::Pending);
    }
    // Remember whether URL fields existed at all so blocked hosts are not reported as missing.
    let mut seen_url = false;
    let mut pick = |v: Option<&Value>, keys: &[&str]| -> Option<String> {
        let v = v?;
        keys.iter().find_map(|key| {
            let raw = match v.get(*key)? {
                Value::String(raw) if !raw.is_empty() => raw,
                _ => return None,
            };
            seen_url = true;
            reqwest::Url::parse(raw)
                .ok()
                .is_some_and(|u| crate::media_download::allowed(&u))
                .then(|| raw.clone())
        })
    };
    match media.kind {
        MediaKind::Video => {
            let thumb = data.get("thumb");
            media.thumbnail = pick(Some(&data), &["thumb", "thumbUrl"])
                .or_else(|| pick(thumb, &["thumbUrl", "hdUrl", "oriUrl", "url"]));
            media.original = pick(data.get("video"), &["url", "videoUrl", "fileUrl"])
                .or_else(|| pick(Some(&data), &["fileUrl", "normal", "url", "videoUrl"]));
            media.preview = media.original.clone();
        }
        MediaKind::File => {
            media.original = pick(Some(&data), &["fileUrl", "url", "normal"]);
        }
        _ => {
            media.thumbnail = pick(Some(&data), &["thumb", "thumbUrl"]);
            media.original = pick(
                Some(&data),
                &["normal", "oriUrl", "hd", "hdUrl", "fileUrl", "url"],
            );
            media.preview = pick(
                Some(&data),
                &[
                    "hd",
                    "hdUrl",
                    "normalUrl",
                    "normal",
                    "oriUrl",
                    "fileUrl",
                    "url",
                ],
            );
        }
    }
    if media.kind == MediaKind::Photo && media.preview.is_none() {
        media.preview = media.thumbnail.clone();
    }
    if media.thumbnail.is_none() && media.original.is_none() {
        return Err(if seen_url {
            RenewFailure::UrlBlocked
        } else {
            RenewFailure::MissingData
        });
    }
    Ok(Reply::Ready(media))
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
    fn ready(reply: std::result::Result<Reply, RenewFailure>) -> Media {
        match reply {
            Ok(Reply::Ready(media)) => media,
            other => panic!("expected ready media, got {other:?}"),
        }
    }
    fn kind(kind: MediaKind) -> Media {
        Media {
            kind,
            ..Default::default()
        }
    }
    #[test]
    fn renewed_photo_and_video_urls_are_validated() {
        let got = ready(parse(
            &json!({"error_code":0,"data":{"thumbUrl":"https://photo.zdn.vn/a","hdUrl":"https://photo.zdn.vn/b"}}),
            kind(MediaKind::Photo),
        ));
        assert!(got.thumbnail.is_some());
        assert_eq!(
            parse(
                &json!({"error_code":0,"data":{"url":"https://evil.invalid/a"}}),
                kind(MediaKind::Photo)
            )
            .unwrap_err(),
            RenewFailure::UrlBlocked
        );
        let got = ready(parse(
            &json!({"error_code":0,"data":{"thumb":{"oriUrl":"https://photo.zdn.vn/a"},"video":{"url":"https://video.zdn.vn/a"}}}),
            kind(MediaKind::Video),
        ));
        assert!(got.original.is_some() && got.thumbnail.is_some());
    }
    #[test]
    fn zalo_pc_decoded_layer_with_normal_hd_thumb_is_read() {
        let inner = json!({"error_code":0,"data":{"normal":"https://photo.zdn.vn/n","hd":"https://photo.zdn.vn/h","thumb":"https://photo.zdn.vn/t"}});
        let got = ready(parse(
            &json!({"error_code":0,"data":inner.to_string()}),
            kind(MediaKind::Photo),
        ));
        assert_eq!(got.original.as_deref(), Some("https://photo.zdn.vn/n"));
        assert_eq!(got.preview.as_deref(), Some("https://photo.zdn.vn/h"));
        assert_eq!(got.thumbnail.as_deref(), Some("https://photo.zdn.vn/t"));
    }
    #[test]
    fn video_thumbnail_as_plain_string_is_kept() {
        let got = ready(parse(
            &json!({"error_code":0,"data":{"error_code":"0","data":{"normal":"https://video.zdn.vn/v.mp4","thumb":"https://photo.zdn.vn/t"}}}),
            kind(MediaKind::Video),
        ));
        assert_eq!(got.thumbnail.as_deref(), Some("https://photo.zdn.vn/t"));
        assert_eq!(got.original.as_deref(), Some("https://video.zdn.vn/v.mp4"));
        let got = ready(parse(
            &json!({"error_code":0,"data":{"fileUrl":"https://video.zdn.vn/f.mp4"}}),
            kind(MediaKind::Video),
        ));
        assert_eq!(got.original.as_deref(), Some("https://video.zdn.vn/f.mp4"));
    }
    #[test]
    fn async_flag_is_pending_but_missing_urls_are_not() {
        assert!(matches!(
            parse(
                &json!({"error_code":0,"data":{"error_code":0,"data":{"async":true}}}),
                kind(MediaKind::Photo)
            ),
            Ok(Reply::Pending)
        ));
        assert!(matches!(
            parse(
                &json!({"error_code":0,"data":{"async":1}}),
                kind(MediaKind::File)
            ),
            Ok(Reply::Pending)
        ));
        assert_eq!(
            parse(
                &json!({"error_code":0,"data":{"error_code":0,"data":{}}}),
                kind(MediaKind::Photo)
            )
            .unwrap_err(),
            RenewFailure::MissingData
        );
    }
    #[test]
    fn failure_reasons_distinguish_layers() {
        let photo = || kind(MediaKind::Photo);
        assert_eq!(
            parse(&json!({"error_code":"-1"}), photo()).unwrap_err(),
            RenewFailure::ServerRejected
        );
        assert_eq!(
            parse(
                &json!({"error_code":0,"data":{"error_code":"15","data":{}}}),
                photo()
            )
            .unwrap_err(),
            RenewFailure::ServerRejected
        );
        assert_eq!(
            parse(&json!({"error_code":0,"data":"not-json"}), photo()).unwrap_err(),
            RenewFailure::DecodeFailed
        );
        assert_eq!(
            parse(&json!({"error_code":0,"data":[1]}), photo()).unwrap_err(),
            RenewFailure::WrongType
        );
        assert_eq!(
            parse(&json!({"error_code":0}), photo()).unwrap_err(),
            RenewFailure::MissingData
        );
    }
}

#[cfg(test)]
mod pending_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    // Real time scaled down 20x from the production 2/4/8 s inside 15 s.
    const DELAYS: [Duration; 3] = [
        Duration::from_millis(100),
        Duration::from_millis(200),
        Duration::from_millis(400),
    ];
    const BUDGET: Duration = Duration::from_millis(750);
    #[tokio::test]
    async fn pending_then_ready_returns_media() {
        let calls = AtomicUsize::new(0);
        let got = poll_pending(BUDGET, &DELAYS, || async {
            if calls.fetch_add(1, Ordering::SeqCst) < 2 {
                Ok(Reply::Pending)
            } else {
                Ok(Reply::Ready(Media::default()))
            }
        })
        .await;
        assert!(matches!(got, Some(Ok(_))));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
    #[tokio::test]
    async fn shared_budget_counts_slow_network_and_stops() {
        let calls = AtomicUsize::new(0);
        let start = tokio::time::Instant::now();
        let got = poll_pending(BUDGET, &DELAYS, || async {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(150)).await;
            Ok(Reply::Pending)
        })
        .await;
        assert!(got.is_none());
        assert!(start.elapsed() <= BUDGET + Duration::from_millis(100));
        // 150 + 100 + 150 + 200 + 150 = 750 ms budget: the 400 ms pause never starts.
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
    #[tokio::test]
    async fn errors_are_not_retried() {
        let calls = AtomicUsize::new(0);
        let got = poll_pending(BUDGET, &DELAYS, || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(anyhow::anyhow!("rejected"))
        })
        .await;
        assert!(matches!(got, Some(Err(_))));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
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
        let Ok(Reply::Ready(got)) = parse(
            &json!({"error_code":0,"data":{"fileUrl":"https://file-stal-3.dlfl.vn/file/renewMedia.doc"}}),
            old,
        ) else {
            panic!("file renewal must succeed");
        };
        assert_eq!(got.name, "Báo cáo.doc");
        assert!(got.original.is_some());
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
