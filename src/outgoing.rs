//! Durable outgoing payloads; retries reuse the original client identity.
use crate::{chat::ChatMessage, message_model::DeliveryState, session::Session};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    Text,
    Quote(Box<ChatMessage>),
    Forward(Box<ChatMessage>),
    Sticker {
        id: u64,
        category: u64,
        kind: u32,
    },
    Attachment {
        path: std::path::PathBuf,
        caption: String,
        #[serde(default)]
        stamp: Option<FileStamp>,
        /// Set when `path` is an HD copy: the user's original photo, used to rebuild a
        /// copy that was cleaned up before a retry.
        #[serde(default)]
        hd_source: Option<HdSource>,
    },
}
/// The original photo an HD copy was made from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HdSource {
    pub path: std::path::PathBuf,
    pub stamp: Option<FileStamp>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileStamp {
    size: u64,
    modified: Option<std::time::SystemTime>,
}
impl FileStamp {
    pub async fn read(path: &std::path::Path) -> Result<Self> {
        let metadata = tokio::fs::metadata(path).await?;
        anyhow::ensure!(metadata.is_file(), "Đường dẫn phải là tập tin thường");
        Ok(Self {
            size: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub payload: Payload,
    pub is_group: bool,
    pub attempt: u32,
    pub error: String,
}
pub fn pending(text: String, is_group: bool, payload: Payload) -> ChatMessage {
    let id = crate::chat::next_client_message_id();
    ChatMessage {
        id: format!("out_{id}"),
        client_message_id: id.to_string(),
        text,
        sender: "Tôi".into(),
        time: crate::ui::clock(),
        ts: crate::login::unix_now() * 1000,
        is_outgoing: true,
        delivery: DeliveryState::Pending,
        outgoing_job: Some(Box::new(Job {
            payload,
            is_group,
            attempt: 1,
            error: String::new(),
        })),
        ..Default::default()
    }
}
pub async fn send(s: &Session, to: &str, message: &ChatMessage) -> Result<ChatMessage> {
    let job = message
        .outgoing_job
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Thiếu dữ liệu gửi lại"))?;
    let id = message.client_message_id.parse()?;
    let mut sent = message.clone();
    let response = match &job.payload {
        Payload::Text => {
            if let Some(url) = crate::chat::extract_first_url(&message.text) {
                let meta = crate::chat::parse_link_metadata(s, &url).await.unwrap_or(
                    crate::chat::LinkMetadata {
                        title: url.clone(),
                        url,
                        desc: String::new(),
                        thumb: String::new(),
                        src: "Liên kết".into(),
                    },
                );
                let result = crate::chat::send_link_message_with_client_id(
                    s,
                    to,
                    &message.text,
                    &meta,
                    job.is_group,
                    id,
                )
                .await?;
                sent.link_url = Some(meta.url);
                sent.link_title = Some(meta.title);
                sent.link_desc = Some(meta.desc);
                sent.link_thumb = Some(meta.thumb);
                sent.link_src = Some(meta.src);
                result
            } else if job.is_group {
                crate::chat::send_group_message_with_client_id(s, to, &message.text, id).await?
            } else {
                crate::chat::send_message_with_client_id(s, to, &message.text, id).await?
            }
        }
        Payload::Forward(original)
            if original.sticker_cate_id == Some(crate::bundled_stickers::CATEGORY) =>
        {
            sent = send_bundled(
                s,
                to,
                job.is_group,
                original
                    .sticker_id
                    .ok_or_else(|| anyhow::anyhow!("Thiếu nhãn GIF"))?,
                id,
            )
            .await?;
            serde_json::Value::Null
        }
        Payload::Forward(original) if crate::chat::forwardable_media(original) => {
            sent = forward_media(s, to, job.is_group, original, id).await?;
            serde_json::Value::Null
        }
        Payload::Forward(original) => {
            crate::chat::forward_message_with_client_id(s, to, job.is_group, original, id).await?
        }
        Payload::Quote(quoted) => {
            crate::chat::send_quote_message_with_client_id(
                s,
                to,
                &message.text,
                quoted,
                job.is_group,
                id,
            )
            .await?
        }
        Payload::Sticker {
            id: sticker,
            category,
            kind: _,
        } if *category == crate::bundled_stickers::CATEGORY => {
            sent = send_bundled(s, to, job.is_group, *sticker, id).await?;
            serde_json::Value::Null
        }
        Payload::Sticker {
            id: sticker,
            category,
            kind,
        } => {
            crate::chat::send_sticker_with_client_id(
                s,
                to,
                *sticker,
                *category,
                *kind,
                job.is_group,
                id,
            )
            .await?
        }
        Payload::Attachment {
            path,
            caption,
            stamp,
            hd_source,
        } => {
            let upload = match hd_source {
                Some(source) => hd_copy_for(s, path, source).await?,
                None => {
                    if let Some(stamp) = stamp {
                        anyhow::ensure!(
                            *stamp == FileStamp::read(path).await?,
                            "Tệp gốc đã thay đổi. Hãy chọn và gửi như một tệp mới"
                        );
                    }
                    path.clone()
                }
            };
            let (_, result) = crate::chat::send_attachment_with_client_id(
                s,
                to,
                job.is_group,
                &upload,
                caption,
                id,
            )
            .await?;
            if hd_source.is_some() {
                // The HD copy has served its purpose; the original stays where it was.
                let _ = tokio::fs::remove_file(&upload).await;
            }
            sent = result;
            serde_json::Value::Null
        }
    };
    if !response.is_null() {
        let id = crate::chat::sent_server_id(&response);
        if !id.is_empty() {
            sent.id = id;
        }
    }
    sent.delivery = DeliveryState::ServerAccepted;
    sent.outgoing_job = None;
    Ok(sent)
}

/// The HD copy to upload: the existing one, or a new one from the unchanged original when
/// it was cleaned up.
async fn hd_copy_for(
    s: &Session,
    copy: &std::path::Path,
    source: &HdSource,
) -> Result<std::path::PathBuf> {
    if tokio::fs::metadata(copy).await.is_ok_and(|m| m.is_file()) {
        return Ok(copy.to_owned());
    }
    if let Some(stamp) = &source.stamp {
        anyhow::ensure!(
            *stamp == FileStamp::read(&source.path).await?,
            "Ảnh gốc đã thay đổi. Hãy chọn và gửi lại ảnh"
        );
    }
    let original = source.path.clone();
    let dir = crate::photo_quality::outgoing_dir(s.account_key());
    Ok(
        tokio::task::spawn_blocking(move || crate::photo_quality::reduce(&original, &dir))
            .await??
            .unwrap_or_else(|| source.path.clone()),
    )
}

/// Largest media file downloaded for a forward.
const FORWARD_LIMIT: u64 = 1024 * 1024 * 1024;

/// Forward a photo, video or file like Zalo PC (`forwardFile` → `ForwardAsync.forward`):
/// ask the server to copy the media for the target with `renewlink` (target conversation,
/// new client ID; videos copy the file and the poster separately), then send
/// `forward` with the copied URLs. If that path cannot be used (missing checksum, copy
/// refused), fall back to downloading the media and sending it again (`_reuploadMedia`),
/// renewing an expired source link once through its source conversation.
async fn forward_media(
    s: &Session,
    to: &str,
    group: bool,
    original: &ChatMessage,
    client_id: u64,
) -> Result<ChatMessage> {
    match forward_by_clone(s, to, group, original, client_id).await {
        Ok(sent) => return Ok(sent),
        Err(error) => crate::logger::log_error(
            "forward clone",
            &format!("{error:#}; re-uploading the media instead"),
        ),
    }
    forward_by_reupload(s, to, group, original, client_id).await
}

async fn forward_by_clone(
    s: &Session,
    to: &str,
    group: bool,
    original: &ChatMessage,
    client_id: u64,
) -> Result<ChatMessage> {
    use crate::message_view::MediaKind;
    let media = crate::message_view::media(original);
    let msg_type = crate::media_renew::forward_type(media.kind)
        .ok_or_else(|| anyhow::anyhow!("Loại media chưa hỗ trợ"))?;
    let mut info = crate::chat::forward_info(original)
        .ok_or_else(|| anyhow::anyhow!("Thiếu thông tin media để sao chép"))?;
    let kind = media.kind;
    let copy = move |info: Value, original_flag: bool| async move {
        crate::media_renew::clone_for(s, to, group, kind, &info, client_id, original_flag).await
    };
    match media.kind {
        MediaKind::Photo => {
            let copied = copy(info.clone(), false).await?;
            info["oriUrl"] = json!(copied.original.clone().unwrap_or_default());
            info["hdUrl"] = json!(copied.preview.clone().unwrap_or_default());
            info["thumbUrl"] = json!(copied.thumbnail.clone().unwrap_or_default());
            info["normalUrl"] = json!("");
            anyhow::ensure!(
                info["oriUrl"].as_str().is_some_and(|u| !u.is_empty())
                    && info["thumbUrl"].as_str().is_some_and(|u| !u.is_empty()),
                "Bản sao ảnh thiếu đường dẫn"
            );
        }
        MediaKind::Video => {
            let mut video = info.clone();
            video["renewType"] = json!(1);
            let mut poster = info.clone();
            poster["renewType"] = json!(0);
            let (video, poster) = tokio::join!(copy(video, false), copy(poster, true));
            let video = video?;
            info["videoUrl"] = json!(video
                .original
                .ok_or_else(|| anyhow::anyhow!("Bản sao video thiếu đường dẫn"))?);
            if let Some(thumb) = poster.ok().and_then(|p| p.original.or(p.thumbnail)) {
                info["thumbUrl"] = json!(thumb);
            }
        }
        MediaKind::File => {
            let copied = copy(info.clone(), false).await?;
            info["url"] = json!(copied
                .original
                .ok_or_else(|| anyhow::anyhow!("Bản sao tệp thiếu đường dẫn"))?);
        }
        MediaKind::None => anyhow::bail!("Loại media chưa hỗ trợ"),
    }
    let response =
        crate::chat::send_forward_message(s, to, group, msg_type, &info, client_id).await?;
    let mut sent = original.clone();
    sent.id = crate::chat::sent_server_id(&response);
    if sent.id.is_empty() {
        sent.id = format!("out_{client_id}");
    }
    sent.client_message_id = client_id.to_string();
    sent.is_outgoing = true;
    sent.sender = "Tôi".into();
    sent.ts = client_id as i64;
    sent.time = crate::ui::clock();
    sent.history_source = None;
    sent.wire_ref = Default::default();
    sent.wire_ref.fields.insert("msgInfo".into(), info.clone());
    let url = |k: &str| {
        info.get(k)
            .and_then(Value::as_str)
            .filter(|u| !u.is_empty())
            .map(str::to_owned)
    };
    match media.kind {
        MediaKind::Photo => {
            sent.link_url = url("oriUrl");
            sent.link_thumb = url("thumbUrl");
        }
        MediaKind::Video => {
            sent.link_url = url("videoUrl");
            sent.link_thumb = url("thumbUrl");
        }
        _ => sent.link_url = url("url"),
    }
    Ok(sent)
}

async fn forward_by_reupload(
    s: &Session,
    to: &str,
    group: bool,
    original: &ChatMessage,
    client_id: u64,
) -> Result<ChatMessage> {
    let media = crate::message_view::media(original);
    let dir = crate::storage::root().join("forward-media");
    crate::storage::private_dir(&dir)?;
    let mut random = [0u8; 8];
    crate::random::fill(&mut random)?;
    let name = crate::media_download::safe_name(&crate::chat::forward_file_name(original, &media));
    let path = dir.join(format!(
        "{}-{name}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ));
    let mut result = match forward_url(&media) {
        Some(url) => download_for_forward(s, &url, &path, &media).await,
        None => Err(anyhow::anyhow!("Tin này không còn đường dẫn media")),
    };
    if result.is_err() {
        let source = match crate::chat::forward_source(original) {
            Some(source) => Some(source),
            None => source_from_store(s, &original.id).await,
        };
        if let Some((thread, source_group)) = source {
            let renewed = crate::media_renew::renew(s, &thread, source_group, original).await;
            if let Some(url) = renewed.ok().as_ref().and_then(forward_url) {
                result = download_for_forward(s, &url, &path, &media).await;
            }
        }
    }
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(error.context(
            "Không chuyển tiếp được media: máy chủ không sao chép được và đường dẫn gốc đã hết hạn",
        ));
    }
    let sent =
        crate::chat::send_attachment_with_client_id(s, to, group, &path, "", client_id).await;
    let _ = tokio::fs::remove_file(&path).await;
    Ok(sent?.1)
}

/// The conversation that stores `msg_id`, when exactly one does.
async fn source_from_store(s: &Session, msg_id: &str) -> Option<(String, bool)> {
    if msg_id.is_empty() {
        return None;
    }
    let account = s.clone();
    let id = msg_id.to_owned();
    tokio::task::spawn_blocking(move || {
        crate::store::MessageStore::open(account.history_path())
            .ok()?
            .unique_thread_of_message(&id)
            .ok()
            .flatten()
    })
    .await
    .ok()
    .flatten()
}

fn forward_url(media: &crate::message_view::Media) -> Option<String> {
    match media.kind {
        crate::message_view::MediaKind::Photo => media.original.clone().or(media.preview.clone()),
        _ => media.original.clone(),
    }
}

async fn download_for_forward(
    s: &Session,
    url: &str,
    path: &std::path::Path,
    media: &crate::message_view::Media,
) -> Result<u64> {
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watch = cancel.clone();
    let session = s.clone();
    crate::media_download::save_classified(
        url,
        path,
        cancel,
        move |bytes, _| {
            if bytes > FORWARD_LIMIT || !session.is_active() {
                watch.store(true, std::sync::atomic::Ordering::Release);
            }
        },
        media.kind != crate::message_view::MediaKind::File,
    )
    .await
}

/// Keep the actual attachment response and reuse the original outgoing client identity.
async fn send_bundled(
    s: &Session,
    to: &str,
    group: bool,
    sticker: u64,
    client_id: u64,
) -> Result<ChatMessage> {
    let path = crate::bundled_stickers::stage(s, sticker).await?;
    let (_, mut sent) =
        crate::chat::send_attachment_with_client_id(s, to, group, &path, "", client_id).await?;
    sent.sticker_id = Some(sticker);
    sent.sticker_cate_id = Some(crate::bundled_stickers::CATEGORY);
    sent.sticker_type = Some(7);
    Ok(sent)
}

fn active_jobs() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static ACTIVE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    ACTIVE.get_or_init(Default::default)
}
pub(crate) fn is_active(id: &str) -> bool {
    active_jobs()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(id)
}
pub(crate) struct Active(String);
impl Active {
    pub(crate) fn start(id: &str) -> Option<Self> {
        active_jobs()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.to_owned())
            .then(|| Self(id.to_owned()))
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        active_jobs()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

/// Keep transport URLs and encrypted request parameters out of the chat bubble.
pub(crate) fn failure_message(error: &anyhow::Error) -> String {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<reqwest::Error>() {
            return if error.is_connect() { "Không kết nối được mạng. Kiểm tra kết nối rồi gửi lại." }
                else if error.is_timeout() { "Hết thời gian chờ xác nhận. Kiểm tra hội thoại trước khi gửi lại để tránh trùng tin." }
                else { "Kết nối bị gián đoạn. Kiểm tra hội thoại trước khi gửi lại để tránh trùng tin." }.into();
        }
    }
    format!("{error}. Kiểm tra hội thoại trước khi gửi lại để tránh trùng tin.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_send_restores_payload_and_ack_removes_retry() {
        let original = pending("hello".into(), false, Payload::Text);
        let mut restored = ChatMessage {
            id: original.id.clone(),
            ..Default::default()
        };
        restored.restore_state(&original.durable_state()).unwrap();
        assert_eq!(restored.delivery, DeliveryState::Failed);
        assert!(restored.outgoing_job.is_some());
        assert_eq!(restored.client_message_id, original.client_message_id);
        let mut retry = restored.clone();
        retry.delivery = DeliveryState::Pending;
        retry.outgoing_job.as_mut().unwrap().attempt += 1;
        let active = Active::start(&retry.client_message_id).unwrap();
        restored.restore_state(&retry.durable_state()).unwrap();
        assert_eq!(restored.delivery, DeliveryState::Pending);
        assert!(Active::start(&retry.client_message_id).is_none());
        retry.delivery = DeliveryState::ServerAccepted;
        retry.outgoing_job = None;
        restored.restore_state(&retry.durable_state()).unwrap();
        assert_eq!(restored.delivery, DeliveryState::ServerAccepted);
        assert!(restored.outgoing_job.is_none());
        drop(active);
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn server_echo_overrides_failed_job_and_late_local_failure() {
        let mut store = crate::store::MessageStore::open(":memory:").unwrap();
        let mut failed = pending("test body".into(), false, Payload::Text);
        failed.delivery = DeliveryState::Failed;
        let snapshot = |message| crate::chat::Thread {
            id: "recipient".into(),
            name: "recipient".into(),
            messages: vec![message],
            ..Default::default()
        };
        crate::store::persist_threads(&mut store, &[snapshot(failed.clone())]).unwrap();
        let mut echo = failed.clone();
        echo.id = "42".into();
        echo.delivery = DeliveryState::Unknown;
        echo.outgoing_job = None;
        crate::store::persist_threads(&mut store, &[snapshot(echo)]).unwrap();
        crate::store::persist_threads(&mut store, &[snapshot(failed)]).unwrap();
        let mut threads = Vec::new();
        store.hydrate_threads(&mut threads, 20).unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].messages.len(), 1);
        let message = &threads[0].messages[0];
        assert_eq!(message.id, "42");
        assert_eq!(message.delivery, DeliveryState::ServerAccepted);
        assert!(message.outgoing_job.is_none());
    }
}
