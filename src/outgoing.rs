//! Durable outgoing payloads; retries reuse the original client identity.
use crate::{chat::ChatMessage, message_model::DeliveryState, session::Session};
use anyhow::Result;
use serde::{Deserialize, Serialize};
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
    },
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
        Payload::Forward(original) if original.sticker_cate_id == Some(crate::bundled_stickers::CATEGORY) => {
            sent=send_bundled(s,to,job.is_group,original.sticker_id.ok_or_else(||anyhow::anyhow!("Thiếu nhãn GIF"))?,id).await?;
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
            sent=send_bundled(s,to,job.is_group,*sticker,id).await?;
            serde_json::Value::Null
        }
        Payload::Sticker {id:sticker,category,kind} => {
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
        } => {
            if let Some(stamp) = stamp {
                anyhow::ensure!(
                    *stamp == FileStamp::read(path).await?,
                    "Tệp gốc đã thay đổi. Hãy chọn và gửi như một tệp mới"
                );
            }
            let (_, result) =
                crate::chat::send_attachment_with_client_id(s, to, job.is_group, path, caption, id)
                    .await?;
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

/// Keep the actual attachment response and reuse the original outgoing client identity.
async fn send_bundled(s:&Session,to:&str,group:bool,sticker:u64,client_id:u64)->Result<ChatMessage> {
    let path=crate::bundled_stickers::stage(s,sticker).await?;
    let (_,mut sent)=crate::chat::send_attachment_with_client_id(s,to,group,&path,"",client_id).await?;
    sent.sticker_id=Some(sticker);sent.sticker_cate_id=Some(crate::bundled_stickers::CATEGORY);sent.sticker_type=Some(7);
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
