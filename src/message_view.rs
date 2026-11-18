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
    // Call history is persisted and restored as ordinary message rows. The helper
    // remains for source compatibility with older callers; it must not delete data.
    let _ = t;
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
fn is_valid_sender_name(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty()
        || t == "0"
        || t == "Một thành viên"
        || t.starts_with("phone:")
        || t.bytes().all(|b| b.is_ascii_digit())
    {
        return false;
    }
    for prefix in ["Người dùng ", "Thành viên #"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let r = rest.trim();
            if !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
        }
    }
    true
}

fn resolve_group_sender<'a>(
    t: &'a Thread,
    sender: &'a str,
    sender_uid: Option<&str>,
    resolve_name: &impl Fn(&str) -> Option<&'a str>,
) -> Option<&'a str> {
    if let Some(uid) = sender_uid.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(name) = resolve_name(uid)
            .map(str::trim)
            .filter(|n| is_valid_sender_name(n))
        {
            return Some(name);
        }
        if let Some(member) = t
            .presentation
            .members
            .iter()
            .find(|m| m.id == uid && is_valid_sender_name(&m.name))
        {
            return Some(member.name.trim());
        }
    }
    let trimmed_sender = sender.trim();
    if !trimmed_sender.is_empty() {
        if let Some(name) = resolve_name(trimmed_sender)
            .map(str::trim)
            .filter(|n| is_valid_sender_name(n))
        {
            return Some(name);
        }
        if let Some(member) = t
            .presentation
            .members
            .iter()
            .find(|m| m.id == trimmed_sender && is_valid_sender_name(&m.name))
        {
            return Some(member.name.trim());
        }
    }
    if is_valid_sender_name(trimmed_sender) {
        return Some(trimmed_sender);
    }
    if let Some(uid) = sender_uid.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(prev) = t.messages.iter().rev().find(|m| {
            crate::chat::sidebar_sender_id(m).as_deref() == Some(uid)
                && is_valid_sender_name(&m.sender)
        }) {
            return Some(prev.sender.trim());
        }
    }
    None
}

pub fn preview(t: &Thread) -> String {
    preview_with_reactor(t, None)
}
/// Resolve a reactor profile without changing the message-summary fallback.
pub fn preview_with_reactor(t: &Thread, reactor_name: Option<&str>) -> String {
    preview_with_resolver(t, reactor_name, |_| None)
}

/// Format sidebar preview text, using `resolve_name(uid)` before thread-local member/message fallbacks.
pub fn preview_with_resolver<'a>(
    t: &'a Thread,
    reactor_name: Option<&str>,
    resolve_name: impl Fn(&str) -> Option<&'a str>,
) -> String {
    let newest = t
        .messages
        .iter()
        .map(|m| crate::clock::normalize_ts_ms(m.ts))
        .chain(std::iter::once(crate::clock::normalize_ts_ms(
            t.presentation.last_message_ts,
        )))
        .max()
        .unwrap_or(0);
    if let Some(reaction) = t
        .presentation
        .reaction_preview
        .as_ref()
        .filter(|reaction| reaction.ts >= newest)
    {
        let resolved_reactor = reactor_name
            .map(str::trim)
            .filter(|name| is_valid_sender_name(name))
            .or_else(|| {
                resolve_name(&reaction.reactor)
                    .map(str::trim)
                    .filter(|name| is_valid_sender_name(name))
            });
        let has_member = t
            .presentation
            .members
            .iter()
            .any(|m| m.id == reaction.reactor && is_valid_sender_name(&m.name));
        let fallback = if t.is_group
            && !reaction.by_self
            && resolved_reactor.is_none()
            && !has_member
            && !is_valid_sender_name(&reaction.reactor)
        {
            Some("Thành viên")
        } else {
            resolved_reactor
        };
        return t.sidebar_preview_with_reactor(fallback);
    }
    let last = t
        .messages
        .iter()
        .filter(|m| !is_call_log(m))
        .max_by_key(|m| m.ts);
    let last_valid = last.filter(|m| {
        crate::clock::normalize_ts_ms(m.ts)
            >= crate::clock::normalize_ts_ms(t.presentation.last_message_ts)
    });
    let (body, sender, sender_uid, outgoing) = if let Some(m) = last_valid {
        (
            summary(m),
            m.sender.as_str(),
            crate::chat::sidebar_sender_id(m),
            m.is_outgoing,
        )
    } else {
        (
            t.preview.clone(),
            t.presentation.last_sender.as_str(),
            (!t.presentation.last_sender_id.is_empty())
                .then(|| t.presentation.last_sender_id.clone()),
            t.presentation.last_outgoing,
        )
    };
    if body.is_empty() {
        return "Chưa có tin nhắn".into();
    }
    let preview_id = last_valid
        .map(|m| m.id.as_str())
        .unwrap_or(&t.presentation.last_message_id);
    let mentioned = !outgoing && t.unread.mention_ids.contains(preview_id);
    let body = if mentioned {
        format!("[Nhắc đến bạn] {body}")
    } else {
        body
    };
    if outgoing {
        return format!("Bạn: {}", body.strip_prefix("Bạn: ").unwrap_or(&body));
    }
    if t.is_group {
        if let Some(name) = resolve_group_sender(t, sender, sender_uid.as_deref(), &resolve_name) {
            let prefix = format!("{name}: ");
            if body.starts_with(&prefix) {
                body
            } else {
                format!("{prefix}{body}")
            }
        } else {
            body
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCategory {
    Pdf,
    Word,
    PowerPoint,
    Excel,
    Cad,
    Other,
}

pub fn file_category(name: &str) -> FileCategory {
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.trim().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => FileCategory::Pdf,
        "doc" | "docx" => FileCategory::Word,
        "ppt" | "pptx" => FileCategory::PowerPoint,
        "xls" | "xlsx" => FileCategory::Excel,
        "dwg" | "dxf" => FileCategory::Cad,
        _ => FileCategory::Other,
    }
}

pub fn file_category_label(category: FileCategory) -> &'static str {
    match category {
        FileCategory::Pdf => "PDF",
        FileCategory::Word => "Word",
        FileCategory::PowerPoint => "PowerPoint",
        FileCategory::Excel => "Excel",
        FileCategory::Cad => "CAD",
        FileCategory::Other => "Tệp",
    }
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

const MEDIA_URL_EXTENSIONS: &[&str] = &[
    ".jpg", ".jpeg", ".png", ".gif", ".webp", ".mp4", ".mov", ".avi", ".m4v", ".webm", ".jxl",
];
const VIDEO_URL_EXTENSIONS: &[&str] = &[".mp4", ".mov", ".avi", ".m4v", ".webm"];
const NON_MEDIA_DOCUMENT_EXTENSIONS: &[&str] = &[
    ".html", ".htm", ".xhtml", ".xlsx", ".xls", ".docx", ".doc", ".pptx", ".ppt", ".pdf", ".zip",
    ".rar", ".7z", ".tar", ".gz", ".txt", ".json", ".csv", ".xml",
];

/// Confirmed Zalo video CDN hosts from evidence and re-notes.
fn is_confirmed_zalo_video_host(host: &str) -> bool {
    let is_zalo_family = host == "zdn.vn"
        || host.ends_with(".zdn.vn")
        || host == "zadn.vn"
        || host.ends_with(".zadn.vn")
        || host.ends_with(".dlmd.me")
        || host.ends_with(".zalo.me")
        || host.ends_with(".zaloapp.com");
    if !is_zalo_family {
        return false;
    }
    host == "video.zdn.vn"
        || host.ends_with(".video.zdn.vn")
        || host == "video.zadn.vn"
        || host.ends_with(".video.zadn.vn")
        || host
            .split('.')
            .any(|label| label == "video" || label.starts_with("video-"))
}

/// Confirmed Zalo photo CDN hosts from evidence and re-notes.
fn is_confirmed_zalo_photo_host(host: &str) -> bool {
    let is_zalo_family = host == "zdn.vn"
        || host.ends_with(".zdn.vn")
        || host == "zadn.vn"
        || host.ends_with(".zadn.vn")
        || host.ends_with(".dlmd.me")
        || host.ends_with(".zalo.me")
        || host.ends_with(".zaloapp.com");
    if !is_zalo_family {
        return false;
    }
    host == "photo.zdn.vn"
        || host.ends_with(".photo.zdn.vn")
        || host == "photo.zadn.vn"
        || host.ends_with(".photo.zadn.vn")
        || host == "zpcloud.zdn.vn"
        || host.split('.').any(|label| {
            label == "photo"
                || label.starts_with("photo-")
                || label.ends_with("-zpc")
                || label == "stc-chat"
        })
}

/// Return true when `url` refers to a video file extension or a confirmed Zalo video CDN host.
pub fn is_video_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url.trim()) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return false;
    }
    let host = match parsed.host_str() {
        Some(h) if !h.is_empty() => h.to_ascii_lowercase(),
        _ => return false,
    };
    let path = parsed.path().to_ascii_lowercase();
    if NON_MEDIA_DOCUMENT_EXTENSIONS
        .iter()
        .any(|ext| path.ends_with(ext))
    {
        return false;
    }
    if VIDEO_URL_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
        return true;
    }
    is_confirmed_zalo_video_host(&host)
}

/// Return true when `url` refers to an image/video file extension or an evidence-backed Zalo media CDN URL.
pub fn is_media_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url.trim()) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return false;
    }
    let host = match parsed.host_str() {
        Some(h) if !h.is_empty() => h.to_ascii_lowercase(),
        _ => return false,
    };
    let path = parsed.path().to_ascii_lowercase();

    // 1. Explicit negative check: document, web page or archive extensions are never media
    if NON_MEDIA_DOCUMENT_EXTENSIONS
        .iter()
        .any(|ext| path.ends_with(ext))
    {
        return false;
    }

    // 2. Explicit positive check: confirmed image or video file extension
    if MEDIA_URL_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
        return true;
    }

    // 3. Confirmed Zalo photo or video CDN hosts from evidence/re-notes (can serve opaque paths)
    is_confirmed_zalo_video_host(&host) || is_confirmed_zalo_photo_host(&host)
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
/// Explicit photo-bundle metadata; nearby timestamps alone never establish an album.
/// Source: PC 26.11.10 compact-app-pc.js, photoBundle.groupLayoutId/totalItemInGroup/idInGroup.
pub(crate) fn photo_bundle(message: &ChatMessage) -> Option<(String, u32)> {
    if media(message).kind != MediaKind::Photo {
        return None;
    }
    fn inspect(value: &Value, depth: u8) -> Option<(String, u32)> {
        if depth > 4 {
            return None;
        }
        let value = json_value(value.clone());
        let id = value.get("groupLayoutId").and_then(|v| {
            v.as_str()
                .map(str::to_owned)
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        });
        if let (Some(id), Some(total)) = (id, value.get("totalItemInGroup").and_then(Value::as_u64))
        {
            if !id.is_empty() && id.len() <= 128 && (2..=100).contains(&total) {
                return Some((id, total as u32));
            }
        }
        for key in ["photoBundle", "params", "content", "data", "meta"] {
            if let Some(found) = value.get(key).and_then(|v| inspect(v, depth + 1)) {
                return Some(found);
            }
        }
        None
    }
    message
        .wire_ref
        .fields
        .get("content")
        .and_then(|v| inspect(v, 0))
        .or_else(|| {
            message
                .wire_ref
                .fields
                .get("photoBundle")
                .and_then(|v| inspect(v, 0))
        })
        .or_else(|| message.history_source.as_ref().and_then(|v| inspect(v, 0)))
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
    fn reaction_activity_replaces_preview_until_a_newer_message_arrives() {
        let mut thread = Thread {
            name: "An".into(),
            ..Default::default()
        };
        thread.presentation.reaction_preview = Some(crate::reactions::Preview {
            reactor: "peer".into(),
            emoji: "❤️".into(),
            target: "m1".into(),
            event_id: "r1".into(),
            ts: 1_800_000_000_000,
            by_self: false,
            target_by_self: true,
        });
        assert!(preview(&thread).contains("đã thả"));
        thread.messages.push(ChatMessage {
            text: "Tin mới".into(),
            ts: 1_800_000_001_000,
            ..Default::default()
        });
        assert!(!preview(&thread).contains("đã thả"));
        assert!(preview(&thread).contains("Tin mới"));
    }
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
    fn call_history_cleanup_is_non_destructive() {
        let call = ChatMessage {
            id: "call_fixture".into(),
            text: "📞 Cuộc gọi đi (0s)".into(),
            ..Default::default()
        };
        let mut thread = Thread {
            messages: vec![call],
            ..Default::default()
        };
        remove_call_logs(&mut thread);
        assert_eq!(thread.messages.len(), 1);
        assert_eq!(thread.messages[0].id, "call_fixture");
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
    fn group_preview_never_shows_mot_thanh_vien_or_raw_numeric_uid() {
        for raw_sender in [
            "",
            "0",
            "84912345678",
            "1234567890123456",
            "phone:0901234567",
            "Một thành viên",
            "Người dùng 123456",
        ] {
            let t = Thread {
                is_group: true,
                messages: vec![ChatMessage {
                    sender: raw_sender.into(),
                    text: "Họp lúc 9h nhé".into(),
                    ts: 2000,
                    ..Default::default()
                }],
                ..Default::default()
            };
            let out = preview(&t);
            assert_eq!(out, "Họp lúc 9h nhé", "raw_sender={raw_sender:?}");
            assert!(!out.contains("Một thành viên"));
        }

        // Outgoing group message always formats as "Bạn: <body/>".
        let outgoing_thread = Thread {
            is_group: true,
            messages: vec![ChatMessage {
                sender: "84912345678".into(),
                text: "Ok bạn".into(),
                is_outgoing: true,
                ts: 3000,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(preview(&outgoing_thread), "Bạn: Ok bạn");
    }
    #[test]
    fn group_preview_resolves_numeric_uid_from_members_messages_or_resolver() {
        let mut m = ChatMessage {
            sender: "99887766".into(),
            text: "Đã gửi file".into(),
            ts: 4000,
            ..Default::default()
        };
        m.wire_ref.sender = Some(serde_json::json!("99887766"));

        let mut with_member = Thread {
            is_group: true,
            messages: vec![m.clone()],
            ..Default::default()
        };
        with_member
            .presentation
            .members
            .push(crate::chat::SidebarMember {
                id: "99887766".into(),
                name: "Minh Anh".into(),
                ..Default::default()
            });
        assert_eq!(preview(&with_member), "Minh Anh: Đã gửi file");

        // External resolver (e.g. server_aliases / contacts) takes precedence over member cache.
        let via_resolver = preview_with_resolver(&with_member, None, |uid| {
            (uid == "99887766").then_some("Em Minh Anh (Cty)")
        });
        assert_eq!(via_resolver, "Em Minh Anh (Cty): Đã gửi file");

        // Falls back to an earlier resolved message from the same UID if member list is empty.
        let mut earlier = ChatMessage {
            sender: "Tuấn".into(),
            text: "Alo".into(),
            ts: 3500,
            ..Default::default()
        };
        earlier.wire_ref.sender = Some(serde_json::json!("99887766"));
        let with_earlier = Thread {
            is_group: true,
            messages: vec![earlier, m],
            ..Default::default()
        };
        assert_eq!(preview(&with_earlier), "Tuấn: Đã gửi file");

        // Group reaction preview with unresolved numeric UID must not leak raw UID or "Một thành viên".
        let mut rx_thread = Thread {
            is_group: true,
            ..Default::default()
        };
        rx_thread.presentation.reaction_preview = Some(crate::reactions::Preview {
            reactor: "7766554433".into(),
            emoji: "👍".into(),
            target: "m1".into(),
            event_id: "r1".into(),
            ts: 5000,
            by_self: false,
            target_by_self: true,
        });
        let rx_out = preview(&rx_thread);
        assert!(!rx_out.contains("7766554433"));
        assert!(!rx_out.contains("Một thành viên"));
        assert!(rx_out.contains("đã thả"));
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

#[cfg(test)]
mod photo_bundle_tests {
    use super::*;
    #[test]
    fn albums_require_explicit_group_identity() {
        let mut message = ChatMessage {
            kind: crate::message_model::MessageKind::Photo,
            ..Default::default()
        };
        assert!(photo_bundle(&message).is_none());
        message.wire_ref.fields.insert("content".into(),serde_json::json!({"params":{"photoBundle":{"groupLayoutId":"bundle-a","totalItemInGroup":3,"idInGroup":0}}}));
        assert_eq!(photo_bundle(&message), Some(("bundle-a".into(), 3)));
        message.kind = crate::message_model::MessageKind::Video;
        assert!(photo_bundle(&message).is_none());
    }
}

#[cfg(test)]
mod media_url_tests {
    use super::*;

    #[test]
    fn recognizes_zalo_cdn_and_media_extensions() {
        // Positive media tests: verified image/video CDN and explicit media extensions
        for media_url in [
            "https://video.zdn.vn/opaque-video-stream",
            "https://photo.zdn.vn/opaque-photo-token",
            "https://photo-stal-16.zdn.vn/w480/abc123",
            "https://f10-zpc.zdn.vn/image?t=1",
            "https://b-f10-zpc.zdn.vn/orig/1",
            "https://cdn.zadn.vn/media/clip.mp4",
            "https://example.com/assets/photo.JPG?size=large",
            "https://example.com/clip.mov",
            "https://example.com/clip.avi",
            "https://example.com/clip.m4v",
            "https://example.com/anim.gif",
            "https://example.com/banner.webp",
        ] {
            assert!(is_media_url(media_url), "expected media url: {media_url}");
        }

        // Negative media tests: documents, web pages, third-party arbitrary paths
        for non_media_url in [
            "https://file.zdn.vn/report.xlsx",
            "https://cdn.zadn.vn/page.html",
            "https://evil.example/photo/page",
            "https://vnexpress.net/tin-tuc-12345.html",
            "https://chat.zalo.me/",
            "https://fake-zdn.vn.evil.com/page",
            "ftp://photo.zdn.vn/test.jpg",
            "https://user:pass@photo.zdn.vn/test.jpg",
            "not-a-url",
            "",
        ] {
            assert!(
                !is_media_url(non_media_url),
                "expected non-media url: {non_media_url}"
            );
        }

        // Video URL tests
        // 1. video.zdn.vn with opaque path returns true
        assert!(is_video_url("https://video.zdn.vn/opaque-video-stream"));
        assert!(is_video_url("https://video-stal-44.dlmd.me/stream"));
        assert!(is_video_url("https://cdn.zadn.vn/clip.mp4"));
        assert!(is_video_url("https://example.com/stream.webm"));

        // 2. photo.zdn.vn with opaque path does NOT automatically become video
        assert!(!is_video_url("https://photo.zdn.vn/opaque-photo-stream"));
        assert!(!is_video_url("https://photo-stal-16.zdn.vn/w480/abc.jpg"));

        // 3. invalid scheme or credentials return false
        assert!(!is_video_url("ftp://video.zdn.vn/clip.mp4"));
        assert!(!is_video_url("https://user:pass@video.zdn.vn/clip.mp4"));
        assert!(!is_video_url("https://file.zdn.vn/report.xlsx"));
        assert!(!is_video_url("not-a-url"));
    }

    #[test]
    fn file_category_handles_known_extensions_case_insensitively() {
        assert_eq!(file_category("report.PDF"), FileCategory::Pdf);
        assert_eq!(file_category("brief.docx"), FileCategory::Word);
        assert_eq!(file_category("slides.PPT"), FileCategory::PowerPoint);
        assert_eq!(file_category("budget.XLSX"), FileCategory::Excel);
        assert_eq!(file_category("plan.DWG"), FileCategory::Cad);
        assert_eq!(file_category("archive.zip"), FileCategory::Other);
    }
}
