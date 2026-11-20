//! Durable outgoing payloads; retries reuse the original client identity.
use crate::{chat::ChatMessage, message_model::DeliveryState, session::Session};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    Text,
    Formatted {
        options: crate::rich_text::SendOptions,
        mentions: Vec<crate::mentions::Mention>,
        quoted: Option<Box<ChatMessage>>,
    },
    Mentioned {
        mentions: Vec<crate::mentions::Mention>,
        quoted: Option<Box<ChatMessage>>,
    },
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
    #[serde(default)]
    pub is_e2ee: bool,
    pub attempt: u32,
    pub error: String,
}
pub fn pending(text: String, is_group: bool, payload: Payload) -> ChatMessage {
    let id = crate::chat::next_client_message_id();
    let attachment_name = match &payload {
        Payload::Attachment { path, .. } => path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        _ => None,
    };
    ChatMessage {
        kind: if attachment_name.is_some() {
            crate::message_model::MessageKind::File
        } else {
            Default::default()
        },
        link_title: attachment_name,
        id: format!("out_{id}"),
        client_message_id: id.to_string(),
        text,
        sender: "Tôi".into(),
        time: crate::ui::clock(),
        ts: crate::login::unix_now() * 1000,
        is_outgoing: true,
        is_e2ee: false,
        delivery: DeliveryState::Pending,
        outgoing_job: Some(Box::new(Job {
            payload,
            is_group,
            is_e2ee: false,
            attempt: 1,
            error: String::new(),
        })),
        ..Default::default()
    }
}

pub fn pending_e2ee(text: String, payload: Payload) -> ChatMessage {
    let id = crate::chat::next_client_message_id();
    ChatMessage {
        id: format!("out_{id}"),
        client_message_id: id.to_string(),
        text,
        sender: "Tôi".into(),
        time: crate::ui::clock(),
        ts: crate::login::unix_now() * 1000,
        is_outgoing: true,
        is_e2ee: true,
        delivery: DeliveryState::Pending,
        outgoing_job: Some(Box::new(Job {
            payload,
            is_group: false,
            is_e2ee: true,
            attempt: 1,
            error: String::new(),
        })),
        ..Default::default()
    }
}

/// Check whether a destination requires E2EE, preserving storage failures.
/// Fail-closed: database read/open errors must NOT be silently swallowed as false.
pub fn check_thread_e2ee(account_key: &str, to_id: &str, is_group: bool) -> Result<bool> {
    if is_group {
        return Ok(false);
    }
    crate::e2ee_integration::check_thread_e2ee(account_key, to_id)
}

pub fn is_thread_e2ee(account_key: &str, to_id: &str, is_group: bool) -> bool {
    check_thread_e2ee(account_key, to_id, is_group).unwrap_or(false)
}

/// Where a plain-text payload goes (audit A01/A02).
#[derive(Debug, PartialEq, Eq)]
pub enum TextRoute {
    /// End-to-end encrypted message (audit A01/A02): never leaks URL to preview,
    /// and fails closed if encryption/send fails without falling back to plaintext.
    E2ee,
    /// Plaintext message with URL: triggers OpenGraph link preview.
    Link(String),
    /// Plaintext group message.
    Group,
    /// Plaintext direct 1-1 message.
    Direct,
}

pub fn route_text(text: &str, is_group: bool, is_e2ee: bool) -> TextRoute {
    if is_e2ee {
        // Audit A01/A02: E2EE messages must never leak URL via plaintext link previews
        // and must go directly to E2EE encryption path.
        TextRoute::E2ee
    } else if let Some(url) = crate::chat::extract_first_url(text) {
        // Plaintext direct (1-1) and group messages with URLs go to link preview
        TextRoute::Link(url)
    } else if is_group {
        TextRoute::Group
    } else {
        TextRoute::Direct
    }
}

pub(crate) const LINK_PREVIEW_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);

async fn bounded_link_preview(
    session: &Session,
    url: &str,
    preview: impl std::future::Future<Output = Result<crate::chat::LinkMetadata>>,
) -> Option<crate::chat::LinkMetadata> {
    if let Some(metadata) = session.cached_link_preview(url, crate::chat::LinkOrigin::Outgoing) {
        session.record_link_preview_cache_hit();
        crate::service_resolver::record_current_cache_status(
            crate::service_resolver::TimingPhase::LinkPreview,
            true,
        );
        crate::service_resolver::record_current_phase(
            crate::service_resolver::TimingPhase::LinkPreview,
            std::time::Duration::ZERO,
            crate::service_resolver::AttemptResult::PreviewCacheHit,
        );
        return Some(metadata);
    }
    crate::service_resolver::record_current_cache_status(
        crate::service_resolver::TimingPhase::LinkPreview,
        false,
    );
    let started = std::time::Instant::now();
    let result = match tokio::time::timeout(LINK_PREVIEW_BUDGET, preview).await {
        Ok(Ok(metadata)) => Some(metadata),
        Ok(Err(_)) | Err(_) => None,
    };
    session.record_link_preview_timing(started.elapsed(), result.is_some());
    crate::service_resolver::record_current_phase(
        crate::service_resolver::TimingPhase::LinkPreview,
        started.elapsed(),
        if result.is_some() {
            crate::service_resolver::AttemptResult::PreviewReady
        } else {
            crate::service_resolver::AttemptResult::PreviewFallback
        },
    );
    if let Some(metadata) = &result {
        session.cache_link_preview(url, crate::chat::LinkOrigin::Outgoing, metadata.clone());
    }
    result
}

/// Record the actual command enqueue, rather than time spent editing a draft.
/// WorkerSender calls this immediately before enqueue; duplicate commands retain
/// the first enqueue timestamp. The bounded map stores only keyed intent tags.
pub(crate) fn mark_queued(message: &ChatMessage) {
    let Some(tag) = crate::service_resolver::intent_tag(&message.client_message_id) else {
        return;
    };
    let mut queued = queued_intents().lock().unwrap_or_else(|e| e.into_inner());
    if queued.len() >= 256 {
        if let Some(oldest) = queued
            .iter()
            .min_by_key(|(_, at)| **at)
            .map(|(tag, _)| tag.clone())
        {
            queued.remove(&oldest);
        }
    }
    queued.entry(tag).or_insert_with(std::time::Instant::now);
}
fn queued_intents(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>> {
    static QUEUED: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>,
    > = std::sync::OnceLock::new();
    QUEUED.get_or_init(Default::default)
}
fn take_enqueued_at(message: &ChatMessage) -> std::time::Instant {
    crate::service_resolver::intent_tag(&message.client_message_id)
        .and_then(|tag| {
            queued_intents()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&tag)
        })
        .unwrap_or_else(std::time::Instant::now)
}
fn record_queue_wait(
    context: &crate::service_resolver::SendTimingContext,
    enqueued: std::time::Instant,
) -> Result<()> {
    let elapsed = enqueued.elapsed();
    crate::service_resolver::record_queue_phase(context, elapsed);
    if elapsed > crate::service_resolver::QUEUE_BUDGET {
        crate::transfer_diagnostics::send_failure(
            crate::transfer_diagnostics::SendFailure::QueueBudget,
            None,
            None,
        );
        return Err(anyhow::Error::new(SendBudgetElapsed(
            crate::service_resolver::TimingPhase::QueueWait,
        ))
        .context(BeforeMessageSend));
    }
    Ok(())
}
fn timing_session_scope(session: &Session) -> usize {
    // Arc pointer addresses can be reused after logout. Include a keyed account
    // fingerprint so the bounded diagnostic ring cannot expose another owner.
    let identity = format!("{}:{}", session.runtime_id(), session.account_key());
    crate::service_resolver::intent_tag(&identity)
        .and_then(|tag| usize::from_str_radix(&tag[..std::mem::size_of::<usize>() * 2], 16).ok())
        .unwrap_or(session.runtime_id())
}
pub(crate) fn send_timings(session: &Session) -> Vec<crate::service_resolver::SendPhaseTiming> {
    crate::service_resolver::send_phase_timings(timing_session_scope(session))
}
fn is_text_payload(payload: &Payload) -> bool {
    matches!(
        payload,
        Payload::Text | Payload::Formatted { .. } | Payload::Mentioned { .. } | Payload::Quote(_)
    )
}

#[derive(Debug)]
struct SendBudgetElapsed(crate::service_resolver::TimingPhase);
impl std::fmt::Display for SendBudgetElapsed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Hết ngân sách chờ {:?}; đối soát trạng thái tin trước khi thử lại",
            self.0
        )
    }
}
impl std::error::Error for SendBudgetElapsed {}

fn timing_outcome(result: &Result<ChatMessage>) -> crate::service_resolver::AttemptResult {
    use crate::service_resolver::AttemptResult as R;
    match result {
        Ok(message) if crate::chat::server_message_id(message).is_some() => R::Success,
        Ok(_) => R::ProtocolFailure,
        Err(error) if error.downcast_ref::<SendBudgetElapsed>().is_some() => R::BudgetExceeded,
        Err(error) => match send_failure_kind(error) {
            SendFailureKind::NotSent => R::PolicyRejected,
            SendFailureKind::ExplicitlyRejected => R::HttpFailure,
            SendFailureKind::Unknown => R::TransportFailure,
        },
    }
}

pub async fn send(s: &Session, to: &str, message: &ChatMessage) -> Result<ChatMessage> {
    use crate::service_resolver::{self as timing, TimingPhase};
    let Some(job) = message.outgoing_job.as_ref() else {
        return send_inner(s, to, message).await;
    };
    let text = is_text_payload(&job.payload);
    let started = take_enqueued_at(message);
    let context = timing::SendTimingContext::from_enqueue(
        timing_session_scope(s),
        &message.client_message_id,
        job.attempt,
        job.is_group,
        text,
        started,
    );
    let queue_result = record_queue_wait(&context, started);
    timing::with_send_timing(context, async {
        let result = if let Err(error) = queue_result {
            Err(error)
        } else if text {
            tokio::time::timeout_at(
                tokio::time::Instant::from_std(started + timing::TEXT_TOTAL_BUDGET),
                send_inner(s, to, message),
            )
            .await
            .unwrap_or_else(|_| {
                Err(anyhow::Error::new(SendBudgetElapsed(
                    TimingPhase::TotalIntent,
                )))
            })
        } else {
            send_inner(s, to, message).await
        };
        timing::record_current_phase(
            TimingPhase::TotalIntent,
            started.elapsed(),
            timing_outcome(&result),
        );
        result
    })
    .await
}

async fn send_inner(s: &Session, to: &str, message: &ChatMessage) -> Result<ChatMessage> {
    let job = message
        .outgoing_job
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Thiếu dữ liệu gửi lại"))
        .context(BeforeMessageSend)?;
    if let Payload::Attachment {
        path,
        stamp: Some(expected),
        hd_source: None,
        ..
    } = &job.payload
    {
        let current = FileStamp::read(path).await.context(BeforeMessageSend)?;
        (|| -> Result<()> {
            anyhow::ensure!(
                *expected == current,
                "Tệp gốc đã thay đổi. Hãy chọn và gửi như một tệp mới"
            );
            Ok(())
        })()
        .context(BeforeMessageSend)?;
    }
    // These reads/validations cannot deliver a chat message. Preserve their
    // causes, but distinguish them from a failed mutation with an unknown ACK.
    let policy_started = std::time::Instant::now();
    let policy = tokio::time::timeout(crate::service_resolver::POLICY_BUDGET, async {
        anyhow::ensure!(s.is_active() && s.login.is_some(), "Chưa đăng nhập hoặc phiên đã đóng");
        anyhow::ensure!(!to.is_empty(), "Thiếu người nhận tin nhắn");
        anyhow::ensure!(
            !(job.is_group && (message.is_e2ee || job.is_e2ee)),
            "Chưa hỗ trợ E2EE thử nghiệm cho nhóm"
        );
        if job.is_group {
            let snapshot = crate::group_state::snapshot(s, to, false).await?;
            anyhow::ensure!(
                snapshot.can_send(s.account_key())?,
                "Nhóm không cho phép tài khoản này gửi tin"
            );
        }
        if let Payload::Forward(original) = &job.payload {
            let (source, group) = crate::chat::forward_source(original)
                .context("Thiếu nguồn chuyển tiếp để xác minh quyền chia sẻ")?;
            crate::conversation_policy::require_shareable(s, &source, group).await?;
        }
        let is_e2ee = message.is_e2ee
            || job.is_e2ee
            || check_thread_e2ee(s.account_key(), to, job.is_group)
                .context("Không thể xác minh trạng thái E2EE; đã hủy gửi để bảo vệ tin nhắn")?;

        // Unverified encrypted content never reaches ordinary upload endpoints.
        anyhow::ensure!(
            !is_e2ee || matches!(job.payload, Payload::Text),
            "Hội thoại E2EE hiện chỉ hỗ trợ gửi văn bản mã hóa; đã chặn nội dung khác để tránh rò rỉ"
        );
        if is_e2ee {
            #[cfg(feature = "experimental-e2ee")]
            crate::e2ee_integration::require_verified_contract()?;
            #[cfg(not(feature = "experimental-e2ee"))]
            return Err(crate::e2ee_integration::unverified_contract_error());
        }
        let id = message.client_message_id.parse::<u64>()
            .context("Định danh tin nhắn gửi không hợp lệ")?;
        Ok::<_, anyhow::Error>((is_e2ee, id))
    }).await.unwrap_or_else(|_|Err(anyhow::Error::new(SendBudgetElapsed(crate::service_resolver::TimingPhase::Policy))));
    crate::service_resolver::record_current_phase(
        crate::service_resolver::TimingPhase::Policy,
        policy_started.elapsed(),
        if policy.is_ok() {
            crate::service_resolver::AttemptResult::Success
        } else if policy
            .as_ref()
            .err()
            .is_some_and(|e| e.downcast_ref::<SendBudgetElapsed>().is_some())
        {
            crate::service_resolver::AttemptResult::BudgetExceeded
        } else {
            crate::service_resolver::AttemptResult::PolicyRejected
        },
    );
    let (is_e2ee, id) = policy.context(BeforeMessageSend)?;
    let mut sent = message.clone();
    let response = match &job.payload {
        Payload::Formatted {
            options,
            mentions,
            quoted,
        } => {
            // Validate metadata before the transport can make the outcome uncertain.
            options
                .apply(&message.text, &mut json!({}))
                .context(BeforeMessageSend)?;
            crate::mentions::validate(&message.text, mentions).context(BeforeMessageSend)?;
            (|| -> Result<()> {
                anyhow::ensure!(
                    job.is_group || mentions.is_empty(),
                    "Mention chỉ hỗ trợ trong nhóm"
                );
                Ok(())
            })()
            .context(BeforeMessageSend)?;
            sent.text_styles = options.styles.clone();
            if let Some(quoted) = quoted {
                crate::chat::send_quote_with_options(
                    s,
                    to,
                    &message.text,
                    quoted,
                    job.is_group,
                    id,
                    mentions,
                    options,
                )
                .await?
            } else if job.is_group {
                crate::chat::send_group_with_options(s, to, &message.text, id, mentions, options)
                    .await?
            } else {
                crate::chat::send_text_with_options(s, to, &message.text, id, options).await?
            }
        }
        Payload::Mentioned { mentions, quoted } => {
            (|| -> Result<()> {
                anyhow::ensure!(
                    job.is_group && !mentions.is_empty(),
                    "Thiếu thông tin mention nhóm"
                );
                crate::mentions::validate(&message.text, mentions)
            })()
            .context(BeforeMessageSend)?;
            if let Some(quoted) = quoted {
                crate::chat::send_quote_mentions(s, to, &message.text, quoted, true, id, mentions)
                    .await?
            } else {
                crate::chat::send_group_mentions(s, to, &message.text, id, mentions).await?
            }
        }
        Payload::Text => {
            // Encrypted messages bypass link previews and ordinary send endpoints.
            // Encryption failures remain failures rather than downgrading to plaintext.
            match route_text(&message.text, job.is_group, is_e2ee) {
                TextRoute::E2ee => {
                    let result =
                        crate::chat::send_e2ee_message(s, s.account_key(), to, &message.text, id)
                            .await
                            .context("Gửi tin nhắn E2EE thất bại")?;
                    sent.is_e2ee = true;
                    result
                }
                TextRoute::Link(url) => {
                    // PC preview wait defaults to 2s (E2, send-latency guide P05).
                    // The deadline covers both parselink and outgoing scraping.
                    let meta = bounded_link_preview(
                        s,
                        &url,
                        crate::chat::parse_link_metadata(
                            s,
                            &url,
                            crate::chat::LinkOrigin::Outgoing,
                        ),
                    )
                    .await;
                    if let Some(meta) = meta {
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
                        // Explicit endpoints prevent routing back into preview.
                        crate::chat::send_group_message_with_client_id(s, to, &message.text, id)
                            .await?
                    } else {
                        crate::chat::send_message_with_client_id(s, to, &message.text, id).await?
                    }
                }
                TextRoute::Group => {
                    crate::chat::send_group_message_with_client_id(s, to, &message.text, id).await?
                }
                TextRoute::Direct => {
                    crate::chat::send_message_with_client_id(s, to, &message.text, id).await?
                }
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
                    .context("Thiếu nhãn GIF")
                    .context(BeforeMessageSend)?,
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
            if let Some(meta) = crate::chat::forward_link_metadata(original) {
                sent.kind = original.kind;
                sent.link_url = Some(meta.url);
                sent.link_title = Some(meta.title);
                sent.link_desc = Some(meta.desc);
                sent.link_thumb = Some(meta.thumb);
                sent.link_src = Some(meta.src);
            }
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
                Some(source) => hd_copy_for(s, path, source)
                    .await
                    .context(BeforeMessageSend)?,
                None => {
                    let actual = FileStamp::read(path).await.context(BeforeMessageSend)?;
                    if let Some(stamp) = stamp {
                        (|| -> Result<()> {
                            anyhow::ensure!(
                                *stamp == actual,
                                "Tệp gốc đã thay đổi. Hãy chọn và gửi như một tệp mới"
                            );
                            Ok(())
                        })()
                        .context(BeforeMessageSend)?;
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
    if crate::chat::server_message_id(&sent).is_some() {
        sent.delivery = sent.delivery.merge(DeliveryState::ServerAccepted);
        sent.outgoing_job = None;
    } else {
        sent.delivery = sent.delivery.merge(DeliveryState::DeliveryUnknown);
    }
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
    let info = match clone_forward_info(s, to, group, original, client_id).await {
        Ok(info) => info,
        Err(_) => {
            // Copying does not send a message. Only this preparation phase may fall back.
            crate::logger::log_error("forward clone", "Media copy failed; trying re-upload");
            return forward_by_reupload(s, to, group, original, client_id).await;
        }
    };
    // A missing response can mean the message was delivered. Never issue a second send.
    let msg_type = crate::media_renew::forward_type(crate::message_view::media(original).kind)
        .context("Loại media chưa hỗ trợ")
        .context(BeforeMessageSend)?;
    let response =
        crate::chat::send_forward_message(s, to, group, msg_type, &info, client_id).await?;
    Ok(cloned_message(original, client_id, info, &response))
}

async fn clone_forward_info(
    s: &Session,
    to: &str,
    group: bool,
    original: &ChatMessage,
    client_id: u64,
) -> Result<Value> {
    use crate::message_view::MediaKind;
    let media = crate::message_view::media(original);
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
    Ok(info)
}

fn cloned_message(
    original: &ChatMessage,
    client_id: u64,
    info: Value,
    response: &Value,
) -> ChatMessage {
    use crate::message_view::MediaKind;
    let media = crate::message_view::media(original);
    // Forwarded content is a new message, not a copy of local flags/reactions/replies.
    let server_id = crate::chat::sent_server_id(response);
    let mut sent = ChatMessage {
        id: if server_id.is_empty() {
            format!("out_{client_id}")
        } else {
            server_id
        },
        client_message_id: client_id.to_string(),
        kind: original.kind,
        text: original.text.clone(),
        text_styles: original.text_styles.clone(),
        link_title: original.link_title.clone(),
        link_desc: original.link_desc.clone(),
        link_src: original.link_src.clone(),
        is_outgoing: true,
        sender: "Tôi".into(),
        ts: client_id as i64,
        time: crate::ui::clock(),
        ..Default::default()
    };
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
    sent
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
    crate::storage::private_dir(&dir).context(BeforeMessageSend)?;
    let mut random = [0u8; 8];
    crate::random::fill(&mut random).context(BeforeMessageSend)?;
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
        ).context(BeforeMessageSend));
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
    let path = crate::bundled_stickers::stage(s, sticker)
        .await
        .context(BeforeMessageSend)?;
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
            return if error.is_connect() { "Không kết nối được máy chủ gửi tin. Có thể do DNS, máy chủ hoặc đường truyền; kiểm tra hội thoại trước khi gửi lại." }
                else if error.is_timeout() { "Hết thời gian chờ xác nhận. Kiểm tra hội thoại trước khi gửi lại để tránh trùng tin." }
                else { "Kết nối bị gián đoạn. Kiểm tra hội thoại trước khi gửi lại để tránh trùng tin." }.into();
        }
    }
    if send_failure_kind(error) == SendFailureKind::NotSent {
        return format!("{error:#}");
    }
    format!("{error}. Kiểm tra hội thoại trước khi gửi lại để tránh trùng tin.")
}

/// Marks a phase that cannot submit the chat-message mutation, even if it reads
/// account policy or prepares an attachment using another service.
#[derive(Debug)]
struct BeforeMessageSend;

impl std::fmt::Display for BeforeMessageSend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Chưa gửi tin nhắn")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendFailureKind {
    NotSent,
    ExplicitlyRejected,
    Unknown,
}

/// Preparation errors cannot deliver a message; server rejections are explicit.
/// All other errors remain uncertain, including untyped failures below send APIs.
pub(crate) fn send_failure_kind(error: &anyhow::Error) -> SendFailureKind {
    if error.downcast_ref::<BeforeMessageSend>().is_some() {
        return SendFailureKind::NotSent;
    }
    if error.chain().any(|cause| {
        cause
            .downcast_ref::<crate::chat::ServerRejected>()
            .is_some_and(|rejection| rejection.code.is_some_and(|code| code != 0))
    }) || crate::session::is_explicit_request_rejection(error)
    {
        SendFailureKind::ExplicitlyRejected
    } else {
        SendFailureKind::Unknown
    }
}

pub(crate) fn failure_delivery(error: &anyhow::Error) -> DeliveryState {
    match send_failure_kind(error) {
        SendFailureKind::NotSent | SendFailureKind::ExplicitlyRejected => DeliveryState::Failed,
        SendFailureKind::Unknown => DeliveryState::DeliveryUnknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn expired_command_queue_keeps_the_intent_and_fails_before_transport() {
        let _fixture = crate::service_resolver::TIMING_FIXTURE.lock().await;
        let session = Session::new(false, false, false).unwrap();
        let message = pending("fixture queued payload".into(), false, Payload::Text);
        let tag = crate::service_resolver::intent_tag(&message.client_message_id).unwrap();
        queued_intents().lock().unwrap().insert(
            tag,
            std::time::Instant::now()
                - crate::service_resolver::QUEUE_BUDGET
                - std::time::Duration::from_millis(1),
        );
        let error = send(&session, "fixture-target", &message)
            .await
            .unwrap_err();
        assert_eq!(send_failure_kind(&error), SendFailureKind::NotSent);
        assert_eq!(message.text, "fixture queued payload");
        assert!(message.outgoing_job.is_some());
        assert!(session.request_attempt_timings().is_empty());
        assert!(send_timings(&session).iter().any(|s| s.phase
            == crate::service_resolver::TimingPhase::QueueWait
            && s.result == crate::service_resolver::AttemptResult::BudgetExceeded));
        assert!(send_timings(&session).iter().any(|s| s.phase
            == crate::service_resolver::TimingPhase::TotalIntent
            && s.elapsed_ms >= 5000));
    }

    #[tokio::test]
    async fn preflight_failure_records_policy_and_total_for_the_same_salted_intent() {
        let _fixture = crate::service_resolver::TIMING_FIXTURE.lock().await;
        let session = Session::new(false, false, false).unwrap();
        let message = pending(
            "sensitive fixture text must not enter timing".into(),
            false,
            Payload::Text,
        );
        let _ = send(&session, "sensitive-recipient", &message)
            .await
            .unwrap_err();
        let samples = send_timings(&session);
        assert!(samples
            .iter()
            .any(|s| s.phase == crate::service_resolver::TimingPhase::Policy));
        assert!(samples
            .iter()
            .any(|s| s.phase == crate::service_resolver::TimingPhase::TotalIntent));
        assert!(samples
            .iter()
            .all(|s| s.intent_tag == samples[0].intent_tag));
        let diagnostic = format!("{samples:?}");
        assert!(!diagnostic.contains(&message.text));
        assert!(!diagnostic.contains("sensitive-recipient"));
    }

    #[tokio::test]
    async fn preview_stall_is_cancelled_at_shared_send_budget() {
        struct Cancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Cancellation {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let session = Session::new(false, false, false).unwrap();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let guard = Cancellation(cancelled.clone());
        let preview = async move {
            let _guard = guard;
            std::future::pending::<Result<crate::chat::LinkMetadata>>().await
        };
        let result = tokio::time::timeout(
            LINK_PREVIEW_BUDGET + std::time::Duration::from_secs(5),
            bounded_link_preview(&session, "https://example.com/fixture", preview),
        )
        .await
        .expect("a stalled preview must not hold send indefinitely");
        assert_eq!(LINK_PREVIEW_BUDGET, std::time::Duration::from_secs(2));
        assert!(
            result.is_none(),
            "the caller must choose its ordinary text route"
        );
        assert!(cancelled.load(std::sync::atomic::Ordering::SeqCst));
        let timing = session.request_attempt_timings();
        assert_eq!(timing.len(), 1);
        assert_eq!(
            timing[0].result,
            crate::service_resolver::AttemptResult::PreviewFallback
        );
    }

    #[tokio::test]
    async fn preview_cache_hit_does_not_start_another_preview() {
        let session = Session::new(false, false, false).unwrap();
        let url = "https://example.com/fixture";
        session.cache_link_preview(
            url,
            crate::chat::LinkOrigin::Outgoing,
            crate::chat::LinkMetadata {
                url: url.into(),
                title: "Fixture title".into(),
                desc: String::new(),
                thumb: String::new(),
                src: "example.com".into(),
            },
        );
        let result = bounded_link_preview(
            &session,
            url,
            std::future::poll_fn(|_| panic!("cached metadata must avoid preview network work")),
        )
        .await
        .unwrap();
        assert_eq!(result.title, "Fixture title");
        assert_eq!(
            session.request_attempt_timings()[0].result,
            crate::service_resolver::AttemptResult::PreviewCacheHit
        );
    }

    #[tokio::test]
    async fn logged_out_send_is_definitely_not_sent() {
        let session = Session::new(false, false, false).unwrap();
        let message = pending("fixture".into(), false, Payload::Text);
        let error = send(&session, "fixture-recipient", &message)
            .await
            .unwrap_err();
        assert_eq!(send_failure_kind(&error), SendFailureKind::NotSent);
        assert_eq!(failure_delivery(&error), DeliveryState::Failed);
        assert!(failure_message(&error).contains("Chưa đăng nhập"));
        assert!(message.outgoing_job.is_some());
        assert!(session.request_attempt_timings().is_empty());
    }

    #[test]
    fn preparation_and_e2ee_context_preserve_typed_error_causes() {
        let preflight = anyhow::Error::new(crate::chat::ServerRejected { code: Some(600) })
            .context(BeforeMessageSend);
        assert_eq!(send_failure_kind(&preflight), SendFailureKind::NotSent);
        assert!(preflight
            .downcast_ref::<crate::chat::ServerRejected>()
            .is_some());

        let e2ee = anyhow::Error::new(crate::chat::ServerRejected { code: Some(600) })
            .context("Gửi tin nhắn E2EE thất bại");
        assert_eq!(
            send_failure_kind(&e2ee),
            SendFailureKind::ExplicitlyRejected
        );
        assert!(e2ee
            .chain()
            .any(|cause| crate::session::is_expired_error(&cause.to_string())));

        let ambiguous =
            anyhow::anyhow!("fixture reset after request").context("Gửi tin nhắn E2EE thất bại");
        assert_eq!(failure_delivery(&ambiguous), DeliveryState::DeliveryUnknown);
    }

    #[test]
    fn text_routing_reaches_link_preview_for_direct_and_group() {
        // Audit A02: the link branch was unreachable for 1-1 messages after the
        // E2EE attempt was inserted ahead of it.
        assert!(matches!(
            route_text("xem thêm https://example.com/bai", false, false),
            TextRoute::Link(_)
        ));
        assert!(matches!(
            route_text("https://example.com", true, false),
            TextRoute::Link(_)
        ));
        assert!(matches!(
            route_text("xin chào", false, false),
            TextRoute::Direct
        ));
        assert!(matches!(
            route_text("xin chào", true, false),
            TextRoute::Group
        ));
    }

    #[test]
    fn text_routing_routes_e2ee_without_leaking_url_preview_a02() {
        // Audit A01/A02: E2EE messages must never leak URL via plaintext link previews
        // and must route to E2ee regardless of URLs in text.
        assert_eq!(
            route_text("xem https://secret.com/token", false, true),
            TextRoute::E2ee
        );
        assert_eq!(
            route_text("https://secret.com", false, true),
            TextRoute::E2ee
        );
        assert_eq!(route_text("tin nhắn bí mật", false, true), TextRoute::E2ee);
    }

    #[test]
    fn forwarded_media_does_not_inherit_source_conversation_state() {
        let original = ChatMessage {
            kind: crate::message_model::MessageKind::Photo,
            reactions: vec!["heart".into()],
            is_pinned: true,
            is_starred: true,
            reply_to_id: Some("old-message".into()),
            history_source: Some(json!({"old":true})),
            ..Default::default()
        };
        let sent = cloned_message(
            &original,
            77,
            json!({"oriUrl":"https://photo.zdn.vn/new", "thumbUrl":"https://photo.zdn.vn/thumb"}),
            &json!({"error_code":0,"data":{"msgId":"99"}}),
        );
        assert!(sent.reactions.is_empty());
        assert!(!sent.is_pinned && !sent.is_starred);
        assert!(sent.reply_to_id.is_none() && sent.history_source.is_none());
        assert_eq!(sent.id, "99");
        assert_eq!(sent.link_url.as_deref(), Some("https://photo.zdn.vn/new"));
    }
    #[test]
    fn interrupted_send_restores_payload_and_ack_removes_retry() {
        let original = pending("hello".into(), false, Payload::Text);
        let mut restored = ChatMessage {
            id: original.id.clone(),
            ..Default::default()
        };
        restored.restore_state(&original.durable_state()).unwrap();
        assert_eq!(restored.delivery, DeliveryState::DeliveryUnknown);
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
    fn late_echo_reconciles_delivery_unknown_without_duplicate_or_regression() {
        let mut store = crate::store::MessageStore::open(":memory:").unwrap();
        let mut uncertain = pending("fixture".into(), false, Payload::Text);
        uncertain.delivery = DeliveryState::DeliveryUnknown;
        let snapshot = |message| crate::chat::Thread {
            id: "fixture-recipient".into(),
            messages: vec![message],
            ..Default::default()
        };
        let mut local_thread = snapshot(uncertain.clone());
        crate::store::persist_threads(&mut store, &[local_thread.clone()]).unwrap();
        let mut echo = uncertain.clone();
        echo.id = "fixture-server-id".into();
        echo.delivery = DeliveryState::Unknown;
        echo.outgoing_job = None;
        crate::chat::merge_thread_into(&mut local_thread, snapshot(echo.clone()), true);
        crate::chat::merge_thread_into(&mut local_thread, snapshot(uncertain.clone()), true);
        assert_eq!(local_thread.messages.len(), 1); // (INV-4)
        assert_eq!(
            local_thread.messages[0].delivery,
            DeliveryState::ServerAccepted
        );
        assert!(local_thread.messages[0].outgoing_job.is_none());

        crate::store::persist_threads(&mut store, &[snapshot(echo)]).unwrap();
        crate::store::persist_threads(&mut store, &[snapshot(uncertain.clone())]).unwrap();
        let mut restored = Vec::new();
        store.hydrate_threads(&mut restored, 20).unwrap();
        assert_eq!(restored[0].messages.len(), 1); // (INV-4)
        let reconciled = &restored[0].messages[0];
        assert_eq!(reconciled.client_message_id, uncertain.client_message_id);
        assert_eq!(reconciled.delivery, DeliveryState::ServerAccepted);
        assert!(reconciled.outgoing_job.is_none());
    }
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

    #[tokio::test]
    async fn e2ee_send_fails_closed_without_plaintext_downgrade_a01() {
        let session = Session::new(false, false, false).unwrap();
        let message = pending_e2ee("tin nhắn tối mật".into(), Payload::Text);

        // When sending to a peer without active session/network, E2EE send MUST fail-closed
        // and return an Err without falling back to plaintext.
        let result = send(&session, "non_existent_peer", &message).await;
        assert!(result.is_err(), "E2EE failure must fail-closed");
        let error = result.err().unwrap();
        assert_eq!(send_failure_kind(&error), SendFailureKind::NotSent);
        assert!(
            session.request_attempt_timings().is_empty(),
            "a logged-out encrypted send must never reach a plaintext endpoint"
        );
    }

    #[test]
    fn only_explicit_server_rejection_is_classified_as_rejected() {
        let rejected = anyhow::Error::new(crate::chat::ServerRejected { code: Some(5) });
        let missing_code = anyhow::Error::new(crate::chat::ServerRejected { code: None });
        let timeout = anyhow::anyhow!("timeout after send");
        assert_eq!(
            send_failure_kind(&rejected),
            SendFailureKind::ExplicitlyRejected
        );
        assert_eq!(send_failure_kind(&missing_code), SendFailureKind::Unknown);
        assert_eq!(send_failure_kind(&timeout), SendFailureKind::Unknown);
    }
}
