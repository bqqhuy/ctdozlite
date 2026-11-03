//! Shared, non-network presentation of persisted and live messages.
use crate::chat::{ChatMessage, Thread};
use serde_json::Value;

fn call_type(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "chat.call" | "chat.call.msg" | "call" | "voip" | "calllog" | "call_log"
    )
}
pub fn is_call_log(m: &ChatMessage) -> bool {
    if m.id.starts_with("call_")
        || m.wire_ref
            .original_type
            .as_ref()
            .and_then(Value::as_str)
            .is_some_and(call_type)
    {
        return true;
    }
    fn contains_call(v: &Value, depth: usize) -> bool {
        if depth > 5 {
            return false;
        }
        if v.get("msgType")
            .and_then(Value::as_str)
            .is_some_and(call_type)
            || v.get("call_type").is_some()
            || (v.get("callId").is_some()
                && (v.get("callState").is_some() || v.get("duration").is_some()))
        {
            return true;
        }
        if let Some(text) = v
            .as_str()
            .filter(|s| s.len() <= 65536 && s.trim_start().starts_with('{'))
        {
            return serde_json::from_str::<Value>(text)
                .ok()
                .is_some_and(|v| contains_call(&v, depth + 1));
        }
        for key in ["content", "params", "meta", "attachsList"] {
            if let Some(child) = v.get(key) {
                if let Some(a) = child.as_array() {
                    if a.iter().take(8).any(|v| contains_call(v, depth + 1)) {
                        return true;
                    }
                } else if contains_call(child, depth + 1) {
                    return true;
                }
            }
        }
        false
    }
    let is_plain_text = m
        .wire_ref
        .original_type
        .as_ref()
        .and_then(Value::as_str)
        .is_some_and(|s| s == "chat.text" || s == "text");
    if is_plain_text {
        return false;
    }
    m.history_source
        .as_ref()
        .is_some_and(|v| contains_call(v, 0))
        || m.wire_ref
            .fields
            .get("content")
            .is_some_and(|v| contains_call(v, 0))
}

fn json_value(v: Value) -> Value {
    if let Some(s) = v
        .as_str()
        .filter(|s| s.len() <= 65536 && s.trim_start().starts_with('{'))
    {
        serde_json::from_str(s).unwrap_or(Value::Null)
    } else {
        v
    }
}

pub fn remove_call_logs(t: &mut Thread) {
    let removed: Vec<_> = t
        .messages
        .iter()
        .filter(|m| is_call_log(m))
        .map(|m| m.id.clone())
        .collect();
    if removed.is_empty() && !t.presentation.last_message_id.starts_with("call_") {
        return;
    }
    let old = t.presentation.last_message_id.clone();
    t.messages.retain(|m| !is_call_log(m));
    for id in &removed {
        t.unread.unread_ids.remove(id);
        t.unread.mention_ids.remove(id);
    }
    if old.starts_with("call_")
        || removed.contains(&old)
        || t.preview.starts_with("📞")
        || t.preview.starts_with("📹")
    {
        t.presentation.last_message_ts = 0;
        t.presentation.last_message_id.clear();
        t.presentation.last_sender.clear();
        t.presentation.last_sender_id.clear();
        t.preview = t
            .messages
            .last()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        t.time = t
            .messages
            .last()
            .map(|m| m.display_time())
            .unwrap_or_default();
    }
    t.refresh_sidebar_presentation();
}

pub fn summary(m: &ChatMessage) -> String {
    if is_call_log(m) {
        return String::new();
    }
    if m.text == "[Tin nhắn đã thu hồi]" {
        return m.text.clone();
    }
    let media = media(m);
    match media.kind {
        MediaKind::Photo => "[Hình ảnh]".into(),
        MediaKind::Video => "[Video]".into(),
        MediaKind::File => format!("[Tệp] {}", media.name),
        MediaKind::None if m.sticker_id.is_some() => "[Nhãn dán]".into(),
        _ => m
            .text
            .split_whitespace()
            .take(128)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(512)
            .collect(),
    }
}
pub fn preview(t: &Thread) -> String {
    let last = t
        .messages
        .iter()
        .filter(|m| !is_call_log(m))
        .max_by_key(|m| m.ts);
    let (body, sender, outgoing) = if let Some(m) = last.filter(|m| {
        crate::clock::normalize_ts_ms(m.ts)
            >= crate::clock::normalize_ts_ms(t.presentation.last_message_ts)
    }) {
        (summary(m), m.sender.as_str(), m.is_outgoing)
    } else {
        (
            t.preview.clone(),
            t.presentation.last_sender.as_str(),
            t.presentation.last_outgoing,
        )
    };
    if body.is_empty() {
        return "Chưa có tin nhắn".into();
    }
    if outgoing {
        return format!("Bạn: {}", body.strip_prefix("Bạn: ").unwrap_or(&body));
    }
    if t.is_group {
        let sender = if sender.is_empty()
            || sender == "0"
            || sender.starts_with("phone:")
            || sender.bytes().all(|b| b.is_ascii_digit())
        {
            "Một thành viên"
        } else {
            sender
        };
        format!("{sender}: {body}")
    } else {
        body
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MediaKind {
    Photo,
    Video,
    File,
    #[default]
    None,
}
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Media {
    pub kind: MediaKind,
    pub thumbnail: Option<String>,
    pub preview: Option<String>,
    pub original: Option<String>,
    pub name: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<u64>,
}
fn url(v: &Value) -> Option<String> {
    let s = v.as_str()?;
    let u = reqwest::Url::parse(s).ok()?;
    (s.len() <= 16384
        && matches!(u.scheme(), "https" | "http")
        && u.username().is_empty()
        && u.password().is_none())
    .then(|| s.to_owned())
}
fn legacy_media(m: &ChatMessage) -> Option<(MediaKind, String)> {
    if m.sticker_id.is_some() || m.text.starts_with('[') {
        return None;
    }
    let raw = m.link_url.as_deref().unwrap_or(m.text.trim()).trim();
    if raw.len() > 16384 || raw.chars().any(char::is_whitespace) {
        return None;
    }
    let parsed = reqwest::Url::parse(raw).ok()?;
    if !crate::media_download::allowed(&parsed) || parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?;
    let on = |family: &str| {
        host == family || host.strip_suffix(family).is_some_and(|p| p.ends_with('.'))
    };
    let bare = m.text.trim() == raw;
    if on("dlfl.vn") {
        let plain = m
            .wire_ref
            .original_type
            .as_ref()
            .and_then(Value::as_str)
            .is_some_and(|s| matches!(s, "chat.text" | "text" | "webchat"));
        let filename = m.text.trim().rsplit_once('.').is_some_and(|(stem, ext)| {
            !stem.is_empty()
                && !ext.is_empty()
                && ext.len() <= 16
                && ext.bytes().all(|b| b.is_ascii_alphanumeric())
        }) && !m.text.contains(['\n', '\r', '/', '\\']);
        if bare
            || (!plain
                && matches!(
                    m.kind,
                    crate::message_model::MessageKind::Unknown
                        | crate::message_model::MessageKind::Link
                )
                && filename)
        {
            return Some((MediaKind::File, raw.into()));
        }
        return None;
    }
    if !bare {
        return None;
    }
    let path = parsed.path().to_ascii_lowercase();
    if path.ends_with(".mp4")
        || path.ends_with(".webm")
        || host.split('.').any(|label| label.starts_with("video-"))
    {
        return Some((MediaKind::Video, raw.into()));
    }
    if [".jpg", ".jpeg", ".png", ".webp", ".gif", ".bmp"]
        .iter()
        .any(|ext| path.ends_with(ext))
        || host.split('.').any(|label| label.starts_with("photo-"))
    {
        return Some((MediaKind::Photo, raw.into()));
    }
    None
}
pub fn media(m: &ChatMessage) -> Media {
    use crate::message_model::MessageKind;
    let legacy = legacy_media(m);
    let kind = if matches!(m.kind, MessageKind::Video)
        || m.link_src.as_deref() == Some("Video Zalo")
        || m.text.starts_with("[Video]")
    {
        MediaKind::Video
    } else if m.is_photo() {
        MediaKind::Photo
    } else if matches!(m.kind, MessageKind::File)
        || m.link_src.as_deref() == Some("Tập tin Zalo")
        || m.text.starts_with("[Tập tin]")
        || m.text.starts_with("[Tệp]")
    {
        MediaKind::File
    } else {
        legacy.as_ref().map_or(MediaKind::None, |v| v.0)
    };
    let mut out = Media {
        kind,
        thumbnail: m.link_thumb.clone().filter(|s| !s.is_empty()),
        original: m
            .link_url
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| legacy.as_ref().map(|v| v.1.clone())),
        name: m.link_title.clone().unwrap_or_default(),
        ..Default::default()
    };
    if kind == MediaKind::File && legacy.as_ref().is_some_and(|v| v.0 == MediaKind::File) {
        if reqwest::Url::parse(&out.name).is_ok() {
            out.name.clear();
        }
        if out.name.is_empty() && m.text.trim() != out.original.as_deref().unwrap_or_default() {
            out.name = m.text.trim().to_owned();
        }
    }
    if kind == MediaKind::None {
        return out;
    }
    if let Some(renewed) = m
        .wire_ref
        .fields
        .get("renewed_media")
        .and_then(|v| serde_json::from_value::<Media>(v.clone()).ok())
    {
        return renewed;
    }
    let mut values = Vec::new();
    if let Some(v) = &m.history_source {
        values.push(v.clone());
        if let Some(a) = v.pointer("/meta/attachsList").and_then(Value::as_array) {
            values.extend(a.iter().take(8).cloned());
        }
    }
    values.extend(m.wire_ref.fields.get("content").cloned());
    values.extend(m.wire_ref.fields.get("msgInfo").cloned());
    let mut thumb = None;
    let mut normal_thumbnail = None;
    let mut normal = None;
    let mut original = None;
    for v in values {
        let v = json_value(v);
        let params = v
            .get("params")
            .cloned()
            .map(json_value)
            .unwrap_or(Value::Null);
        let content = v
            .get("content")
            .cloned()
            .map(json_value)
            .unwrap_or(Value::Null);
        for v in [&v, &params, &content] {
            for k in ["thumbUrl", "thumb", "previewThumb"] {
                thumb = thumb.or_else(|| v.get(k).and_then(url));
            }
            normal_thumbnail = normal_thumbnail.or_else(|| v.get("normalUrl").and_then(url));
            for k in ["normalUrl", "rawUrl"] {
                normal = normal.or_else(|| v.get(k).and_then(url));
            }
            let keys: &[&str] = if kind == MediaKind::Video {
                &["videoUrl", "href", "url"]
            } else {
                &["hdUrl", "hd", "oriUrl", "href", "url", "fileUrl"]
            };
            for k in keys {
                original = original.or_else(|| v.get(*k).and_then(url));
            }
            if out.name.is_empty() {
                out.name = v
                    .get("fileName")
                    .or_else(|| v.get("title"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .chars()
                    .take(255)
                    .collect();
            }
            let number = |k| {
                v.get(k)
                    .and_then(|n| n.as_u64().or_else(|| n.as_str()?.parse().ok()))
            };
            out.width = out.width.or_else(|| {
                number("width")
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n > 0 && *n <= 65536)
            });
            out.height = out.height.or_else(|| {
                number("height")
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n > 0 && *n <= 65536)
            });
            out.duration = out.duration.or_else(|| number("duration"));
        }
    }
    out.thumbnail = thumb.or(out.thumbnail).or(normal_thumbnail);
    out.original = original.or(out.original).or_else(|| normal.clone());
    out.preview = normal
        .or_else(|| out.original.clone())
        .or_else(|| out.thumbnail.clone());
    if out.name.is_empty() {
        out.name = match kind {
            MediaKind::Video => "Video.mp4",
            MediaKind::Photo => "Hình ảnh.jpg",
            _ => "Tệp đính kèm",
        }
        .into();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn call_provenance_is_hidden_but_ordinary_text_is_preserved() {
        let mut m = ChatMessage {
            text: "Cuộc gọi".into(),
            ..Default::default()
        };
        assert!(!is_call_log(&m));
        m.id = "call_fixture".into();
        assert!(is_call_log(&m));
    }
    #[test]
    fn group_preview_uses_sender_of_the_actual_latest_message() {
        let t = Thread {
            is_group: true,
            messages: vec![ChatMessage {
                sender: "An".into(),
                text: "Xin chào".into(),
                ts: 1000,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(preview(&t), "An: Xin chào");
    }
    #[test]
    fn quality_aliases_are_kept_separate() {
        let mut m = ChatMessage {
            kind: crate::message_model::MessageKind::Photo,
            ..Default::default()
        };
        m.wire_ref.fields.insert("content".into(), serde_json::json!({"thumbUrl":"https://example.invalid/thumb","normalUrl":"https://example.invalid/preview","hdUrl":"https://example.invalid/original"}));
        let view = media(&m);
        assert!(view.thumbnail.unwrap().ends_with("/thumb"));
        assert!(view.preview.unwrap().ends_with("/preview"));
        assert!(view.original.unwrap().ends_with("/original"));
    }
}

#[cfg(test)]
mod legacy_media_tests {
    use super::*;
    #[test]
    fn old_video_url_and_filename_rows_become_media_without_rewriting_messages() {
        let source = "https://video-stal-44.dlmd.me/no/file";
        let m = ChatMessage {
            text: source.into(),
            link_url: Some(source.into()),
            ..Default::default()
        };
        let view = media(&m);
        assert_eq!(view.kind, MediaKind::Video);
        assert_eq!(view.original.as_deref(), Some(source));
        assert_eq!(m.text, source);
        let file = ChatMessage {
            text: "Báo cáo.doc".into(),
            link_url: Some("https://file-stal-3.dlfl.vn/file/renewMedia.doc".into()),
            ..Default::default()
        };
        let view = media(&file);
        assert_eq!(view.kind, MediaKind::File);
        assert_eq!(view.name, "Báo cáo.doc");
    }
    #[test]
    fn ordinary_text_and_lookalike_hosts_are_not_legacy_media() {
        let mut m = ChatMessage {
            text: "Gửi bạn báo cáo.doc".into(),
            link_url: Some("https://file-stal-3.dlfl.vn/file".into()),
            kind: crate::message_model::MessageKind::Text,
            ..Default::default()
        };
        assert_eq!(media(&m).kind, MediaKind::None);
        for raw in [
            "https://video-stal-44.dlmd.me.evil.invalid/a",
            "https://evildlfl.vn/a",
            "https://u:p@video-stal-44.dlmd.me/a",
        ] {
            m.text = raw.into();
            m.link_url = Some(raw.into());
            assert_eq!(media(&m).kind, MediaKind::None);
        }
    }
}
