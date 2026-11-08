//! Chat data: friend/conversation list + last messages, all over REST.
//!
//! Notes: `re-notes/02_chat_data.md`.

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::session::{obj, Session};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outgoing_job: Option<Box<crate::outgoing::Job>>,
    #[serde(default)]
    pub kind: crate::message_model::MessageKind,
    #[serde(default)]
    pub wire_ref: crate::message_model::WireMessageRef,
    #[serde(default)]
    pub unread_policy: crate::message_model::UnreadPolicy,
    #[serde(default)]
    pub delivery: crate::message_model::DeliveryState,
    pub id: String,
    pub sender: String,
    pub text: String,
    pub time: String,
    pub is_outgoing: bool,
    pub ts: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker_cate_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker_type: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_sender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reaction_summaries: Vec<crate::reactions::ReactionSummary>,
    #[serde(default)]
    pub reaction_snapshot: bool,
    #[serde(default)]
    pub reaction_authoritative_snapshot: bool,
    #[serde(skip)]
    pub reaction_snapshot_fresh: bool,
    #[serde(default)]
    pub reaction_snapshot_provenance: crate::reactions::ReactionSnapshotProvenance,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub reaction_versions: std::collections::BTreeMap<String, crate::reactions::ReactionVersion>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_message_id: String,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub is_starred: bool,
    /// Display-only rich-text spans over `text` (UTF-16 offsets); `text` itself is unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_styles: Vec<crate::rich_text::TextStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_thumb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_source: Option<Value>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub resolved_mention_uids: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_phone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_avatar: Option<String>,
    /// WP16-3: E2EE status flag for message decryption indicator
    #[serde(default)]
    pub is_e2ee: bool,
}

impl ChatMessage {
    /// Durable wire/receipt/reaction metadata, separate from searchable message columns.
    pub(crate) fn durable_state(&self) -> Value {
        json!({"kind":self.kind,"wire_ref":self.wire_ref,"unread_policy":self.unread_policy,
            "outgoing_job":self.outgoing_job,"delivery":self.delivery,"client_message_id":self.client_message_id,
            "reaction_summaries":self.reaction_summaries,"reaction_snapshot":self.reaction_snapshot,
            "reaction_authoritative_snapshot":self.reaction_authoritative_snapshot,
            "reaction_snapshot_provenance":self.reaction_snapshot_provenance,
            "reaction_versions":self.reaction_versions,"history_source":self.history_source,
            "resolved_mention_uids":self.resolved_mention_uids})
    }

    /// Merge only known metadata keys; persisted content cannot overwrite row identity.
    /// # Errors
    /// Returns malformed metadata errors without modifying the message.
    pub(crate) fn restore_state(&mut self, state: &Value) -> Result<()> {
        anyhow::ensure!(state.is_object(), "Invalid message metadata");
        let mut candidate = serde_json::to_value(&*self)?;
        for key in [
            "kind",
            "wire_ref",
            "unread_policy",
            "delivery",
            "outgoing_job",
            "client_message_id",
            "reaction_summaries",
            "reaction_snapshot",
            "reaction_authoritative_snapshot",
            "reaction_snapshot_provenance",
            "reaction_versions",
            "history_source",
            "resolved_mention_uids",
        ] {
            if let Some(value) = state.get(key) {
                candidate[key] = value.clone();
            }
        }
        let restored: Self = serde_json::from_value(candidate)?;
        let previous_attempt = self
            .outgoing_job
            .as_ref()
            .map(|job| job.attempt)
            .unwrap_or(0);
        self.merge_reaction_metadata(&restored);
        if matches!(
            self.delivery,
            crate::message_model::DeliveryState::ServerAccepted
                | crate::message_model::DeliveryState::Delivered
                | crate::message_model::DeliveryState::Read
        ) {
            self.outgoing_job = None;
        } else if let Some(job) = restored.outgoing_job {
            if job.attempt >= previous_attempt {
                if job.attempt > previous_attempt {
                    self.delivery = restored.delivery;
                }
                self.outgoing_job = Some(job);
            }
        }
        if self.outgoing_job.is_some()
            && self.delivery == crate::message_model::DeliveryState::Pending
            && !crate::outgoing::is_active(&self.client_message_id)
        {
            self.delivery = crate::message_model::DeliveryState::Failed;
            if let Some(job) = self.outgoing_job.as_mut() {
                job.error = "Lần gửi trước bị gián đoạn; kiểm tra cuộc trò chuyện trước khi gửi lại để tránh trùng tin".into();
            }
        }
        Ok(())
    }

    /// Remove recalled content while preserving the original routing/message identifiers.
    pub(crate) fn mark_recalled(&mut self) {
        self.outgoing_job = None;
        self.text = "[Tin nhắn đã thu hồi]".into();
        self.kind = crate::message_model::MessageKind::Recall;
        self.sticker_id = None;
        self.sticker_cate_id = None;
        self.sticker_type = None;
        self.link_url = None;
        self.link_title = None;
        self.link_desc = None;
        self.link_thumb = None;
        self.link_src = None;
        self.reply_to_text = None;
        self.reply_to_sender = None;
        self.reply_to_id = None;
        self.history_source = None;
        self.resolved_mention_uids.clear();
        self.wire_ref.fields.retain(|key, _| {
            [
                "msgId",
                "globalMsgId",
                "lastMsgId",
                "id",
                "cliMsgId",
                "clientId",
                "uidFrom",
                "fromUid",
                "fromId",
                "idTo",
                "toId",
                "grid",
                "groupId",
                "ts",
            ]
            .contains(&key.as_str())
        });
    }

    pub fn merge_message_metadata(&mut self, incoming: &Self) -> bool {
        let previous_mentions = self.resolved_mention_uids.clone();
        self.resolved_mention_uids
            .extend(incoming.resolved_mention_uids.clone());
        let before = (
            self.kind,
            self.wire_ref.clone(),
            self.unread_policy.clone(),
            self.delivery,
            self.history_source.clone(),
        );
        if incoming.kind != crate::message_model::MessageKind::Unknown
            && self.kind != crate::message_model::MessageKind::Recall
        {
            self.kind = incoming.kind;
        }
        if !incoming.wire_ref.fields.is_empty() {
            let mut fields = self.wire_ref.fields.clone();
            fields.extend(incoming.wire_ref.fields.clone());
            self.wire_ref = crate::message_model::WireMessageRef::from_raw(&Value::Object(
                fields.into_iter().collect(),
            ));
            if incoming.unread_policy.params_ext_present {
                self.unread_policy = incoming.unread_policy.clone();
            }
        }
        if incoming.history_source.is_some() {
            self.history_source = incoming.history_source.clone();
        }
        self.delivery = self.delivery.merge(incoming.delivery);
        if self.outgoing_job.is_some()
            && incoming.is_outgoing
            && !self.client_message_id.is_empty()
            && self.client_message_id == incoming.client_message_id
            && server_message_id(incoming).is_some()
        {
            self.delivery = self
                .delivery
                .merge(crate::message_model::DeliveryState::ServerAccepted);
            self.outgoing_job = None;
        }
        previous_mentions != self.resolved_mention_uids
            || before
                != (
                    self.kind,
                    self.wire_ref.clone(),
                    self.unread_policy.clone(),
                    self.delivery,
                    self.history_source.clone(),
                )
    }

    /// Keep aggregate counts and event replay cursors across cache/network merges.
    pub fn merge_reaction_metadata(&mut self, incoming: &Self) -> bool {
        let model_changed = self.merge_message_metadata(incoming);
        let previous_provenance = self.reaction_snapshot_provenance.clone();
        let before = (
            self.reaction_summaries.clone(),
            self.reactions.clone(),
            self.client_message_id.clone(),
            self.reaction_snapshot,
            self.reaction_authoritative_snapshot,
            self.reaction_versions.clone(),
        );
        if self.client_message_id.is_empty() {
            self.client_message_id = incoming.client_message_id.clone();
        }
        let accept_snapshot = incoming.reaction_authoritative_snapshot
            && (incoming.reaction_snapshot_fresh || !self.reaction_authoritative_snapshot)
            && self.reaction_snapshot_provenance.accepts_snapshot(
                &incoming.reaction_snapshot_provenance,
                &self.reaction_versions,
                incoming.reaction_snapshot_fresh,
            );

        if accept_snapshot {
            self.reaction_snapshot_provenance = incoming.reaction_snapshot_provenance.clone();
            self.reaction_summaries = incoming.reaction_summaries.clone();
            self.reaction_authoritative_snapshot = true;
            self.reaction_snapshot = true;
            for (user, version) in &mut self.reaction_versions {
                version.covered_events.append(&mut version.events);
                version.baseline = incoming
                    .reaction_summaries
                    .iter()
                    .filter_map(|s| Some((s.r_type?, *s.reactor_counts.get(user)?)))
                    .collect();
                version.ledger_initialized = true;
                version.snapshot_baseline = true;
            }
            for (user, incoming_version) in &incoming.reaction_versions {
                let covered = self
                    .reaction_versions
                    .get(user)
                    .map(|v| v.covered_events.clone())
                    .unwrap_or_default();
                let mut version = incoming_version.clone();
                version.covered_events.extend(covered);
                if incoming.reaction_snapshot_fresh {
                    version.covered_events.append(&mut version.events);
                    version.baseline = incoming
                        .reaction_summaries
                        .iter()
                        .filter_map(|s| Some((s.r_type?, *s.reactor_counts.get(user)?)))
                        .collect();
                    version.ledger_initialized = true;
                    version.snapshot_baseline = true;
                }
                self.reaction_versions.insert(user.clone(), version);
            }
        }
        if !self.reaction_authoritative_snapshot
            && !incoming.reaction_authoritative_snapshot
            && (incoming.reaction_snapshot || !incoming.reaction_summaries.is_empty())
            && self.reaction_versions.is_empty()
        {
            self.reaction_summaries = incoming.reaction_summaries.clone();
            self.reactions = crate::reactions::legacy_emojis(&self.reaction_summaries);
            self.reaction_snapshot = incoming.reaction_snapshot;
        }
        if !self.reaction_authoritative_snapshot
            && !incoming.reaction_authoritative_snapshot
            && !self.reaction_versions.is_empty()
            && (incoming.reaction_snapshot || !incoming.reaction_summaries.is_empty())
        {
            crate::reactions::merge_untracked_snapshot(
                &mut self.reaction_summaries,
                &incoming.reaction_summaries,
                &self.reaction_versions,
                incoming.reaction_snapshot && incoming.reaction_versions.is_empty(),
            );
        }
        for (user, version) in &incoming.reaction_versions {
            if accept_snapshot {
                continue;
            }
            if !self.reaction_versions.contains_key(user) {
                if self.reaction_authoritative_snapshot {
                    for event in &version.events {
                        if event.r_type >= -1 {
                            self.apply_reaction(event);
                        }
                    }
                    continue;
                }
                self.reaction_versions.insert(user.clone(), version.clone());
                if version.ledger_initialized {
                    version.rebuild(&mut self.reaction_summaries, user);
                }
            } else {
                for event in &version.events {
                    if event.r_type >= -1 {
                        self.apply_reaction(event);
                    }
                }
            }
        }
        if !self.reaction_summaries.is_empty()
            || incoming.reaction_snapshot
            || !incoming.reaction_versions.is_empty()
        {
            self.reactions = crate::reactions::legacy_emojis(&self.reaction_summaries);
        } else if self.reactions.is_empty()
            && self.reaction_versions.is_empty()
            && !self.reaction_snapshot
        {
            self.reactions = incoming.reactions.clone();
        }
        model_changed
            || previous_provenance != self.reaction_snapshot_provenance
            || before
                != (
                    self.reaction_summaries.clone(),
                    self.reactions.clone(),
                    self.client_message_id.clone(),
                    self.reaction_snapshot,
                    self.reaction_authoritative_snapshot,
                    self.reaction_versions.clone(),
                )
    }

    pub fn apply_reaction(&mut self, event: &crate::reactions::ReactionEvent) -> bool {
        if event.sender_id.is_empty() || event.r_type < -1 {
            return false;
        }
        if self.reaction_summaries.is_empty() && !self.reactions.is_empty() {
            self.reaction_summaries =
                crate::reactions::parse_reactions(&json!({"reactions":self.reactions}));
        }
        let changed = self
            .reaction_versions
            .entry(event.sender_id.clone())
            .or_default()
            .record(
                &mut self.reaction_summaries,
                event,
                self.reaction_authoritative_snapshot,
            );
        self.reactions = crate::reactions::legacy_emojis(&self.reaction_summaries);
        self.reaction_snapshot = true;
        changed
    }

    /// Derive visible time from the epoch so stale cached UTC strings cannot leak into the UI.
    pub fn display_time(&self) -> String {
        let time = crate::clock::message_time(self.ts);
        if time.is_empty() {
            self.time.clone()
        } else {
            time
        }
    }
    /// Recognize actual photo messages and direct image files on trusted Zalo photo CDNs.
    pub fn is_photo(&self) -> bool {
        if self.kind == crate::message_model::MessageKind::Recall
            || self.text == "[Tin nhắn đã thu hồi]"
        {
            return false;
        }
        self.kind == crate::message_model::MessageKind::Photo
            || self.link_src.as_deref() == Some("Hình ảnh Zalo")
            || self.text.trim().starts_with("[Hình ảnh]")
            || self.link_url.as_deref().is_some_and(direct_zalo_photo)
    }
    /// Suppress transport URLs while retaining a real photo caption.
    pub fn photo_caption(&self) -> Option<&str> {
        let transport_text = |text: &str| {
            self.link_url
                .iter()
                .chain(self.link_thumb.iter())
                .any(|url| {
                    text == url
                        || text
                            == url
                                .trim_start_matches("https://")
                                .trim_start_matches("http://")
                })
                || direct_zalo_photo(text)
        };
        self.link_desc
            .as_deref()
            .into_iter()
            .chain(std::iter::once(self.text.as_str()))
            .map(str::trim)
            .find(|text| {
                !text.is_empty()
                    && !matches!(*text, "[Hình ảnh]" | "[Photo]")
                    && !transport_text(text)
            })
    }

    /// Repair cached media metadata without changing captions or signed URL bytes.
    pub fn hydrate_media_metadata(&mut self) -> bool {
        if self.text == "[Tin nhắn đã thu hồi]" {
            return false;
        }
        let before = (
            self.link_url.clone(),
            self.link_thumb.clone(),
            self.link_src.clone(),
            self.kind,
            self.text.clone(),
        );
        let source_kind = self.history_source.as_ref().and_then(|source| {
            let kind = source
                .get("msgType")
                .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()));
            match kind {
                Some(2..=4) => Some("Hình ảnh Zalo"),
                Some(19) => Some("Video Zalo"),
                _ => None,
            }
        });
        if let Some(source) = &self.history_source {
            if let Some(attachment) = source
                .get("meta")
                .and_then(|m| m.get("attachsList"))
                .and_then(Value::as_array)
                .and_then(|a| a.first())
            {
                let (url, _, _, thumb, _) = extract_link_info(attachment);
                if self.link_url.is_none() {
                    self.link_url = url;
                }
                if self.link_thumb.is_none() {
                    self.link_thumb = thumb;
                }
            }
        }
        let label_kind = match self.text.trim() {
            "[Photo]" | "[Hình ảnh]" => Some("Hình ảnh Zalo"),
            "[Video]" => Some("Video Zalo"),
            _ => None,
        };
        let photo = self.is_photo();
        if let Some(kind) = source_kind.or(label_kind).or(if photo {
            Some("Hình ảnh Zalo")
        } else {
            None
        }) {
            self.link_src = Some(kind.into());
        }
        if self.link_src.as_deref() == Some("Hình ảnh Zalo") {
            self.kind = crate::message_model::MessageKind::Photo;
            if self.photo_caption().is_none() {
                self.text = "[Hình ảnh]".into();
            }
        }
        before
            != (
                self.link_url.clone(),
                self.link_thumb.clone(),
                self.link_src.clone(),
                self.kind,
                self.text.clone(),
            )
    }

    /// Fill independently missing card fields when an existing URL already exists.
    // The parser layer wires this helper in the dependent protocol change.
    #[allow(dead_code)]
    fn merge_media_metadata(&mut self, incoming: &Self) -> bool {
        if self.text == "[Tin nhắn đã thu hồi]" {
            return false;
        }
        let before = (
            self.link_url.clone(),
            self.link_title.clone(),
            self.link_desc.clone(),
            self.link_thumb.clone(),
            self.link_src.clone(),
        );
        if self.link_title.is_none() {
            self.link_title = incoming.link_title.clone();
        }
        if self.link_desc.is_none() {
            self.link_desc = incoming.link_desc.clone();
        }
        if self.link_thumb.is_none() {
            self.link_thumb = incoming.link_thumb.clone();
        }
        if self.link_src.is_none()
            || matches!(
                incoming.link_src.as_deref(),
                Some("Hình ảnh Zalo" | "Video Zalo")
            )
        {
            self.link_src = incoming.link_src.clone().or_else(|| self.link_src.clone());
        }
        self.hydrate_media_metadata();
        before
            != (
                self.link_url.clone(),
                self.link_title.clone(),
                self.link_desc.clone(),
                self.link_thumb.clone(),
                self.link_src.clone(),
            )
    }

    /// Compute the static PNG URL for a Zalo sticker (`compact-app-pc.b7bec208c4c8f73e09f8.js:7739583` & `18589292`).
    pub fn sticker_image_url(&self) -> Option<String> {
        let sid = self.sticker_id?;
        if sid == 0 {
            return None;
        }
        if self.sticker_cate_id == Some(0) && (1..=30).contains(&sid) {
            Some(format!(
                "https://stc-chat.zdn.vn/images/stickers/default/thumb/{sid}.png"
            ))
        } else {
            Some(format!(
                "https://api.zaloapp.com/api/emoticon/sticker/webpc?eid={sid}&size=130"
            ))
        }
    }

    pub fn sticker_cache_key(&self) -> Option<String> {
        let sid = self.sticker_id?;
        if sid == 0 {
            return None;
        }
        let cid = self.sticker_cate_id.unwrap_or(0);
        Some(format!("stk_{cid}_{sid}"))
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Thread {
    #[serde(default)]
    pub presentation: SidebarPresentation,
    pub id: String,
    pub name: String,
    pub preview: String,
    pub time: String,
    pub avatar: Option<String>,
    pub is_group: bool,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub unread: crate::unread::UnreadState,
}

/// Only direct image paths on known Zalo image-serving families are photo cards.
fn direct_zalo_photo(text: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(text) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let trusted = crate::endpoints::PHOTO_CDN_DOMAINS.iter().any(|suffix| {
        host == *suffix
            || host
                .strip_suffix(suffix)
                .is_some_and(|prefix| prefix.ends_with('.'))
    });
    let path = url.path().to_ascii_lowercase();
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && trusted
        && [".jpg", ".jpeg", ".png", ".webp", ".gif", ".bmp"]
            .iter()
            .any(|ext| path.ends_with(ext))
}

/// Durable sidebar metadata, including metadata-only startup snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SidebarPresentation {
    pub inbox: crate::inbox::Metadata,
    pub server_alias: Option<String>,
    pub base_name: Option<String>,
    pub archived: Option<bool>,
    #[serde(skip)]
    pub metadata_verified: bool,
    pub reaction_preview: Option<crate::reactions::Preview>,
    #[serde(skip)]
    pub reaction_preview_fresh: bool,
    /// Volatile permissions must be fetched again; membership versions do not validate cached ACLs.
    #[serde(skip)]
    pub send_policy: Option<GroupSendPolicy>,
    /// Opaque sync partition identity; never used as an outbound recipient ID.
    pub global_id: String,
    pub members: Vec<SidebarMember>,
    /// Server totalMember only; never inferred from message senders or avatar samples.
    pub member_count: Option<u64>,
    pub last_message_ts: i64,
    pub last_message_id: String,
    pub last_sender: String,
    pub last_sender_id: String,
    pub last_outgoing: bool,
}

/// Server group lock policy; role decisions use account UID, never display names.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GroupSendPolicy {
    /// Whether ordinary members are prevented from posting.
    pub locked: bool,
    /// Creator and administrator UIDs as supplied by group metadata.
    pub managers: Vec<String>,
}
impl GroupSendPolicy {
    /// Parse the observed getmg-v2 fields without inferring roles from member order.
    pub fn parse(info: &Value) -> Option<Self> {
        let raw = info.pointer("/setting/lockSendMsg")?;
        let locked = match raw {
            Value::Bool(value) => *value,
            Value::Number(value) => match value.as_i64()? {
                0 => false,
                1 => true,
                _ => return None,
            },
            Value::String(value) => match value.as_str() {
                "0" => false,
                "1" => true,
                _ => return None,
            },
            _ => return None,
        };
        let mut managers = Vec::new();
        if let Some(creator) = pick_str(info, &["creatorId"]) {
            managers.push(creator);
        }
        if let Some(admins) = info.get("adminIds").and_then(Value::as_array) {
            for admin in admins {
                let id = admin
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| admin.as_u64().map(|v| v.to_string()));
                if let Some(id) = id {
                    if !managers.contains(&id) {
                        managers.push(id);
                    }
                }
            }
        }
        Some(Self { locked, managers })
    }

    /// A locked group requires a known management role before sending.
    pub fn allows(&self, uid: &str) -> bool {
        !self.locked || (!uid.is_empty() && self.managers.iter().any(|id| id == uid))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SidebarMember {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    /// `creator`, `admin` or empty, from `creatorId` / `adminIds`.
    pub role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarFontStyle {
    Regular,
    Semibold,
}

impl Thread {
    pub fn sidebar_font_style(&self) -> SidebarFontStyle {
        if self.unread.has_badge() {
            SidebarFontStyle::Semibold
        } else {
            SidebarFontStyle::Regular
        }
    }

    /// Rebuild the sidebar after deleting messages, including an older surviving latest row.
    pub(crate) fn refresh_after_removal(&mut self) {
        self.presentation.last_message_ts = 0;
        self.presentation.last_message_id.clear();
        self.presentation.last_sender.clear();
        self.presentation.last_sender_id.clear();
        self.presentation.last_outgoing = false;
        self.preview = self
            .messages
            .iter()
            .max_by_key(|m| crate::clock::normalize_ts_ms(m.ts))
            .map(crate::message_view::summary)
            .unwrap_or_default();
        self.refresh_sidebar_presentation();
    }

    pub fn refresh_sidebar_presentation(&mut self) {
        if let Some(message) = self
            .messages
            .iter()
            .max_by_key(|m| crate::clock::normalize_ts_ms(m.ts))
        {
            let ts = crate::clock::normalize_ts_ms(message.ts);
            if ts >= crate::clock::normalize_ts_ms(self.presentation.last_message_ts) {
                let sender_id = sidebar_sender_id(message)
                    .or_else(|| {
                        (message.id == self.presentation.last_message_id
                            && ts
                                == crate::clock::normalize_ts_ms(self.presentation.last_message_ts))
                        .then(|| self.presentation.last_sender_id.clone())
                    })
                    .unwrap_or_default();
                let keep_resolved = message.id == self.presentation.last_message_id
                    && ts == crate::clock::normalize_ts_ms(self.presentation.last_message_ts)
                    && sender_id == self.presentation.last_sender_id
                    && !self.presentation.last_sender.is_empty()
                    && !self
                        .presentation
                        .last_sender
                        .bytes()
                        .all(|b| b.is_ascii_digit())
                    && message.sender.bytes().all(|b| b.is_ascii_digit());
                self.presentation.last_message_ts = message.ts;
                self.presentation.last_message_id = message.id.clone();
                if !keep_resolved {
                    self.presentation.last_sender = message.sender.clone();
                }
                self.presentation.last_sender_id = sender_id;
                self.presentation.last_outgoing = message.is_outgoing;
            }
        }
    }

    pub fn sidebar_preview(&self) -> String {
        self.sidebar_preview_with_reactor(None)
    }
    /// A resolved profile name does not imply that the reactor is a current group member.
    pub fn sidebar_preview_with_reactor(&self, reactor_name: Option<&str>) -> String {
        if let Some(reaction) = &self.presentation.reaction_preview {
            let message_ts = self
                .messages
                .iter()
                .map(|m| crate::clock::normalize_ts_ms(m.ts))
                .chain(std::iter::once(crate::clock::normalize_ts_ms(
                    self.presentation.last_message_ts,
                )))
                .max()
                .unwrap_or(0);
            if reaction.ts >= message_ts {
                let who = if reaction.by_self {
                    "Bạn"
                } else {
                    self.presentation
                        .members
                        .iter()
                        .find(|member| member.id == reaction.reactor && !member.name.is_empty())
                        .map(|member| member.name.as_str())
                        .unwrap_or(reactor_name.unwrap_or(if self.is_group {
                            &reaction.reactor
                        } else {
                            &self.name
                        }))
                };
                return format!(
                    "{who} đã thả {} vào {}",
                    reaction.emoji,
                    if reaction.target_by_self {
                        "tin nhắn của bạn"
                    } else {
                        "một tin nhắn"
                    }
                );
            }
        }

        let latest = self
            .messages
            .iter()
            .max_by_key(|m| crate::clock::normalize_ts_ms(m.ts))
            .filter(|m| {
                crate::clock::normalize_ts_ms(m.ts)
                    > crate::clock::normalize_ts_ms(self.presentation.last_message_ts)
                    || self.presentation.last_sender.is_empty()
            });
        let (sender, outgoing) = latest
            .map(|m| (m.sender.as_str(), m.is_outgoing))
            .unwrap_or((
                &self.presentation.last_sender,
                self.presentation.last_outgoing,
            ));
        let sender = self
            .presentation
            .members
            .iter()
            .find(|member| member.id == sender && !member.name.trim().is_empty())
            .map(|member| member.name.as_str())
            .unwrap_or(sender);
        let text = self.preview.replace('\n', " ");
        if text.is_empty() {
            return text;
        }
        if outgoing {
            if text.starts_with("Bạn: ") {
                text
            } else {
                format!("Bạn: {text}")
            }
        } else if self.is_group
            && !sender.trim().is_empty()
            && !sender.trim().chars().all(|c| c.is_ascii_digit())
        {
            let prefix = format!("{}: ", sender.trim());
            if text.starts_with(&prefix) {
                text
            } else {
                format!("{prefix}{text}")
            }
        } else {
            text
        }
    }

    pub fn sidebar_activity_ts(&self) -> i64 {
        self.messages
            .iter()
            .map(|m| crate::clock::normalize_ts_ms(m.ts))
            .chain(std::iter::once(crate::clock::normalize_ts_ms(
                self.presentation.last_message_ts,
            )))
            .chain(
                self.presentation
                    .reaction_preview
                    .as_ref()
                    .map(|reaction| reaction.ts),
            )
            .max()
            .unwrap_or(0)
    }

    pub fn sidebar_timestamp(&self, now_ms: i64) -> String {
        let ts = self.sidebar_activity_ts();
        if ts > 0 {
            crate::clock::sidebar_time_at(ts, now_ms)
        } else {
            self.time.clone()
        }
    }

    /// Parse only group roster metadata observed in the original getmg-v2 consumer.
    pub fn apply_group_sidebar_metadata(&mut self, info: &Value) {
        self.presentation.metadata_verified = true;
        if let Some(policy) = GroupSendPolicy::parse(info) {
            self.presentation.send_policy = Some(policy);
        }
        if let Some(id) = pick_str(info, &["globalId"]) {
            self.presentation.global_id = id;
        }
        if let Some(count) = info
            .get("totalMember")
            .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
        {
            self.presentation.member_count = Some(count);
        }
        let mut ids = Vec::new();
        if let Some(list) = info.get("memVerList").and_then(Value::as_array) {
            for item in list.iter().filter_map(Value::as_str) {
                let id = item.split('_').next().unwrap_or_default();
                if !id.is_empty() && !ids.iter().any(|s| s == id) {
                    ids.push(id.to_owned());
                }
            }
        } else if let Some(map) = info.get("memberVerMap").and_then(Value::as_object) {
            ids.extend(map.keys().cloned());
        }
        let mut leaders = Vec::new();
        let creator = pick_str(info, &["creatorId"]);
        if let Some(creator) = creator.clone() {
            leaders.push(creator);
        }
        let admins: Vec<String> = info
            .get("adminIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        leaders.extend(admins.iter().cloned());
        let mut ordered = Vec::new();
        for id in leaders.into_iter().chain(ids.iter().cloned()) {
            if ids.contains(&id) && !ordered.contains(&id) {
                ordered.push(id);
            }
        }
        ids = ordered;
        let top = info.get("topMember").and_then(Value::as_array);
        if let Some(top) = top {
            for profile in top {
                if let Some(id) = pick_str(profile, &["id", "userId", "uid"]) {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
        }
        if !ids.is_empty() {
            let previous = std::mem::take(&mut self.presentation.members);
            self.presentation.members =
                ids.into_iter()
                    .map(|id| {
                        let profile = top.and_then(|list| {
                            list.iter().find(|p| {
                                pick_str(p, &["id", "userId", "uid"]).as_ref() == Some(&id)
                            })
                        });
                        let mut member = previous.iter().find(|m| m.id == id).cloned().unwrap_or(
                            SidebarMember {
                                id,
                                ..Default::default()
                            },
                        );
                        if let Some(profile) = profile {
                            if let Some(name) =
                                pick_str(profile, &["displayName", "zaloName", "name"])
                            {
                                member.name = name;
                            }
                            if let Some(avatar) = pick_str(profile, &["avatar", "avt"]) {
                                member.avatar = Some(avatar);
                            }
                        }
                        member.role = if creator.as_deref() == Some(member.id.as_str()) {
                            "creator".into()
                        } else if admins.contains(&member.id) {
                            "admin".into()
                        } else {
                            String::new()
                        };
                        member
                    })
                    .collect();
        }
    }
}

pub fn sidebar_sender_id(message: &ChatMessage) -> Option<String> {
    let raw = message.wire_ref.sender.as_ref().or_else(|| {
        message
            .history_source
            .as_ref()
            .and_then(|v| v.get("senderId"))
    })?;
    let id = match raw {
        Value::String(v) => v.clone(),
        Value::Number(v) if v.is_u64() => v.to_string(),
        _ => return None,
    };
    (!id.is_empty() && id != "0" && id.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// Return `true` if a thread name is empty or a client placeholder (`"Người dùng <id>"`,
/// `"Nhóm <id>"`, or a bare numeric ID). Real names such as `"Nhóm bạn thân"` are kept.
pub fn is_generic_thread_name(name: &str) -> bool {
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let t = name.trim();
    t.is_empty()
        || numeric(t)
        || t.strip_prefix("Người dùng ")
            .is_some_and(|rest| numeric(rest.trim()))
        || t.strip_prefix("Nhóm ")
            .is_some_and(|rest| numeric(rest.trim()))
}

/// Extract the stable avatar filename (e.g. `"00000000000000000000000000000001.jpg"`) from a Zalo CDN URL.
pub fn avatar_file_key(avatar: Option<&str>) -> Option<String> {
    let url = avatar?.trim();
    if url.is_empty() {
        return None;
    }
    let path = url.split('?').next()?;
    let fname = path.rsplit('/').next()?.trim();
    if fname.len() >= 12
        && (fname.ends_with(".jpg") || fname.ends_with(".png") || fname.ends_with(".webp"))
    {
        Some(fname.to_ascii_lowercase())
    } else {
        None
    }
}

/// Extract the `?time=...` expiry timestamp from a Zalo avatar URL so newer session avatars win.
pub fn avatar_url_time(avatar: Option<&str>) -> i64 {
    let Some(url) = avatar else {
        return 0;
    };
    let Some(query) = url.split('?').nth(1) else {
        return 0;
    };
    for pair in query.split('&') {
        if let Some(val) = pair.strip_prefix("time=") {
            if let Ok(ts) = val.parse::<i64>() {
                return ts;
            }
        }
    }
    0
}

/// Display names and avatars are not identities: multiple contacts can share
/// both. Never migrate a conversation to another recipient using those fields.
pub fn threads_represent_same_contact(a: &Thread, b: &Thread) -> bool {
    !a.id.is_empty() && a.id != "0" && a.is_group == b.is_group && a.id == b.id
}

fn pending_message_id(id: &str) -> bool {
    id.starts_with("out_") || id.starts_with("local_")
}

/// Compare explicit identities only; content and timestamps cannot acknowledge a send.
pub(crate) fn same_message(existing: &ChatMessage, incoming: &ChatMessage) -> bool {
    let mut identities = MessageIdentityIndex::default();
    identities.insert(existing, 0);
    identities.find(std::slice::from_ref(existing), incoming) == Some(0)
}

fn wire_identity(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value)
            if !value.trim().is_empty() && value.bytes().any(|byte| byte != b'0') =>
        {
            Some(value.clone())
        }
        Value::Number(value) => value.as_u64().filter(|id| *id > 0).map(|id| id.to_string()),
        _ => None,
    }
}

fn wire_sender(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => value.as_u64().map(|id| id.to_string()),
        _ => None,
    }
}

pub(crate) fn server_message_id(message: &ChatMessage) -> Option<String> {
    let wire_server = wire_identity(message.wire_ref.server_id.as_ref());
    let wire_client = wire_identity(message.wire_ref.client_id.as_ref());
    if pending_message_id(&message.id)
        || message.id.is_empty()
        || wire_client.as_deref() == Some(message.id.as_str())
    {
        wire_server
    } else {
        wire_identity(Some(&Value::String(message.id.clone())))
    }
}

pub(crate) fn client_message_id(message: &ChatMessage) -> Option<String> {
    let wire = wire_identity(message.wire_ref.client_id.as_ref());
    if message.client_message_id.trim().is_empty() || message.client_message_id == "0" {
        wire
    } else if wire
        .as_ref()
        .is_some_and(|id| id != &message.client_message_id)
    {
        None
    } else {
        Some(message.client_message_id.clone())
    }
}

// Most identities have one row. Allocate a set only for actual collisions.
#[derive(Default)]
struct IdentityRows {
    single: Option<usize>,
    collisions: Option<std::collections::HashSet<usize>>,
}
impl IdentityRows {
    fn insert(&mut self, index: usize) {
        if let Some(rows) = &mut self.collisions {
            rows.insert(index);
        } else if let Some(previous) = self.single {
            if previous != index {
                self.collisions = Some(std::collections::HashSet::from([previous, index]));
                self.single = None;
            }
        } else {
            self.single = Some(index);
        }
    }
    fn remove(&mut self, index: &usize) {
        if let Some(rows) = &mut self.collisions {
            rows.remove(index);
            if rows.len() <= 1 {
                self.single = rows.iter().next().copied();
                self.collisions = None;
            }
        } else if self.single.as_ref() == Some(index) {
            self.single = None;
        }
    }
    fn len(&self) -> usize {
        self.collisions
            .as_ref()
            .map_or(usize::from(self.single.is_some()), |rows| rows.len())
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn unique(&self) -> Option<usize> {
        self.single
    }
}

#[derive(Default)]
struct MessageIdentityIndex {
    rows: std::collections::HashMap<(u8, String), IdentityRows>,
}

impl MessageIdentityIndex {
    fn keys(message: &ChatMessage) -> impl Iterator<Item = (u8, String)> {
        let server = server_message_id(message)
            .map(|id| (0, id))
            .or_else(|| pending_message_id(&message.id).then(|| (1, message.id.clone())));
        // Old caches may use a client reference as their canonical stored ID.
        let legacy = (!pending_message_id(&message.id))
            .then(|| wire_identity(Some(&Value::String(message.id.clone()))))
            .flatten()
            .map(|id| (3, id));
        let client = client_message_id(message).map(|id| (2, id));
        [server, legacy, client].into_iter().flatten()
    }

    fn insert(&mut self, message: &ChatMessage, index: usize) {
        for key in Self::keys(message) {
            self.rows.entry(key).or_default().insert(index);
        }
    }

    fn remove(&mut self, message: &ChatMessage, index: usize) {
        for key in Self::keys(message) {
            if let Some(rows) = self.rows.get_mut(&key) {
                rows.remove(&index);
            }
        }
    }

    fn find(&self, messages: &[ChatMessage], incoming: &ChatMessage) -> Option<usize> {
        for key in Self::keys(incoming) {
            let Some(rows) = self.rows.get(&key).filter(|rows| !rows.is_empty()) else {
                continue;
            };
            // A colliding identity is not evidence for selecting any particular row.
            if rows.len() != 1 {
                return None;
            }
            let index = rows.unique()?;
            let existing = &messages[index];
            if existing.is_outgoing != incoming.is_outgoing {
                return None;
            }
            if let (Some(a), Some(b)) = (
                wire_sender(existing.wire_ref.sender.as_ref()),
                wire_sender(incoming.wire_ref.sender.as_ref()),
            ) {
                if a != b && !(existing.is_outgoing && (a == "0" || b == "0")) {
                    return None;
                }
            }
            if key.0 != 2 {
                if key.0 == 3 {
                    if let (Some(a), Some(b)) = (
                        wire_identity(existing.wire_ref.server_id.as_ref()),
                        wire_identity(incoming.wire_ref.server_id.as_ref()),
                    ) {
                        if a != b {
                            return None;
                        }
                    }
                }
                return Some(index);
            }
            // Client-only packets can replay before a server ID is known. A client
            // reference never collapses two distinct pending or server messages.
            let existing_server = server_message_id(existing);
            let incoming_server = server_message_id(incoming);
            let existing_local = pending_message_id(&existing.id) && existing_server.is_none();
            let incoming_local = pending_message_id(&incoming.id) && incoming_server.is_none();
            let client_only_replay = existing_server.is_none()
                && incoming_server.is_none()
                && (existing.id.is_empty() || incoming.id.is_empty());
            return (client_only_replay
                || (existing.is_outgoing
                    && ((existing_local && incoming_server.is_some())
                        || (incoming_local && existing_server.is_some()))))
            .then_some(index);
        }
        None
    }
}

/// Merge a local send acknowledgement without replacing an existing server clock.
pub(crate) fn merge_sent_message(thread: &mut Thread, mut message: ChatMessage) {
    let mut identities = MessageIdentityIndex::default();
    for (row, existing) in thread.messages.iter().enumerate() {
        identities.insert(existing, row);
    }
    if identities
        .find(&thread.messages, &message)
        .is_some_and(|row| server_message_id(&thread.messages[row]).is_some())
    {
        // The acknowledgement receipt time is not the server's message timestamp.
        message.ts = 0;
        message.time.clear();
    }
    merge_thread_into(
        thread,
        Thread {
            messages: vec![message],
            ..Default::default()
        },
        false,
    );
}

/// Merge `incoming` thread into `target`. When `incoming_id_is_authoritative` is `true`,
/// `target.id` is migrated to `incoming.id` so actions (chat/VoIP call) always target the active session's `userId`.
pub fn merge_thread_into(
    target: &mut Thread,
    mut incoming: Thread,
    incoming_id_is_authoritative: bool,
) {
    crate::message_view::remove_call_logs(&mut incoming);
    crate::message_view::remove_call_logs(target);
    if incoming_id_is_authoritative && !incoming.id.is_empty() && incoming.id != "0" {
        target.id = incoming.id;
    }
    if !incoming.name.is_empty()
        && (!is_generic_thread_name(&incoming.name) || is_generic_thread_name(&target.name))
    {
        target.name = incoming.name;
    }
    if incoming.avatar.is_some()
        && (target.avatar.is_none()
            || incoming_id_is_authoritative
            || avatar_url_time(incoming.avatar.as_deref())
                >= avatar_url_time(target.avatar.as_deref()))
    {
        target.avatar = incoming.avatar;
    }
    target
        .presentation
        .inbox
        .merge(&incoming.presentation.inbox, incoming_id_is_authoritative);
    if incoming.presentation.archived.is_some()
        && (incoming_id_is_authoritative || target.presentation.archived.is_none())
    {
        target.presentation.archived = incoming.presentation.archived;
    }
    if !incoming.presentation.global_id.is_empty() {
        target.presentation.global_id = incoming.presentation.global_id.clone();
    }
    if incoming.presentation.send_policy.is_some() {
        target.presentation.send_policy = incoming.presentation.send_policy.clone();
    }
    if incoming.presentation.reaction_preview_fresh
        || incoming
            .presentation
            .reaction_preview
            .as_ref()
            .is_some_and(|new| {
                target
                    .presentation
                    .reaction_preview
                    .as_ref()
                    .is_none_or(|old| new.ts > old.ts)
            })
    {
        target.presentation.reaction_preview = incoming.presentation.reaction_preview.clone();
    }
    target.is_group = target.is_group || incoming.is_group;
    target.unread.merge_metadata(&incoming.unread);

    // Repeated text and timestamps are not identities. Keep ambiguous keys instead of
    // choosing the first candidate, especially when multiple pending sends collide.
    let mut identities = MessageIdentityIndex::default();
    for (index, message) in target.messages.iter().enumerate() {
        identities.insert(message, index);
    }
    for msg in incoming.messages {
        if msg.text == "sendBubbleMessage" {
            continue;
        }
        let found = identities.find(&target.messages, &msg);
        if let Some(idx) = found {
            let existing_msg = &mut target.messages[idx];
            identities.remove(existing_msg, idx);
            existing_msg.merge_reaction_metadata(&msg);
            if existing_msg.id.starts_with("out_")
                && !msg.id.is_empty()
                && !pending_message_id(&msg.id)
            {
                existing_msg.id = msg.id.clone();
            }
            if msg.ts > 0 && existing_msg.ts != msg.ts {
                existing_msg.ts = msg.ts;
                existing_msg.time = msg.time;
            }
            let recalled = existing_msg.text.trim() == "[Tin nhắn đã thu hồi]"
                || msg.text.trim() == "[Tin nhắn đã thu hồi]";
            if recalled {
                existing_msg.mark_recalled();
            }
            if existing_msg.sticker_id.is_none() && msg.sticker_id.is_some() {
                existing_msg.sticker_id = msg.sticker_id;
                existing_msg.sticker_cate_id = msg.sticker_cate_id;
                existing_msg.sticker_type = msg.sticker_type;
            }
            if existing_msg.reply_to_text.is_none() && msg.reply_to_text.is_some() {
                existing_msg.reply_to_id = msg.reply_to_id;
                existing_msg.reply_to_sender = msg.reply_to_sender;
                existing_msg.reply_to_text = msg.reply_to_text;
            }
            if existing_msg.link_url.is_none() && msg.link_url.is_some() {
                existing_msg.link_url = msg.link_url;
                existing_msg.link_title = msg.link_title;
                existing_msg.link_desc = msg.link_desc;
                existing_msg.link_thumb = msg.link_thumb;
                existing_msg.link_src = msg.link_src;
                if !msg.text.is_empty() {
                    existing_msg.text = msg.text;
                }
            }
            if existing_msg.reactions.is_empty() && !msg.reactions.is_empty() {
                existing_msg.reactions = msg.reactions;
            }
            if recalled {
                existing_msg.mark_recalled();
            }
            existing_msg.is_pinned = existing_msg.is_pinned || msg.is_pinned;
            existing_msg.is_starred = existing_msg.is_starred || msg.is_starred;
            identities.insert(existing_msg, idx);
        } else {
            let idx = target.messages.len();
            identities.insert(&msg, idx);
            target.messages.push(msg);
        }
    }

    target.messages.retain(|m| m.text != "sendBubbleMessage");
    target.messages.sort_by_key(|m| m.ts);

    if let Some(newest) = target.messages.last() {
        if !newest.text.is_empty() {
            target.preview = newest.text.replace('\n', " ");
            target.time = newest.time.clone();
        }
    } else if !incoming.preview.is_empty() && target.preview.is_empty() {
        target.preview = incoming.preview;
        if !incoming.time.is_empty() {
            target.time = incoming.time;
        }
    } else if target.time.is_empty() && !incoming.time.is_empty() {
        target.time = incoming.time;
    }
}

/// Deduplicate `threads` in-place, merging entries that represent the same contact/group
/// by the same server ID and kind. Names and avatars never authorize migration.
pub fn deduplicate_threads(
    threads: &mut Vec<Thread>,
    authoritative_ids: Option<&std::collections::HashSet<String>>,
) {
    let mut merged: Vec<Thread> = Vec::with_capacity(threads.len());
    for mut t in threads.drain(..) {
        if t.id.is_empty() || t.id == "0" {
            continue;
        }
        t.messages.retain(|m| m.text != "sendBubbleMessage");
        if let Some(newest) = t.messages.last() {
            if !newest.text.is_empty() && (t.preview.is_empty() || t.preview == "sendBubbleMessage")
            {
                t.preview = newest.text.replace('\n', " ");
                t.time = newest.time.clone();
            }
        }
        if let Some(existing) = merged.iter_mut().find(|e| {
            if threads_represent_same_contact(e, &t) {
                return true;
            }
            if let Some(auth) = authoritative_ids {
                if auth.contains(&e.id) && auth.contains(&t.id) {
                    return false;
                }
            }
            threads_represent_same_contact(e, &t)
        }) {
            let incoming_auth = if let Some(auth) = authoritative_ids {
                auth.contains(&t.id) && !auth.contains(&existing.id)
            } else {
                // In cache order, later entries or entries with newer avatar URL timestamps come from newer sessions
                let t_time = avatar_url_time(t.avatar.as_deref());
                let e_time = avatar_url_time(existing.avatar.as_deref());
                if t_time != e_time {
                    t_time > e_time
                } else {
                    true
                }
            };
            merge_thread_into(existing, t, incoming_auth);
        } else {
            merged.push(t);
        }
    }
    *threads = merged;
}

/// Load locally cached conversations and message history.
pub fn load_threads_cache(s: &Session) -> Vec<Thread> {
    if s.account_key().is_empty() || !s.is_active() {
        return Vec::new();
    }
    if let Ok(text) = std::fs::read_to_string(s.cache_path()) {
        if let Ok(mut threads) = serde_json::from_str::<Vec<Thread>>(&text) {
            for thread in &mut threads {
                for message in &mut thread.messages {
                    if message.delivery == crate::message_model::DeliveryState::Pending
                        && message.outgoing_job.is_some()
                        && !crate::outgoing::is_active(&message.client_message_id)
                    {
                        message.delivery = crate::message_model::DeliveryState::Failed;
                        if let Some(job) = message.outgoing_job.as_mut() {
                            job.error = "Lần gửi trước bị gián đoạn; kiểm tra trước khi gửi lại để tránh trùng tin".into();
                        }
                    }
                }
            }
            // Cached previews are non-authoritative; deletion markers must win at startup too.
            match crate::store::MessageStore::open(s.history_path())
                .and_then(|store| store.filter_deleted(&mut threads))
            {
                Ok(()) => {}
                Err(_) => {
                    eprintln!("Cannot filter cached deletion identities");
                    return Vec::new();
                }
            }
            deduplicate_threads(&mut threads, None);
            return threads;
        }
    }
    Vec::new()
}

/// Persist conversations and message history locally so they survive app restarts.
pub fn save_threads_cache(s: &Session, threads: &[Thread]) {
    if s.account_key().is_empty() || !s.is_active() {
        return;
    }
    let keep = crate::store::MessageStore::PRELOAD_MSGS;
    let mut slim: Vec<Thread> = threads
        .iter()
        .map(|t| {
            let mut t = t.clone();
            let skip = t.messages.len().saturating_sub(keep);
            t.messages.drain(..skip);
            t
        })
        .collect();
    if crate::store::MessageStore::open(s.history_path())
        .and_then(|store| store.filter_deleted(&mut slim))
        .is_err()
    {
        eprintln!("Cannot filter history cache before saving");
        return;
    }
    if let Ok(text) = serde_json::to_string(&slim) {
        if let Err(e) = crate::storage::atomic_write(&s.cache_path(), text.as_bytes()) {
            eprintln!("Cannot save history cache: {e}");
        }
    }
}

/// `/api/social/friend/getfriends` — the friend list.
///
/// Uses `s.domains.friend` (defaulting to `tt-friend-wpa.chat.zalo.me`), falling back
/// to `s.domains.profile` only if `domains.friend` is empty.
pub async fn get_friends(s: &Session) -> Result<Vec<Thread>> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[
        ("incInvalid", json!(1)),
        ("page", json!(1)),
        ("count", json!(20000)),
        ("avatar_size", json!(160)),
        ("actiontime", json!(0)),
        ("imei", json!(s.imei)),
    ]);
    let params = match s.encoded_params(&body) {
        Ok(p) => format!("params={p}"),
        Err(e) => {
            crate::logger::log_error("chat::get_friends::encoded_params", &e.to_string());
            eprintln!("! get_friends: no session key for encoded_params ({e}), falling back to plain_params");
            Session::plain_params(&body)
        }
    };
    let hosts = [&s.domains.friend, &s.domains.profile];
    let mut resp_opt = None;
    let mut last_err = None;
    for host in hosts {
        if host.is_empty() {
            continue;
        }
        let url = format!(
            "{}/api/social/friend/getfriends?{}{}",
            host,
            s.common_params(),
            if params.is_empty() {
                String::new()
            } else {
                format!("&{params}")
            }
        );
        match s.get(&url).await {
            Ok(r) => {
                resp_opt = Some(s.maybe_decrypt(r, s.session_key_bytes()).await);
                break;
            }
            Err(e) => {
                last_err = Some(e);
            }
        }
    }
    let resp = match resp_opt {
        Some(r) => r,
        None => return Err(last_err.unwrap_or_else(|| anyhow::anyhow!("get_friends failed"))),
    };
    let data_root = response_data(resp)?;
    let mut threads: Vec<Thread> = match &data_root {
        Value::Array(items) => items.iter().filter_map(parse_thread).collect(),
        Value::Object(o) if o.contains_key("friends") => o["friends"]
            .as_array()
            .map(|a| a.iter().filter_map(parse_thread).collect())
            .unwrap_or_default(),
        Value::Object(o) if o.get("data").and_then(Value::as_array).is_some() => o["data"]
            .as_array()
            .map(|a| a.iter().filter_map(parse_thread).collect())
            .unwrap_or_default(),
        _ => anyhow::bail!("Phản hồi danh bạ không có danh sách hợp lệ; giữ lịch sử cục bộ"),
    };

    // If Cloud của tôi (send2me) is available, place it at the top
    if let Some(li) = &s.login {
        if !li.send2me_id.is_empty() && !threads.iter().any(|t| t.id == li.send2me_id) {
            threads.insert(
                0,
                Thread {
                    presentation: Default::default(),
                    unread: Default::default(),
                    id: li.send2me_id.clone(),
                    name: "Cloud của tôi (Lưu trữ)".into(),
                    preview: "".into(),
                    time: crate::ui::clock(),
                    avatar: None,
                    is_group: false,
                    messages: Vec::new(),
                },
            );
        }
    }

    // Merge user groups into threads list
    if let Ok(mut groups) = get_groups(s).await {
        for g in groups.drain(..) {
            if let Some(existing) = threads.iter_mut().find(|t| t.id == g.id && t.is_group) {
                merge_thread_into(existing, g, true);
            } else {
                threads.push(g);
            }
        }
    }

    resolve_thread_names(s, &mut threads).await;

    Ok(threads)
}

fn parse_thread(v: &Value) -> Option<Thread> {
    let id = pick_str(v, &["userId", "uid", "id", "friendId", "globalId"])?;
    let name = pick_str(v, &["displayName", "zaloName", "display_name", "name"])
        .or_else(|| {
            v.get("profile_displayName")
                .or_else(|| v.get("profile").and_then(|p| p.get("displayName")))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| id.clone());
    let avatar = pick_str(v, &["avatar", "avt", "profile_avatar"]);
    let is_group = is_group_flag(v);
    let time = v
        .get("lastActionTime")
        .and_then(|val| match val {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.parse::<i64>().ok(),
            _ => None,
        })
        .map(crate::clock::message_time)
        .unwrap_or_default();

    Some(Thread {
        presentation: SidebarPresentation {
            global_id: pick_str(v, &["globalId"]).unwrap_or_default(),
            ..Default::default()
        },
        unread: Default::default(),
        id,
        name,
        preview: String::new(),
        time,
        avatar,
        is_group,
        messages: Vec::new(),
    })
}

fn pick_str(v: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        match v.get(*k) {
            Some(Value::String(s)) if !s.is_empty() => return Some(s.clone()),
            Some(Value::Number(n)) => return Some(n.to_string()),
            _ => {}
        }
    }
    None
}

// HTTP 200 (including an empty long-poll-style response) is not a send receipt.
// Keep the server error marker used by session expiry classification, without
// copying arbitrary server text or decrypted payloads into diagnostics.
fn confirmed_send(response: Value) -> Result<Value> {
    let response = confirmed_upload(response)?;
    anyhow::ensure!(
        !sent_server_id(&response).is_empty(),
        "Máy chủ chưa trả ID xác nhận gửi tin; kiểm tra hội thoại trước khi gửi lại"
    );
    Ok(response)
}

fn confirmed_upload(response: Value) -> Result<Value> {
    let response = confirmed_mutation(response)?;
    anyhow::ensure!(
        response
            .get("data")
            .is_some_and(|v| v.is_object() || v.is_array()),
        "Máy chủ chưa xác nhận dữ liệu gửi; kiểm tra hội thoại trước khi gửi lại"
    );
    Ok(response)
}

/// Require an explicit server acknowledgement before changing local mutation state.
/// # Errors
/// Rejects missing codes, server rejections and opaque/undecodable response bodies.
pub(crate) fn confirmed_mutation(mut response: Value) -> Result<Value> {
    let code = response
        .get("error_code")
        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
        .context("Máy chủ chưa xác nhận thao tác; kiểm tra hội thoại trước khi thử lại")?;
    anyhow::ensure!(code == 0, "server error {code}: thao tác bị từ chối");
    if let Some(text) = response.get("data").and_then(Value::as_str) {
        response["data"] = serde_json::from_str(text)
            .context("Không đọc được xác nhận thao tác; chưa thay đổi dữ liệu cục bộ")?;
    }
    anyhow::ensure!(
        response
            .get("data")
            .is_some_and(|v| v.is_null() || v.is_object() || v.is_array()),
        "Máy chủ chưa xác nhận thao tác; chưa thay đổi dữ liệu cục bộ"
    );
    if let Some(code) = response.pointer("/data/error_code") {
        anyhow::ensure!(
            code.as_i64().or_else(|| code.as_str()?.parse().ok()) == Some(0),
            "Máy chủ từ chối thao tác"
        );
    }
    Ok(response)
}

/// Read a confirmed server message ID without treating a client reference or zero as one.
pub(crate) fn sent_server_id(response: &Value) -> String {
    response
        .get("data")
        .and_then(|data| wire_identity(data.get("msgId")))
        .unwrap_or_default()
}

/// Allocate a millisecond-shaped request ID once per send, including simultaneous sends.
pub(crate) fn next_client_message_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_ID: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let previous = LAST_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |last| {
            Some(now.max(last.saturating_add(1)))
        })
        .unwrap_or_else(|last| last);
    now.max(previous.saturating_add(1))
}

/// Wait for the server to announce an uploaded file's processed URL.
const ASYNC_FILE_URL_WAIT: std::time::Duration = std::time::Duration::from_secs(90);

/// Send a 1-1 text message: `POST {domains.chat}/api/message/sms?{common}`
pub async fn send_message(s: &Session, to_id: &str, text: &str) -> Result<Value> {
    send_message_with_client_id(s, to_id, text, next_client_message_id()).await
}

/// Preserve the request client ID for reconciliation with a later server echo.
/// # Errors
/// Returns session, transport or server rejection failures.
pub(crate) async fn send_message_with_client_id(
    s: &Session,
    to_id: &str,
    text: &str,
    now: u64,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[
        ("toid", json!(to_id)),
        ("message", json!(text)),
        ("clientId", json!(now)),
        ("imei", json!(s.imei)),
    ]);
    let params = s.encoded_params(&body)?;

    let host = if s.domains.chat.contains("-msg-chat.")
        || s.domains.chat.contains("tt-chat2-wpa")
        || s.domains.chat.is_empty()
    {
        "https://tt-chat3-wpa.api.zaloapp.com"
    } else {
        &s.domains.chat
    };

    let url = format!("{host}/api/message/sms?{}", s.common_params());
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    confirmed_send(resp)
}

/// Send a group text message: `POST {domains.group}/api/group/sendmsg?{common}`
pub async fn send_group_message(s: &Session, grid: &str, text: &str) -> Result<Value> {
    send_group_message_with_client_id(s, grid, text, next_client_message_id()).await
}

/// Preserve the request client ID for reconciliation with a later server echo.
/// # Errors
/// Returns session, transport or server rejection failures.
pub(crate) async fn send_group_message_with_client_id(
    s: &Session,
    grid: &str,
    text: &str,
    now: u64,
) -> Result<Value> {
    send_group_mentions(s, grid, text, now, &[]).await
}

/// Send explicit UTF-16 mention recipients through the regular group session path.
/// # Errors
/// Rejects invalid ranges before I/O, or returns session/transport/server failures.
pub(crate) async fn send_group_mentions(
    s: &Session,
    grid: &str,
    text: &str,
    now: u64,
    mentions: &[crate::mentions::Mention],
) -> Result<Value> {
    crate::mentions::validate(text, mentions)?;
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean_grid = grid
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G');
    let mut body = obj(&[
        ("grid", json!(clean_grid)),
        ("message", json!(text)),
        ("clientId", json!(now)),
        ("imei", json!(s.imei)),
        ("visibility", json!(0)),
    ]);
    let endpoint = if mentions.is_empty() {
        "sendmsg"
    } else {
        "mention"
    };
    if !mentions.is_empty() {
        body["mentionInfo"] = json!(serde_json::to_string(mentions)?);
        body["language"] = json!("vi");
    }
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/group/{endpoint}?{}",
        s.domains.group,
        s.common_params()
    );
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    confirmed_send(resp)
}

/// Format a byte count into a human-readable size string (e.g. `"1.4 MB"`, `"320 KB"`).
pub fn format_file_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Upload and send a photo, video, or general file attachment to a 1-1 or group conversation.
///
/// Implements Zalo PC 26.10.11's two-stage chunked upload (`photo_original/upload` -> `photo_original/send`
/// for images; `asyncfile/upload` -> `asyncfile/msg` for videos/files) with automatic fallback to
/// single-shot `message/photo` (`11152`) and `message/sharefile_full` (`11146`).
pub async fn send_attachment(
    s: &Session,
    to_id: &str,
    is_group: bool,
    file_path: &std::path::Path,
    caption: &str,
) -> Result<(Value, ChatMessage)> {
    send_attachment_with_client_id(
        s,
        to_id,
        is_group,
        file_path,
        caption,
        next_client_message_id(),
    )
    .await
}

pub(crate) async fn send_attachment_with_client_id(
    s: &Session,
    to_id: &str,
    is_group: bool,
    file_path: &std::path::Path,
    caption: &str,
    client_id: u64,
) -> Result<(Value, ChatMessage)> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let _permit = crate::limits::UPLOAD_JOBS
        .acquire()
        .await
        .map_err(|_| anyhow::anyhow!("Hàng đợi tải lên đã đóng"))?;
    let mut source = crate::upload::Source::open(file_path).await?;
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("attachment.bin")
        .to_string();
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let is_image = matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
    );
    let is_video = matches!(ext.as_str(), "mp4" | "mov" | "mkv" | "webm" | "avi");

    let file_host = if s.domains.file.is_empty() {
        "https://tt-files-wpa.zaloapp.com"
    } else {
        s.domains.file.trim_end_matches('/')
    };
    let clean_target = if is_group {
        to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G')
    } else {
        to_id
    };
    let total_size = source.size;
    let chunk_size: usize = 1_048_576; // 1 MiB chunks
    let total_chunks = total_size.div_ceil(chunk_size as u64);
    let target_type = if is_group { 11 } else { 2 };

    if is_image {
        let (width, height) = source
            .dimensions
            .ok_or_else(|| anyhow::anyhow!("Không đọc được kích thước hình ảnh"))?;
        let mime = match ext.as_str() {
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            _ => "image/jpeg",
        };
        let upload_ep = if is_group {
            "/api/group/photo_original/upload"
        } else {
            "/api/message/photo_original/upload"
        };

        let mut last_up_dec = Value::Null;
        for idx in 0..total_chunks {
            let chunk = source.next_chunk().await?;
            let chunk_id = idx + 1;
            let mut m = serde_json::Map::new();
            m.insert("totalChunk".into(), json!(total_chunks));
            m.insert("fileName".into(), json!(file_name));
            m.insert("clientId".into(), json!(client_id));
            m.insert("totalSize".into(), json!(total_size));
            m.insert("imei".into(), json!(s.imei));
            m.insert("isE2EE".into(), json!(0));
            m.insert("jxl".into(), json!(0));
            m.insert("chunkId".into(), json!(chunk_id));
            if is_group {
                m.insert("grid".into(), json!(clean_target));
            } else {
                m.insert("toid".into(), json!(clean_target));
            }
            let params = s.encoded_params(&Value::Object(m))?;
            let url = format!(
                "{file_host}{upload_ep}?{}&params={params}&type={target_type}",
                s.common_params()
            );
            let up_raw = s
                .post_multipart(&url, "chunkContent", &file_name, mime, &chunk)
                .await?;
            last_up_dec = confirmed_upload(s.maybe_decrypt(up_raw, s.session_key_bytes()).await)?;
        }

        source.verify().await?;
        let data_node = last_up_dec.get("data").unwrap_or(&last_up_dec);
        let photo_id = pick_str(data_node, &["photoId", "clientFileId"]);
        let normal_url =
            pick_str(data_node, &["normalUrl", "hdUrl", "oriUrl", "url"]).unwrap_or_default();
        let hd_url = pick_str(data_node, &["hdUrl", "normalUrl", "oriUrl"])
            .unwrap_or_else(|| normal_url.clone());
        let thumb_url = pick_str(data_node, &["thumbUrl", "normalUrl", "hdUrl"])
            .unwrap_or_else(|| normal_url.clone());

        let send_resp = if let Some(pid) = photo_id.filter(|_| !normal_url.is_empty()) {
            let send_ep = if is_group {
                "/api/group/photo_original/send"
            } else {
                "/api/message/photo_original/send"
            };
            let pid_val = pid
                .parse::<u64>()
                .map(|n| json!(n))
                .unwrap_or_else(|_| json!(pid));
            let mut send_obj = serde_json::Map::new();
            send_obj.insert("photoId".into(), pid_val);
            send_obj.insert("clientId".into(), json!(client_id));
            send_obj.insert("desc".into(), json!(caption));
            send_obj.insert("width".into(), json!(width));
            send_obj.insert("height".into(), json!(height));
            send_obj.insert("rawUrl".into(), json!(normal_url));
            send_obj.insert("hdUrl".into(), json!(hd_url));
            send_obj.insert("thumbUrl".into(), json!(thumb_url));
            send_obj.insert("oriUrl".into(), json!(normal_url));
            send_obj.insert("normalUrl".into(), json!(normal_url));
            send_obj.insert("hdSize".into(), json!(total_size));
            send_obj.insert("zsource".into(), json!(-1));
            send_obj.insert("jcp".into(), json!("{\"convertible\":true}"));
            send_obj.insert("imei".into(), json!(s.imei));
            if is_group {
                send_obj.insert("grid".into(), json!(clean_target));
                send_obj.insert("visibility".into(), json!(0));
            } else {
                send_obj.insert("toid".into(), json!(clean_target));
            }
            let params = s.encoded_params(&Value::Object(send_obj))?;
            let url = format!("{file_host}{send_ep}?{}", s.common_params());
            let r = s.post_form(&url, format!("params={params}")).await?;
            s.maybe_decrypt(r, s.session_key_bytes()).await
        } else {
            // Fallback: single-shot `/api/message/photo` or `/api/group/sendpt`
            let single_ep = if is_group {
                "/api/group/sendpt"
            } else {
                "/api/message/photo"
            };
            let mut body_obj = serde_json::Map::new();
            body_obj.insert("clientId".into(), json!(client_id));
            body_obj.insert("desc".into(), json!(caption));
            body_obj.insert("width".into(), json!(width));
            body_obj.insert("height".into(), json!(height));
            body_obj.insert("imei".into(), json!(s.imei));
            if is_group {
                body_obj.insert("grid".into(), json!(clean_target));
                body_obj.insert("visibility".into(), json!(0));
            } else {
                body_obj.insert("toid".into(), json!(clean_target));
            }
            let params = s.encoded_params(&Value::Object(body_obj))?;
            let url = format!(
                "{file_host}{single_ep}?{}&params={params}&type={target_type}",
                s.common_params()
            );
            let r = s
                .post_multipart_file(
                    &url,
                    "fileContent",
                    &file_name,
                    mime,
                    source.rewind().await?,
                    total_size,
                )
                .await?;
            s.maybe_decrypt(r, s.session_key_bytes()).await
        };

        let send_resp = confirmed_send(send_resp)?;

        let final_thumb = if !thumb_url.is_empty() {
            thumb_url
        } else {
            format!("local://{}", file_path.display())
        };
        let final_url = if !hd_url.is_empty() {
            hd_url
        } else {
            file_path.display().to_string()
        };
        source.verify().await?;

        let msg_id = send_resp
            .get("data")
            .and_then(|d| wire_identity(d.get("msgId")))
            .unwrap_or_else(|| format!("out_{client_id}"));
        let now_str = crate::ui::clock();
        let display_text = if !caption.trim().is_empty() {
            caption.trim().to_string()
        } else {
            format!("[Hình ảnh] {file_name}")
        };
        let cm = ChatMessage {
            id: msg_id,
            client_message_id: client_id.to_string(),
            sender: "Tôi".into(),
            text: display_text.clone(),
            time: now_str,
            is_outgoing: true,
            ts: client_id as i64,
            link_url: Some(final_url),
            link_title: Some(format!(
                "{file_name} ({width}×{height} · {})",
                format_file_size(total_size)
            )),
            link_desc: if caption.trim().is_empty() {
                None
            } else {
                Some(caption.trim().to_string())
            },
            link_thumb: Some(final_thumb),
            link_src: Some("Hình ảnh Zalo".into()),
            ..Default::default()
        };
        return Ok((send_resp, cm));
    }

    // MP4/MOV use the native video path. Unsupported preparation/upload falls back to a
    // file; waiting for a processed URL fails explicitly instead of uploading again.
    if crate::video_meta::is_video_message(file_path) {
        match send_video_message(
            s,
            file_path,
            &mut source,
            is_group,
            clean_target,
            caption,
            client_id,
        )
        .await
        {
            Ok(VideoSend::Sent(result)) => return Ok(*result),
            Ok(VideoSend::Fallback(reason)) => {
                crate::logger::log_error(
                    "video message",
                    &format!("{reason:#}; sending as a file"),
                );
                source = crate::upload::Source::open(file_path).await?;
            }
            Err(error) => return Err(error),
        }
    }

    // Video / Audio / Document / Archive upload (`asyncfile/upload` -> `asyncfile/msg`, fallback `sharefile_full`)
    let mime = match ext.as_str() {
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "txt" | "md" | "log" => "text/plain",
        _ => "application/octet-stream",
    };
    let checksum = source.checksum.clone();
    let upload_ep = if is_group {
        "/api/group/asyncfile/upload"
    } else {
        "/api/message/asyncfile/upload"
    };

    let mut last_up_dec = Value::Null;
    // WP15-4: ResumableUpload persistence — checkpoint each completed chunk so that
    // a retry after a network drop can resume from the last confirmed chunk boundary.
    let upload_id = format!("asyncfile-{client_id}");
    let mut resumable = crate::upload::ResumableUploadState::new(
        upload_id.clone(),
        file_name.clone(),
        total_size,
        chunk_size,
    );
    // Open store once; errors are non-fatal — upload still proceeds, just without
    // persistence (degraded mode).
    let resume_store = crate::store::MessageStore::open(s.history_path()).ok();
    // Resume from the last checkpoint when it still describes *this* upload:
    // same file, same size, same chunk layout.  A mismatched checkpoint is
    // discarded rather than trusted — the server would reject the chunks.
    let mut resume_from = 0usize;
    let total_chunks_usize = total_chunks as usize;
    if let Some(ref st) = resume_store {
        if let Some(saved) = st.load_upload_state(&upload_id).ok().flatten() {
            if saved.total_size == total_size
                && saved.chunk_size == chunk_size
                && saved.file_name == file_name
                && saved.total_chunks == total_chunks_usize
            {
                // Only trust the contiguous prefix the server confirmed.
                let mut contiguous = 0usize;
                for idx in 0..total_chunks_usize {
                    if saved.completed_chunks.contains(&idx) {
                        contiguous = idx + 1;
                    } else {
                        break;
                    }
                }
                if contiguous > 0 && contiguous < total_chunks_usize {
                    resume_from = contiguous;
                    resumable = saved;
                    source.skip_chunks(resume_from, chunk_size).await?;
                } else if contiguous >= total_chunks_usize {
                    // Every chunk was confirmed but the final response never
                    // reached us — the file id is unknown, so start over.
                    let _ = st.delete_upload_state(&upload_id);
                }
            } else {
                let _ = st.delete_upload_state(&upload_id);
            }
        }
    }
    for idx in resume_from..total_chunks_usize {
        let chunk = source.next_chunk().await?;
        let chunk_id = idx + 1;
        let mut m = serde_json::Map::new();
        m.insert("totalChunk".into(), json!(total_chunks));
        m.insert("fileName".into(), json!(file_name));
        m.insert("clientId".into(), json!(client_id));
        m.insert("totalSize".into(), json!(total_size));
        m.insert("imei".into(), json!(s.imei));
        m.insert("isE2EE".into(), json!(0));
        m.insert("chunkId".into(), json!(chunk_id));
        if is_group {
            m.insert("grid".into(), json!(clean_target));
        } else {
            m.insert("toid".into(), json!(clean_target));
        }
        let params = s.encoded_params(&Value::Object(m))?;
        let url = format!(
            "{file_host}{upload_ep}?{}&params={params}&type={target_type}",
            s.common_params()
        );
        let up_raw = s
            .post_multipart(&url, "chunkContent", &file_name, mime, &chunk)
            .await?;
        last_up_dec = confirmed_upload(s.maybe_decrypt(up_raw, s.session_key_bytes()).await)?;
        // Checkpoint: mark this chunk completed so a retry can skip re-uploading it.
        resumable.mark_chunk_complete(idx);
        if let Some(ref st) = resume_store {
            let _ = st.save_upload_state(&resumable);
        }
    }
    // Upload complete: remove the checkpoint entry.
    if let Some(ref st) = resume_store {
        let _ = st.delete_upload_state(&upload_id);
    }

    source.verify().await?;
    let data_node = last_up_dec.get("data").unwrap_or(&last_up_dec);
    let file_id = pick_str(data_node, &["fileId", "clientFileId"]);
    anyhow::ensure!(
        file_id
            .as_deref()
            .is_none_or(crate::upload_events::valid_id),
        "Máy chủ trả mã tệp không hợp lệ"
    );
    let mut file_url = pick_str(data_node, &["fileUrl", "url", "normalUrl"]).unwrap_or_default();
    // Zalo processes the upload asynchronously and announces the CDN URL with a `file_done` /
    // `video_done` control; Zalo PC sends `asyncfile/msg` only after receiving it.
    if file_url.is_empty() {
        if let Some(fid) = file_id.as_deref() {
            match crate::upload_events::wait(s.account_key(), fid, ASYNC_FILE_URL_WAIT).await {
                Some(url) => file_url = url,
                None => anyhow::bail!("Chưa nhận được đường dẫn tệp đã xử lý; tệp chưa được gửi"),
            }
        }
    }

    anyhow::ensure!(
        !file_url.is_empty(),
        "Máy chủ chưa cấp đường dẫn tệp; tệp chưa được gửi"
    );
    let send_resp = if let Some(fid) = file_id {
        let msg_ep = if is_group {
            "/api/group/asyncfile/msg"
        } else {
            "/api/message/asyncfile/msg"
        };
        let fid_val = fid
            .parse::<u64>()
            .map(|n| json!(n))
            .unwrap_or_else(|_| json!(fid));
        let mut msg_obj = serde_json::Map::new();
        msg_obj.insert("fileId".into(), fid_val);
        if !file_url.is_empty() {
            msg_obj.insert("fileUrl".into(), json!(file_url));
        }
        msg_obj.insert("checksum".into(), json!(checksum));
        msg_obj.insert("checksumSha".into(), json!(""));
        msg_obj.insert("extension".into(), json!(ext));
        msg_obj.insert("totalSize".into(), json!(total_size));
        msg_obj.insert("fileName".into(), json!(file_name));
        msg_obj.insert("clientId".into(), json!(client_id));
        msg_obj.insert("fType".into(), json!(if is_video { 2 } else { 1 }));
        msg_obj.insert("fileCount".into(), json!(0));
        msg_obj.insert("fdata".into(), json!("{}"));
        msg_obj.insert("imei".into(), json!(s.imei));
        if is_group {
            msg_obj.insert("grid".into(), json!(clean_target));
        } else {
            msg_obj.insert("toid".into(), json!(clean_target));
        }
        let params = s.encoded_params(&Value::Object(msg_obj))?;
        let url = format!("{file_host}{msg_ep}?{}", s.common_params());
        let r = s.post_form(&url, format!("params={params}")).await?;
        s.maybe_decrypt(r, s.session_key_bytes()).await
    } else {
        json!({ "error_code": -1 })
    };

    let send_resp = confirmed_send(send_resp)?;

    let final_url = if !file_url.is_empty() {
        file_url
    } else {
        send_resp
            .get("data")
            .and_then(|d| pick_str(d, &["fileUrl", "url", "href"]))
            .unwrap_or_default()
    };
    let msg_id = send_resp
        .get("data")
        .and_then(|d| wire_identity(d.get("msgId")))
        .unwrap_or_else(|| format!("out_{client_id}"));
    let kind_label = if is_video { "Video" } else { "Tập tin" };
    let display_text = if !caption.trim().is_empty() {
        caption.trim().to_string()
    } else {
        format!("[{kind_label}] {file_name}")
    };
    let cm = ChatMessage {
        id: msg_id,
        client_message_id: client_id.to_string(),
        sender: "Tôi".into(),
        text: display_text,
        time: crate::ui::clock(),
        is_outgoing: true,
        ts: client_id as i64,
        link_url: if final_url.is_empty() {
            None
        } else {
            Some(final_url)
        },
        link_title: Some(format!("{file_name} ({})", format_file_size(total_size))),
        link_desc: if caption.trim().is_empty() {
            None
        } else {
            Some(caption.trim().to_string())
        },
        link_src: Some(format!("{kind_label} Zalo")),
        ..Default::default()
    };
    Ok((send_resp, cm))
}

/// Outcome of the video-message path.
enum VideoSend {
    Sent(Box<(Value, ChatMessage)>),
    /// Nothing was sent yet; the caller may send the file through `asyncfile` instead.
    Fallback(anyhow::Error),
}

/// Upload chunks of `source` to a FILE-service upload endpoint with the parameters of the
/// Zalo PC uploader (`buildSendChunksInfo`): `totalChunk, fileName, clientId, totalSize,
/// imei, chunkId, toid|grid` plus `&type=2|11`. Returns the decoded reply of the last chunk.
#[allow(clippy::too_many_arguments)]
async fn upload_chunks(
    s: &Session,
    endpoint: &str,
    file_name: &str,
    mime: &str,
    source: &mut crate::upload::Source,
    is_group: bool,
    target: &str,
    client_id: u64,
) -> Result<Value> {
    let file_host = s.domains.file.trim_end_matches('/');
    anyhow::ensure!(!file_host.is_empty(), "Thiếu máy chủ tệp");
    let total = source.size;
    let chunks = total.div_ceil(crate::upload::CHUNK as u64).max(1);
    let mut last = Value::Null;
    for chunk_id in 1..=chunks {
        let chunk = source.next_chunk().await?;
        let mut m = serde_json::Map::new();
        m.insert("totalChunk".into(), json!(chunks));
        m.insert("fileName".into(), json!(file_name));
        m.insert("clientId".into(), json!(client_id));
        m.insert("totalSize".into(), json!(total));
        m.insert("imei".into(), json!(s.imei));
        m.insert("chunkId".into(), json!(chunk_id));
        m.insert(if is_group { "grid" } else { "toid" }.into(), json!(target));
        let params = s.encoded_params(&Value::Object(m))?;
        let url = format!(
            "{file_host}{endpoint}?{}&params={params}&type={}",
            s.common_params(),
            if is_group { 11 } else { 2 }
        );
        let raw = s
            .post_multipart(&url, "chunkContent", file_name, mime, &chunk)
            .await?;
        last = confirmed_upload(s.maybe_decrypt(raw, s.session_key_bytes()).await)?;
    }
    source.verify().await?;
    Ok(last)
}

/// Send a `msgType` forward message (`/api/message/forward`, `/api/group/forward`) as Zalo PC
/// `forwardMessage` does: `{toId, imei}` or `{grid, visibility}` plus `msgType`, `clientId`
/// and the JSON `msgInfo`.
/// # Errors
/// Returns transport errors and server rejections.
pub async fn send_forward_message(
    s: &Session,
    to_id: &str,
    is_group: bool,
    msg_type: u8,
    info: &Value,
    client_id: u64,
) -> Result<Value> {
    anyhow::ensure!(s.login.is_some() && s.is_active(), "Chưa đăng nhập");
    let file_host = s.domains.file.trim_end_matches('/');
    anyhow::ensure!(!file_host.is_empty(), "Thiếu máy chủ tệp");
    let mut body = serde_json::Map::new();
    let path = if is_group {
        let grid = to_id.trim_start_matches("group_").trim_start_matches('g');
        body.insert("grid".into(), json!(grid));
        body.insert("visibility".into(), json!(0));
        "/api/group/forward"
    } else {
        body.insert("toId".into(), json!(to_id));
        body.insert("imei".into(), json!(s.imei));
        "/api/message/forward"
    };
    body.insert("msgType".into(), json!(msg_type));
    body.insert("clientId".into(), json!(client_id));
    body.insert("msgInfo".into(), json!(info.to_string()));
    let params = s.encoded_params(&Value::Object(body))?;
    let url = format!("{file_host}{path}?{}", s.common_params());
    let response = s.post_form(&url, format!("params={params}")).await?;
    confirmed_send(s.maybe_decrypt(response, s.session_key_bytes()).await)
}

/// Zalo PC video message: upload the file (`video/file/upload`, URL announced by a
/// `video_done` control), upload a poster (`video/thumb/upload`), then send
/// `forward` `msgType 5` with `{videoUrl, thumbUrl, duration, width, height, fileSize,
/// properties, title}` (`sendVideoMessage` / `_forwardMessages`).
async fn send_video_message(
    s: &Session,
    path: &std::path::Path,
    source: &mut crate::upload::Source,
    is_group: bool,
    target: &str,
    caption: &str,
    client_id: u64,
) -> Result<VideoSend> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("video.mp4")
        .to_owned();
    let probe_path = path.to_owned();
    let (info, poster) = tokio::task::spawn_blocking(move || {
        (
            crate::video_meta::probe(&probe_path).unwrap_or_default(),
            crate::video_meta::poster_jpeg(&probe_path),
        )
    })
    .await
    .unwrap_or_default();
    let scope = if is_group { "group" } else { "message" };
    let mime = if file_name.to_ascii_lowercase().ends_with(".mov") {
        "video/quicktime"
    } else {
        "video/mp4"
    };
    let uploaded = match upload_chunks(
        s,
        &format!("/api/{scope}/video/file/upload"),
        &file_name,
        mime,
        source,
        is_group,
        target,
        client_id,
    )
    .await
    {
        Ok(reply) => reply,
        Err(error) => return Ok(VideoSend::Fallback(error.context("Tải video lên thất bại"))),
    };
    let data = uploaded.get("data").unwrap_or(&uploaded);
    let mut video_url = pick_str(data, &["fileUrl", "url"]).unwrap_or_default();
    if video_url.is_empty() {
        if let Some(file_id) = pick_str(data, &["fileId"]) {
            anyhow::ensure!(
                crate::upload_events::valid_id(&file_id),
                "Máy chủ trả mã video không hợp lệ"
            );
            video_url = crate::upload_events::wait(s.account_key(), &file_id, ASYNC_FILE_URL_WAIT)
                .await
                .unwrap_or_default();
        }
    }
    if video_url.is_empty() {
        anyhow::bail!("Máy chủ chưa xử lý xong video; video chưa được gửi");
    }
    let mut thumb_url = String::new();
    if let Some(jpeg) = poster {
        let dir = crate::storage::root().join("video-posters");
        let poster_path = dir.join(format!("{client_id}.jpg"));
        if crate::storage::atomic_write(&poster_path, &jpeg).is_ok() {
            if let Ok(mut poster_source) = crate::upload::Source::open(&poster_path).await {
                let reply = upload_chunks(
                    s,
                    &format!("/api/{scope}/video/thumb/upload"),
                    "thumb.jpg",
                    "image/jpeg",
                    &mut poster_source,
                    is_group,
                    target,
                    next_client_message_id(),
                )
                .await;
                if let Ok(reply) = reply {
                    let d = reply.get("data").unwrap_or(&reply);
                    thumb_url = pick_str(d, &["oriUrl", "hdUrl", "normalUrl", "thumbUrl", "url"])
                        .unwrap_or_default();
                }
            }
            let _ = tokio::fs::remove_file(&poster_path).await;
        }
    }
    let msg_info = json!({
        "videoUrl": video_url,
        "thumbUrl": thumb_url,
        "duration": info.duration_ms.max(1),
        "width": info.width,
        "height": info.height,
        "fileSize": source.size,
        "properties": Value::Null,
        "title": caption.trim(),
    });
    let response = send_forward_message(s, target, is_group, 5, &msg_info, client_id).await?;
    let msg_id = response
        .get("data")
        .and_then(|d| wire_identity(d.get("msgId")))
        .unwrap_or_else(|| format!("out_{client_id}"));
    let mut cm = ChatMessage {
        id: msg_id,
        client_message_id: client_id.to_string(),
        sender: "Tôi".into(),
        text: if caption.trim().is_empty() {
            format!("[Video] {file_name}")
        } else {
            caption.trim().to_owned()
        },
        time: crate::ui::clock(),
        is_outgoing: true,
        ts: client_id as i64,
        link_url: Some(video_url),
        link_thumb: (!thumb_url.is_empty()).then_some(thumb_url),
        link_title: Some(format!("{file_name} ({})", format_file_size(source.size))),
        link_src: Some("Video Zalo".into()),
        kind: crate::message_model::MessageKind::Video,
        ..Default::default()
    };
    cm.wire_ref.fields.insert("msgInfo".into(), msg_info);
    Ok(VideoSend::Sent(Box::new((response, cm))))
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LinkMetadata {
    pub url: String,
    pub title: String,
    pub desc: String,
    pub thumb: String,
    pub src: String,
}

/// Extract the first `https://` or `http://` URL from a message text string.
pub fn extract_first_url(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let pos = trimmed
        .find("https://")
        .or_else(|| trimmed.find("http://"))?;
    let after = &trimmed[pos..];
    let end = after.find(char::is_whitespace).unwrap_or(after.len());
    let url = after[..end].trim_end_matches([')', ']', ',', ';']);
    if url.len() >= 10 {
        Some(url.to_string())
    } else {
        None
    }
}

fn unescape_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

fn extract_meta_attr(tag: &str, attr_name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    for pat in [
        format!("{attr_name}=\""),
        format!("{attr_name}='"),
        format!("{attr_name} = \""),
    ] {
        if let Some(idx) = lower.find(&pat) {
            let quote_char = pat.chars().last().unwrap_or('"');
            let rest = &tag[idx + pat.len()..];
            if let Some(end) = rest.find(quote_char) {
                let val = unescape_html_entities(&rest[..end]);
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// Scrape OpenGraph / Twitter Card / HTML `<meta>` tags directly from a webpage URL.
pub async fn scrape_opengraph_metadata(url: &str) -> Option<LinkMetadata> {
    // Link previews fetch third-party URLs; they must use the same proxy as Zalo traffic.
    let client = crate::proxy::apply(reqwest::Client::builder())
        .ok()?
        .timeout(std::time::Duration::from_millis(3800))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .ok()?;
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (compatible; Discordbot/2.0; +https://discordapp.com)",
        )
        .header("Accept-Language", "vi-VN,vi;q=0.9,en-US;q=0.8")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = crate::limits::response_bytes(resp, 131_072).await.ok()?;
    let take_len = bytes.len().min(131_072);
    let html = String::from_utf8_lossy(&bytes[..take_len]);

    let mut og_title = String::new();
    let mut og_desc = String::new();
    let mut og_thumb = String::new();
    let mut og_site = String::new();

    let mut cursor = html.as_ref();
    while let Some(start) = cursor.find('<') {
        let after = &cursor[start..];
        let Some(end) = after.find('>') else {
            break;
        };
        let tag = &after[..=end];
        cursor = &after[end + 1..];

        if tag.len() >= 5 && tag.as_bytes()[1..5].eq_ignore_ascii_case(b"meta") {
            let key = extract_meta_attr(tag, "property")
                .or_else(|| extract_meta_attr(tag, "name"))
                .unwrap_or_default()
                .to_ascii_lowercase();
            let Some(content) = extract_meta_attr(tag, "content") else {
                continue;
            };
            match key.as_str() {
                "og:title" | "twitter:title" | "lark:url:video_title" if og_title.is_empty() => {
                    og_title = content;
                }
                "og:description" | "twitter:description" | "description" if og_desc.is_empty() => {
                    og_desc = content;
                }
                "og:image" | "twitter:image" | "lark:url:video_cover_image_url"
                    if og_thumb.is_empty() && content.starts_with("http") =>
                {
                    og_thumb = content;
                }
                "og:site_name" if og_site.is_empty() => {
                    og_site = content;
                }
                _ => {}
            }
        } else if og_title.is_empty()
            && tag.len() >= 6
            && tag.as_bytes()[1..6].eq_ignore_ascii_case(b"title")
        {
            if let Some(close) = cursor.find("</") {
                let raw_t = unescape_html_entities(&cursor[..close]);
                if !raw_t.is_empty() {
                    og_title = raw_t;
                }
            }
        }
    }

    let domain = url
        .split("://")
        .nth(1)
        .and_then(|r| r.split('/').next())
        .unwrap_or("Liên kết")
        .to_string();
    Some(LinkMetadata {
        url: url.to_string(),
        title: og_title,
        desc: og_desc,
        thumb: og_thumb,
        src: if !og_site.is_empty() { og_site } else { domain },
    })
}

fn is_placeholder_zalo_thumb(thumb: &str) -> bool {
    thumb.is_empty()
        || thumb.contains("feed_thumb_link")
        || thumb.contains("im_parselink")
        || thumb.contains("aHR0cHM6Ly9yZXMtemFsby56YWRuLnZuL3VwbG9hZC9tZWRpYS8yMDE5LzEwLzE1L2ZlZWRfdGh1bWJfbGlua19fMV9fMTU3MTEzMzEyMjc3OF85Mzc4MC5wbmc=")
}

/// Parse rich link metadata (`title`, `desc`, `thumb`, `src`) via Zalo's `/api/message/parselink`
/// (`cmd 11285`) combined with direct OpenGraph HTML scraping fallback.
pub async fn parse_link_metadata(s: &Session, raw_url: &str) -> Result<LinkMetadata> {
    let url = raw_url.trim().to_string();
    let domain = url
        .split("://")
        .nth(1)
        .and_then(|r| r.split('/').next())
        .unwrap_or("Liên kết")
        .to_string();
    let mut meta = LinkMetadata {
        url: url.clone(),
        title: String::new(),
        desc: String::new(),
        thumb: String::new(),
        src: domain.clone(),
    };

    if s.login.is_some() {
        let file_host = if s.domains.file.is_empty() {
            "https://tt-files-wpa.chat.zalo.me"
        } else {
            s.domains.file.trim_end_matches('/')
        };
        let body = obj(&[
            ("link", json!(url)),
            ("version", json!(1)),
            ("imei", json!(s.imei)),
        ]);
        if let Ok(params) = s.encoded_params(&body) {
            let parse_url = format!(
                "{file_host}/api/message/parselink?{}&params={params}",
                s.common_params()
            );
            if let Ok(resp) = s.get(&parse_url).await {
                let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
                let d = dec
                    .get("data")
                    .and_then(|x| x.get("data").or(Some(x)))
                    .unwrap_or(&dec);
                if let Some(t) = d.get("title").and_then(Value::as_str).map(str::trim) {
                    if !t.is_empty() && t != url {
                        meta.title = t.to_string();
                    }
                }
                if let Some(ds) = d.get("desc").and_then(Value::as_str).map(str::trim) {
                    if !ds.is_empty() && ds != url {
                        meta.desc = ds.to_string();
                    }
                }
                if let Some(th) = d.get("thumb").and_then(Value::as_str).map(str::trim) {
                    if !is_placeholder_zalo_thumb(th) {
                        meta.thumb = th.to_string();
                    }
                }
                if let Some(sr) = d.get("src").and_then(Value::as_str).map(str::trim) {
                    if !sr.is_empty() {
                        meta.src = sr.to_string();
                    }
                }
            }
        }
    }

    if meta.title.is_empty() || meta.thumb.is_empty() || meta.desc.is_empty() {
        if let Some(og) = scrape_opengraph_metadata(&url).await {
            if meta.title.is_empty() && !og.title.is_empty() {
                meta.title = og.title;
            }
            if meta.desc.is_empty() && !og.desc.is_empty() {
                meta.desc = og.desc;
            }
            if meta.thumb.is_empty() && !og.thumb.is_empty() {
                meta.thumb = og.thumb;
            }
            if (meta.src.is_empty() || meta.src == domain) && !og.src.is_empty() {
                meta.src = og.src;
            }
        }
    }

    if meta.title.is_empty() {
        meta.title = domain;
    }
    Ok(meta)
}

/// Send a rich link message (`sendLinkMessage` @ `compact-app-pc.js:14319196`):
/// - 1-1 (`cmd 11286`): `POST {domains.chat}/api/message/link?{common}`
/// - Group (`cmd 11193`): `POST {domains.group}/api/group/sendlink?{common}`
pub async fn send_link_message(
    s: &Session,
    to_id: &str,
    text: &str,
    meta: &LinkMetadata,
    is_group: bool,
) -> Result<Value> {
    send_link_message_with_client_id(s, to_id, text, meta, is_group, next_client_message_id()).await
}

/// Preserve the request client ID for reconciliation with a later server echo.
/// # Errors
/// Returns session, transport or server rejection failures.
pub(crate) async fn send_link_message_with_client_id(
    s: &Session,
    to_id: &str,
    text: &str,
    meta: &LinkMetadata,
    is_group: bool,
    now: u64,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let msg_str = if text.trim().is_empty() {
        meta.url.as_str()
    } else {
        text.trim()
    };
    let title_str = if meta.title.trim().is_empty() {
        meta.url.as_str()
    } else {
        meta.title.trim()
    };
    let media_json = json!({
        "type": 0,
        "count": 0,
        "mediaTitle": "",
        "artist": "",
        "streamUrl": "",
        "stream_icon": ""
    })
    .to_string();

    async {
        if is_group {
            let clean_grid = to_id
                .trim_start_matches("group_")
                .trim_start_matches('g')
                .trim_start_matches('G');
            let body = obj(&[
                ("grid", json!(clean_grid)),
                ("msg", json!(msg_str)),
                ("href", json!(meta.url)),
                ("src", json!(meta.src)),
                ("title", json!(title_str)),
                ("desc", json!(meta.desc)),
                ("thumb", json!(meta.thumb)),
                ("type", json!(0)),
                ("media", json!(media_json)),
                ("clientId", json!(now)),
                ("mentionInfo", json!("[]")),
                ("language", json!("vi")),
                ("ttl", json!(0)),
                ("imei", json!(s.imei)),
            ]);
            let params = s.encoded_params(&body)?;
            let url = format!(
                "{}/api/group/sendlink?{}",
                s.domains.group,
                s.common_params()
            );
            let resp = s.post_form(&url, format!("params={params}")).await?;
            let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    anyhow::bail!("sendlink group error {code}");
                }
            }
            Ok(dec)
        } else {
            let host = if s.domains.chat.contains("-msg-chat.")
                || s.domains.chat.contains("tt-chat2-wpa")
                || s.domains.chat.is_empty()
            {
                "https://tt-chat3-wpa.api.zaloapp.com"
            } else {
                &s.domains.chat
            };
            let body = obj(&[
                ("toId", json!(to_id)),
                ("msg", json!(msg_str)),
                ("href", json!(meta.url)),
                ("src", json!(meta.src)),
                ("title", json!(title_str)),
                ("desc", json!(meta.desc)),
                ("thumb", json!(meta.thumb)),
                ("type", json!(0)),
                ("media", json!(media_json)),
                ("clientId", json!(now)),
                ("mentionInfo", json!("")),
                ("language", json!("vi")),
                ("ttl", json!(0)),
                ("imei", json!(s.imei)),
            ]);
            let params = s.encoded_params(&body)?;
            let url = format!("{host}/api/message/link?{}", s.common_params());
            let resp = s.post_form(&url, format!("params={params}")).await?;
            let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    anyhow::bail!("sendlink error {code}");
                }
            }
            Ok(dec)
        }
    }
    .await
    .and_then(confirmed_send)
}

/// Send a sticker (`sendSticker` in `compact-app-pc.b7bec208c4c8f73e09f8.js:14325410`):
/// - 1-1 (`cmd 11154`): `POST {domains.chat}/api/message/sticker?{common}`
/// - Group (`cmd 11165`): `POST {domains.group}/api/group/sticker?{common}`
pub async fn send_sticker(
    s: &Session,
    to_id: &str,
    sticker_id: u64,
    cate_id: u64,
    sticker_type: u32,
    is_group: bool,
) -> Result<Value> {
    send_sticker_with_client_id(
        s,
        to_id,
        sticker_id,
        cate_id,
        sticker_type,
        is_group,
        next_client_message_id(),
    )
    .await
}

/// Preserve the request client ID for reconciliation with a later server echo.
/// # Errors
/// Returns session, transport or server rejection failures.
pub(crate) async fn send_sticker_with_client_id(
    s: &Session,
    to_id: &str,
    sticker_id: u64,
    cate_id: u64,
    sticker_type: u32,
    is_group: bool,
    now: u64,
) -> Result<Value> {
    anyhow::ensure!(
        cate_id != crate::bundled_stickers::CATEGORY,
        "GIF cục bộ phải được gửi qua luồng attachment"
    );
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }

    let url = if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        let body = obj(&[
            ("grid", json!(clean_grid)),
            ("stickerId", json!(sticker_id)),
            ("cateId", json!(cate_id)),
            ("type", json!(sticker_type)),
            ("clientId", json!(now)),
            ("imei", json!(s.imei)),
            ("zsource", json!(101)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/group/sticker?{}",
            s.domains.group,
            s.common_params()
        );
        let form = format!("params={params}");
        let resp = s.post_form(&url, form).await?;
        let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
        if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
            if code != 0 {
                let msg = resp
                    .get("error_message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                anyhow::bail!("server error {code}: {msg}");
            }
        }
        return Ok(resp);
    } else {
        let host = if s.domains.chat.contains("-msg-chat.")
            || s.domains.chat.contains("tt-chat2-wpa")
            || s.domains.chat.is_empty()
        {
            "https://tt-chat3-wpa.api.zaloapp.com"
        } else {
            &s.domains.chat
        };
        format!("{host}/api/message/sticker?{}", s.common_params())
    };

    let body = obj(&[
        ("toid", json!(to_id)),
        ("stickerId", json!(sticker_id)),
        ("cateId", json!(cate_id)),
        ("type", json!(sticker_type)),
        ("clientId", json!(now)),
        ("imei", json!(s.imei)),
        ("zsource", json!(101)),
    ]);
    let params = s.encoded_params(&body)?;
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let msg = resp
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            anyhow::bail!("server error {code}: {msg}");
        }
    }
    Ok(resp)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StickerPackInfo {
    pub id: u64,
    pub name: String,
    pub total_image: u32,
    pub icon_url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StickerItem {
    pub sticker_id: u64,
    pub cate_id: u64,
    pub sticker_type: u32,
    pub label: String,
}

/// Fetch the complete Zalo Sticker Store catalog (`getStickerCatsV2` -> `getAllCate` in `compact-app-pc.js:14288962`).
///
/// Returns ~390 official Zalo sticker packs sorted by newest/featured first and caches them to
/// `stickers_cache/store_packs.json`.
pub async fn fetch_sticker_store_packs(s: &Session) -> Result<Vec<StickerPackInfo>> {
    let cache_dir = crate::storage::cache_dir();
    let _ = std::fs::create_dir_all(&cache_dir);
    let cache_path = cache_dir.join("store_packs.json");
    if let Ok(raw_cache) = std::fs::read_to_string(&cache_path) {
        if let Ok(cached) = serde_json::from_str::<Vec<StickerPackInfo>>(&raw_cache) {
            if cached.len() >= 20 {
                return Ok(cached);
            }
        }
    }

    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sticker_host = if s.domains.sticker.is_empty() {
        "https://tt-sticker-wpa.chat.zalo.me"
    } else {
        s.domains.sticker.trim_end_matches('/')
    };
    let key = s.session_key_bytes();
    if key.is_empty() {
        anyhow::bail!("thiếu session key để tải kho nhãn dán");
    }
    let enc_empty = crate::crypto::encode_aes(key, "")?;
    let mut encoded_param = String::with_capacity(enc_empty.len() + 16);
    for b in enc_empty.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded_param.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded_param, "%{b:02X}");
            }
        }
    }

    let list_url = format!(
        "{sticker_host}/api/message/sticker/category/list/v2?{}&params={encoded_param}",
        s.common_params()
    );
    let resp = s.get(&list_url).await?;
    let dec = s.maybe_decrypt(resp, key).await;
    let data = dec.get("data").unwrap_or(&dec);

    let ex_ids: std::collections::HashSet<u64> = data
        .get("exCateIds")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();

    let data_url = data
        .get("dataUrl")
        .and_then(Value::as_str)
        .unwrap_or("https://zalo-api.zadn.vn/api/emoticon/pcweb/getAllCate")
        .replace("http://", "https://");

    let cdn_resp = s.http.get(&data_url).send().await?;
    let raw_bytes =
        crate::limits::response_bytes(cdn_resp, crate::limits::INFLATED_BYTES as usize).await?;
    let json_text = if raw_bytes.len() >= 2 && raw_bytes[0] == 0x1f && raw_bytes[1] == 0x8b {
        let bytes = crate::limits::read_inflated(flate2::read::GzDecoder::new(&raw_bytes[..]))?;
        String::from_utf8(bytes)?
    } else {
        String::from_utf8_lossy(&raw_bytes).into_owned()
    };

    let arr: Vec<Value> = serde_json::from_str(&json_text)?;
    let mut packs = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(id) = item.get("id").and_then(Value::as_u64) else {
            continue;
        };
        if id == 0 || ex_ids.contains(&id) {
            continue;
        }
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() || name.to_lowercase().contains("test") {
            continue;
        }
        let total_image = item.get("totalImage").and_then(Value::as_u64).unwrap_or(16) as u32;
        let icon_url = item
            .get("iconUrl")
            .or_else(|| item.get("thumbImg"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .replace("http://", "https://");
        packs.push(StickerPackInfo {
            id,
            name,
            total_image,
            icon_url,
        });
    }

    // Sort newest packs first (`id` descending) so modern Zalo packs appear at the top of the store
    packs.sort_by_key(|a| std::cmp::Reverse(a.id));
    if let Ok(txt) = serde_json::to_string(&packs) {
        let _ = std::fs::write(&cache_path, txt);
    }
    Ok(packs)
}

/// Fetch all stickers belonging to a specific category (`getStickerCategoryDetail` — `cmd 11284`).
pub async fn fetch_sticker_pack_items(
    s: &Session,
    cate_id: u64,
    pack_name: &str,
) -> Result<Vec<StickerItem>> {
    if cate_id == 0 {
        return Ok((1..=30)
            .map(|id| StickerItem {
                sticker_id: id,
                cate_id: 0,
                sticker_type: 7,
                label: format!("Củ hành #{id}"),
            })
            .collect());
    }

    let cache_dir = crate::storage::cache_dir();
    let _ = std::fs::create_dir_all(&cache_dir);
    let cache_path = cache_dir.join(format!("pack_{cate_id}.json"));
    if let Ok(raw_cache) = std::fs::read_to_string(&cache_path) {
        if let Ok(cached) = serde_json::from_str::<Vec<StickerItem>>(&raw_cache) {
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
    }

    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sticker_host = if s.domains.sticker.is_empty() {
        "https://tt-sticker-wpa.chat.zalo.me"
    } else {
        s.domains.sticker.trim_end_matches('/')
    };
    let params = s.encoded_params(&json!({ "cid": cate_id.to_string() }))?;
    let url = format!(
        "{sticker_host}/api/message/sticker/category/sticker_detail?{}&params={params}",
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let arr = dec
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| dec.as_array())
        .cloned()
        .unwrap_or_default();

    let mut items = Vec::with_capacity(arr.len());
    for (idx, it) in arr.iter().enumerate() {
        let Some(sid) = it
            .get("id")
            .or_else(|| it.get("stickerId"))
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let cid = it.get("cateId").and_then(Value::as_u64).unwrap_or(cate_id);
        let stype = it.get("type").and_then(Value::as_u64).unwrap_or(7) as u32;
        items.push(StickerItem {
            sticker_id: sid,
            cate_id: cid,
            sticker_type: stype,
            label: format!("{pack_name} #{}", idx + 1),
        });
    }

    if !items.is_empty() {
        if let Ok(txt) = serde_json::to_string(&items) {
            let _ = std::fs::write(&cache_path, txt);
        }
    }
    Ok(items)
}

/// Normalize a sticker keyword by converting non-breaking spaces (`\u{a0}`), collapsing
/// consecutive whitespace to a single ASCII space, and converting to lowercase.
pub fn normalize_sticker_keyword(s: &str) -> String {
    s.replace('\u{a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Strip Vietnamese diacritics to lowercase ASCII (`simpleStripVietnamese` in `compact-app-pc.js:12955017`).
pub fn strip_vn_for_sticker_kw(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for raw in s.chars() {
        for c in raw.to_lowercase() {
            let mapped = match c {
                'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ'
                | 'ắ' | 'ặ' | 'ẳ' | 'ẵ' => 'a',
                'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' => {
                    'e'
                }
                'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' => 'i',
                'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ'
                | 'ớ' | 'ợ' | 'ở' | 'ỡ' => 'o',
                'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' => {
                    'u'
                }
                'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
                'đ' => 'd',
                '\u{a0}' => ' ',
                other => other,
            };
            out.push(mapped);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Map common unaccented Vietnamese keywords, English terms, or chat slang to the accented
/// Vietnamese form indexed by Zalo's `/api/message/sticker/suggest/stickers` (`cmd 11741`) and
/// `/api/message/sticker/search` (`cmd 11747`) endpoints.
pub fn canonical_accented_vn_for_sticker(stripped: &str) -> Option<&'static str> {
    match stripped {
        "nho" | "nho em" | "nho anh" | "nho qua" | "miss" | "miss u" | "miss you" => Some("nhớ"),
        "khoc" | "huhu" | "huhuhu" | "hic" | "cry" => Some("khóc"),
        "buon" | "buon qua" | "sad" | "tui than" => Some("buồn"),
        "vui" | "vui ve" | "vui qua" | "happy" => Some("vui"),
        "cuoi" | "mac cuoi" | "buon cuoi" => Some("cười"),
        "yeu" | "iu" | "yeu em" | "yeu anh" | "iu em" | "iu anh" | "love" | "love u"
        | "love you" | "tha tim" | "tim" => Some("yêu"),
        "thuong" | "thuong em" | "thuong anh" | "thuong qua" => Some("thương"),
        "gian" | "gian doi" | "doi hon" | "tuc" | "buc" | "buc minh" | "cay" | "angry" => {
            Some("giận")
        }
        "met" | "met qua" | "met moi" | "duoi" | "duoi qua" | "tired" => Some("mệt"),
        "doi" | "doi bung" | "doi qua" | "hungry" => Some("đói"),
        "an" | "an com" | "di an" => Some("ăn cơm"),
        "ca phe" | "cf" | "cafe" | "coffee" => Some("cà phê"),
        "tra sua" | "trasua" => Some("trà sữa"),
        "nhau" | "di nhau" | "bia" | "dzoo" | "dzo" => Some("nhậu"),
        "ngu" | "di ngu" | "buon ngu" | "ngu ngon" | "ngu ngon nhe" | "ngu ngon nha"
        | "good night" | "goodnight" | "gn" | "g9" => Some("ngủ"),
        "chao" | "xin chao" | "chao ban" | "chao anh" | "chao chi" | "chao em" | "he lo"
        | "helo" => Some("xin chào"),
        "cam on" | "cam on ban" | "cam on anh" | "cam on chi" | "cam on em" | "cam on nhieu"
        | "thank" | "thanks" | "thank you" | "tks" | "tkss" | "thank kiu" => Some("cảm ơn"),
        "xin loi" | "xin loi nha" | "xin loi nhe" | "sorry" | "sr" | "so ry" => Some("xin lỗi"),
        "tam biet" | "tam biet nhe" | "bye" | "byebye" | "good bye" | "goodbye" | "bibi" | "pp" => {
            Some("tạm biệt")
        }
        "chuc mung" | "happy birthday" | "hpbd" | "sinh nhat" | "cmsn" => Some("chúc mừng"),
        "tet" | "chuc mung nam moi" | "nam moi" => Some("tết"),
        "co len" | "co gang" | "fighting" => Some("cố lên"),
        "tuyet" | "tuyet voi" | "xuat sac" | "dinh" | "dinh qua" | "chuan" | "duoc" => {
            Some("tuyệt vời")
        }
        "ngai" | "ngai qua" | "ngai nhi" | "xau ho" => Some("ngại"),
        "so" | "so qua" | "scared" => Some("sợ"),
        "lanh" | "lanh qua" | "lanh khong" | "ret" | "ret qua" => Some("lạnh"),
        "nong" | "nong qua" | "nong buc" => Some("nóng"),
        "tien" | "luong" => Some("tiền"),
        "het tien" | "ngheo" | "ngheo qua" | "vi rong" => Some("hết tiền"),
        "quy" | "lay" | "xin" | "van xin" => Some("lạy"),
        "ban" | "ban qua" | "busy" => Some("bận"),
        "chan" | "chan qua" | "chan doi" | "boring" => Some("chán"),
        "cho" | "cho chut" | "cho xiu" | "doi chut" | "doi xiu" | "wait" => Some("chờ chút"),
        "di choi" | "choi" | "dao pho" => Some("đi chơi"),
        "di lam" | "lam viec" | "deadline" | "code" => Some("làm"),
        "hong" | "hong chuyen" | "ngac nhien" => Some("ngạc nhiên"),
        _ => None,
    }
}

/// Build the deduplicated list of query variants (accented + unaccented + canonical synonym)
/// to maximize recall across both Zalo Sticker Suggest (`cmd 11741`) and Search (`cmd 11747`).
pub fn canonical_vn_sticker_queries(keyword: &str) -> Vec<String> {
    let norm = normalize_sticker_keyword(keyword);
    if norm.is_empty() {
        return Vec::new();
    }
    let stripped = strip_vn_for_sticker_kw(&norm);
    let mut queries = vec![norm.clone()];
    if !stripped.is_empty() && stripped != norm && !queries.contains(&stripped) {
        queries.push(stripped.clone());
    }
    if let Some(accented) = canonical_accented_vn_for_sticker(&stripped) {
        let acc_s = accented.to_string();
        if !queries.contains(&acc_s) {
            queries.push(acc_s.clone());
        }
        let acc_stripped = strip_vn_for_sticker_kw(&acc_s);
        if !acc_stripped.is_empty() && !queries.contains(&acc_stripped) {
            queries.push(acc_stripped);
        }
    }
    queries.truncate(4);
    queries
}

/// Fetch official Zalo contextual sticker trigger keywords (`getStickerListKeyWords` — `cmd 11740`).
/// Includes both normalized accented/raw forms and ASCII-folded forms so typing accented or
/// unaccented Vietnamese triggers suggestions reliably.
pub async fn fetch_sticker_suggest_keywords(s: &Session) -> Result<Vec<String>> {
    let cache_dir = crate::storage::cache_dir();
    let _ = std::fs::create_dir_all(&cache_dir);
    let cache_path = cache_dir.join("suggest_keywords_v2.json");
    if let Ok(raw_cache) = std::fs::read_to_string(&cache_path) {
        if let Ok(cached) = serde_json::from_str::<Vec<String>>(&raw_cache) {
            if cached.len() >= 100 {
                return Ok(cached);
            }
        }
    }
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sticker_host = if s.domains.sticker.is_empty() {
        "https://tt-sticker-wpa.chat.zalo.me"
    } else {
        s.domains.sticker.trim_end_matches('/')
    };
    let params = s.encoded_params(&json!({ "imei": s.imei }))?;
    let url = format!(
        "{sticker_host}/api/message/sticker/suggest/keywords?{}&params={params}",
        s.common_params()
    );
    let resp = match s.get(&url).await {
        Ok(r) => r,
        Err(_) => {
            let fallback_url = format!(
                "https://tt-sticker-wpa.zaloapp.com/api/message/sticker/suggest/keywords?{}&params={params}",
                s.common_params()
            );
            s.get(&fallback_url).await?
        }
    };
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let mut seen = std::collections::HashSet::new();
    let mut kws: Vec<String> = Vec::new();
    if let Some(arr) = dec
        .get("data")
        .or(Some(&dec))
        .and_then(|d| d.get("keywords"))
        .and_then(Value::as_array)
    {
        for raw_v in arr {
            let Some(raw_str) = raw_v.as_str() else {
                continue;
            };
            let norm = normalize_sticker_keyword(raw_str);
            if norm.is_empty() || norm.starts_with("zkwd_") {
                continue;
            }
            if seen.insert(norm.clone()) {
                kws.push(norm.clone());
            }
            let stripped = strip_vn_for_sticker_kw(&norm);
            if !stripped.is_empty() && seen.insert(stripped.clone()) {
                kws.push(stripped);
            }
        }
    }
    if !kws.is_empty() {
        if let Ok(txt) = serde_json::to_string(&kws) {
            let _ = std::fs::write(&cache_path, txt);
        }
    }
    Ok(kws)
}

async fn query_sticker_endpoints_once(
    s: &Session,
    sticker_host: &str,
    queries: &[String],
) -> Vec<(u64, u64, u32, u8)> {
    let mut futs = Vec::with_capacity(queries.len() * 2);
    for (q_idx, q) in queries.iter().enumerate() {
        // 1) Contextual suggest endpoint (`getStickerSuggestion` — `cmd 11741`)
        let sugg_body = obj(&[
            ("keyword", json!(q)),
            ("gif", json!(1)),
            ("guggy", json!(0)),
            ("imei", json!(s.imei)),
        ]);
        if let Ok(params) = s.encoded_params(&sugg_body) {
            let url = format!(
                "{sticker_host}/api/message/sticker/suggest/stickers?{}&params={params}",
                s.common_params()
            );
            let is_primary = q_idx == 0;
            futs.push(async move {
                let Ok(resp) = s.get(&url).await else {
                    return Vec::new();
                };
                let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
                let arr = dec
                    .get("data")
                    .or(Some(&dec))
                    .and_then(|d| d.get("sugg_sticker"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut items = Vec::with_capacity(arr.len());
                for it in arr {
                    let Some(sid) = it
                        .get("sticker_id")
                        .or_else(|| it.get("id"))
                        .and_then(Value::as_u64)
                    else {
                        continue;
                    };
                    let cid = it
                        .get("cate_id")
                        .or_else(|| it.get("cateId"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let stype = it.get("type").and_then(Value::as_u64).unwrap_or(7) as u32;
                    items.push((cid, sid, stype, if is_primary { 3u8 } else { 2u8 }));
                }
                items
            });
        }
    }

    let mut search_futs = Vec::with_capacity(queries.len());
    for (q_idx, q) in queries.iter().enumerate() {
        // 2) Full sticker tag search endpoint (`searchSticker` — `cmd 11747`)
        let search_body = obj(&[
            ("keyword", json!(q)),
            ("limit", json!(30)),
            ("srcType", json!(0)),
            ("imei", json!(s.imei)),
        ]);
        if let Ok(params) = s.encoded_params(&search_body) {
            let url = format!(
                "{sticker_host}/api/message/sticker/search?{}&params={params}",
                s.common_params()
            );
            let is_primary = q_idx == 0;
            search_futs.push(async move {
                let Ok(resp) = s.get(&url).await else {
                    return Vec::new();
                };
                let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
                let arr = dec
                    .get("data")
                    .and_then(Value::as_array)
                    .or_else(|| dec.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut items = Vec::with_capacity(arr.len());
                for it in arr {
                    let Some(sid) = it
                        .get("sticker_id")
                        .or_else(|| it.get("id"))
                        .and_then(Value::as_u64)
                    else {
                        continue;
                    };
                    let cid = it
                        .get("cate_id")
                        .or_else(|| it.get("cateId"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let stype = it.get("type").and_then(Value::as_u64).unwrap_or(7) as u32;
                    items.push((cid, sid, stype, if is_primary { 3u8 } else { 2u8 }));
                }
                items
            });
        }
    }

    let (sugg_batches, search_batches) = tokio::join!(
        futures_util::future::join_all(futs),
        futures_util::future::join_all(search_futs)
    );

    // Merge and boost stickers that matched across multiple endpoints/queries
    let mut score_map: std::collections::HashMap<u64, (u64, u32, u8)> =
        std::collections::HashMap::new();
    for batch in sugg_batches.into_iter().chain(search_batches) {
        for (cid, sid, stype, weight) in batch {
            score_map
                .entry(sid)
                .and_modify(|e| {
                    e.2 = e.2.saturating_add(weight);
                    if e.0 == 0 && cid > 0 {
                        e.0 = cid;
                    }
                })
                .or_insert((cid, stype, weight));
        }
    }

    score_map
        .into_iter()
        .map(|(sid, (cid, stype, score))| (cid, sid, stype, score))
        .collect()
}

/// Search Zalo stickers by Vietnamese, English, or emoji keyword by querying both
/// `/api/message/sticker/suggest/stickers` (`cmd 11741`) and `/api/message/sticker/search` (`cmd 11747`)
/// concurrently across accented and unaccented Vietnamese variants.
pub async fn search_stickers_by_keyword(s: &Session, keyword: &str) -> Result<Vec<StickerItem>> {
    let kw = normalize_sticker_keyword(keyword);
    if kw.is_empty() {
        return Ok(Vec::new());
    }
    let cache_dir = crate::storage::cache_dir();
    let _ = std::fs::create_dir_all(&cache_dir);
    let cache_path = cache_dir.join(format!("suggest_v2_{}.json", crate::crypto::md5_hex(&kw)));
    if let Ok(raw_cache) = std::fs::read_to_string(&cache_path) {
        if let Ok(cached) = serde_json::from_str::<Vec<StickerItem>>(&raw_cache) {
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
    }
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let sticker_host = if s.domains.sticker.is_empty() {
        "https://tt-sticker-wpa.chat.zalo.me"
    } else {
        s.domains.sticker.trim_end_matches('/')
    };

    let queries = canonical_vn_sticker_queries(&kw);
    let mut raw_items = query_sticker_endpoints_once(s, sticker_host, &queries).await;

    // If a multi-word sentence/phrase (e.g. "anh ơi đi chơi", "hôm nay mệt quá") had no direct match,
    // automatically fallback to its trailing 2-word phrase (`tail2`) or trailing word (`last_word`).
    if raw_items.is_empty() {
        let words: Vec<&str> = kw.split_whitespace().collect();
        if words.len() >= 2 && words.len() <= 5 {
            let mut fallback_phrases = Vec::new();
            if words.len() >= 3 {
                fallback_phrases.push(format!(
                    "{} {}",
                    words[words.len() - 2],
                    words[words.len() - 1]
                ));
            }
            let last_w = words[words.len() - 1];
            if last_w.chars().count() >= 2 {
                fallback_phrases.push(last_w.to_string());
            }
            for fb in fallback_phrases {
                let fb_queries = canonical_vn_sticker_queries(&fb);
                raw_items = query_sticker_endpoints_once(s, sticker_host, &fb_queries).await;
                if !raw_items.is_empty() {
                    break;
                }
            }
        }
    }

    // Prioritize modern illustrated Zalo character packs (`cate_id` descending, matching Zalo PC's
    // `pointCate` ranking) and exclude legacy round-emoji/photo-meme packs when character packs exist.
    const LEGACY_NON_CHAR_PACKS: &[u64] = &[
        0, 50, 10788, 11190, 11984, 11998, 12002, 12003, 12024, 12658,
    ];
    let has_modern_char = raw_items
        .iter()
        .any(|(cid, _, _, _)| *cid >= 10000 && !LEGACY_NON_CHAR_PACKS.contains(cid));
    if has_modern_char {
        raw_items.retain(|(cid, _, _, _)| *cid >= 10000 && !LEGACY_NON_CHAR_PACKS.contains(cid));
    }
    // Sort by relevance score first (matched in both search + suggest), then newest pack (`cate_id` desc)
    raw_items.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| b.0.cmp(&a.0)));

    // Keep at most 2 stickers from the same pack in the first pass so suggestions show diverse characters,
    // then fill up to 24 items if needed for Sticker Picker search grids.
    let mut pack_counts: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(24);
    let mut deferred = Vec::new();
    for &(cid, sid, stype, _) in &raw_items {
        let cnt = pack_counts.entry(cid).or_insert(0);
        if *cnt >= 2 {
            deferred.push((cid, sid, stype));
            continue;
        }
        *cnt += 1;
        let idx = out.len() + 1;
        out.push(StickerItem {
            sticker_id: sid,
            cate_id: cid,
            sticker_type: stype,
            label: format!("{kw} #{idx}"),
        });
        if out.len() >= 24 {
            break;
        }
    }
    for (cid, sid, stype) in deferred {
        if out.len() >= 24 {
            break;
        }
        let idx = out.len() + 1;
        out.push(StickerItem {
            sticker_id: sid,
            cate_id: cid,
            sticker_type: stype,
            label: format!("{kw} #{idx}"),
        });
    }

    if !out.is_empty() {
        if let Ok(txt) = serde_json::to_string(&out) {
            let _ = std::fs::write(&cache_path, txt);
        }
    }
    Ok(out)
}

/// Send a quote/reply message (`sendQuoteMessage` / `sendGroupQuoteMessage` in `compact-app-pc.b7bec208c4c8f73e09f8.js:14317494`):
/// - 1-1 (`cmd 11771`): `POST {domains.chat}/api/message/quote?{common}`
/// - Group (`cmd 11197`): `POST {domains.group}/api/group/quote?{common}`
///
/// Missing reference metadata fails before sending; uncertain delivery is never replayed.
pub async fn send_quote_message(
    s: &Session,
    to_id: &str,
    text: &str,
    quoted: &ChatMessage,
    is_group: bool,
) -> Result<Value> {
    send_quote_message_with_client_id(s, to_id, text, quoted, is_group, next_client_message_id())
        .await
}

/// Preserve the request client ID for reconciliation with a later server echo.
/// # Errors
/// Returns session, transport or server rejection failures.
pub(crate) async fn send_quote_message_with_client_id(
    s: &Session,
    to_id: &str,
    text: &str,
    quoted: &ChatMessage,
    is_group: bool,
    now: u64,
) -> Result<Value> {
    send_quote_mentions(s, to_id, text, quoted, is_group, now, &[]).await
}

/// Send a quote retaining explicit group mention identities.
/// # Errors
/// Rejects invalid mentions/reference metadata or propagates send failures.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn send_quote_mentions(
    s: &Session,
    to_id: &str,
    text: &str,
    quoted: &ChatMessage,
    is_group: bool,
    now: u64,
    mentions: &[crate::mentions::Mention],
) -> Result<Value> {
    crate::mentions::validate(text, mentions)?;
    anyhow::ensure!(
        is_group || mentions.is_empty(),
        "Mention chỉ hỗ trợ trong nhóm"
    );
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }

    let qmsg_owner = if quoted.is_outgoing {
        s.account_key().to_owned()
    } else {
        wire_identity(quoted.wire_ref.sender.as_ref())
            .or_else(|| {
                (!quoted.sender.is_empty() && quoted.sender.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| quoted.sender.clone())
            })
            .or_else(|| (!is_group).then(|| to_id.to_owned()))
            .context("Thiếu định danh người gửi của tin trích dẫn")?
    };
    anyhow::ensure!(
        !qmsg_owner.is_empty() && qmsg_owner != "0",
        "Thiếu định danh người gửi của tin trích dẫn"
    );
    let qmsg_id_num = server_message_id(quoted)
        .context("Tin chưa có ID máy chủ; hãy chờ xác nhận trước khi trả lời")?
        .parse::<u64>()
        .context("ID tin trích dẫn không hợp lệ")?;
    let qmsg_cli_id = client_message_id(quoted)
        .context("Thiếu client ID của tin trích dẫn; không gửi thay bằng tin thường")?
        .parse::<u64>()
        .context("Client ID tin trích dẫn không hợp lệ")?;
    let qmsg_ts = u64::try_from(quoted.ts)
        .ok()
        .filter(|ts| *ts > 0)
        .context("Thiếu thời gian của tin trích dẫn")?;

    async {
        if quoted.id.starts_with("out_") || quoted.id.starts_with("local_") || quoted.id.is_empty()
        {
            anyhow::bail!("local quote id");
        }
        if is_group {
            let clean_grid = to_id
                .trim_start_matches("group_")
                .trim_start_matches('g')
                .trim_start_matches('G');
            let mut body = obj(&[
                ("grid", json!(clean_grid)),
                ("message", json!(text)),
                ("clientId", json!(now)),
                ("language", json!("vi")),
                ("qmsgOwner", json!(qmsg_owner)),
                ("qmsgId", json!(qmsg_id_num)),
                ("qmsgCliId", json!(qmsg_cli_id)),
                ("qmsgType", json!(1)),
                ("qmsg", json!(quoted.text)),
                ("qmsgTs", json!(qmsg_ts)),
                ("visibility", json!(0)),
                ("ttl", json!(0)),
                ("imei", json!(s.imei)),
                ("zsource", json!(101)),
            ]);
            if !mentions.is_empty() {
                body["mentionInfo"] = json!(serde_json::to_string(mentions)?);
            }
            let params = s.encoded_params(&body)?;
            let url = format!("{}/api/group/quote?{}", s.domains.group, s.common_params());
            let resp = s.post_form(&url, format!("params={params}")).await?;
            let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    anyhow::bail!("quote error {code}");
                }
            }
            Ok(resp)
        } else {
            let host = if s.domains.chat.contains("-msg-chat.")
                || s.domains.chat.contains("tt-chat2-wpa")
                || s.domains.chat.is_empty()
            {
                "https://tt-chat3-wpa.api.zaloapp.com"
            } else {
                &s.domains.chat
            };
            let body = obj(&[
                ("toid", json!(to_id)),
                ("message", json!(text)),
                ("clientId", json!(now)),
                ("qmsgOwner", json!(qmsg_owner)),
                ("qmsgId", json!(qmsg_id_num)),
                ("qmsgCliId", json!(qmsg_cli_id)),
                ("qmsgType", json!(1)),
                ("qmsgTs", json!(qmsg_ts)),
                ("qmsg", json!(quoted.text)),
                ("imei", json!(s.imei)),
                ("ttl", json!(0)),
                ("zsource", json!(101)),
            ]);
            let params = s.encoded_params(&body)?;
            let url = format!("{host}/api/message/quote?{}", s.common_params());
            let resp = s.post_form(&url, format!("params={params}")).await?;
            let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = resp.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    anyhow::bail!("quote error {code}");
                }
            }
            Ok(resp)
        }
    }
    .await
    .and_then(confirmed_send)
}

/// Whether the verified forwarding paths can preserve this message's content.
pub fn can_forward_message(msg: &ChatMessage) -> bool {
    if msg.kind == crate::message_model::MessageKind::Recall || msg.text == "[Tin nhắn đã thu hồi]"
    {
        return false;
    }
    if forwardable_media(msg) {
        return true;
    }
    if msg.sticker_id.is_some_and(|id| id > 0) {
        return true;
    }
    matches!(
        msg.kind,
        crate::message_model::MessageKind::Text | crate::message_model::MessageKind::Unknown
    ) && msg.link_url.is_none()
        && msg.link_thumb.is_none()
        && msg.link_src.is_none()
        && !["[Tập tin]", "[Video]", "[Hình ảnh]", "[Tin nhắn thoại]"]
            .iter()
            .any(|label| msg.text.starts_with(label))
}

/// Photos, videos and files with a downloadable URL are forwarded by re-upload.
pub fn forwardable_media(msg: &ChatMessage) -> bool {
    if msg.kind == crate::message_model::MessageKind::Recall
        || msg.sticker_id.is_some_and(|id| id > 0)
    {
        return false;
    }
    let media = crate::message_view::media(msg);
    match media.kind {
        crate::message_view::MediaKind::Photo => {
            media.original.is_some() || media.preview.is_some()
        }
        crate::message_view::MediaKind::Video | crate::message_view::MediaKind::File => {
            media.original.is_some()
        }
        crate::message_view::MediaKind::None => false,
    }
}

/// Key in `wire_ref.fields` naming the conversation a forwarded message came from.
const FORWARD_SOURCE: &str = "forward_source";

/// Remember where a message being forwarded lives, for media link renewal.
pub fn set_forward_source(msg: &mut ChatMessage, thread: &str, is_group: bool) {
    msg.wire_ref.fields.insert(
        FORWARD_SOURCE.into(),
        json!({"thread": thread, "group": is_group}),
    );
}

/// The source conversation recorded by [`set_forward_source`].
pub fn forward_source(msg: &ChatMessage) -> Option<(String, bool)> {
    let source = msg.wire_ref.fields.get(FORWARD_SOURCE)?;
    let thread = source.get("thread")?.as_str().filter(|t| !t.is_empty())?;
    Some((thread.to_owned(), source.get("group")?.as_bool()?))
}

/// The forward `msgInfo` Zalo PC builds from a received media message
/// (`ForwardService._forwardMessages`). `None` when a required field is missing, e.g. a
/// file without its MD5 checksum, which Zalo PC refuses to forward.
pub fn forward_info(msg: &ChatMessage) -> Option<Value> {
    use crate::message_view::MediaKind;
    let media = crate::message_view::media(msg);
    // Merge the message content, its `params` and a stored msgInfo into one lookup.
    let mut fields = serde_json::Map::new();
    let parse = |v: &Value| match v {
        Value::String(raw) => serde_json::from_str::<Value>(raw).unwrap_or(Value::Null),
        other => other.clone(),
    };
    for key in ["msgInfo", "content"] {
        if let Some(Value::Object(map)) = msg.wire_ref.fields.get(key).map(parse) {
            if let Some(Value::Object(params)) = map.get("params").map(parse) {
                for (k, v) in params {
                    fields.entry(k).or_insert(v);
                }
            }
            for (k, v) in map {
                fields.entry(k).or_insert(v);
            }
        }
    }
    let number = |keys: &[&str]| -> Option<u64> {
        keys.iter().find_map(|k| {
            fields
                .get(*k)
                .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
        })
    };
    let text = |keys: &[&str]| -> Option<String> {
        keys.iter().find_map(|k| {
            fields
                .get(*k)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        })
    };
    match media.kind {
        MediaKind::Photo => {
            let ori = media.original.clone().or(media.preview.clone())?;
            let thumb = media.thumbnail.clone().unwrap_or_else(|| ori.clone());
            Some(json!({
                "title": "",
                "oriUrl": ori,
                "thumbUrl": thumb,
                "hdUrl": text(&["hdUrl", "hd"]).unwrap_or_default(),
                "width": media.width.map(u64::from).or_else(|| number(&["width"])),
                "height": media.height.map(u64::from).or_else(|| number(&["height"])),
                "properties": Value::Null,
                "fileSize": number(&["fileSize"]),
                "hdSize": number(&["hdSize"]),
                "thumbSize": number(&["thumbSize"]),
            }))
        }
        MediaKind::Video => Some(json!({
            "videoUrl": media.original.clone()?,
            "thumbUrl": media.thumbnail.clone().unwrap_or_default(),
            "duration": media.duration.or_else(|| number(&["duration"])).unwrap_or(1).max(1),
            "width": number(&["video_width", "width"]).or(media.width.map(u64::from)),
            "height": number(&["video_height", "height"]).or(media.height.map(u64::from)),
            "fileSize": number(&["fileSize", "file_size"]),
            "properties": Value::Null,
            "title": "",
        })),
        MediaKind::File => {
            let checksum = text(&["checksum"])
                .filter(|c| c.len() == 32 && c.bytes().all(|b| b.is_ascii_hexdigit()))?;
            let name = forward_file_name(msg, &media);
            let extension = text(&["fileExt"]).unwrap_or_else(|| {
                std::path::Path::new(&name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default()
                    .to_owned()
            });
            Some(json!({
                "url": media.original.clone()?,
                "fileName": name,
                "extension": extension,
                "checksum": checksum,
                "size": number(&["fileSize", "size", "totalSize"]),
                "fType": number(&["fType"]).unwrap_or(1),
            }))
        }
        MediaKind::None => None,
    }
}

/// File name for a forwarded media copy: the original name when known, else by kind.
pub fn forward_file_name(msg: &ChatMessage, media: &crate::message_view::Media) -> String {
    let named = [media.name.as_str(), msg.link_title.as_deref().unwrap_or("")]
        .into_iter()
        .map(str::trim)
        .find(|n| !n.is_empty() && n.contains('.'));
    if let Some(name) = named {
        return name.to_owned();
    }
    match media.kind {
        crate::message_view::MediaKind::Video => "video.mp4".into(),
        crate::message_view::MediaKind::File => "tep".into(),
        _ => {
            let url = media
                .original
                .as_deref()
                .or(media.preview.as_deref())
                .unwrap_or("");
            let path = url.split('?').next().unwrap_or("").to_ascii_lowercase();
            let ext = [".png", ".webp", ".gif", ".jpeg"]
                .into_iter()
                .find(|e| path.ends_with(e))
                .unwrap_or(".jpg");
            format!("anh{ext}")
        }
    }
}

/// Forward a message or sticker to another conversation (`forwardMultiMessage` @ `compact-app-pc.js:14323900`).
pub async fn forward_message(
    s: &Session,
    target_id: &str,
    target_is_group: bool,
    msg: &ChatMessage,
) -> Result<Value> {
    forward_message_with_client_id(s, target_id, target_is_group, msg, next_client_message_id())
        .await
}

/// Preserve the request client ID for reconciliation with a later server echo.
/// # Errors
/// Returns session, transport or server rejection failures.
pub(crate) async fn forward_message_with_client_id(
    s: &Session,
    target_id: &str,
    target_is_group: bool,
    msg: &ChatMessage,
    now: u64,
) -> Result<Value> {
    anyhow::ensure!(
        can_forward_message(msg) && !forwardable_media(msg),
        "Chưa hỗ trợ chuyển tiếp trực tiếp loại tin này; media được chuyển tiếp bằng cách tải về rồi gửi lại"
    );
    if let Some(sid) = msg.sticker_id {
        if sid > 0 {
            let cid = msg.sticker_cate_id.unwrap_or(0);
            let stype = msg.sticker_type.unwrap_or(7);
            return send_sticker_with_client_id(
                s,
                target_id,
                sid,
                cid,
                stype,
                target_is_group,
                now,
            )
            .await;
        }
    }
    if target_is_group {
        send_group_message_with_client_id(s, target_id, &msg.text, now).await
    } else {
        send_message_with_client_id(s, target_id, &msg.text, now).await
    }
}

/// Convert Zalo reaction icon code to visual emoji.
pub fn icon_to_emoji(icon: &str) -> &'static str {
    match icon {
        "/-strong" => "👍",
        "/-heart" => "❤️",
        ":>" => "😆",
        ":o" => "😮",
        ":-((" | ":-(( " => "😢",
        ":-h" => "😠",
        _ => "❤️",
    }
}

/// Map a visual reaction emoji or Zalo reaction code to `(rIcon, rType)` (`compact-app-pc.js:10848900`).
pub fn reaction_code_and_type(emoji_or_code: &str) -> (&'static str, i32) {
    match emoji_or_code {
        "" | "/-clear" => ("", -1),
        "👍" | "/-strong" => ("/-strong", 3),
        "❤️" | "❤" | "/-heart" => ("/-heart", 5),
        "😆" | ":>" => (":>", 0),
        "😮" | ":o" => (":o", 32),
        "😢" | ":-(( " | ":-((" => (":-((", 2),
        "😠" | ":-h" => (":-h", 20),
        _ => ("/-heart", 5),
    }
}

/// Send a message reaction (`sendReactionMessage` in `compact-app-pc.b7bec208c4c8f73e09f8.js:14317097`, cmd `12410` / `12411`).
pub async fn send_reaction(
    s: &Session,
    to_id: &str,
    is_group: bool,
    msg: &ChatMessage,
    reaction: &str,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let now = next_client_message_id();

    let (r_icon, r_type) = reaction_code_and_type(reaction);
    let g_msg_id_val = server_message_id(msg).context("Thiếu ID máy chủ của tin nhắn")?;
    let c_msg_id = client_message_id(msg)
        .context("Thiếu client ID của tin nhắn")?
        .parse::<u64>()
        .context("Client ID tin nhắn không hợp lệ")?;

    let inner_msg = json!({
        "rMsg": [{
            "gMsgID": g_msg_id_val,
            "cMsgID": c_msg_id,
            "msgType": reaction_message_type(msg)?
        }],
        "rIcon": r_icon,
        "rType": r_type,
        "source": 6
    })
    .to_string();

    let react_list = json!([{
        "message": inner_msg,
        "clientId": now
    }]);

    let (url, body) = if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        (
            format!(
                "{}/api/group/reaction?{}",
                s.domains.reaction.trim_end_matches('/'),
                s.common_params()
            ),
            obj(&[
                ("react_list", react_list),
                ("grid", json!(clean_grid)),
                ("imei", json!(s.imei)),
            ]),
        )
    } else {
        (
            format!(
                "{}/api/message/reaction?{}",
                s.domains.reaction.trim_end_matches('/'),
                s.common_params()
            ),
            obj(&[("react_list", react_list), ("toid", json!(to_id))]),
        )
    };

    let params = s.encoded_params(&body)?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let mut result = confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)?;
    result["client_action_id"] = json!(now.to_string());
    Ok(result)
}

/// Preserve the PC internal message kind; archive numeric codes are not reaction codes.
pub fn reaction_message_type(msg: &ChatMessage) -> Result<i64> {
    use crate::message_model::MessageKind as K;
    let kind = if msg.kind == K::Unknown {
        if msg.sticker_id.is_some() {
            K::Sticker
        } else {
            match crate::message_view::media(msg).kind {
                crate::message_view::MediaKind::Photo => K::Photo,
                crate::message_view::MediaKind::Video => K::Video,
                crate::message_view::MediaKind::File => K::File,
                _ => K::Text,
            }
        }
    } else {
        msg.kind
    };
    Ok(match kind {
        K::Text => 1,
        K::Photo => 2,
        K::Voice => 3,
        K::Sticker => 4,
        K::Contact | K::Link => 6,
        K::Gif => 7,
        K::Location => 17,
        K::Video => 18,
        K::File => 19,
        _ => anyhow::bail!("Loại tin này chưa hỗ trợ thả cảm xúc"),
    })
}

/// Delete a message for me (`deleteOneOneMessageV2` / `deleteGroupMessageV2` @ `compact-app-pc.js:14373319`, cmd `11779` / `11834`).
pub async fn delete_message_for_me(
    s: &Session,
    to_id: &str,
    is_group: bool,
    msg: &ChatMessage,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let owner_id = if msg.is_outgoing {
        s.account_key().to_owned()
    } else {
        wire_identity(msg.wire_ref.sender.as_ref())
            .or_else(|| (!is_group).then(|| to_id.to_owned()))
            .context("Thiếu định danh người gửi của tin nhắn")?
    };
    let cli_id = client_message_id(msg).context("Thiếu client ID của tin nhắn")?;
    let server_id = server_message_id(msg).context("Thiếu ID máy chủ của tin nhắn")?;

    let msgs_arr = json!([{
        "cliMsgId": cli_id,
        "globalMsgId": server_id,
        "ownerId": owner_id,
        "destId": to_id
    }]);

    if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        let body = obj(&[
            ("grid", json!(clean_grid)),
            ("cliMsgId", json!(now)),
            ("msgs", msgs_arr),
            ("onlyMe", json!(1)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/group/deletemsg?{}",
            s.domains.group,
            s.common_params()
        );
        let resp = s.post_form(&url, format!("params={params}")).await?;
        confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    } else {
        let host = if s.domains.chat.contains("-msg-chat.")
            || s.domains.chat.contains("tt-chat2-wpa")
            || s.domains.chat.is_empty()
        {
            "https://tt-chat3-wpa.api.zaloapp.com"
        } else {
            &s.domains.chat
        };
        let body = obj(&[
            ("toid", json!(to_id)),
            ("cliMsgId", json!(now)),
            ("msgs", msgs_arr),
            ("onlyMe", json!(1)),
            ("imei", json!(s.imei)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!("{host}/api/message/delete?{}", s.common_params());
        let resp = s.post_form(&url, format!("params={params}")).await?;
        confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    }
}

/// Recall / undo an outgoing message for everyone (`undoMessage` @ `compact-app-pc.js:14376599`, cmd `11281` / `11192`).
pub async fn recall_message(
    s: &Session,
    to_id: &str,
    is_group: bool,
    msg: &ChatMessage,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    anyhow::ensure!(msg.is_outgoing, "Chỉ có thể thu hồi tin do bạn gửi");
    let cli_id = client_message_id(msg).context("Thiếu client ID của tin nhắn")?;
    let server_id =
        server_message_id(msg).context("Thiếu ID máy chủ; chưa thể thu hồi tin nhắn")?;

    if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        let body = obj(&[
            ("msgId", json!(server_id)),
            ("grid", json!(clean_grid)),
            ("clientId", json!(now)),
            ("visibility", json!(0)),
            ("cliMsgIdUndo", json!(cli_id)),
            ("imei", json!(s.imei)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/group/undomsg?{}",
            s.domains.group,
            s.common_params()
        );
        let resp = s.post_form(&url, format!("params={params}")).await?;
        confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    } else {
        let host = if s.domains.chat.contains("-msg-chat.")
            || s.domains.chat.contains("tt-chat2-wpa")
            || s.domains.chat.is_empty()
        {
            "https://tt-chat3-wpa.api.zaloapp.com"
        } else {
            &s.domains.chat
        };
        let body = obj(&[
            ("msgId", json!(server_id)),
            ("toid", json!(to_id)),
            ("clientId", json!(now)),
            ("cliMsgIdUndo", json!(cli_id)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!("{host}/api/message/undo?{}", s.common_params());
        let resp = s.post_form(&url, format!("params={params}")).await?;
        confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    }
}

/// Send a contact card to a 1-1 conversation or group (`/api/message/contact` cmd 11774 or `/api/group/contact` cmd 11827).
#[allow(clippy::too_many_arguments)]
pub async fn send_contact_card(
    s: &Session,
    to_id: &str,
    is_group: bool,
    contact_id: &str,
    contact_name: &str,
    phone: &str,
    avatar: &str,
    client_id: u64,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        let body = obj(&[
            ("grid", json!(clean_grid)),
            ("clientId", json!(client_id)),
            ("contactId", json!(contact_id)),
            ("contactName", json!(contact_name)),
            ("phone", json!(phone)),
            ("avatar", json!(avatar)),
            ("visibility", json!(0)),
            ("imei", json!(s.imei)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!("{}/api/group/contact?{}", s.domains.group, s.common_params());
        let resp = s.post_form(&url, format!("params={params}")).await?;
        confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    } else {
        let host = if s.domains.chat.contains("-msg-chat.")
            || s.domains.chat.contains("tt-chat2-wpa")
            || s.domains.chat.is_empty()
        {
            "https://tt-chat3-wpa.api.zaloapp.com"
        } else {
            &s.domains.chat
        };
        let body = obj(&[
            ("toId", json!(to_id)),
            ("clientId", json!(client_id)),
            ("contactId", json!(contact_id)),
            ("contactName", json!(contact_name)),
            ("phone", json!(phone)),
            ("avatar", json!(avatar)),
            ("imei", json!(s.imei)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!("{host}/api/message/contact?{}", s.common_params());
        let resp = s.post_form(&url, format!("params={params}")).await?;
        confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    }
}

/// Target descriptor for multi-undo batch request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UndoTargetItem {
    pub cli_msg_id: u64,
    pub cli_msg_id_undo: String,
    pub global_msg_id: String,
}

/// Batch recall / multi-undo messages for everyone (`undoMultiMessage` cmd 12115 / 12116).
pub async fn multi_undo_messages(
    s: &Session,
    to_id: &str,
    is_group: bool,
    targets: &[UndoTargetItem],
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    if targets.is_empty() {
        return Ok(json!({"error_code": 0}));
    }
    let msgs: Vec<Value> = targets
        .iter()
        .map(|t| {
            json!({
                "cliMsgId": t.cli_msg_id,
                "cliMsgIdUndo": t.cli_msg_id_undo,
                "globalMsgId": t.global_msg_id,
            })
        })
        .collect();

    if is_group {
        let clean_grid = to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G');
        let body = obj(&[
            ("grid", json!(clean_grid)),
            ("msgs", json!(msgs)),
            ("imei", json!(s.imei)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!("{}/api/group/multi_undomsg?{}", s.domains.group, s.common_params());
        let resp = s.post_form(&url, format!("params={params}")).await?;
        confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    } else {
        let host = if s.domains.chat.contains("-msg-chat.")
            || s.domains.chat.contains("tt-chat2-wpa")
            || s.domains.chat.is_empty()
        {
            "https://tt-chat3-wpa.api.zaloapp.com"
        } else {
            &s.domains.chat
        };
        let body = obj(&[
            ("toid", json!(to_id)),
            ("msgs", json!(msgs)),
            ("imei", json!(s.imei)),
        ]);
        let params = s.encoded_params(&body)?;
        let url = format!("{host}/api/message/multi_undo?{}", s.common_params());
        let resp = s.post_form(&url, format!("params={params}")).await?;
        confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
    }
}

/// `GET /api/preloadconvers/get-last-msgs` (cmd 10053).
///
/// [O] request: GET `{convers_api}/api/preloadconvers/get-last-msgs?{common}&params={encodeAES(json)}`
/// where payload is `{ threadIdLocalMsgId: map_object, imei }`.
pub async fn get_last_msgs(s: &Session, thread_ids: &[String]) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let mut map = serde_json::Map::new();
    // [O] `getLastMessagesForPreview(e)` in `compact-app-pc.js:14385933` passes `{}` on preload
    // to fetch all recent conversation previews without overflowing URL length.
    if thread_ids.len() <= 25 {
        for id in thread_ids {
            let clean = id.trim_start_matches("group_").trim_start_matches('g');
            let key = if id.starts_with("group_") || id.starts_with('g') {
                format!("{clean}_1")
            } else {
                format!("{clean}_0")
            };
            map.insert(key, json!("1"));
        }
    }
    let map_str = serde_json::to_string(&map).unwrap_or_else(|_| "{}".into());
    let body = obj(&[
        ("threadIdLocalMsgId", json!(map_str)),
        ("imei", json!(s.imei)),
    ]);
    let params = match s.encoded_params(&body) {
        Ok(p) => format!("params={p}"),
        Err(e) => {
            crate::logger::log_error("chat::get_last_msgs::encoded_params", &e.to_string());
            eprintln!("! get_last_msgs: no session key for encoded_params ({e}), falling back to plain_params");
            Session::plain_params(&body)
        }
    };

    let url = format!(
        "{}/api/preloadconvers/get-last-msgs?{}&{params}",
        s.domains.convers_api,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(resp)
}

/// `GET /api/pinconvers/list` (cmd 12120) — pinned threads.
pub async fn get_pinned(s: &Session) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("imei", json!(s.imei))]);
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/pinconvers/list?{}&params={params}",
        s.domains.conversation,
        s.common_params()
    );
    let response = s.get(&url).await?;
    Ok(s.maybe_decrypt(response, s.session_key_bytes()).await)
}

/// `GET /api/group/history` (cmd 11240) — group message history.
pub async fn get_group_history(s: &Session, group_id: &str, count: u32) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let grid = group_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G');
    let body = obj(&[
        ("grid", json!(grid)),
        ("count", json!(count)),
        ("imei", json!(s.imei)),
    ]);
    let params = match s.encoded_params(&body) {
        Ok(p) => format!("params={p}"),
        Err(e) => {
            crate::logger::log_error("chat::get_group_history::encoded_params", &e.to_string());
            Session::plain_params(&body)
        }
    };
    let url = format!(
        "{}/api/group/history?{}&{params}",
        s.domains.group,
        s.common_params()
    );
    match s.get(&url).await {
        Ok(resp) => Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await),
        Err(_) => get_group_history_raw(s, grid, count).await,
    }
}

/// Byte-exact replica of the shipped bundle's `getHistoryMessage(e, t)`.
///
/// [O] `compact-app-pc:14364564`:
/// `n = {grid: e, count: t}`
/// `getGroupDomain() + "/api/group/history?" + _getCommonParams() + "&params=" + encodeURIComponent(encodeAES(JSON.stringify(n)))`
/// Signing tag: **11240**.
///
/// Note: the bundle sends **only** `grid` and `count` — no `imei`, no `signkey` on this
/// path. Kept as a separate raw probe so the guessed `get_group_history` above can be
/// compared against it without breaking existing callers.
pub async fn get_group_history_raw(s: &Session, grid: &str, count: u32) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    // [O] getGroupIdFromConversationId: strip the group-id prefix if present.
    let grid = grid
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G');
    let body = obj(&[("grid", json!(grid)), ("count", json!(count))]);
    let params = match s.encoded_params(&body) {
        Ok(p) => format!("params={p}"),
        Err(e) => {
            crate::logger::log_error(
                "chat::get_group_history_raw::encoded_params",
                &e.to_string(),
            );
            Session::plain_params(&body)
        }
    };
    let url = format!(
        "{}/api/group/history?{}&{params}",
        s.domains.group,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(resp)
}

/// Unwrap encrypted/stringified JSON data without dropping server errors.
fn response_data(mut value: Value) -> Result<Value> {
    for _ in 0..4 {
        if let Some(code) = value
            .get("error_code")
            .and_then(Value::as_i64)
            .filter(|code| *code != 0)
        {
            anyhow::bail!("server error {code}: metadata unavailable");
        }
        if let Some(text) = value.as_str() {
            value = serde_json::from_str(text)?;
        } else if let Some(data) = value.get("data") {
            value = data.clone();
        } else {
            return Ok(value);
        }
    }
    Ok(value)
}

/// List memberships then fetch full metadata in bounded batches. Server versions
/// are not cached metadata: version zero explicitly requests names and avatars.
pub async fn get_groups(s: &Session) -> Result<Vec<Thread>> {
    if s.login.is_none() {
        return Ok(Vec::new());
    }
    let url = format!(
        "{}/api/group/getlg/v4?{}",
        s.domains.group,
        s.common_params()
    );
    let data = response_data(
        s.maybe_decrypt(s.get(&url).await?, s.session_key_bytes())
            .await,
    )?;
    let map = data
        .get("gridVerMap")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("Danh sách nhóm thiếu gridVerMap"))?;
    let cached = load_threads_cache(s);
    let mut groups: Vec<Thread> = map
        .keys()
        .map(|id| {
            let previous = cached.iter().find(|t| t.is_group && &t.id == id);
            Thread {
                presentation: Default::default(),
                unread: Default::default(),
                id: id.clone(),
                name: previous
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| format!("Nhóm {id}")),
                preview: previous.map(|t| t.preview.clone()).unwrap_or_default(),
                time: previous.map(|t| t.time.clone()).unwrap_or_default(),
                avatar: previous.and_then(|t| t.avatar.clone()),
                is_group: true,
                messages: Vec::new(),
            }
        })
        .collect();
    let removed = crate::group_metadata::refresh(s, map, &mut groups).await?;
    groups.retain(|group| !removed.contains(&group.id));
    Ok(groups)
}

/// Fetch bounded zero-version group metadata for call membership and display names.
///
/// # Errors
/// Propagates transport, authentication and malformed-envelope errors.
pub async fn fetch_group_profiles(
    s: &Session,
    group_ids: &[String],
) -> Result<(
    serde_json::Map<String, Value>,
    std::collections::HashSet<String>,
)> {
    anyhow::ensure!(s.login.is_some() && s.is_active(), "Chưa đăng nhập");
    anyhow::ensure!(group_ids.len() <= 50, "Chỉ tải tối đa 50 nhóm mỗi lần");
    let versions: serde_json::Map<String, Value> = group_ids
        .iter()
        .filter(|id| !id.trim().is_empty())
        .map(|id| (id.clone(), json!(0)))
        .collect();
    let params = s.encoded_params(&json!({"gridVerMap": serde_json::to_string(&versions)?}))?;
    let url = format!(
        "{}/api/group/getmg-v2?{}",
        s.domains.group,
        s.common_params()
    );
    let response = s.post_form(&url, format!("params={params}")).await?;
    let data = response_data(s.maybe_decrypt(response, s.session_key_bytes()).await)?;
    let raw = data
        .get("gridInfoMap")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("Metadata nhóm thiếu gridInfoMap"))?;
    let mut profiles = serde_json::Map::new();
    for (id, value) in raw {
        profiles.insert(id.clone(), response_data(value.clone())?);
    }
    let removed = data
        .get("removedsGroup")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    Ok((profiles, removed))
}

/// Resolve opaque phone-sync user identities through the profile service.
/// # Errors
/// Returns transport, protocol or authentication failures. Never accepts names as identities.
pub(crate) async fn resolve_sync_ids(
    s: &Session,
    ids: &[String],
) -> Result<std::collections::HashMap<String, String>> {
    anyhow::ensure!(
        s.is_active() && s.login.is_some() && ids.len() <= 50,
        "Invalid sync identity request"
    );
    if ids.is_empty() {
        return Ok(Default::default());
    }
    let params =
        s.encoded_params(&json!({"globalUids":serde_json::to_string(ids)?,"imei":s.imei}))?;
    let url = format!(
        "{}/api/gid/decrypt?{}&params={params}",
        s.domains.profile,
        s.common_params()
    );
    let data = response_data(
        s.maybe_decrypt(s.get(&url).await?, s.session_key_bytes())
            .await,
    )?;
    let map = data
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Invalid sync identity envelope"))?;
    Ok(ids
        .iter()
        .filter_map(|id| {
            let uid = pick_str(&json!({"id":map.get(id)?}), &["id"])?;
            // A routing ID must be a real numeric server identity, never an opaque partition.
            (!uid.is_empty() && uid.bytes().all(|c| c.is_ascii_digit()) && uid != "0")
                .then(|| (id.clone(), uid))
        })
        .collect())
}

/// Resolve names for known group IDs without reloading all memberships.
pub(crate) async fn resolve_group_metadata(
    s: &Session,
    groups: &mut [Thread],
) -> Result<std::collections::HashSet<String>> {
    let revision = s.group_revision.load(std::sync::atomic::Ordering::SeqCst);
    let mut removed = std::collections::HashSet::new();
    for chunk in groups.chunks_mut(50) {
        let versions: serde_json::Map<String, Value> =
            chunk.iter().map(|g| (g.id.clone(), json!(0))).collect();
        let body = json!({"gridVerMap": serde_json::to_string(&versions)?});
        let params = s.encoded_params(&body)?;
        let url = format!(
            "{}/api/group/getmg-v2?{}",
            s.domains.group,
            s.common_params()
        );
        let metadata = async {
            let resp = s.post_form(&url, format!("params={params}")).await?;
            response_data(s.maybe_decrypt(resp, s.session_key_bytes()).await)
        }
        .await;
        match metadata {
            Ok(info) => {
                if let Some(info_map) = info.get("gridInfoMap").and_then(Value::as_object) {
                    for group in chunk {
                        if let Some(raw) = info_map.get(&group.id) {
                            if let Ok(info) = response_data(raw.clone()) {
                                group.apply_group_sidebar_metadata(&info);
                                if let Some(name) =
                                    pick_str(&info, &["name", "groupName", "displayName"])
                                        .filter(|n| !n.trim().is_empty())
                                {
                                    group.name = name;
                                }
                                if let Some(avatar) = pick_str(&info, &["avt", "fullAvt", "avatar"])
                                    .filter(|n| !n.is_empty())
                                {
                                    group.avatar = Some(avatar);
                                }
                            }
                        }
                    }
                }
                if let Some(ids) = info.get("removedsGroup").and_then(Value::as_array) {
                    for id in ids {
                        if let Some(id) = id.as_str() {
                            removed.insert(id.to_owned());
                        }
                    }
                }
            }
            Err(e) => {
                if crate::session::is_expired_error(&e.to_string()) {
                    return Err(e);
                }
                // Retain membership and previously verified names on partial outage.
                crate::logger::log_error("group metadata", &e.to_string());
            }
        }
    }
    if revision != s.group_revision.load(std::sync::atomic::Ordering::SeqCst) {
        for group in groups {
            group.presentation.send_policy = None;
        }
    }
    Ok(removed)
}

/// `POST /api/social/friend/getminiprofiles` (cmd 12134) — get basic profiles (name, avatar) for user IDs.
pub async fn get_mini_profiles(
    s: &Session,
    user_ids: &[String],
) -> Result<serde_json::Map<String, Value>> {
    if s.login.is_none() || user_ids.is_empty() {
        return Ok(serde_json::Map::new());
    }
    let body = obj(&[("friend_ids", json!(user_ids)), ("imei", json!(s.imei))]);
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/social/friend/getminiprofiles?{}",
        s.domains.profile,
        s.common_params()
    );
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;

    if let Some(data) = resp.get("data").and_then(Value::as_object) {
        Ok(data.clone())
    } else {
        Ok(serde_json::Map::new())
    }
}

/// Largest roster resolved for the member panel; larger groups show the first ones.
const MEMBER_PANEL_LIMIT: usize = 1000;

/// Load one group's roster (`getmg-v2`) and fill names/avatars with `getminiprofiles`.
/// Returns the members (leaders first) and the server's total member count.
/// # Errors
/// Returns session and group-info failures; missing profiles keep the bare ID.
pub async fn group_members(
    s: &Session,
    group_id: &str,
) -> Result<(Vec<SidebarMember>, Option<u64>)> {
    let clean = group_id
        .trim_start_matches("group_")
        .trim_start_matches('g');
    anyhow::ensure!(
        !clean.is_empty() && clean.bytes().all(|b| b.is_ascii_digit()),
        "Mã nhóm không hợp lệ"
    );
    let (map, _) = fetch_group_profiles(s, &[clean.to_owned()]).await?;
    let info = map
        .get(clean)
        .or_else(|| map.values().next())
        .context("Máy chủ không trả thông tin nhóm")?;
    let mut thread = Thread {
        id: clean.to_owned(),
        is_group: true,
        ..Default::default()
    };
    thread.apply_group_sidebar_metadata(info);
    let mut members = thread.presentation.members;
    members.truncate(MEMBER_PANEL_LIMIT);
    let missing: Vec<String> = members
        .iter()
        .filter(|m| m.name.is_empty() || m.avatar.is_none())
        .map(|m| m.id.clone())
        .collect();
    for chunk in missing.chunks(50) {
        anyhow::ensure!(s.is_active(), "Đã đăng xuất");
        let Ok(profiles) = get_mini_profiles(s, chunk).await else {
            continue;
        };
        for member in members.iter_mut().filter(|m| chunk.contains(&m.id)) {
            if let Some(p) = profiles.get(&member.id) {
                if member.name.is_empty() {
                    member.name =
                        pick_str(p, &["displayName", "zaloName", "name"]).unwrap_or_default();
                }
                if member.avatar.is_none() {
                    member.avatar =
                        pick_str(p, &["avatar", "avt"]).filter(|a| !a.ends_with("/default"));
                }
            }
        }
    }
    Ok((members, thread.presentation.member_count))
}

/// Automatically resolve real display names and avatars for threads that only have placeholder names.
pub async fn resolve_thread_names(s: &Session, threads: &mut [Thread]) {
    let mut unknown_groups: Vec<Thread> = threads
        .iter()
        .filter(|t| {
            t.is_group
                && (t.name.is_empty() || t.name == t.id || t.name == format!("Nhóm {}", t.id))
        })
        .cloned()
        .collect();
    if !unknown_groups.is_empty() {
        if let Err(e) = resolve_group_metadata(s, &mut unknown_groups).await {
            crate::logger::log_error("group name resolution", &e.to_string());
        }
        for resolved in unknown_groups {
            if let Some(thread) = threads
                .iter_mut()
                .find(|t| t.id == resolved.id && t.is_group)
            {
                thread.name = resolved.name;
                thread.avatar = resolved.avatar.or_else(|| thread.avatar.clone());
            }
        }
    }
    let unknown_ids: Vec<String> = threads
        .iter()
        .filter(|t| {
            !t.is_group
                && !t.name.contains("Cloud của tôi")
                && (t.presentation.inbox.official_account.is_none()
                    || t.name.starts_with("Người dùng ")
                    || t.name.is_empty()
                    || t.avatar.as_deref().unwrap_or("").is_empty())
        })
        .map(|t| t.id.clone())
        .collect();

    if !unknown_ids.is_empty() {
        for chunk in unknown_ids.chunks(50) {
            if let Ok(profiles) = get_mini_profiles(s, chunk).await {
                for t in threads.iter_mut() {
                    if let Some(p) = profiles.get(&t.id) {
                        t.presentation.inbox.apply_profile(p);
                        if t.name.starts_with("Người dùng ") || t.name.is_empty() {
                            if let Some(name) =
                                pick_str(p, &["zaloName", "displayName", "name", "display_name"])
                            {
                                if !name.is_empty() {
                                    t.name = name;
                                }
                            }
                        }
                        if let Some(ava) = pick_str(p, &["avatar", "avt", "profile_avatar"]) {
                            if !ava.is_empty() {
                                t.avatar = Some(ava);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Per-queue cursor for `getChatFromPolling` (`MultiQueueId` in module `1iR4`, `sync-v2-sub-worker.js:1620000`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct QueueCursor {
    #[serde(rename = "lastId")]
    pub last_id: String,
    pub ids: Vec<String>,
}

/// State machine for Zalo's native HTTP long-polling message receiver (`getChatFromPolling`, cmd `99100`).
///
/// [O] Observed in `sync-v2-sub-worker.b7bec208c4c8f73e09f8.js` (offsets `1620000`, `3498103`, `14557981`):
/// - `R = new y({ type: "MSG", qosId: 91021, queueIds: [1, 2, 3], ... })`
/// - `getLastActionIds()` builds:
///   `"{lastActionId},{lastActionIdOther}," + encodeURIComponent(btoa(JSON.stringify(queues)) + ",1")`
/// - `getChatFromPolling(params)`:
///   `GET {zpw_chts_new[0]}/?zpw_ver=694&zpw_type=30&params={params}&ts={unix_sec}`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PollState {
    pub last_action_id: String,
    pub last_action_id_other: String,
    pub queues: std::collections::BTreeMap<String, QueueCursor>,
    #[serde(skip)]
    catchup: crate::queue_catchup::CatchupGuard,
}

/// Cursor equality; the runtime catch-up guard is not part of the saved state.
impl PartialEq for PollState {
    fn eq(&self, other: &Self) -> bool {
        self.last_action_id == other.last_action_id
            && self.last_action_id_other == other.last_action_id_other
            && self.queues == other.queues
    }
}

impl Default for PollState {
    fn default() -> Self {
        let mut queues = std::collections::BTreeMap::new();
        for qid in ["1", "2", "3"] {
            queues.insert(
                qid.to_string(),
                QueueCursor {
                    last_id: "1".into(),
                    ids: Vec::new(),
                },
            );
        }
        Self {
            last_action_id: "1".into(),
            last_action_id_other: "1".into(),
            queues,
            catchup: Default::default(),
        }
    }
}

fn max_action_id(a: &str, b: &str) -> String {
    let na = a.parse::<u128>().unwrap_or(0);
    let nb = b.parse::<u128>().unwrap_or(0);
    if nb >= na && nb > 0 {
        b.to_string()
    } else if na > 0 {
        a.to_string()
    } else {
        "1".into()
    }
}

impl PollState {
    /// Whether the periodic HTTP receiver may issue a request after a stalled cursor cooldown.
    pub(crate) fn catchup_ready(&mut self) -> bool {
        self.catchup.ready(std::time::Instant::now())
    }

    fn receive_poll_response(&mut self, response: &Value) {
        self.catchup.seed(self.encode_params());
        self.update_from_response(response);
        let outer = response.get("data").unwrap_or(response);
        let data = outer.get("data").filter(|v| v.is_object()).unwrap_or(outer);
        let more = data.get("more").and_then(Value::as_i64) == Some(1)
            || pick_str(data, &["resetLastActionId"]).is_some_and(|r| r != "0" && !r.is_empty());
        let continuing =
            self.catchup
                .observe(Some(self.encode_params()), more, std::time::Instant::now());
        crate::transfer_diagnostics::queue(
            0,
            (0, 0),
            if !more {
                crate::transfer_diagnostics::QueueDecision::Complete
            } else if continuing {
                crate::transfer_diagnostics::QueueDecision::Continue
            } else {
                crate::transfer_diagnostics::QueueDecision::Repeated
            },
            0,
        );
    }

    pub fn encode_params(&self) -> String {
        use base64::Engine;
        let q_json = serde_json::to_string(&self.queues).unwrap_or_else(|_| "{}".into());
        let b64 = base64::engine::general_purpose::STANDARD.encode(q_json.as_bytes());
        let tail = crate::session::urlencode(&format!("{b64},1"));
        format!(
            "{},{},{}",
            self.last_action_id, self.last_action_id_other, tail
        )
    }

    pub fn update_from_response(&mut self, resp: &Value) {
        let mut data_node = resp.get("data").unwrap_or(resp);
        if let Some(inner) = data_node.get("data").filter(|v| v.is_object()) {
            data_node = inner;
        }
        if let Some(reset_id) = pick_str(data_node, &["resetLastActionId"]) {
            if reset_id != "0" && reset_id.parse::<u128>().unwrap_or(0) > 0 {
                self.last_action_id = reset_id.clone();
                self.last_action_id_other = "1".into();
                for q in self.queues.values_mut() {
                    q.last_id = reset_id.clone();
                    q.ids.clear();
                }
                return;
            }
        }
        if let Some(lai) = pick_str(data_node, &["lastActionId"]) {
            if lai != "0" {
                self.last_action_id = max_action_id(&self.last_action_id, &lai);
            }
        }
        if let Some(laio) = pick_str(data_node, &["lastActionIdOther"]) {
            if laio != "0" {
                self.last_action_id_other = max_action_id(&self.last_action_id_other, &laio);
            }
        }
        if let Some(qs) = data_node.get("queueStatus").and_then(Value::as_object) {
            for (qid, qval) in qs {
                // Message queues the server reports for this session ("6" = 511_0,
                // "15" = 510_1, "16" = 511_1) are tracked from the start of what it kept.
                // Untracked, lp_chat repeats the same `more = 1` page indefinitely (upstream
                // PR #69 evidence 02/11, group queue "16"); lp_chat accepted these queues.
                // Unknown queues ("7", "8") stay untracked.
                if matches!(qid.as_str(), "6" | "15" | "16") && !self.queues.contains_key(qid) {
                    self.queues.insert(
                        qid.clone(),
                        QueueCursor {
                            last_id: "1".into(),
                            ids: Vec::new(),
                        },
                    );
                    continue;
                }
                let Some(entry) = self.queues.get_mut(qid) else {
                    continue;
                };
                if let Some(lid) = pick_str(qval, &["lastId"]) {
                    if lid != "0" {
                        entry.last_id = max_action_id(&entry.last_id, &lid);
                    }
                }
                if let Some(ids_arr) = qval.get("ids").and_then(Value::as_array) {
                    let max_lid = entry.last_id.parse::<u128>().unwrap_or(u128::MAX);
                    let mut new_ids: Vec<String> = ids_arr
                        .iter()
                        .filter_map(|v| match v {
                            Value::String(s) if !s.is_empty() => Some(s.clone()),
                            Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        })
                        .filter(|s| s.parse::<u128>().map(|n| n <= max_lid).unwrap_or(true))
                        .collect();
                    if !new_ids.is_empty() {
                        // [O] `QueueControl.getLastActionIdsArray` (`sync-v2-sub-worker.js:1606950`):
                        // sort descending, take at most 10 <= lastId, then reverse so `ids` is ascending.
                        new_ids.sort_by_key(|s| std::cmp::Reverse(s.parse::<u128>().unwrap_or(0)));
                        new_ids.dedup();
                        new_ids.truncate(10);
                        new_ids.reverse();
                        entry.ids = new_ids;
                    }
                }
            }
        }
    }
}

/// Fetch cloud message history (`getCM` in `sync-v2-sub-worker.b7bec208c4c8f73e09f8.js:14595433`, cmd `12506` / `12507`).
///
/// [O] Observed in module `1EWQ` (`sync-v2-sub-worker.js:1545933`):
/// Cloud message history (`/api/cm/getrecentv2` and `/api/cm/getoldv2` on `group_cloud_message`)
/// is enabled for Group conversations (`g<groupId>`) and My Cloud (`sendToMeId`).
pub async fn get_cloud_messages(
    s: &Session,
    conv_id: &str,
    global_msg_id: &str,
    is_old: bool,
    count: u32,
) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let clean_id = conv_id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G');
    let is_send2me = s
        .login
        .as_ref()
        .map(|l| !l.send2me_id.is_empty() && l.send2me_id == clean_id)
        .unwrap_or(false);
    let mut map = serde_json::Map::new();
    map.insert("groupId".into(), json!(clean_id));
    map.insert(
        "globalMsgId".into(),
        json!(if global_msg_id.is_empty() || global_msg_id == "0" {
            crate::history_sync::NEWEST_CURSOR
        } else {
            global_msg_id
        }),
    );
    map.insert("count".into(), json!(count));
    map.insert("msgIds".into(), json!([]));
    map.insert("imei".into(), json!(s.imei));
    if is_send2me {
        map.insert("isOA".into(), json!(1));
    }
    map.insert(
        "src".into(),
        json!(if global_msg_id.is_empty()
            || global_msg_id == "0"
            || global_msg_id == crate::history_sync::NEWEST_CURSOR
        {
            1
        } else {
            3
        }),
    );

    let body = Value::Object(map);
    let params = s.encoded_params(&body)?;
    let ep = if is_old {
        "/api/cm/getoldv2"
    } else {
        "/api/cm/getrecentv2"
    };
    let host = if s.domains.cloud_msg.is_empty() {
        "https://tt-group-cm.api.zaloapp.com"
    } else {
        &s.domains.cloud_msg
    };
    let url = format!(
        "{}{ep}?{}&params={params}&nretry=0",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let mut dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(d_str) = dec.get("data").and_then(Value::as_str) {
        let trimmed = d_str.trim();
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        {
            if let Ok(parsed_data) = serde_json::from_str::<Value>(trimmed) {
                dec["data"] = parsed_data;
            }
        }
    }
    Ok(dec)
}

/// Call Zalo's native HTTP long-polling message endpoint (`getChatFromPolling`, cmd `99100`).
///
/// Uses a 12-second timeout matching `lp_chat` long-polling windows (`sync-v2-sub-worker.js:3498030`).
pub async fn get_chat_from_polling(s: &Session, state: &mut PollState) -> Result<Value> {
    if !state.catchup.ready(std::time::Instant::now()) {
        return Ok(json!({"error_code":0,"data":{}}));
    }
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let host = if s.domains.lp_chat.is_empty() {
        "https://p4-msg-chat.zaloapp.com"
    } else {
        &s.domains.lp_chat
    };
    let ts = crate::login::unix_now();
    let url = format!(
        "{}/?{}&params={}&ts={ts}",
        host.trim_end_matches('/'),
        s.common_params(),
        state.encode_params()
    );
    let fut = s.get(&url);
    match tokio::time::timeout(std::time::Duration::from_millis(12000), fut).await {
        Ok(Ok(resp)) => {
            let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
            if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
                if code != 0 {
                    let msg = dec
                        .get("error_message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    anyhow::bail!("server error {code}: {msg}");
                }
            }
            state.receive_poll_response(&dec);
            Ok(dec)
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(json!({"error_code": 0, "data": {}})),
    }
}

/// In-memory marks for messages proven to have been read from SQLite.
pub(crate) type MessageFingerprints = std::collections::HashMap<(String, String), u64>;

pub(crate) fn message_fingerprint(message: &ChatMessage) -> Option<u64> {
    use std::hash::Hasher;
    struct Writer(std::collections::hash_map::DefaultHasher);
    impl std::io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.write(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer(std::collections::hash_map::DefaultHasher::new());
    // Display time is reconstructed on hydration and does not change a stored row.
    // Exhaustive destructuring makes new fields an explicit fingerprint decision.
    let ChatMessage {
        outgoing_job,
        kind,
        wire_ref,
        unread_policy,
        delivery,
        id,
        sender,
        text,
        time: _,
        is_outgoing,
        ts,
        sticker_id,
        sticker_cate_id,
        sticker_type,
        reply_to_id,
        reply_to_sender,
        reply_to_text,
        reactions,
        reaction_summaries,
        reaction_snapshot,
        reaction_authoritative_snapshot,
        reaction_snapshot_fresh: _,
        reaction_snapshot_provenance,
        reaction_versions,
        client_message_id,
        is_pinned,
        is_starred,
        text_styles,
        link_url,
        link_title,
        link_desc,
        link_thumb,
        link_src,
        history_source,
        resolved_mention_uids,
        contact_id,
        contact_name,
        contact_phone,
        contact_avatar,
        is_e2ee,
    } = message;
    serde_json::to_writer(
        &mut writer,
        &(
            (
                outgoing_job,
                kind,
                wire_ref,
                unread_policy,
                delivery,
                id,
                sender,
                text,
                is_outgoing,
                ts,
                sticker_id,
                sticker_cate_id,
            ),
            (
                sticker_type,
                reply_to_id,
                reply_to_sender,
                reply_to_text,
                reactions,
                reaction_summaries,
                reaction_snapshot,
                reaction_authoritative_snapshot,
                reaction_snapshot_provenance,
                reaction_versions,
                client_message_id,
                is_pinned,
            ),
            (
                is_starred,
                text_styles,
                link_url,
                link_title,
                link_desc,
                link_thumb,
                link_src,
                history_source,
                resolved_mention_uids,
            ),
            (
                contact_id,
                contact_name,
                contact_phone,
                contact_avatar,
                is_e2ee,
            ),
        ),
    )
    .ok()?;
    Some(writer.0.finish())
}

/// Fingerprint a known-durable snapshot; arbitrary network previews are not a baseline.
#[cfg(test)]
fn message_fingerprints(threads: &[Thread]) -> MessageFingerprints {
    threads
        .iter()
        .flat_map(|t| {
            t.messages.iter().filter_map(|m| {
                message_fingerprint(m).map(|mark| ((t.id.clone(), m.id.clone()), mark))
            })
        })
        .collect()
}

/// Preserve thread metadata while omitting only messages proven unchanged in SQLite.
pub(crate) fn changed_since(threads: &[Thread], before: &MessageFingerprints) -> Vec<Thread> {
    threads
        .iter()
        .map(|thread| Thread {
            id: thread.id.clone(),
            name: thread.name.clone(),
            preview: thread.preview.clone(),
            time: thread.time.clone(),
            avatar: thread.avatar.clone(),
            is_group: thread.is_group,
            presentation: thread.presentation.clone(),
            unread: thread.unread.clone(),
            messages: thread
                .messages
                .iter()
                .filter(|m| {
                    m.id.is_empty()
                        || message_fingerprint(m).is_none_or(|now| {
                            before.get(&(thread.id.clone(), m.id.clone())) != Some(&now)
                        })
                })
                .cloned()
                .collect(),
        })
        .collect()
}

/// Poll and persist for CLI/TUI callers. GUI owns its separate durable commit boundary.
pub async fn poll_new_messages(
    s: &Session,
    state: &mut PollState,
    threads: &mut Vec<Thread>,
) -> Result<()> {
    receive_new_messages(s, state, threads).await?;
    if !s.is_active() || s.account_key().is_empty() {
        return Ok(());
    }
    let account = s.clone();
    let snapshot = threads.clone();
    *threads = tokio::task::spawn_blocking(move || -> Result<Vec<Thread>> {
        anyhow::ensure!(account.is_active(), "Đã đăng xuất");
        let mut snapshot = snapshot;
        let mut store = crate::store::MessageStore::open(account.history_path())?;
        store.filter_deleted(&mut snapshot)?;
        crate::store::persist_threads(&mut store, &snapshot)?;
        save_threads_cache(&account, &snapshot);
        Ok(snapshot)
    })
    .await??;
    Ok(())
}

/// Receive and merge network updates without writing history or cache.
/// # Errors
/// Returns authentication errors. GUI callers persist the batch before committing cursors.
pub(crate) async fn receive_new_messages(
    s: &Session,
    state: &mut PollState,
    threads: &mut Vec<Thread>,
) -> Result<()> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let profile_id = s.account_key();
    // 1. Poll live/offline message queues via `getChatFromPolling` (follow `more == 1` / `resetLastActionId` up to 3 batches)
    for _ in 0..3 {
        let Ok(lp_resp) = get_chat_from_polling(s, state).await else {
            break;
        };
        merge_previews_with_profile(threads, &lp_resp, Some(profile_id));
        let data_node = lp_resp.get("data").unwrap_or(&lp_resp);
        let more = data_node.get("more").and_then(Value::as_i64).unwrap_or(0);
        let has_reset = pick_str(data_node, &["resetLastActionId"])
            .map(|r| r != "0" && !r.is_empty())
            .unwrap_or(false);
        if (more == 0 && !has_reset) || !state.catchup_ready() {
            break;
        }
    }
    // 2. Poll conversation previews via `get-last-msgs`
    if let Ok(preview_resp) = get_last_msgs(s, &[]).await {
        merge_previews_with_profile(threads, &preview_resp, Some(profile_id));
    }
    // Historical group/Cloud pages belong to explicit history sync. Re-reading every
    // group's latest 50 messages here makes startup O(groups), even with a warm database.
    // Live/offline group queues and conversation previews above supply current messages.
    // 3. Resolve display names for any newly discovered threads
    resolve_thread_names(s, threads).await;
    threads.retain(|t| !t.messages.is_empty() || !t.name.starts_with("Người dùng "));
    deduplicate_threads(threads, None);

    Ok(())
}

fn extract_content_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed == "sendBubbleMessage" {
                return None;
            }
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    if let Some(inner) = extract_content_text(&parsed) {
                        if !inner.is_empty() {
                            return Some(inner);
                        }
                    }
                    // It is a JSON object with no text/caption fields (like sticker {catId, id} or photo {href}).
                    // Return None so parse_chat_message can assign a clean descriptive label like [Nhãn dán] or [Hình ảnh]
                    // instead of displaying the raw JSON string in chat.
                    return None;
                }
            }
            Some(s.clone())
        }
        Value::Object(o) => {
            let href = o
                .get("href")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|h| {
                    !h.is_empty() && (h.starts_with("http://") || h.starts_with("https://"))
                });
            for k in ["description", "caption", "text", "msg", "title"] {
                if let Some(Value::String(s)) = o.get(k) {
                    let t = s.trim();
                    if !t.is_empty() && t != "sendBubbleMessage" {
                        return Some(s.clone());
                    }
                }
            }
            if let Some(Value::String(s)) = o.get("fileName").or_else(|| o.get("name")) {
                if !s.trim().is_empty() {
                    return Some(format!("[Tập tin] {s}"));
                }
            }
            if let Some(link) = href {
                return Some(link.to_string());
            }
            None
        }
        _ => None,
    }
}

fn normalize_msg_type(it: &Value) -> String {
    let raw = it
        .get("msgType")
        .or_else(|| it.get("messageType"))
        .or_else(|| it.get("type"));
    match raw {
        Some(Value::String(s)) => s.to_ascii_lowercase(),
        Some(Value::Number(n)) => match n.as_i64() {
            Some(1) => "chat.text".into(),
            Some(2) => "chat.photo".into(),
            Some(3) => "chat.voice".into(),
            Some(4) => "chat.video".into(),
            Some(5) => "chat.sticker".into(),
            Some(19) => "chat.file".into(),
            Some(24) => "chat.location".into(),
            Some(31) => "chat.undo".into(),
            _ => n.to_string(),
        },
        _ => String::new(),
    }
}

fn extract_sticker_info(v: &Value) -> Option<(u64, u64, u32)> {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    return extract_sticker_info(&parsed);
                }
            }
            None
        }
        Value::Object(o) => {
            let parse_u64 = |val: &Value| -> Option<u64> {
                match val {
                    Value::Number(n) => n.as_u64(),
                    Value::String(s) => s.parse::<u64>().ok(),
                    _ => None,
                }
            };
            let cat_id = o
                .get("catId")
                .or_else(|| o.get("cateId"))
                .and_then(parse_u64);
            let stk_id = o
                .get("stickerId")
                .or_else(|| o.get("id"))
                .and_then(parse_u64);
            let stk_type = o
                .get("type")
                .and_then(parse_u64)
                .map(|t| t as u32)
                .unwrap_or(7);
            if let Some(sid) = stk_id {
                if cat_id.is_some()
                    || o.contains_key("stickerId")
                    || o.contains_key("catId")
                    || o.contains_key("cateId")
                {
                    return Some((sid, cat_id.unwrap_or(0), stk_type));
                }
            }
            None
        }
        _ => None,
    }
}

type LinkInfo = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Extract rich link/media card metadata (`link_url`, `link_title`, `link_desc`, `link_thumb`, `link_src`)
/// from a Zalo `chat.recommended` (`recommened.link`) or rich-object `content`.
fn extract_link_info(v: &Value) -> LinkInfo {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    return extract_link_info(&parsed);
                }
            }
            (None, None, None, None, None)
        }
        Value::Object(o) => {
            let href = o
                .get("href")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|h| {
                    !h.is_empty() && (h.starts_with("http://") || h.starts_with("https://"))
                })
                .map(str::to_string);
            let Some(url) = href else {
                return (None, None, None, None, None);
            };
            let thumb = o
                .get("thumb")
                .or_else(|| o.get("thumbUrl"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| {
                    !t.is_empty() && (t.starts_with("http://") || t.starts_with("https://"))
                })
                .map(str::to_string);
            let mut title = o
                .get("title")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string);
            let desc = o
                .get("description")
                .or_else(|| o.get("caption"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_string);
            let mut src = None;
            if let Some(params_str) = o.get("params").and_then(Value::as_str) {
                if let Ok(p_json) = serde_json::from_str::<Value>(params_str.trim()) {
                    if title.is_none() {
                        title = p_json
                            .get("mediaTitle")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string);
                    }
                    src = p_json
                        .get("src")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                }
            }
            if src.is_none() {
                src = url
                    .split("://")
                    .nth(1)
                    .and_then(|rest| rest.split('/').next())
                    .filter(|h| !h.is_empty())
                    .map(str::to_string);
            }
            (Some(url), title, desc, thumb, src)
        }
        _ => (None, None, None, None, None),
    }
}

type ContactInfo = (Option<String>, Option<String>, Option<String>, Option<String>);

fn extract_contact_info(v: &Value) -> ContactInfo {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    return extract_contact_info(&parsed);
                }
            }
            (None, None, None, None)
        }
        Value::Object(o) => {
            let id = o
                .get("contactId")
                .or_else(|| o.get("userId"))
                .or_else(|| o.get("contactUid"))
                .or_else(|| o.get("uid"))
                .and_then(|val| match val {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                });
            let name = o
                .get("contactName")
                .or_else(|| o.get("name"))
                .or_else(|| o.get("displayName"))
                .or_else(|| o.get("dName"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let phone = o
                .get("phone")
                .or_else(|| o.get("phoneNumber"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let avatar = o
                .get("avatar")
                .or_else(|| o.get("thumb"))
                .or_else(|| o.get("avatarUrl"))
                .and_then(Value::as_str)
                .map(str::to_string);
            (id, name, phone, avatar)
        }
        _ => (None, None, None, None),
    }
}

pub(crate) fn parse_chat_message(it: &Value) -> Option<ChatMessage> {
    parse_chat_message_with_profile(it, None)
}

pub(crate) fn parse_chat_message_with_profile(it: &Value, profile_id: Option<&str>) -> Option<ChatMessage> {
    let msg_type = normalize_msg_type(it);

    let raw_content_val = it
        .get("content")
        .or_else(|| {
            it.get("message")
                .and_then(|m| m.get("content"))
                .or_else(|| it.get("message"))
        })
        .or_else(|| it.get("msg"));

    let mut sticker_meta = raw_content_val.and_then(extract_sticker_info);
    if sticker_meta.is_none()
        && (msg_type == "chat.sticker" || msg_type == "sticker" || msg_type == "5")
    {
        sticker_meta = extract_sticker_info(it);
    }
    let (link_url, link_title, link_desc, link_thumb, link_src) = raw_content_val
        .map(extract_link_info)
        .unwrap_or((None, None, None, None, None));
    let (contact_id, contact_name, contact_phone, contact_avatar) = raw_content_val
        .map(extract_contact_info)
        .unwrap_or((None, None, None, None));

    let mut content = raw_content_val
        .and_then(extract_content_text)
        .unwrap_or_default();

    // WP16-3: E2EE receive path — decrypt incoming encrypted messages
    let mut is_e2ee = false;
    if let Some(prof_id) = profile_id {
        if let Some(sender_id) = it.get("uidFrom").or_else(|| it.get("idFrom")).and_then(|v| v.as_str()) {
            if let Some((plaintext, _identity_changed)) = try_decrypt_e2ee_content(prof_id, sender_id, it) {
                content = plaintext;
                is_e2ee = true;
                // TODO: Show identity_changed warning in UI (WP16-3 Mục 3)
            }
        }
    }

    if msg_type == "chat.contact" || msg_type == "contact" || msg_type == "12" {
        if let Some(ref name) = contact_name {
            content = format!("[Danh thiếp] {name}");
        } else {
            content = "[Danh thiếp]".to_string();
        }
    }
    // Styles index the original text; generated labels below never carry styles.
    let content_for_styles = content.clone();
    let is_undo_type = matches!(msg_type.as_str(), "chat.undo" | "undo" | "31");
    if is_undo_type {
        content = "[Tin nhắn đã thu hồi]".to_string();
        sticker_meta = None;
    } else if sticker_meta.is_some() && (content.is_empty() || content.starts_with('{')) {
        content = "[Nhãn dán]".to_string();
    }

    if content.is_empty() {
        content = match msg_type.as_str() {
            "chat.sticker" | "sticker" | "5" => "[Nhãn dán]".to_string(),
            "chat.photo" | "photo" | "2" => "[Hình ảnh]".to_string(),
            "chat.voice" | "voice" | "3" => "[Tin nhắn thoại]".to_string(),
            "chat.video" | "video" | "4" => "[Video]".to_string(),
            "chat.file" | "file" | "19" => "[Tập tin]".to_string(),
            "chat.location" | "location" | "24" => "[Vị trí]".to_string(),
            "chat.contact" | "contact" => "[Danh thiếp]".to_string(),
            "chat.undo" | "undo" | "31" => "[Tin nhắn đã thu hồi]".to_string(),
            "chat.recommended" | "share.file" => "[Chia sẻ]".to_string(),
            _ => {
                let obj = it.get("content").or_else(|| it.get("message"));
                if let Some(o) = obj {
                    if o.get("catId").is_some()
                        || o.get("stickerId").is_some()
                        || o.get("id").is_some()
                    {
                        "[Nhãn dán]".to_string()
                    } else if o.get("href").is_some() || o.get("thumb").is_some() {
                        "[Hình ảnh]".to_string()
                    } else if o.is_object() {
                        "[Tin nhắn]".to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
        };
    }

    let undo_target_id = if is_undo_type {
        raw_content_val.and_then(|rv| match rv {
            Value::Object(_) => pick_str(rv, &["globalMsgId", "msgId", "cliMsgId"]),
            Value::String(s) if s.trim().starts_with('{') => {
                serde_json::from_str::<Value>(s.trim())
                    .ok()
                    .and_then(|parsed| pick_str(&parsed, &["globalMsgId", "msgId", "cliMsgId"]))
            }
            _ => None,
        })
    } else {
        None
    };

    // Preserve the persisted canonical ID contract until a schema-aware migration
    // can replace legacy cliMsgId keys without duplicating cached messages.
    let msg_id = undo_target_id
        .or_else(|| {
            ["msgId", "cliMsgId", "globalMsgId", "lastMsgId", "id"]
                .iter()
                .find_map(|key| wire_identity(it.get(*key)))
        })
        .unwrap_or_default();

    if content.is_empty() && msg_id.is_empty() {
        return None;
    }

    let uid_from = it
        .get("uidFrom")
        .or_else(|| it.get("fromUid"))
        .or_else(|| it.get("fromId"))
        .or_else(|| it.get("senderId"))
        .or_else(|| it.get("sender"))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .unwrap_or_default();

    let display_sender = it
        .get("dName")
        .or_else(|| it.get("displayName"))
        .or_else(|| it.get("fromName"))
        .or_else(|| it.get("senderName"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| uid_from.clone());

    let is_outgoing = uid_from == "0"
        || uid_from.is_empty()
        || it
            .get("isOutgoing")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || it
            .get("is_outgoing")
            .and_then(Value::as_i64)
            .map(|n| n == 1)
            .unwrap_or(false);

    let ts = it
        .get("ts")
        .or_else(|| it.get("cliMsgId"))
        .or_else(|| it.get("sendTime"))
        .or_else(|| it.get("serverTime"))
        .or_else(|| it.get("time"))
        .and_then(|v| match v {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.parse::<i64>().ok(),
            _ => None,
        })
        .unwrap_or(0);

    let time = if ts > 0 {
        crate::clock::message_time(ts)
    } else {
        crate::ui::clock()
    };

    let (reply_to_id, reply_to_sender, reply_to_text) =
        if let Some(q) = it.get("quote").filter(|v| v.is_object()) {
            let q_id = pick_str(q, &["cliMsgId", "globalMsgId", "msgId"]);
            let q_sender = pick_str(q, &["fromD", "dName", "ownerId", "fromUid", "uidFrom"]);
            let q_text = pick_str(q, &["msg", "message", "content"]).or_else(|| {
                q.get("msg")
                    .or_else(|| q.get("content"))
                    .and_then(extract_content_text)
            });
            (q_id, q_sender, q_text)
        } else {
            (None, None, None)
        };

    let text_styles = match raw_content_val {
        Some(value) if content == content_for_styles => {
            crate::rich_text::parse_rtf_styles(value, &content)
        }
        _ => Vec::new(),
    };
    let mut message = ChatMessage {
        kind: if is_undo_type {
            crate::message_model::MessageKind::Recall
        } else {
            crate::message_model::MessageKind::from_wire_name(&msg_type)
        },
        wire_ref: crate::message_model::WireMessageRef::from_raw(it),
        unread_policy: crate::message_model::UnreadPolicy::from_raw(it),
        reaction_summaries: crate::reactions::parse_reactions(it),
        id: msg_id,
        client_message_id: ["cliMsgId", "clientId", "client_message_id"]
            .iter()
            .find_map(|key| wire_identity(it.get(*key)))
            .unwrap_or_default(),
        sender: display_sender,
        text: content,
        time,
        is_outgoing,
        ts,
        sticker_id: sticker_meta.map(|(sid, _, _)| sid),
        sticker_cate_id: sticker_meta.map(|(_, cid, _)| cid),
        sticker_type: sticker_meta.map(|(_, _, st)| st),
        reply_to_id,
        reply_to_sender,
        reply_to_text,
        reactions: extract_reactions(it),
        is_pinned: false,
        is_starred: false,
        link_url,
        link_title,
        link_desc,
        link_thumb,
        link_src,
        text_styles,
        contact_id,
        contact_name,
        contact_phone,
        contact_avatar,
        is_e2ee,
        ..Default::default()
    };
    message.hydrate_media_metadata();
    Some(message)
}

fn extract_reactions(it: &Value) -> Vec<String> {
    let mut res = Vec::new();
    if let Some(r) = it.get("reacts").or_else(|| it.get("reactions")) {
        if let Some(arr) = r.get("reactions").and_then(Value::as_array) {
            for item in arr {
                if let Some(icon) = item.get("rIcon").and_then(Value::as_str) {
                    let em = icon_to_emoji(icon);
                    if !res.contains(&em.to_string()) {
                        res.push(em.to_string());
                    }
                }
            }
        }
    }
    res
}

fn is_group_flag(v: &Value) -> bool {
    v.get("isGroup")
        .map(|g| g.as_bool() == Some(true) || g.as_i64() == Some(1))
        .unwrap_or(false)
}

fn extract_message_thread_id_ext(
    entry: &Value,
    m_val: &Value,
    in_group_bucket: bool,
    cloud_thread_id: Option<&str>,
) -> Option<(String, bool)> {
    let is_group = in_group_bucket || is_group_flag(m_val) || is_group_flag(entry);

    // 1. Explicit `grid` field or `groupMsgs` bucket (`e.msgKey = GROUPID_PREFIX + e.idTo` in `sync-v2-sub-worker.js:3500340`)
    for obj in [m_val, entry] {
        if let Some(grid) = pick_str(obj, &["grid", "groupId"]) {
            if !grid.is_empty() && grid != "0" {
                let clean = grid.trim_start_matches("group_").to_string();
                return Some((clean, true));
            }
        }
    }
    if is_group {
        if let Some(gid) = pick_str(m_val, &["idTo", "toid", "toId", "threadId", "conversId"])
            .or_else(|| pick_str(entry, &["idTo", "toid", "toId", "threadId", "conversId"]))
        {
            if !gid.is_empty() && gid != "0" {
                let clean = gid.trim_start_matches("group_").to_string();
                return Some((clean, true));
            }
        }
    }

    // 2. 1-on-1 or self ("Cloud của tôi") message routing
    // [O] `sync-v2-sub-worker.js:3500260`:
    //   `"0" == e.idTo && "0" == e.uidFrom && (e.idTo = "" + this.userId)`
    // [O] `sync-v2-sub-worker.js:829500` (`_filterDuplicatedMessages`):
    //   `e.isGroup ? e.msgKey : "0" == e.idTo ? e.uidFrom : e.idTo`
    let uid_from = pick_str(m_val, &["uidFrom", "fromUid", "fromId", "senderId"])
        .or_else(|| pick_str(entry, &["uidFrom", "fromUid", "fromId", "senderId"]))
        .unwrap_or_default();
    let id_to = pick_str(m_val, &["idTo", "toid", "toId"])
        .or_else(|| pick_str(entry, &["idTo", "toid", "toId"]))
        .unwrap_or_default();

    if (id_to.is_empty() || id_to == "0") && (uid_from.is_empty() || uid_from == "0") {
        if let Some(cid) = cloud_thread_id {
            if !cid.is_empty() {
                return Some((cid.to_string(), false));
            }
        }
    }

    if !uid_from.is_empty()
        && uid_from != "0"
        && (id_to.is_empty() || id_to == "0" || Some(id_to.as_str()) == cloud_thread_id)
    {
        // Incoming 1-1 message: peer sent it to us -> thread ID is `uid_from`
        Some((uid_from, false))
    } else if !id_to.is_empty() && id_to != "0" {
        // Outgoing 1-1 message or conversation wrapper with explicit `idTo` -> thread ID is `id_to`
        Some((id_to, false))
    } else if !uid_from.is_empty() && uid_from != "0" {
        Some((uid_from, false))
    } else {
        pick_str(
            m_val,
            &["threadId", "conversId", "userId", "friendId", "id"],
        )
        .or_else(|| {
            pick_str(
                entry,
                &["threadId", "conversId", "userId", "friendId", "id"],
            )
        })
        .filter(|s| s != "0" && !s.is_empty())
        .map(|s| (s, is_group))
    }
}

/// Fold a `get-last-msgs` or `getChatFromPolling` response into thread previews and conversation messages.
pub fn merge_previews(threads: &mut Vec<Thread>, resp: &Value) {
    merge_previews_with_profile(threads, resp, None)
}

/// Internal: merge previews with optional profile_id for E2EE decryption
fn merge_previews_with_profile(threads: &mut Vec<Thread>, resp: &Value, profile_id: Option<&str>) {
    let mut entries: Vec<(&Value, bool)> = Vec::new();
    let parsed_str_data: Option<Value> = resp
        .get("data")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str::<Value>(s.trim()).ok());
    let mut data_node = parsed_str_data
        .as_ref()
        .unwrap_or_else(|| resp.get("data").unwrap_or(resp));
    if let Some(inner_data) = data_node
        .get("data")
        .filter(|v| v.is_object() || v.is_array())
    {
        data_node = inner_data;
    }
    let cloud_tid: Option<String> = threads
        .iter()
        .find(|t| t.name.contains("Cloud của tôi"))
        .map(|t| t.id.clone());

    if let Some(arr) = data_node.as_array() {
        for item in arr {
            entries.push((item, is_group_flag(item)));
        }
    } else if let Some(obj) = data_node.as_object() {
        let mut found_list = false;
        for (key, is_grp) in [
            ("msgs", false),
            ("groupMsgs", true),
            ("pageMsgs", false),
            ("conversations", false),
            ("threads", false),
            ("allmsgs", false),
        ] {
            if let Some(arr) = obj.get(key).and_then(Value::as_array) {
                for item in arr {
                    entries.push((item, is_grp || is_group_flag(item)));
                }
                found_list = true;
            }
        }
        if let Some(cu_arr) = obj
            .get("clearUnreads")
            .or_else(|| obj.get("clearUnreadInfo"))
            .or_else(|| obj.get("statusMap").and_then(|v| v.get("clearUnreads")))
            .and_then(Value::as_array)
        {
            for cu in cu_arr {
                let Some(tid) = pick_str(cu, &["idTo", "threadId", "userId"]) else {
                    continue;
                };
                if tid.is_empty() || tid == "0" {
                    continue;
                }
                let is_grp = is_group_flag(cu);
                // Only auto-create 1-1 conversation threads from `clearUnreads`; disbanded groups
                // may linger in `clearUnreads` after removal, whereas active groups come from `get_groups`.
                if !threads.iter().any(|t| {
                    t.id.trim_start_matches('g') == tid.trim_start_matches('g')
                        && t.is_group == is_grp
                }) {
                    threads.push(Thread {
                        presentation: Default::default(),
                        unread: Default::default(),
                        id: tid.clone(),
                        name: format!("Người dùng {tid}"),
                        preview: String::new(),
                        time: String::new(),
                        avatar: None,
                        is_group: is_grp,
                        messages: Vec::new(),
                    });
                }
            }
            found_list = true;
        }
        if !found_list {
            for (_, val) in obj {
                if val.is_object() {
                    entries.push((val, is_group_flag(val)));
                }
            }
        }
    }

    let mut message_indices: Vec<MessageIdentityIndex> = threads
        .iter()
        .map(|thread| {
            let mut index = MessageIdentityIndex::default();
            for (row, message) in thread.messages.iter().enumerate() {
                index.insert(message, row);
            }
            index
        })
        .collect();

    for (entry, is_grp_bucket) in entries {
        let mut raw_msgs: Vec<(&Value, bool)> = Vec::new();
        if let Some(arr) = entry.get("messages").and_then(Value::as_array) {
            for m in arr {
                raw_msgs.push((m, is_grp_bucket || is_group_flag(m)));
            }
        }
        if let Some(arr) = entry.get("msgs").and_then(Value::as_array) {
            for m in arr {
                raw_msgs.push((m, is_grp_bucket || is_group_flag(m)));
            }
        }
        if let Some(arr) = entry.get("groupMsgs").and_then(Value::as_array) {
            for m in arr {
                raw_msgs.push((m, true));
            }
        }
        if let Some(obj) = entry.get("lastMsg").filter(|v| v.is_object()) {
            raw_msgs.push((obj, is_grp_bucket || is_group_flag(obj)));
        }
        if let Some(obj) = entry.get("message").filter(|v| v.is_object()) {
            raw_msgs.push((obj, is_grp_bucket || is_group_flag(obj)));
        }
        if raw_msgs.is_empty()
            && (entry.get("content").is_some()
                || entry.get("msg").is_some()
                || entry.get("msgId").is_some())
        {
            raw_msgs.push((entry, is_grp_bucket));
        }

        for (m_val, is_grp) in raw_msgs {
            let Some(parsed) = parse_chat_message_with_profile(m_val, profile_id) else {
                continue;
            };

            let Some((tid, thread_is_group)) =
                extract_message_thread_id_ext(entry, m_val, is_grp, cloud_tid.as_deref())
            else {
                continue;
            };

            if let Some(thread_index) = threads.iter().position(|thread| thread.id == tid) {
                let t = &mut threads[thread_index];
                t.presentation.inbox.apply(entry);
                let identities = &mut message_indices[thread_index];
                if thread_is_group {
                    t.is_group = true;
                }
                if let Some(row) = identities.find(&t.messages, &parsed) {
                    let existing = &mut t.messages[row];
                    identities.remove(existing, row);
                    existing.merge_reaction_metadata(&parsed);
                    if existing.id.starts_with("out_")
                        && !parsed.id.is_empty()
                        && !parsed.id.starts_with("out_")
                    {
                        existing.id = parsed.id;
                    }
                    if parsed.ts > 0 && existing.ts != parsed.ts {
                        existing.ts = parsed.ts;
                        existing.time = parsed.time;
                    }
                    if parsed.text.trim() == "[Tin nhắn đã thu hồi]" {
                        existing.text = "[Tin nhắn đã thu hồi]".to_string();
                        existing.sticker_id = None;
                        existing.link_url = None;
                        existing.link_thumb = None;
                    }
                    if existing.sticker_id.is_none() && parsed.sticker_id.is_some() {
                        existing.sticker_id = parsed.sticker_id;
                        existing.sticker_cate_id = parsed.sticker_cate_id;
                        existing.sticker_type = parsed.sticker_type;
                    }
                    if existing.reply_to_text.is_none() && parsed.reply_to_text.is_some() {
                        existing.reply_to_id = parsed.reply_to_id;
                        existing.reply_to_sender = parsed.reply_to_sender;
                        existing.reply_to_text = parsed.reply_to_text;
                    }
                    if existing.link_url.is_none() && parsed.link_url.is_some() {
                        existing.link_url = parsed.link_url;
                        existing.link_title = parsed.link_title;
                        existing.link_desc = parsed.link_desc;
                        existing.link_thumb = parsed.link_thumb;
                        existing.link_src = parsed.link_src;
                        if !parsed.text.is_empty() {
                            existing.text = parsed.text;
                        }
                    }
                    if !parsed.reactions.is_empty() {
                        existing.reactions = parsed.reactions;
                    }
                    existing.is_pinned = existing.is_pinned || parsed.is_pinned;
                    existing.is_starred = existing.is_starred || parsed.is_starred;
                    identities.insert(existing, row);
                } else {
                    identities.insert(&parsed, t.messages.len());
                    t.messages.push(parsed);
                }
            } else {
                // New thread detected from incoming message!
                let default_name = if thread_is_group {
                    format!("Nhóm {}", tid)
                } else {
                    format!("Người dùng {}", tid)
                };
                let mut identities = MessageIdentityIndex::default();
                identities.insert(&parsed, 0);
                message_indices.push(identities);
                threads.push(Thread {
                    presentation: SidebarPresentation {
                        inbox: {
                            let mut metadata = crate::inbox::Metadata::default();
                            metadata.apply(entry);
                            metadata
                        },
                        ..Default::default()
                    },
                    unread: Default::default(),
                    id: tid,
                    name: default_name,
                    preview: parsed.text.clone(),
                    time: parsed.time.clone(),
                    avatar: None,
                    is_group: thread_is_group,
                    messages: vec![parsed],
                });
            }
        }
    }

    crate::unread::merge_read_points(threads, data_node);
    crate::unread::merge_mute_metadata(threads, data_node);
    for t in threads.iter_mut() {
        t.unread.observe(&t.messages);
    }
    for t in threads.iter_mut() {
        if !t.messages.is_empty() {
            t.messages.sort_by_key(|m| m.ts);
            if let Some(newest) = t.messages.last() {
                if !newest.text.is_empty() {
                    t.preview = newest.text.replace('\n', " ");
                    t.time = newest.time.clone();
                }
            }
        }
    }
}

/// Merge group history response into a thread's messages, sorted by timestamp and deduplicated by msg_id.
pub fn merge_group_history(thread: &mut Thread, resp: &Value) {
    let mut raw_msgs: Vec<&Value> = Vec::new();
    let parsed_str_data: Option<Value> = resp
        .get("data")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str::<Value>(s.trim()).ok());
    let mut data_node = parsed_str_data
        .as_ref()
        .unwrap_or_else(|| resp.get("data").unwrap_or(resp));
    if let Some(inner_data) = data_node
        .get("data")
        .filter(|v| v.is_object() || v.is_array())
    {
        data_node = inner_data;
    }

    if let Some(arr) = data_node.as_array() {
        raw_msgs.extend(arr);
    } else if let Some(obj) = data_node.as_object() {
        for key in ["msgs", "groupMsgs", "messages"] {
            if let Some(arr) = obj.get(key).and_then(Value::as_array) {
                raw_msgs.extend(arr);
            }
        }
    }

    let clean_thread_id = thread
        .id
        .trim_start_matches("group_")
        .trim_start_matches('g')
        .trim_start_matches('G')
        .to_string();

    let mut identities = MessageIdentityIndex::default();
    for (row, message) in thread.messages.iter().enumerate() {
        identities.insert(message, row);
    }

    for it in raw_msgs {
        if !clean_thread_id.is_empty() {
            if let Some(id_to) = pick_str(it, &["idTo", "toId", "grid", "groupId"]) {
                let clean_to = id_to
                    .trim_start_matches("group_")
                    .trim_start_matches('g')
                    .trim_start_matches('G');
                if !clean_to.is_empty() && clean_to != "0" && clean_to != clean_thread_id {
                    continue;
                }
            }
        }
        if let Some(m) = parse_chat_message(it) {
            if let Some(row) = identities.find(&thread.messages, &m) {
                let existing = &mut thread.messages[row];
                identities.remove(existing, row);
                existing.merge_reaction_metadata(&m);
                if existing.id.starts_with("out_") && !m.id.is_empty() && !m.id.starts_with("out_")
                {
                    existing.id = m.id.clone();
                }
                if existing.link_url.is_none() && m.link_url.is_some() {
                    existing.link_url = m.link_url;
                    existing.link_title = m.link_title;
                    existing.link_desc = m.link_desc;
                    existing.link_thumb = m.link_thumb;
                    existing.link_src = m.link_src;
                    if !m.text.is_empty() {
                        existing.text = m.text;
                    }
                }
                identities.insert(existing, row);
            } else {
                identities.insert(&m, thread.messages.len());
                thread.messages.push(m);
            }
        }
    }

    thread.messages.sort_by_key(|m| m.ts);

    if let Some(newest) = thread.messages.last() {
        if !newest.text.is_empty() {
            thread.preview = newest.text.replace('\n', " ");
            thread.time = newest.time.clone();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum FriendshipStatus {
    #[default]
    Unknown,
    Friend,
    OutgoingPending,
    IncomingPending,
    Stranger,
}

/// Parse `/api/friend/reqstatus` (`cmd 11430`) JSON into `FriendshipStatus`
/// (`compact-app-pc.js:8351970`: `is_friend`, `is_requested` = I requested you, `is_requesting` = you requested me).
pub fn parse_friendship_status_response(resp: &Value) -> FriendshipStatus {
    let mut d = resp;
    for _ in 0..4 {
        if d.get("error_code")
            .is_some_and(|code| crate::api_response::integer(code) != Some(0))
        {
            return FriendshipStatus::Unknown;
        }
        if let Some(inner) = d.get("data").filter(|data| data.is_object()) {
            d = inner;
        } else {
            break;
        }
    }
    let flag = |snake: &str, camel: &str| {
        let value = d.get(snake).or_else(|| d.get(camel))?;
        match value {
            Value::Bool(value) => Some(*value),
            _ => match crate::api_response::integer(value)? {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            },
        }
    };
    if resp
        .get("error_code")
        .is_some_and(|code| crate::api_response::integer(code) != Some(0))
    {
        return FriendshipStatus::Unknown;
    }
    let friend = flag("is_friend", "isFriend");
    let incoming = flag("is_requesting", "isRequesting");
    let outgoing = flag("is_requested", "isRequested");
    if friend == Some(true) {
        return FriendshipStatus::Friend;
    }
    if incoming == Some(true) {
        return FriendshipStatus::IncomingPending;
    }
    if outgoing == Some(true) {
        return FriendshipStatus::OutgoingPending;
    }
    if friend != Some(false) || incoming != Some(false) || outgoing != Some(false) {
        return FriendshipStatus::Unknown;
    }
    FriendshipStatus::Stranger
}

/// Query `/api/friend/reqstatus` (`cmd 11430`) and return the typed `FriendshipStatus`.
pub async fn check_friendship_status(s: &Session, fid: &str) -> Result<FriendshipStatus> {
    let resp = get_friend_request_status(s, fid).await?;
    Ok(parse_friendship_status_response(&resp))
}
/// Check friend request status with a target user (`/api/friend/reqstatus`, cmd `11430`).
pub async fn get_friend_request_status(s: &Session, fid: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("fid", json!(fid)), ("imei", json!(s.imei))]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/reqstatus?{}&params={enc}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.get(&url).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Send a friend request to a target user (`/api/friend/sendreq`, cmd `11424`).
pub async fn send_friend_request(s: &Session, to_id: &str, msg: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[
        ("toid", json!(to_id)),
        ("msg", json!(msg)),
        ("reqsrc", json!(30)),
        ("imei", json!(s.imei)),
        ("language", json!("vi")),
    ]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/sendreq?{}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={enc}")).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let err_msg = dec
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            anyhow::bail!("sendreq error {code}: {err_msg}");
        }
    }
    Ok(dec)
}

/// Accept a pending incoming friend request (`/api/friend/accept`, cmd `11427`).
pub async fn accept_friend_request(s: &Session, fid: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("fid", json!(fid)), ("language", json!("vi"))]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/accept?{}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={enc}")).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            let err_msg = dec
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            anyhow::bail!("accept error {code}: {err_msg}");
        }
    }
    Ok(dec)
}

/// Cancel / undo a pending outgoing friend request (`/api/friend/undo`, cmd `11441`).
pub async fn undo_friend_request(s: &Session, fid: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("fid", json!(fid)), ("imei", json!(s.imei))]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/undo?{}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={enc}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Reject a pending incoming friend request (`/api/friend/reject`, cmd `11428`).
pub async fn reject_friend_request(s: &Session, fid: &str) -> Result<Value> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let body = obj(&[("fid", json!(fid)), ("imei", json!(s.imei))]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        &s.domains.friend
    };
    let url = format!(
        "{}/api/friend/reject?{}",
        host.trim_end_matches('/'),
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={enc}")).await?;
    Ok(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PhoneLookupResult {
    pub user_id: String,
    pub display_name: String,
    pub zalo_name: String,
    pub phone: String,
    pub avatar: String,
    pub status: String,
    #[serde(default)]
    pub friendship: FriendshipStatus,
}

/// Normalize a Vietnamese or international phone string for Zalo's `/api/friend/profile/get`.
///
/// Converts leading `0` to `84` (e.g. `0912345678` -> `84912345678`), strips `+`, spaces, dots, and dashes.
pub fn normalize_phone_for_lookup(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if let Some(rest) = digits.strip_prefix('0') {
        format!("84{rest}")
    } else {
        digits
    }
}

/// Look up a Zalo user profile by phone number (`getUserByPhone` in `compact-app-pc.js:14306443`, cmd `11420`).
///
/// Endpoint: `GET {domains.friend}/api/friend/profile/get?{common}&params={encodeAES({ phone, avatar_size: 240, language: "vi", imei, reqSrc: 40 })}`
pub async fn find_user_by_phone(s: &Session, raw_phone: &str) -> Result<PhoneLookupResult> {
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let normalized = normalize_phone_for_lookup(raw_phone);
    if normalized.len() < 8 {
        anyhow::bail!("Số điện thoại không hợp lệ (cần ít nhất 9 chữ số)");
    }
    let body = obj(&[
        ("phone", json!(normalized)),
        ("avatar_size", json!(240)),
        ("language", json!("vi")),
        ("imei", json!(s.imei)),
        ("reqSrc", json!(40)),
    ]);
    let enc = s.encoded_params(&body)?;
    let host = if s.domains.friend.is_empty() {
        "https://tt-friend-wpa.chat.zalo.me"
    } else {
        s.domains.friend.trim_end_matches('/')
    };
    let url = format!(
        "{host}/api/friend/profile/get?{}&params={enc}",
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let dec = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    if let Some(code) = dec.get("error_code").and_then(Value::as_i64) {
        if code != 0 {
            if code == 216 {
                anyhow::bail!(
                    "Số điện thoại chưa đăng ký Zalo hoặc người dùng đã chặn tìm kiếm qua SĐT (mã 216)"
                );
            }
            let err_msg = dec
                .get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("Không tìm thấy tài khoản");
            anyhow::bail!("{err_msg} (mã {code})");
        }
    }

    let data = dec.get("data").unwrap_or(&dec);
    let Some(uid) = pick_str(data, &["uid", "userId", "id", "globalId"]) else {
        anyhow::bail!("Không tìm thấy tài khoản Zalo ứng với số điện thoại này");
    };
    let display_name = pick_str(
        data,
        &["display_name", "displayName", "zalo_name", "zaloName"],
    )
    .unwrap_or_else(|| raw_phone.trim().to_string());
    let zalo_name = pick_str(
        data,
        &["zalo_name", "zaloName", "display_name", "displayName"],
    )
    .unwrap_or_else(|| display_name.clone());
    let avatar = pick_str(data, &["avatar", "avt"]).unwrap_or_default();
    let status = pick_str(data, &["status"]).unwrap_or_default();

    let friendship = check_friendship_status(s, &uid)
        .await
        .unwrap_or(FriendshipStatus::Unknown);

    Ok(PhoneLookupResult {
        user_id: uid,
        display_name,
        zalo_name,
        phone: raw_phone.trim().to_string(),
        avatar,
        status,
        friendship,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unauthenticated_calls_fail_cleanly() {
        let s = Session::new(false, false, false).unwrap();
        let err1 = get_friends(&s).await.unwrap_err();
        assert_eq!(err1.to_string(), "chưa đăng nhập");

        let err2 = get_last_msgs(&s, &["test_tid".into()]).await.unwrap_err();
        assert_eq!(err2.to_string(), "chưa đăng nhập");

        let err3 = send_message(&s, "test_to", "hello").await.unwrap_err();
        assert_eq!(err3.to_string(), "chưa đăng nhập");

        let err4 = get_pinned(&s).await.unwrap_err();
        assert_eq!(err4.to_string(), "chưa đăng nhập");

        let err5 = get_group_history(&s, "test_gid", 20).await.unwrap_err();
        assert_eq!(err5.to_string(), "chưa đăng nhập");
    }

    fn sample_thread(id: &str) -> Thread {
        Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: id.into(),
            name: format!("Name {id}"),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: Vec::new(),
        }
    }

    #[test]
    fn merge_thread_into_matches_exact_client_reference() {
        let m = |id: &str, text: &str, ts: i64, out: bool| ChatMessage {
            id: id.into(),
            client_message_id: if id == "out_9" || id == "77" {
                "client-9".into()
            } else {
                String::new()
            },
            text: text.into(),
            ts,
            time: format!("{}", ts / 60_000),
            is_outgoing: out,
            ..Default::default()
        };
        let mut target = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "t".into(),
            name: "A".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: vec![
                m("1", "chào", 1_000_000, false),
                m("out_9", "ok", 2_000_000, true),
            ],
        };
        let incoming = Thread {
            messages: vec![
                m("1", "chào", 1_000_000, false),   // same id -> no duplicate
                m("77", "ok", 2_002_000, true),     // exact client reference -> takes server ID
                m("78", "ok", 9_000_000_000, true), // same text, other day -> new message
                m("79", "mới", 3_000_000, false),   // new
            ],
            ..target.clone()
        };
        merge_thread_into(&mut target, incoming, false);
        let ids: Vec<&str> = target.messages.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "77", "79", "78"]);
    }

    #[test]
    fn test_merge_previews_single_message_object() {
        let mut threads = vec![sample_thread("u1")];
        let resp = json!({
            "error_code": 0,
            "data": [
                {
                    "idTo": "u1",
                    "message": {
                        "msgId": "m_single",
                        "content": "Single message text",
                        "ts": 1729501000000_i64,
                        "uidFrom": "u1"
                    }
                }
            ]
        });

        merge_previews(&mut threads, &resp);
        assert_eq!(threads[0].messages.len(), 1);
        assert_eq!(threads[0].messages[0].id, "m_single");
        assert_eq!(threads[0].messages[0].text, "Single message text");
        assert_eq!(threads[0].preview, "Single message text");
    }

    #[test]
    fn test_merge_previews_last_msg_object() {
        let mut threads = vec![sample_thread("u2")];
        let resp = json!({
            "error_code": 0,
            "data": [
                {
                    "idTo": "u2",
                    "lastMsg": {
                        "msgId": "m_last",
                        "content": "LastMsg text",
                        "ts": 1729502000000_i64,
                        "uidFrom": "u2"
                    }
                }
            ]
        });

        merge_previews(&mut threads, &resp);
        assert_eq!(threads[0].messages.len(), 1);
        assert_eq!(threads[0].messages[0].id, "m_last");
        assert_eq!(threads[0].preview, "LastMsg text");
    }

    #[test]
    fn test_merge_previews_messages_array_three_items() {
        let mut threads = vec![sample_thread("u3")];
        // Provide items out of order to verify timestamp ascending sort
        let resp = json!({
            "error_code": 0,
            "data": [
                {
                    "idTo": "u3",
                    "messages": [
                        {
                            "msgId": "m_item1",
                            "content": "First message",
                            "ts": 1000_i64,
                            "uidFrom": "u3"
                        },
                        {
                            "msgId": "m_item3",
                            "content": "Third message",
                            "ts": 3000_i64,
                            "uidFrom": "u3"
                        },
                        {
                            "msgId": "m_item2",
                            "content": "Second message",
                            "ts": 2000_i64,
                            "uidFrom": "0"
                        }
                    ]
                }
            ]
        });

        merge_previews(&mut threads, &resp);
        assert_eq!(threads[0].messages.len(), 3);
        assert_eq!(threads[0].messages[0].id, "m_item1");
        assert_eq!(threads[0].messages[0].text, "First message");
        assert_eq!(threads[0].messages[1].id, "m_item2");
        assert_eq!(threads[0].messages[1].text, "Second message");
        assert!(threads[0].messages[1].is_outgoing);
        assert_eq!(threads[0].messages[2].id, "m_item3");
        assert_eq!(threads[0].messages[2].text, "Third message");
        // Preview must be from the newest message (m_item3)
        assert_eq!(threads[0].preview, "Third message");
    }

    #[test]
    fn test_merge_previews_entry_with_no_message() {
        let mut threads = vec![sample_thread("u4")];
        threads[0].preview = "Existing preview".into();
        let resp = json!({
            "error_code": 0,
            "data": [
                {
                    "idTo": "u4"
                },
                {
                    "idTo": "u4",
                    "messages": []
                }
            ]
        });

        merge_previews(&mut threads, &resp);
        assert!(threads[0].messages.is_empty());
        assert_eq!(threads[0].preview, "Existing preview");
    }

    #[test]
    fn test_merge_group_history_fixture() {
        let mut thread = sample_thread("grp_123");
        thread.is_group = true;

        let resp = json!({
            "error_code": 0,
            "data": {
                "groupMsgs": [
                    {
                        "msgId": "gm_2",
                        "content": "Second group message",
                        "ts": 2000_i64,
                        "uidFrom": "member_2"
                    },
                    {
                        "msgId": "gm_1",
                        "content": "First group message",
                        "ts": 1000_i64,
                        "uidFrom": "member_1"
                    },
                    {
                        "msgId": "gm_2",
                        "content": "Duplicate second message",
                        "ts": 2000_i64,
                        "uidFrom": "member_2"
                    }
                ]
            }
        });

        merge_group_history(&mut thread, &resp);
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.messages[0].id, "gm_1");
        assert_eq!(thread.messages[1].id, "gm_2");
        assert_eq!(thread.preview, "Second group message");
    }

    #[test]
    fn test_merge_previews_incoming_1_1_and_group_and_cloud() {
        let mut cloud = sample_thread("9000000000000000001");
        cloud.name = "Cloud của tôi (Lưu trữ)".into();
        let friend = sample_thread("1000000000000000001");
        let mut group = sample_thread("888999000");
        group.is_group = true;
        let mut threads = vec![cloud, friend, group];

        let resp = json!({
            "error_code": 0,
            "data": {
                "msgs": [
                    {
                        "msgId": "in_1",
                        "content": "Tin nhan den tu ban be",
                        "idTo": "0",
                        "uidFrom": "1000000000000000001",
                        "ts": "1729505000000"
                    },
                    {
                        "msgId": "cloud_1",
                        "content": "Tin nhan tu gui vao Cloud",
                        "idTo": "0",
                        "uidFrom": "0",
                        "ts": "1729506000000"
                    }
                ],
                "groupMsgs": [
                    {
                        "msgId": "grp_msg_1",
                        "content": "Tin nhan trong nhom",
                        "idTo": "888999000",
                        "uidFrom": "111222333",
                        "dName": "Nguyen Van B",
                        "ts": "1729507000000"
                    }
                ]
            }
        });

        merge_previews(&mut threads, &resp);
        // Cloud message routed to `9000000000000000001`
        assert_eq!(threads[0].messages.len(), 1);
        assert_eq!(threads[0].messages[0].text, "Tin nhan tu gui vao Cloud");
        assert!(threads[0].messages[0].is_outgoing);

        // Incoming 1-1 routed to `1000000000000000001`
        assert_eq!(threads[1].messages.len(), 1);
        assert_eq!(threads[1].messages[0].text, "Tin nhan den tu ban be");
        assert!(!threads[1].messages[0].is_outgoing);

        // Group message routed to `888999000` (not `111222333`)
        assert_eq!(threads[2].messages.len(), 1);
        assert_eq!(threads[2].messages[0].text, "Tin nhan trong nhom");
        assert_eq!(threads[2].messages[0].sender, "Nguyen Van B");
        assert!(!threads[2].messages[0].is_outgoing);
    }

    #[test]
    fn test_poll_state_encode_and_update() {
        let mut ps = PollState::default();
        let encoded = ps.encode_params();
        assert!(encoded.starts_with("1,1,"));
        assert!(encoded.ends_with("%2C1"));

        let resp = json!({
            "data": {
                "lastActionId": "8399086511016",
                "lastActionIdOther": "8399036593265",
                "queueStatus": {
                    "1": {
                        "lastId": "8399086511016",
                        "ids": ["8399086511016"]
                    },
                    "15": {
                        "lastId": "8399036593265",
                        "ids": ["8399036593265"]
                    }
                }
            }
        });
        ps.update_from_response(&resp);
        assert_eq!(ps.last_action_id, "8399086511016");
        assert_eq!(ps.last_action_id_other, "8399036593265");
        assert_eq!(ps.queues.get("1").unwrap().last_id, "8399086511016");
        assert_eq!(
            ps.queues.get("1").unwrap().ids,
            vec!["8399086511016".to_string()]
        );
        // A message queue the server reports ("15" = 510_1) is tracked from the start of what
        // it kept; lp_chat accepted 15/16 on a real account (upstream PR #69, 02/11).
        assert_eq!(ps.queues.get("15").unwrap().last_id, "1");
    }

    #[test]
    fn test_parse_chat_message_sticker_json_string() {
        let msg_val = json!({
            "msgId": "stk_1",
            "msgType": "chat.sticker",
            "content": "{\"catId\":1002,\"id\":2005,\"type\":1}",
            "uidFrom": "user_a",
            "ts": 1729508000000_i64
        });
        let parsed = parse_chat_message(&msg_val).expect("must parse sticker");
        assert_eq!(parsed.text, "[Nhãn dán]");
        assert_eq!(parsed.id, "stk_1");
        assert_eq!(parsed.sticker_id, Some(2005));
        assert_eq!(parsed.sticker_cate_id, Some(1002));
        assert_eq!(parsed.sticker_type, Some(1));
        assert_eq!(
            parsed.sticker_image_url().as_deref(),
            Some("https://api.zaloapp.com/api/emoticon/sticker/webpc?eid=2005&size=130")
        );
    }

    #[test]
    fn test_parse_chat_message_integer_msg_types() {
        // Integer msgType = 5 (sticker) with empty content
        let stk = json!({
            "msgId": "stk_int",
            "msgType": 5,
            "content": "",
            "uidFrom": "user_b",
            "ts": 1000_i64
        });
        assert_eq!(parse_chat_message(&stk).unwrap().text, "[Nhãn dán]");

        // Integer msgType = 2 (photo)
        let photo = json!({
            "msgId": "photo_int",
            "msgType": 2,
            "content": "",
            "uidFrom": "user_b",
            "ts": 2000_i64
        });
        assert_eq!(parse_chat_message(&photo).unwrap().text, "[Hình ảnh]");

        // Integer msgType = 3 (voice)
        let voice = json!({
            "msgId": "voice_int",
            "msgType": 3,
            "content": "",
            "uidFrom": "user_b",
            "ts": 3000_i64
        });
        assert_eq!(parse_chat_message(&voice).unwrap().text, "[Tin nhắn thoại]");

        // Integer msgType = 19 (file)
        let file = json!({
            "msgId": "file_int",
            "msgType": 19,
            "content": "",
            "uidFrom": "user_b",
            "ts": 4000_i64
        });
        assert_eq!(parse_chat_message(&file).unwrap().text, "[Tập tin]");

        // Integer msgType = 31 (undo)
        let undo = json!({
            "msgId": "undo_int",
            "msgType": 31,
            "content": "",
            "uidFrom": "user_b",
            "ts": 5000_i64
        });
        assert_eq!(
            parse_chat_message(&undo).unwrap().text,
            "[Tin nhắn đã thu hồi]"
        );
    }

    #[test]
    fn test_parse_chat_message_with_plain_message_string() {
        let msg = json!({
            "msgId": "msg_str",
            "message": "Xin chao buoi sang",
            "uidFrom": "user_c",
            "dName": "Charlie",
            "ts": 1000_i64
        });
        let parsed = parse_chat_message(&msg).unwrap();
        assert_eq!(parsed.text, "Xin chao buoi sang");
        assert_eq!(parsed.sender, "Charlie");
    }

    #[test]
    fn test_parse_thread_boolean_is_group_and_string_time() {
        let val = json!({
            "userId": "grp_test",
            "displayName": "Nhom Chem Gio",
            "isGroup": true,
            "lastActionTime": "1729505000000"
        });
        let thread = parse_thread(&val).unwrap();
        assert!(thread.is_group);
        assert_eq!(thread.name, "Nhom Chem Gio");
        assert!(!thread.time.is_empty());
    }

    #[test]
    fn test_distinct_ids_with_same_name_and_avatar_stay_separate() {
        let old_dtc = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000005".into(),
            name: "Đtc".into(),
            preview: "Hi".into(),
            time: "10:21".into(),
            avatar: Some("https://s120-26-ava-talk.zadn.vn/5/00000000000000000000000000000001.jpg?key=abc&time=1797763127".into()),
            is_group: false,
            messages: vec![ChatMessage {
                id: "m1".into(),
                sender: "Đtc".into(),
                text: "Hi".into(),
                time: "10:21".into(),
                is_outgoing: false,
                ts: 1792578088952,
                ..Default::default()
            }],
        };
        let new_dtc = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000006".into(),
            name: "Đtc".into(),
            preview: "".into(),
            time: "16:48".into(),
            avatar: Some("https://s160-26-ava-talk.zadn.vn/5/00000000000000000000000000000001.jpg?key=xyz&time=1797876325".into()),
            is_group: false,
            messages: Vec::new(),
        };
        let mut threads = vec![old_dtc, new_dtc];
        deduplicate_threads(&mut threads, None);
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "1000000000000000005");
        assert_eq!(threads[1].id, "1000000000000000006");
        assert_eq!(threads[0].messages.len(), 1);
        assert!(threads[1].messages.is_empty());
    }

    #[test]
    fn test_merge_group_history_parses_stringified_cloud_data() {
        let mut cloud_thread = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "9000000000000000002".into(),
            name: "Cloud của tôi (Lưu trữ)".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: Vec::new(),
        };
        let raw_cloud_resp = json!({
            "error_code": 0,
            "data": r#"{"error":0,"groupMsgs":[{"msgId":"1000000000001","cliMsgId":"1700000000001","msgType":"chat.recommended","uidFrom":"0","idTo":"9000000000000000002","dName":"User Test","ts":"1700000000001","content":{"title":"","description":"Ghi chú đám mây","href":"https://zalo.me/test","action":"recommened.link"}}]}"#
        });
        merge_group_history(&mut cloud_thread, &raw_cloud_resp);
        assert_eq!(cloud_thread.messages.len(), 1);
        assert!(cloud_thread.messages[0].text.contains("Ghi chú đám mây"));
        assert_eq!(
            cloud_thread.messages[0].link_url.as_deref(),
            Some("https://zalo.me/test")
        );
    }

    #[test]
    fn test_strip_vn_for_sticker_kw() {
        assert_eq!(strip_vn_for_sticker_kw("Xin chào"), "xin chao");
        assert_eq!(strip_vn_for_sticker_kw("Cảm ơn nhiều"), "cam on nhieu");
        assert_eq!(strip_vn_for_sticker_kw("Ngủ ngon nhé"), "ngu ngon nhe");
        assert_eq!(strip_vn_for_sticker_kw("Nguyễn Văn An"), "nguyen van an");
        assert_eq!(
            normalize_sticker_keyword("  iu\u{a0}that\u{a0}  "),
            "iu that"
        );

        let q1 = canonical_vn_sticker_queries("Mệt quá");
        assert!(q1.contains(&"mệt quá".to_string()));
        assert!(q1.contains(&"met qua".to_string()));

        let q2 = canonical_vn_sticker_queries("khoc");
        assert!(q2.contains(&"khoc".to_string()));
        assert!(q2.contains(&"khóc".to_string()));
    }

    #[test]
    fn test_parse_friendship_status_response() {
        let r_friend = json!({"error_code": 0, "data": {"is_friend": 1, "is_requested": 0, "is_requesting": 0}});
        assert_eq!(
            parse_friendship_status_response(&r_friend),
            FriendshipStatus::Friend
        );

        let r_inc = json!({"error_code": 0, "data": {"is_friend": 0, "is_requested": 0, "is_requesting": 1}});
        assert_eq!(
            parse_friendship_status_response(&r_inc),
            FriendshipStatus::IncomingPending
        );

        let r_out = json!({"error_code": 0, "data": {"is_friend": 0, "is_requested": 1, "is_requesting": 0}});
        assert_eq!(
            parse_friendship_status_response(&r_out),
            FriendshipStatus::OutgoingPending
        );

        let r_stranger = json!({"error_code": 0, "data": {"is_friend": 0, "is_requested": 0, "is_requesting": 0}});
        assert_eq!(
            parse_friendship_status_response(&r_stranger),
            FriendshipStatus::Stranger
        );
    }

    #[test]
    fn incomplete_friendship_metadata_is_unknown_not_stranger() {
        for response in [
            json!({}),
            json!({"error_code":0,"data":{}}),
            json!({"data":{"is_friend":0}}),
            json!({"error_code":600,"data":{"is_friend":0,"is_requested":0,"is_requesting":0}}),
            json!({"error_code":0,"data":{"error_code":600,"data":{"is_friend":0,"is_requested":0,"is_requesting":0}}}),
        ] {
            assert_eq!(
                parse_friendship_status_response(&response),
                FriendshipStatus::Unknown
            );
        }
    }

    #[test]
    fn test_prevent_cloud_message_pollution_across_threads() {
        let mut contact = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000009".into(),
            name: "Người dùng Test".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: Vec::new(),
        };
        let raw_cloud_resp = json!({
            "error_code": 0,
            "data": {
                "groupMsgs": [{
                    "msgId": "1000000000002",
                    "msgType": "webchat",
                    "uidFrom": "0",
                    "idTo": "9000000000000000001",
                    "dName": "User Test",
                    "ts": "1700000000002",
                    "content": "Ghi chú Cloud của tôi"
                }]
            }
        });
        merge_group_history(&mut contact, &raw_cloud_resp);
        assert!(contact.messages.is_empty());

        let cloud = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "9000000000000000001".into(),
            name: "Cloud của tôi (Lưu trữ)".into(),
            preview: "Ghi chú Cloud của tôi".into(),
            time: "12:00".into(),
            avatar: None,
            is_group: false,
            messages: vec![ChatMessage {
                id: "1000000000002".into(),
                sender: "Tôi".into(),
                text: "Ghi chú Cloud của tôi".into(),
                time: "12:00".into(),
                is_outgoing: true,
                ts: 1700000000002,
                ..Default::default()
            }],
        };
        let polluted_contact = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1000000000000000009".into(),
            name: "Người dùng Test".into(),
            preview: "Ghi chú Cloud của tôi".into(),
            time: "12:00".into(),
            avatar: None,
            is_group: false,
            messages: vec![ChatMessage {
                id: "1000000000002".into(),
                sender: "Tôi".into(),
                text: "Ghi chú Cloud của tôi".into(),
                time: "12:00".into(),
                is_outgoing: true,
                ts: 1700000000002,
                ..Default::default()
            }],
        };
        let mut list = vec![cloud, polluted_contact];
        deduplicate_threads(&mut list, None);
        assert_eq!(list[0].messages.len(), 1);
        // Equal message IDs in separate conversations do not prove pollution.
        assert_eq!(list[1].messages.len(), 1);
    }
}

#[cfg(test)]
mod message_identity_tests {
    use super::*;
    fn message(id: &str, outgoing: bool) -> ChatMessage {
        ChatMessage {
            id: id.into(),
            text: "OK".into(),
            sender: "User".into(),
            time: "10:00".into(),
            ts: 1000,
            is_outgoing: outgoing,
            ..Default::default()
        }
    }
    #[test]
    fn repeated_real_messages_survive_merge_and_server_replay_is_deduplicated() {
        let mut target = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "1".into(),
            name: "Contact".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: vec![message("a", false)],
        };
        let mut update = target.clone();
        update.messages = vec![message("b", false)];
        merge_thread_into(&mut target, update.clone(), true);
        assert_eq!(target.messages.len(), 2);
        merge_thread_into(&mut target, update, true);
        assert_eq!(target.messages.len(), 2);
        assert!(!same_message(&message("a", true), &message("b", true)));
    }
    #[test]
    fn optimistic_matching_is_only_for_outgoing_local_vs_server() {
        let mut local = message("out_local", true);
        local.client_message_id = "client-1".into();
        let mut server = message("server", true);
        server.client_message_id = "client-1".into();
        assert!(same_message(&local, &server));
        assert!(!same_message(
            &message("out_local", true),
            &message("server", true)
        ));
        assert!(!same_message(
            &message("out_local", false),
            &message("server", false)
        ));
        assert!(!same_message(
            &message("out_a", true),
            &message("out_b", true)
        ));
        let mut extreme = message("server", true);
        extreme.ts = i64::MIN;
        assert!(!same_message(&message("out_local", true), &extreme));
    }
}

#[cfg(test)]
mod message_state_defaults_tests {
    use super::*;

    #[test]
    fn legacy_message_when_metadata_is_absent_uses_conservative_defaults() {
        let message: ChatMessage = serde_json::from_value(json!({
            "id": "msg-0001", "sender": "user-1", "text": "Hello",
            "time": "", "is_outgoing": false, "ts": 1
        }))
        .unwrap();
        assert_eq!(message.kind, crate::message_model::MessageKind::Unknown);
        assert_eq!(
            message.delivery,
            crate::message_model::DeliveryState::default()
        );
        assert!(message.wire_ref.fields.is_empty());
        assert!(message.reaction_summaries.is_empty());
        assert!(message.resolved_mention_uids.is_empty());
    }

    #[test]
    fn legacy_thread_when_sidebar_and_unread_are_absent_roundtrips() {
        let thread: Thread = serde_json::from_value(json!({
            "id": "thread-1", "name": "Example", "preview": "", "time": "",
            "avatar": null, "is_group": false, "messages": []
        }))
        .unwrap();
        assert_eq!(thread.presentation, SidebarPresentation::default());
        assert_eq!(thread.unread, crate::unread::UnreadState::default());
        let restored: Thread =
            serde_json::from_value(serde_json::to_value(&thread).unwrap()).unwrap();
        assert_eq!(restored.presentation, thread.presentation);
        assert_eq!(restored.unread, thread.unread);
    }
}

#[cfg(test)]
mod send_ack_tests {
    use super::*;
    #[test]
    fn invalid_and_unconfirmed_sends_do_not_appear_successful() {
        for response in [
            Value::Null,
            json!({}),
            json!({"error_code":0,"data":null}),
            json!({"error_code":0,"data":"not-json-or-undeciphered"}),
            json!({"error_code":114,"data":{}}),
        ] {
            assert!(confirmed_send(response).is_err());
        }
        assert!(confirmed_send(json!({"error_code":0,"data":{"msgId":"msg-1"}})).is_ok());
        let response =
            confirmed_send(json!({"error_code":"0","data":"{\"msgId\":\"msg-1\"}"})).unwrap();
        assert_eq!(response["data"]["msgId"], "msg-1");
        assert!(crate::session::is_expired_error(
            &confirmed_send(json!({"error_code":"600"}))
                .unwrap_err()
                .to_string()
        ));
    }
    #[test]
    fn persisted_metadata_cannot_change_message_routing_identity() {
        let mut message = ChatMessage {
            id: "msg-1".into(),
            sender: "user-1".into(),
            text: "hello".into(),
            ..Default::default()
        };
        message.restore_state(&json!({"id":"other-id","sender":"other-user","text":"different","delivery":"read"})).unwrap();
        assert_eq!(message.id, "msg-1");
        assert_eq!(message.sender, "user-1");
        assert_eq!(message.text, "hello");
        assert_eq!(message.delivery, crate::message_model::DeliveryState::Read);
    }
}

#[cfg(test)]
mod refresh_write_tests {
    use super::*;
    fn thread(id: &str, messages: &[(&str, &str)]) -> Thread {
        Thread {
            id: id.into(),
            messages: messages
                .iter()
                .map(|(id, text)| ChatMessage {
                    id: (*id).into(),
                    text: (*text).into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn only_new_or_changed_messages_and_new_threads_are_rewritten() {
        let before = vec![
            thread("a", &[("1", "x"), ("2", "y")]),
            thread("b", &[("3", "z")]),
        ];
        let marks = message_fingerprints(&before);
        assert!(changed_since(&before, &marks)
            .iter()
            .all(|t| t.messages.is_empty()));
        let mut after = before.clone();
        after[0].messages[1].text = "[Tin nhắn đã thu hồi]".into();
        after[0].messages.push(ChatMessage {
            id: "4".into(),
            ..Default::default()
        });
        after.push(thread("c", &[]));
        let changed = changed_since(&after, &marks);
        let ids: Vec<(String, Vec<String>)> = changed
            .iter()
            .map(|t| {
                (
                    t.id.clone(),
                    t.messages.iter().map(|m| m.id.clone()).collect(),
                )
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                ("a".into(), vec!["2".into(), "4".into()]),
                ("b".into(), vec![]),
                ("c".into(), vec![])
            ]
        );
    }
}

#[cfg(test)]
mod forward_media_tests {
    use super::*;
    #[test]
    fn forward_info_follows_zalo_pc_shapes() {
        let mut video = ChatMessage {
            kind: crate::message_model::MessageKind::Video,
            link_url: Some("https://video.zdn.vn/v.mp4".into()),
            link_thumb: Some("https://photo.zdn.vn/t.jpg".into()),
            ..Default::default()
        };
        video.wire_ref.fields.insert(
            "content".into(),
            json!({"href":"https://video.zdn.vn/v.mp4","params":"{\"duration\":4200,\"video_width\":720,\"video_height\":1280,\"fileSize\":\"99\"}"}),
        );
        let info = forward_info(&video).unwrap();
        assert_eq!(info["videoUrl"], "https://video.zdn.vn/v.mp4");
        assert_eq!(info["thumbUrl"], "https://photo.zdn.vn/t.jpg");
        assert_eq!(
            (info["width"].as_u64(), info["height"].as_u64()),
            (Some(720), Some(1280))
        );
        assert_eq!(info["fileSize"], 99);
        let photo = ChatMessage {
            kind: crate::message_model::MessageKind::Photo,
            link_url: Some("https://photo-stal-1.zdn.vn/jpg/a.jpg".into()),
            ..Default::default()
        };
        let info = forward_info(&photo).unwrap();
        assert_eq!(info["oriUrl"], info["thumbUrl"]);
        let mut file = ChatMessage {
            kind: crate::message_model::MessageKind::File,
            link_url: Some("https://f.dlfl.vn/a/report.pdf".into()),
            link_title: Some("report.pdf".into()),
            ..Default::default()
        };
        // Zalo PC refuses to forward a file without a valid MD5 checksum.
        assert!(forward_info(&file).is_none());
        file.wire_ref.fields.insert(
            "content".into(),
            json!({"params":{"checksum":"0123456789abcdef0123456789abcdef","fileSize":12,"fileExt":"pdf"}}),
        );
        let info = forward_info(&file).unwrap();
        assert_eq!(info["extension"], "pdf");
        assert_eq!(info["size"], 12);
        assert_eq!(info["fType"], 1);
    }
    #[test]
    fn photos_files_and_videos_with_urls_are_forwardable() {
        let photo = ChatMessage {
            kind: crate::message_model::MessageKind::Photo,
            link_url: Some("https://photo-stal-1.zdn.vn/jpg/a.jpg".into()),
            ..Default::default()
        };
        assert!(forwardable_media(&photo));
        assert!(can_forward_message(&photo));
        assert!(forward_file_name(&photo, &crate::message_view::media(&photo)).ends_with(".jpg"));
        let mut unnamed = crate::message_view::media(&photo);
        unnamed.name.clear();
        assert_eq!(
            forward_file_name(&ChatMessage::default(), &unnamed),
            "anh.jpg"
        );
        let mut sourced = photo.clone();
        assert_eq!(forward_source(&sourced), None);
        set_forward_source(&mut sourced, "g12", true);
        assert_eq!(forward_source(&sourced), Some(("g12".to_string(), true)));
        let recalled = ChatMessage {
            kind: crate::message_model::MessageKind::Recall,
            ..photo.clone()
        };
        assert!(!forwardable_media(&recalled));
        let text = ChatMessage {
            text: "xin chào".into(),
            ..Default::default()
        };
        assert!(!forwardable_media(&text));
        assert!(can_forward_message(&text));
    }
}

#[cfg(test)]
mod send_identity_tests {
    use super::*;

    #[test]
    fn receipt_requires_server_id_and_never_substitutes_client_id() {
        for response in [
            json!({"data":{"msgId":0}}),
            json!({"data":{"msgId":"000"}}),
            json!({"data":{"cliMsgId":"client-1"}}),
        ] {
            assert!(sent_server_id(&response).is_empty());
        }
        assert_eq!(
            sent_server_id(&json!({"data":{"msgId":"9007199254740999"}})),
            "9007199254740999"
        );
    }
    #[test]
    fn request_ids_are_unique_when_sends_share_a_clock_tick() {
        let ids: Vec<_> = (0..10_000).map(|_| next_client_message_id()).collect();
        assert!(ids.windows(2).all(|ids| ids[0] < ids[1]));
    }

    #[tokio::test]
    async fn request_client_id_survives_verified_send_variants() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        // Only loopback fixture traffic is used; all keys and identifiers are synthetic.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let fixture = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..9 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                    assert!(headers.len() < 16_384);
                }
                let headers = String::from_utf8(headers).unwrap();
                let size: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                assert!(size < 16_384);
                let mut body = vec![0; size];
                stream.read_exact(&mut body).unwrap();
                let query = reqwest::Url::parse(&format!(
                    "http://fixture.invalid/?{}",
                    String::from_utf8(body).unwrap()
                ))
                .unwrap();
                let encrypted = query
                    .query_pairs()
                    .find(|(key, _)| key == "params")
                    .unwrap()
                    .1
                    .into_owned();
                let body: Value =
                    serde_json::from_str(&crate::crypto::decode_aes(&[7; 32], &encrypted).unwrap())
                        .unwrap();
                let path = headers
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_string();
                requests.push((path, body));
                let response = r#"{"error_code":0,"data":{"msgId":"server-a"}}"#;
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", response.len(), response).unwrap();
            }
            requests
        });
        let mut session = Session::new(false, false, false).unwrap();
        session.login = Some(crate::session::LoginInfo {
            uin: "fixture-user".into(),
            zpw_enk: String::new(),
            zpw_sek: String::new(),
            display_name: "Fixture".into(),
            send2me_id: String::new(),
            avatar: String::new(),
        });
        session.secret_key = Some(vec![7; 32]);
        session.domains.chat = host.clone();
        session.domains.group = host;
        let client_id = 1_793_425_680_123;
        send_message_with_client_id(&session, "contact-1", "Text", client_id)
            .await
            .unwrap();
        send_group_message_with_client_id(&session, "group-1", "Text", client_id)
            .await
            .unwrap();
        send_link_message_with_client_id(
            &session,
            "contact-1",
            "Text",
            &LinkMetadata {
                url: "https://example.invalid/link".into(),
                title: "Title".into(),
                desc: String::new(),
                thumb: String::new(),
                src: String::new(),
            },
            false,
            client_id,
        )
        .await
        .unwrap();
        send_sticker_with_client_id(&session, "contact-1", 1, 2, 7, false, client_id)
            .await
            .unwrap();
        send_quote_message_with_client_id(
            &session,
            "contact-1",
            "Text",
            &ChatMessage {
                id: "123".into(),
                client_message_id: "777".into(),
                ts: 1700000000000,
                ..Default::default()
            },
            false,
            client_id,
        )
        .await
        .unwrap();
        forward_message_with_client_id(
            &session,
            "contact-1",
            false,
            &ChatMessage {
                text: "Forward".into(),
                ..Default::default()
            },
            client_id,
        )
        .await
        .unwrap();
        forward_message_with_client_id(
            &session,
            "contact-1",
            false,
            &ChatMessage {
                sticker_id: Some(1),
                ..Default::default()
            },
            client_id,
        )
        .await
        .unwrap();
        let draft = crate::mentions::Draft {
            text: "😀 @An hello".into(),
            mentions: vec![crate::mentions::Mention {
                uid: "42".into(),
                pos: 3,
                len: 3,
                kind: 0,
            }],
        };
        send_group_mentions(
            &session,
            "group_12",
            &draft.text,
            client_id,
            &draft.mentions,
        )
        .await
        .unwrap();
        let quoted = ChatMessage {
            id: "123".into(),
            client_message_id: "777".into(),
            ts: 1700000000000,
            wire_ref: crate::message_model::WireMessageRef {
                sender: Some(json!("42")),
                ..Default::default()
            },
            ..Default::default()
        };
        send_quote_mentions(
            &session,
            "group_12",
            &draft.text,
            &quoted,
            true,
            client_id,
            &draft.mentions,
        )
        .await
        .unwrap();
        let requests = fixture.join().unwrap();
        assert_eq!(requests.len(), 9);
        assert!(requests[7].0.starts_with("/api/group/mention?"));
        assert!(requests[8].0.starts_with("/api/group/quote?"));
        for (_, body) in &requests[7..] {
            assert_eq!(body["grid"], json!("12"));
            let mentions: Value =
                serde_json::from_str(body["mentionInfo"].as_str().unwrap()).unwrap();
            assert_eq!(mentions, json!([{"uid":"42","pos":3,"len":3,"type":0}]));
        }
        for (_, body) in requests {
            assert_eq!(body["clientId"], json!(client_id));
        }
    }
}

#[cfg(test)]
mod live_photo_time_tests {
    use super::*;
    #[test]
    fn live_timestamp_uses_local_formatter_and_cached_clock_is_not_authoritative() {
        let ts = 1793603040000i64;
        let mut msg=parse_chat_message(&json!({"msgId":"fixture-time","uidFrom":"123","ts":ts,"msgType":"webchat","content":"hello"})).unwrap();
        assert_eq!(msg.time, crate::clock::message_time(ts));
        msg.time = "07:04".into();
        assert_eq!(msg.display_time(), crate::clock::message_time(ts));
    }
    #[test]
    fn live_and_cached_direct_photos_use_image_metadata_without_losing_captions() {
        let url = "https://photo-stal-16.zdn.vn/fake/photo.jpg?token=synthetic";
        let mut msg=parse_chat_message(&json!({"msgId":"fixture-photo","uidFrom":"123","msgType":"chat.photo","content":{"href":url,"thumb":url}})).unwrap();
        assert!(msg.is_photo());
        assert_eq!(msg.link_src.as_deref(), Some("Hình ảnh Zalo"));
        assert_eq!(msg.link_url.as_deref(), Some(url));
        assert!(msg.photo_caption().is_none());
        msg.text = "Ảnh có chú thích".into();
        assert_eq!(msg.photo_caption(), Some("Ảnh có chú thích"));
        msg.kind = crate::message_model::MessageKind::Link;
        msg.link_src = None;
        msg.text = url.into();
        msg.hydrate_media_metadata();
        assert!(msg.is_photo());
        assert!(msg.photo_caption().is_none());
        assert!(!direct_zalo_photo("https://zdn.vn.evil.invalid/photo.jpg"));
        assert!(!direct_zalo_photo("https://example.invalid/article.jpg"));
        msg.mark_recalled();
        assert!(!msg.is_photo());
    }
}

#[cfg(test)]
mod rich_text_parse_tests {
    use super::*;

    #[test]
    fn rtf_message_keeps_plain_text_and_carries_styles_when_received() {
        let content = json!({"action":"rtf","title":"Chú ý: họp lúc 9h","params":"{\"styles\":[{\"start\":0,\"len\":6,\"st\":\"b\"}],\"ver\":0}"});
        let msg = parse_chat_message(&json!({"msgId":"fixture-rtf","uidFrom":"user-1","msgType":"webchat","content":content})).unwrap();
        assert_eq!(msg.text, "Chú ý: họp lúc 9h");
        assert_eq!(msg.text_styles.len(), 1);
        assert!(msg.text_styles[0].bold);
    }

    #[test]
    fn recalled_message_drops_styles_when_text_is_replaced() {
        let content = json!({"action":"rtf","title":"bí mật","params":"{\"styles\":[{\"start\":0,\"len\":2,\"st\":\"b\"}]}"});
        let msg = parse_chat_message(&json!({"msgId":"fixture-undo","uidFrom":"user-1","msgType":"chat.undo","content":content})).unwrap();
        assert!(msg.text_styles.is_empty());
    }
}

/// Delete a conversation for this account after an explicit user confirmation.
/// # Errors
/// Requires a real latest client identity and a successful decrypted server response.
pub async fn delete_conversation(s: &Session, thread: &Thread) -> Result<()> {
    anyhow::ensure!(s.is_active(), "Đã đăng xuất");
    let last = thread
        .messages
        .iter()
        .max_by_key(|m| m.ts)
        .context("Chưa có tin cuối; hãy cập nhật hội thoại rồi thử lại")?;
    let client = last
        .wire_ref
        .client_id
        .as_ref()
        .and_then(|v| {
            v.as_str()
                .map(str::to_owned)
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        })
        .or_else(|| (!last.client_message_id.is_empty()).then(|| last.client_message_id.clone()))
        .context("Thiếu định danh tin cuối; hãy cập nhật hội thoại trước khi xóa")?;
    let id = thread
        .id
        .strip_prefix("group_")
        .or_else(|| thread.id.strip_prefix('g'))
        .unwrap_or(&thread.id);
    anyhow::ensure!(
        !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()),
        "Định danh hội thoại không hợp lệ"
    );
    let body = if thread.is_group {
        json!({"grid":id,"cliMsgId":client,"conver":{},"onlyMe":1,"imei":s.imei})
    } else {
        json!({"toid":id,"cliMsgId":client,"conver":{},"onlyMe":1,"imei":s.imei})
    };
    let (domain, path) = if thread.is_group {
        (&s.domains.group, "/api/group/deleteconver")
    } else {
        (&s.domains.chat, "/api/message/deleteconver")
    };
    let params = s.encoded_params(&body)?;
    let url = format!("{domain}{path}?{}", s.common_params());
    let response = s
        .post_form(&url, format!("params={params}"))
        .await
        .map_err(|_| {
            anyhow::anyhow!("Không thể xác nhận xóa hội thoại; dữ liệu trên máy được giữ")
        })?;
    let response = s.maybe_decrypt(response, s.session_key_bytes()).await;
    anyhow::ensure!(
        response.get("error_code").and_then(Value::as_i64) == Some(0),
        "Máy chủ chưa xác nhận xóa hội thoại; dữ liệu trên máy được giữ"
    );
    Ok(())
}

#[cfg(test)]
mod catchup_progress_tests {
    use super::*;
    #[test]
    fn repeated_more_response_stops_without_inventing_a_cursor() {
        let mut state = PollState::default();
        let response = json!({"error_code":0,"data":{"more":1,"lastActionId":"100","queueStatus":{"1":{"lastId":"100","ids":["99","100"]}}}});
        state.receive_poll_response(&response);
        assert!(state.catchup_ready());
        let cursor = state.encode_params();
        state.receive_poll_response(&response);
        assert!(!state.catchup_ready());
        assert_eq!(state.encode_params(), cursor);
    }
    #[test]
    fn missing_cursor_or_repeated_reset_cannot_drive_an_infinite_loop() {
        let mut state = PollState::default();
        state.receive_poll_response(&json!({"data":{"more":1}}));
        assert!(!state.catchup_ready());
        let mut state = PollState::default();
        let reset = json!({"data":{"resetLastActionId":"30"}});
        state.receive_poll_response(&reset);
        assert!(state.catchup_ready());
        state.receive_poll_response(&reset);
        assert!(!state.catchup_ready());
    }
    #[test]
    fn independent_progress_and_completed_pages_remain_receivable() {
        let mut state = PollState::default();
        for id in ["10", "11", "12"] {
            state.receive_poll_response(&json!({"data":{"more":1,"lastActionId":id}}));
            assert!(state.catchup_ready());
        }
        state.receive_poll_response(&json!({"data":{"more":0,"lastActionId":"12"}}));
        assert!(state.catchup_ready());
    }
}

#[cfg(test)]
mod catchup_measurement {
    use super::*;
    #[test]
    #[ignore = "controlled synthetic pagination measurement"]
    fn repeated_queue_response_benchmark() {
        let response = json!({"data":{"more":1,"lastActionId":"100","queueStatus":{"1":{"lastId":"100","ids":["99","100"]}}}});
        for guarded in [false, true] {
            let mut measurements = Vec::new();
            let mut requests = 0;
            for _ in 0..20 {
                let mut state = PollState::default();
                let start = std::time::Instant::now();
                let mut count = 0;
                for _ in 0..1800 {
                    count += 1;
                    if guarded {
                        state.receive_poll_response(&response);
                        if !state.catchup_ready() {
                            break;
                        }
                    } else {
                        state.update_from_response(&response);
                    }
                }
                requests = count;
                measurements.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            measurements.sort_by(f64::total_cmp);
            println!(
                "{}",
                json!({"task":if guarded{"guarded_repeated_cursor"}else{"legacy_repeated_cursor"},"iterations":20,"dataset_max_responses":1800,"responses_processed":requests,"min_ms":measurements[0],"avg_ms":measurements.iter().sum::<f64>()/20.0,"p50_ms":measurements[10],"p95_ms":measurements[18],"p99_ms":measurements[19],"environment":"Linux x86_64 debug; no network or account"})
            );
            assert_eq!(requests, if guarded { 2 } else { 1800 });
        }
    }
}

#[cfg(test)]
mod reported_queue_tests {
    use super::*;
    #[test]
    fn reported_group_queue_is_tracked_and_advances_while_unknown_queues_stay_untracked() {
        let mut state = PollState::default();
        let reply = json!({"data":{"more":1,"lastActionId":"100","queueStatus":{
            "1":{"lastId":"100","ids":[]},
            "16":{"lastId":"8442899323544","ids":["8442899323544"]},
            "7":{"lastId":"100","evict":1}
        }}});
        state.update_from_response(&reply);
        assert_eq!(state.queues["16"].last_id, "1");
        assert!(!state.queues.contains_key("7"));
        state.update_from_response(&reply);
        assert_eq!(state.queues["16"].last_id, "8442899323544");
        let queues = serde_json::to_string(&state.queues).unwrap();
        assert!(queues.contains("\"16\""));
    }
}

#[cfg(test)]
mod generic_name_tests {
    use super::*;
    #[test]
    fn placeholders_are_generic_but_real_names_starting_with_nhom_are_kept() {
        for name in ["", "  ", "12345", "Nhóm 6611225", "Người dùng 42"] {
            assert!(is_generic_thread_name(name), "{name}");
        }
        for name in ["Nhóm bạn thân", "Nhóm 12A1 cũ", "Người dùng mới", "An"] {
            assert!(!is_generic_thread_name(name), "{name}");
        }
    }
}

#[cfg(test)]
mod reaction_kind_regression_tests {
    use super::*;
    use crate::message_model::MessageKind as K;
    #[test]
    fn reactions_use_internal_codes_instead_of_archive_codes() {
        for (kind, code) in [
            (K::Text, 1),
            (K::Photo, 2),
            (K::Voice, 3),
            (K::Sticker, 4),
            (K::Link, 6),
            (K::Video, 18),
            (K::File, 19),
        ] {
            assert_eq!(
                reaction_message_type(&ChatMessage {
                    kind,
                    ..Default::default()
                })
                .unwrap(),
                code
            );
        }
        assert!(reaction_message_type(&ChatMessage {
            kind: K::Recall,
            ..Default::default()
        })
        .is_err());
    }
    #[test]
    fn local_ack_preserves_other_reactors_and_deduplicates_server_echo() {
        let mut m = ChatMessage::default();
        let other = crate::reactions::ReactionEvent {
            sender_id: "other".into(),
            client_event_id: "1".into(),
            r_type: 5,
            emoji: "❤️".into(),
            ts: 100,
            ..Default::default()
        };
        let mine = crate::reactions::ReactionEvent {
            sender_id: "me".into(),
            client_event_id: "2".into(),
            r_type: 3,
            emoji: "👍".into(),
            ts: 101,
            ..Default::default()
        };
        m.apply_reaction(&other);
        m.apply_reaction(&mine);
        let before = m.reaction_summaries.clone();
        assert!(!m.apply_reaction(&mine));
        assert_eq!(before, m.reaction_summaries);
        assert_eq!(m.reaction_summaries.len(), 2);
    }
}

#[cfg(test)]
mod group_send_policy_tests {
    use super::*;
    #[test]
    fn locked_group_requires_explicit_manager_identity() {
        for locked in [json!(1), json!("1"), json!(true)] {
            let policy = GroupSendPolicy::parse(&json!({
                "setting":{"lockSendMsg":locked}, "creatorId":"owner",
                "adminIds":["admin", 42]
            }))
            .unwrap();
            assert!(policy.allows("owner"));
            assert!(policy.allows("admin"));
            assert!(policy.allows("42"));
            assert!(!policy.allows("member"));
            assert!(!policy.allows(""));
        }
        assert!(
            GroupSendPolicy::parse(&json!({"setting":{"lockSendMsg":0}}))
                .unwrap()
                .allows("member")
        );
        assert!(GroupSendPolicy::parse(&json!({})).is_none());
        assert!(GroupSendPolicy::parse(&json!({"setting":{"lockSendMsg":2}})).is_none());
    }
}

#[cfg(test)]
mod volatile_group_policy_tests {
    use super::*;
    #[test]
    fn cached_sidebar_metadata_never_reinstates_an_old_group_send_permission() {
        let metadata = SidebarPresentation {
            send_policy: Some(GroupSendPolicy {
                locked: false,
                managers: vec!["12".into()],
            }),
            member_count: Some(50),
            ..Default::default()
        };
        let json = serde_json::to_value(&metadata).unwrap();
        assert!(json.get("send_policy").is_none());
        let restored: SidebarPresentation = serde_json::from_value(json).unwrap();
        assert!(restored.send_policy.is_none());
        assert_eq!(restored.member_count, Some(50));
        let legacy: SidebarPresentation = serde_json::from_value(
            serde_json::json!({"send_policy":{"locked":false,"managers":["12"]}}),
        )
        .unwrap();
        assert!(legacy.send_policy.is_none());
    }
}

#[cfg(test)]
mod contact_and_batch_tests {
    use super::*;

    #[test]
    fn parse_chat_contact_message() {
        let wire = json!({
            "msgId": "12345",
            "cliMsgId": "67890",
            "msgType": "chat.contact",
            "content": {
                "contactId": "987654321",
                "contactName": "Nguyễn Văn A",
                "phone": "0912345678",
                "avatar": "https://example.invalid/avatar.png"
            },
            "uidFrom": "111222333",
            "ts": 1700000000000_i64,
        });
        let msg = parse_chat_message(&wire).expect("should parse contact message");
        assert_eq!(msg.kind, crate::message_model::MessageKind::Contact);
        assert_eq!(msg.contact_id.as_deref(), Some("987654321"));
        assert_eq!(msg.contact_name.as_deref(), Some("Nguyễn Văn A"));
        assert_eq!(msg.contact_phone.as_deref(), Some("0912345678"));
        assert_eq!(msg.contact_avatar.as_deref(), Some("https://example.invalid/avatar.png"));
        assert_eq!(msg.text, "[Danh thiếp] Nguyễn Văn A");
    }

    #[test]
    fn parse_contact_json_string_content() {
        let wire = json!({
            "msgId": "22222",
            "cliMsgId": "33333",
            "msgType": "contact",
            "content": "{\"userId\":\"55555\",\"name\":\"Trần Thị B\"}",
            "uidFrom": "111222333",
            "ts": 1700000001000_i64,
        });
        let msg = parse_chat_message(&wire).expect("should parse string-encoded contact");
        assert_eq!(msg.kind, crate::message_model::MessageKind::Contact);
        assert_eq!(msg.contact_id.as_deref(), Some("55555"));
        assert_eq!(msg.contact_name.as_deref(), Some("Trần Thị B"));
        assert_eq!(msg.text, "[Danh thiếp] Trần Thị B");
    }

    #[test]
    fn contact_serialization_roundtrip() {
        let msg = ChatMessage {
            id: "msg_1".into(),
            kind: crate::message_model::MessageKind::Contact,
            contact_id: Some("user_99".into()),
            contact_name: Some("Lê Văn C".into()),
            contact_phone: Some("0988776655".into()),
            contact_avatar: Some("https://img.zalo.me/ava.jpg".into()),
            text: "[Danh thiếp] Lê Văn C".into(),
            ..Default::default()
        };
        let serialized = serde_json::to_string(&msg).expect("serialization");
        let deserialized: ChatMessage = serde_json::from_str(&serialized).expect("deserialization");
        assert_eq!(deserialized.contact_id.as_deref(), Some("user_99"));
        assert_eq!(deserialized.contact_name.as_deref(), Some("Lê Văn C"));
        assert_eq!(deserialized.contact_phone.as_deref(), Some("0988776655"));
        assert_eq!(deserialized.contact_avatar.as_deref(), Some("https://img.zalo.me/ava.jpg"));
    }
}

// E2EE helper: check if conversation has E2EE enabled
pub fn is_conversation_e2ee(convo: &serde_json::Value) -> bool {
    convo.get("isE2EE")
        .and_then(|v| v.as_u64())
        .map(|n| n > 0)
        .unwrap_or(false)
}

// E2EE send path wrapper (called from worker when E2EE enabled)
pub async fn send_e2ee_message(
    s: &Session,
    profile_id: &str,
    to_id: &str,
    text: &str,
    now: u64,
) -> Result<Value> {
    use base64::Engine;
    
    let ciphertext = crate::e2ee_integration::encrypt_message(s, profile_id, to_id, text).await?;
    
    let body = obj(&[
        ("toid", json!(to_id)),
        ("message", json!(base64::engine::general_purpose::STANDARD.encode(&ciphertext))),
        ("clientId", json!(now)),
        ("imei", json!(s.imei)),
        ("isE2EE", json!(1)),
    ]);
    let params = s.encoded_params(&body)?;
    let host = &s.domains.chat;
    let url = format!("{host}/api/e2ee/pc/message/sms?{}", s.common_params());
    let form = format!("params={params}");
    let resp = s.post_form(&url, form).await?;
    let resp = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    confirmed_send(resp)
}

// E2EE decrypt helper for parse_chat_message
pub fn try_decrypt_e2ee_content(
    profile_id: &str,
    peer_id: &str,
    raw: &Value,
) -> Option<(String, bool)> {
    use base64::Engine;
    
    let e2ee_status = raw.get("e2eeStatus")
        .and_then(|v| v.as_i64())
        .filter(|&code| (3000..4000).contains(&code))?;
    
    let ciphertext_b64 = raw.get("content")
        .or_else(|| raw.get("message"))
        .and_then(|v| v.as_str())?;
    
    let ciphertext = base64::engine::general_purpose::STANDARD.decode(ciphertext_b64).ok()?;
    
    match crate::e2ee_integration::decrypt_message(profile_id, peer_id, &ciphertext) {
        Ok(decrypted) => Some((decrypted.text, decrypted.identity_changed)),
        Err(e) => {
            eprintln!("E2EE decrypt failed (status {e2ee_status}): {e}");
            Some(("[Tin nhắn mã hoá - không giải mã được]".to_string(), false))
        }
    }
}
