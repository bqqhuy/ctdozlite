//! Complete native desktop GUI powered by `eframe` and `egui`.
//!
//! Designed with the UI/UX Pro Max Minimalism & Swiss Style system:
//! - Zero-idle-CPU reactive architecture (event-driven repaint, zero per-frame Thread cloning)
//! - Custom vector icons rendered via `egui::Painter` (zero texture/SVG bloat)
//! - High-contrast Light & Dark semantic color tokens (`#0068FF` / `#2563EB` primary, `#059669` online accent)
//! - Filterable conversation sidebar (`Tất cả`, `Cá nhân`, `Nhóm`), avatar badges, and grouped chat bubbles
//! - Built-in Zalo Sticker rendering & picker (`[Nhãn dán]`)

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontId, Id, Key, Layout, Margin, Pos2, Rect, RichText,
    Rounding, Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};

use crate::chat::{ChatMessage, Thread};
use crate::logger::{self, FindingLevel, LogEntry, ScanFinding};
use crate::qr::QrLogin;
use crate::session::Session;

mod account_state;
mod account_switcher;
mod auth_guard;
mod avatar_panel;
mod backup_panel;
mod board_panel;
mod composer_drafts;
mod contact_panel;
mod contacts;
mod content_panel;
mod doc_viewer;
mod download_panel;
mod file_drop;
mod group_entry_panel;
mod group_panel;
mod hidden_panel;
mod labels_panel;
mod mention_composer;
mod messages;
mod modals;
pub(super) use modals::render_modal_backdrop;
mod policy_panel;
mod privacy_panel;
mod profile_panel;
mod schedule_modal;
mod search_panel;
mod stickers;
mod theme;
mod tools_panel;
mod worker;
mod workspace;

use account_state::*;
use messages::*;
use stickers::*;
use theme::*;
pub use theme::{emoji_to_tex_key, load_app_icon, lucide, REACTION_LIST};
pub use worker::WorkerCmd;
use worker::*;

/// Whether an update represents live delivery or a historical/cache read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThreadUpdateSource {
    Live,
    History,
}

fn merge_sent_message(thread: &mut Thread, message: ChatMessage) {
    crate::chat::merge_sent_message(thread, message);
}

fn apply_outgoing_state(app: &mut ZaloGuiApp, thread_id: String, message: ChatMessage) {
    if let Some(session) = &mut app.forward_session {
        session.update_outgoing_state(&thread_id, &message);
    }
    if let Some(thread) = app.threads.iter_mut().find(|t| t.id == thread_id) {
        if !thread.is_group
            && message.is_outgoing
            && matches!(
                message.delivery,
                crate::message_model::DeliveryState::ServerAccepted
                    | crate::message_model::DeliveryState::Delivered
                    | crate::message_model::DeliveryState::Read
            )
        {
            thread.presentation.inbox.responded_by_me = Some(true);
        }
        if let Some(existing) = thread
            .messages
            .iter_mut()
            .find(|m| m.client_message_id == message.client_message_id)
        {
            if existing.outgoing_job.as_ref().is_some_and(|old| {
                message
                    .outgoing_job
                    .as_ref()
                    .is_none_or(|new| new.attempt >= old.attempt)
            }) {
                *existing = message;
            }
        } else {
            thread.messages.push(message);
        }
        thread.preview = thread
            .messages
            .last()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        thread.time = thread
            .messages
            .last()
            .map(|m| m.time.clone())
            .unwrap_or_default();
        thread.refresh_sidebar_presentation();
        app.persist_threads_async();
    }
}

pub const ZALO_SPRITE_FRAME_DELAY: std::time::Duration = std::time::Duration::from_millis(80);

#[derive(Clone, Debug)]
pub struct DecodedSpriteData {
    pub image: egui::ColorImage,          // Dải ngang chứa 1 hoặc nhiều frame
    pub frames: u16,                      // Số frame (width / height)
    pub delays: Vec<std::time::Duration>, // Thời gian hiển thị từng frame
}

#[derive(Clone)]
pub struct StickerSprite {
    pub texture: egui::TextureHandle,
    pub frames: u16,
    pub delays: Vec<std::time::Duration>,
}

pub enum GuiEvent {
    E2eeTrialPrepared {
        owner: String,
        peer_id: String,
        result: Result<(), String>,
    },
    AppSettingsSaved(Result<(), String>),
    ProfilesLoaded {
        owner: String,
        result: Result<Vec<crate::profiles::Profile>, String>,
    },
    SafetyNumberLoaded {
        owner: String,
        peer_id: String,
        peer_name: String,
        safety_number: String,
        is_verified: bool,
        identity_changed: bool,
    },
    IdentityTrusted {
        owner: String,
        peer_id: String,
        is_trusted: bool,
    },
    SafetyNumberFailed {
        owner: String,
        peer_id: String,
        message: String,
    },
    DocPreviewLoaded {
        owner: String,
        runtime_id: usize,
        context: worker::ChatImage,
        file_name: String,
        content: String,
    },
    ReactorProfiles {
        owner: String,
        ids: Vec<String>,
        result: Result<serde_json::Map<String, serde_json::Value>, String>,
    },
    GroupContent {
        owner: String,
        thread: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::group_content::Reply, String>,
    },
    ConversationPolicy {
        loading: bool,
        owner: String,
        generation: u64,
        result: Result<crate::conversation_policy::Snapshot, String>,
    },
    Expired {
        owner: String,
        ids: Vec<(String, String)>,
    },
    DesktopBackup {
        owner: String,
        generation: u64,
        imported: bool,
        result: Result<crate::desktop_backup::Report, String>,
    },
    DesktopBackupPath {
        owner: String,
        generation: u64,
        path: Option<std::path::PathBuf>,
    },

    ActivityUpdated {
        owner: String,
        uid: String,
        status: crate::last_activity::Status,
    },
    AccountMetadataInvalidated {
        owner: String,
        privacy: bool,
        relations: bool,
        business: bool,
    },
    AccountMetadata {
        owner: String,
        snapshot: Box<crate::account_metadata::Snapshot>,
    },
    GroupInvalidated {
        owner: String,
        revision: u64,
        groups: HashSet<String>,
        all: bool,
    },
    GroupKindLoaded {
        revision: u64,
        owner: String,
        group_id: String,
        kind: Option<crate::chat::GroupKind>,
    },
    GroupPolicy {
        revision: u64,
        owner: String,
        group_id: String,
        result: Result<crate::chat::GroupSendPolicy, String>,
    },
    Group {
        owner: String,
        target: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::groups::Reply, String>,
    },
    /// Group roster for the info drawer: members and server total.
    GroupMembersLoaded {
        /// Account that requested it; rosters of another account are dropped.
        owner: String,
        group_id: String,
        result: Result<(Vec<crate::chat::SidebarMember>, Option<u64>), String>,
    },
    /// People typing now (`presence::typing`), for `owner`.
    Typing {
        owner: String,
        events: Vec<crate::presence::Typing>,
    },
    /// Outgoing messages of `owner` whose delivery state rose: `(thread, msg_id, state)`.
    Receipts {
        owner: String,
        changed: Vec<(String, String, crate::message_model::DeliveryState)>,
    },
    DroppedFilePrepared {
        owner: String,
        thread: String,
        id: u64,
        result: Result<file_drop::Prepared, String>,
    },
    ComposerDraftLoaded {
        owner: String,
        thread: String,
        generation: u64,
        result: Result<crate::mentions::Draft, String>,
    },
    ComposerDraftSaved {
        owner: String,
        thread: String,
        generation: u64,
        text: crate::mentions::Draft,
        result: Result<(), String>,
    },
    MediaStarted {
        owner: String,
        thread: Option<(String, bool)>,
        id: String,
        name: String,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    },
    MediaProgress {
        owner: String,
        id: String,
        bytes: u64,
        total: Option<u64>,
    },
    MediaTransientClosed {
        owner: String,
        id: String,
    },
    MediaFinished {
        owner: String,
        id: String,
        result: Result<std::path::PathBuf, String>,
    },
    ArchiveMutationFinished {
        owner: String,
        runtime_id: usize,
        batch: u64,
        thread: String,
        recall: bool,
        results: Vec<(ChatMessage, Result<(), String>)>,
    },
    ArchiveHistoryLoaded {
        owner: String,
        runtime_id: usize,
        thread: String,
        generation: u64,
        result: Result<crate::store::HistoryPage, String>,
    },
    MediaBatchFinished {
        owner: String,
        batch: u64,
        results: Vec<(String, Result<std::path::PathBuf, String>)>,
    },
    Tools {
        owner: String,
        result: Result<tools_panel::ToolsResult, String>,
    },
    MediaNeighbor {
        owner: String,
        thread: String,
        anchor: String,
        result: Result<Option<ChatMessage>, String>,
    },
    ClipboardImageRead {
        owner: String,
        thread: String,
        image: Option<(std::path::PathBuf, u32, u32, egui::ColorImage)>,
    },
    AttachmentPickerClosed {
        owner: String,
    },
    AttachmentDraftPicked {
        owner: String,
        runtime_id: usize,
        thread: String,
        paths: Result<Vec<std::path::PathBuf>, String>,
    },
    /// A region capture ready to preview, or why there is none.
    ScreenCaptureFinished {
        owner: String,
        thread_id: String,
        is_group: bool,
        result: Result<(std::path::PathBuf, u32, u32, egui::ColorImage), String>,
    },
    /// A single wake-up represents any number of live deltas while hidden.
    LiveThreadsReady(std::sync::Arc<crate::live_updates::LiveUpdates>),
    ConversationPreferences {
        owner: String,
        values: HashMap<String, crate::store::ConversationPreference>,
    },
    ThreadDeleted {
        owner: String,
        thread: String,
    },
    ThreadHistoryCleared {
        owner: String,
        thread_id: String,
    },
    ConversationRefreshStatus {
        running: bool,
        success: bool,
        message: String,
    },
    PhoneSyncStatus {
        running: bool,
        success: bool,
        message: String,
    },
    HistorySyncStatus {
        running: bool,
        success: bool,
        message: String,
    },
    QrReady {
        qr_login: QrLogin,
        qr_image: egui::ColorImage,
    },
    QrScanned {
        name: String,
    },
    LoginSuccess {
        name: String,
        uin: String,
        avatar: Option<String>,
        runtime_id: usize,
    },
    LoginFailed(String),
    E2eePeersHydrated {
        owner: String,
        peers: Vec<String>,
    },
    E2eePeerRequired {
        owner: String,
        peer_id: String,
    },
    ThreadsUpdated {
        threads: Vec<Thread>,
        source: ThreadUpdateSource,
    },
    ConversationHistoryLoaded {
        runtime_id: usize,
        threads: Vec<Thread>,
        status: String,
    },
    LocalCallRecord {
        runtime_id: usize,
        outgoing: bool,
        owner: String,
        thread_id: String,
        text: String,
        msg_id: String,
    },
    OutgoingState {
        owner: String,
        runtime_id: usize,
        thread_id: String,
        message: ChatMessage,
    },
    VoiceOutgoingState {
        owner: String,
        runtime_id: usize,
        thread_id: String,
        message: ChatMessage,
    },
    VoiceSendRejected {
        owner: String,
        runtime_id: usize,
        message: String,
    },
    VoiceSendStatus {
        owner: String,
        runtime_id: usize,
        message: String,
    },
    MessageSent {
        client_message_id: String,
        thread_id: String,
        text: String,
        msg_id: String,
    },
    StickerSent {
        client_message_id: String,
        thread_id: String,
        msg_id: String,
        sticker_id: u64,
        cate_id: u64,
        sticker_type: u32,
    },
    AttachmentSent {
        thread_id: String,
        msg: ChatMessage,
    },
    PreviewImageReady {
        owner: String,
        expires_at: Option<i64>,
        key: String,
        image: egui::ColorImage,
        permit: crate::media_resources::MediaJobPermit,
    },
    MediaRenewed {
        thread: String,
        message: Box<ChatMessage>,
    },
    UpdateChecked(Result<Option<crate::updater::ReleaseUpdate>, String>),
    UpdatePrepared(Result<crate::updater::ReadyUpdate, String>),
    UpdateLaunched(Result<(), String>),
    ImageBusy {
        key: String,
    },
    ImageUnavailable {
        key: String,
        failure: crate::media_recovery::Failure,
    },
    VideoFrameReady {
        generation: u64,
        key: String,
        image: egui::ColorImage,
    },
    StickerImageReady {
        key: String,
        image: egui::ColorImage,
    },
    StickerSpriteReady {
        key: String,
        sprite: DecodedSpriteData,
        permit: Option<crate::media_resources::MediaJobPermit>,
    },
    E2eeRequirementResetResult {
        owner: String,
        peer_id: String,
        success: bool,
    },
    StickerStoreLoaded(Vec<crate::chat::StickerPackInfo>),
    StickerPackItemsLoaded {
        cate_id: u64,
        name: String,
        items: Vec<crate::chat::StickerItem>,
    },
    StickerSearchResults {
        keyword: String,
        items: Vec<crate::chat::StickerItem>,
    },
    StickerSuggestKeywordsLoaded(Vec<String>),
    ComposerStickerSuggestions {
        keyword: String,
        items: Vec<crate::chat::StickerItem>,
    },
    LinkPreviewReady {
        owner: String,
        runtime_id: usize,
        url: String,
        result: Result<crate::chat::LinkMetadata, String>,
    },
    PhoneLookupSuccess(crate::chat::PhoneLookupResult),
    PhoneLookupFailed(String),
    FriendStatusChecked {
        owner: String,
        runtime_id: usize,
        generation: u64,
        user_id: String,
        result: Result<crate::chat::FriendshipStatus, String>,
    },
    QuoteSent {
        client_message_id: String,
        thread_id: String,
        text: String,
        msg_id: String,
        quoted: ChatMessage,
    },
    NotificationSoundPrepared(bool),
    SettingsDevicesLoaded {
        generation: u64,
        result: Result<crate::media::CallDeviceLists, String>,
    },
    MessagePinsLoaded {
        owner: String,
        thread: String,
        generation: u64,
        result: Result<crate::board::Page, String>,
    },
    CloudUsageLoaded {
        owner: String,
        generation: u64,
        result: Result<crate::zcloud::Usage, String>,
    },
    Board {
        owner: String,
        thread: String,
        generation: u64,
        mutation: bool,
        list: bool,
        result: Result<crate::board::Page, String>,
    },
    ServerPins {
        owner: String,
        snapshot: crate::server_pins::Snapshot,
    },
    Reacted {
        owner: String,
        intent_id: u64,
        action_id: String,
        thread_id: String,
        msg_id: String,
        ts: i64,
        reaction: String,
    },
    ReactionFailed {
        owner: String,
        intent_id: u64,
        thread_id: String,
        msg_id: String,
        ts: i64,
        message: String,
    },
    MessageDeleted {
        owner: String,
        thread_id: String,
        msg_id: String,
        ts: i64,
    },
    MessageRecalled {
        owner: String,
        thread_id: String,
        msg_id: String,
        ts: i64,
    },
    SessionExpired(String),
    Contact {
        owner: String,
        target: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::contact_actions::Reply, String>,
    },
    Archive {
        owner: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::archive::Snapshot, String>,
    },
    ManualMarks {
        owner: String,
        generation: u64,
        mutation: bool,
        result: Result<Vec<crate::unread::ManualUnreadMark>, String>,
    },
    Privacy {
        owner: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::privacy::Snapshot, String>,
    },
    Labels {
        owner: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::labels::Snapshot, String>,
    },
    Profile {
        owner: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::profile::Snapshot, String>,
    },
    AvatarHistory {
        owner: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::avatar_history::Reply, String>,
    },
    Hidden {
        owner: String,
        generation: Option<u64>,
        view: crate::hidden_chat::Snapshot,
        result: Result<(), String>,
    },
    AvatarPicked {
        owner: String,
        generation: u64,
        path: Option<std::path::PathBuf>,
    },
    AvatarPrepared {
        owner: String,
        generation: u64,
        result: Result<std::sync::Arc<crate::avatar::Draft>, String>,
    },
    AvatarUploaded {
        owner: String,
        generation: u64,
        result: Result<Option<crate::profile::Snapshot>, String>,
    },
    GroupEntry {
        owner: String,
        generation: u64,
        mutation: bool,
        result: Result<crate::group_entry::Reply, String>,
    },
    CallStateChanged {
        owner: String,
        call_id: i64,
        state: String,
    },
    CallAnsweredElsewhere {
        owner: String,
        call_id: i64,
    },
    CallIdentity {
        owner: String,
        thread_id: String,
        call_id: i64,
    },
    IncomingCall {
        owner: String,
        call_id: i64,
        caller_id: String,
        caller_name: String,
        is_video: bool,
        is_group: bool,
    },
    CallRejected {
        owner: String,
        partner_id: String,
        call_id: Option<i64>,
        code: i64,
        reason: String,
    },
    StatusUpdate(String),
    ServerMediaLoaded {
        runtime_id: usize,
        thread_id: String,
        media_type: u8,
        page_num: u32,
        page: crate::server_media::ServerMediaPage,
        generation: u64,
    },
    ServerMediaLoadFailed {
        runtime_id: usize,
        thread_id: String,
        media_type: u8,
        generation: u64,
        error: String,
    },
    CloudViewerKeyLoaded(crate::zcloud::CloudViewerKey),
    BusinessCatalogLoaded(Vec<crate::business::ProductCatalogItem>),
    DevicesLoaded(Vec<crate::device::DeviceSession>),
    TwoFactorStatusLoaded(crate::account_recovery::TwoFactorStatus),
    GroupTodosLoaded {
        runtime_id: usize,
        thread_id: String,
        todos: Vec<crate::todo::TodoItem>,
    },
    GroupPollsLoaded {
        runtime_id: usize,
        thread_id: String,
        polls: Vec<crate::poll::PollItem>,
    },
    BatchRecallCompleted {
        thread_id: String,
        recalled_ids: Vec<String>,
        count: usize,
    },
    LoggedOut,
    LogoutFailed {
        message: String,
    },
    DownloadFolderPicked(Option<Result<std::path::PathBuf, String>>),
    DownloadFolderFailed(String),
    CameraUnavailable(String),
    UploadProgress {
        owner: String,
        runtime_id: usize,
        upload_id: String,
        completed: u64,
        total: u64,
    },
    UploadResumeAvailable {
        sessions: Vec<crate::upload::ResumableUploadState>,
    },
    ProfileSwitched {
        session: Box<Session>,
    },
    AppUnlockCompleted {
        request_id: u64,
        runtime_id: usize,
        verified: bool,
    },
    AppUnlockFailed {
        request_id: u64,
        runtime_id: usize,
        message: String,
    },
    MessageTranslated {
        thread_id: String,
        msg_id: String,
        translated_text: String,
        source_lang: Option<String>,
        text_hash: u64,
        generation: u64,
    },
    MessageTranslateFailed {
        thread_id: String,
        msg_id: String,
        error: String,
        generation: u64,
    },
    VoiceTranscriptLoaded {
        thread_id: String,
        msg_id: String,
        transcript: String,
        generation: u64,
    },
    VoiceTranscriptFailed {
        thread_id: String,
        msg_id: String,
        error: String,
        generation: u64,
    },
}

#[derive(Debug, Clone)]
struct ActiveCallOverlay {
    call_id: Option<i64>,
    partner_id: String,
    partner_name: String,
    is_video: bool,
    is_group: bool,
    state_label: String,
    started_at: std::time::Instant,
    mic_muted: bool,
    cam_enabled: bool,
    is_incoming_ringing: bool,
    error_code: Option<i64>,
    error_reason: Option<String>,
}

#[derive(Clone, Copy)]
struct VideoPlayerState {
    playing: bool,
    position_secs: f32,
    duration_secs: f32,
    volume: f32,
}

#[derive(Clone, Debug)]
pub struct SafetyNumberModalState {
    pub peer_id: String,
    pub peer_name: String,
    pub safety_number: String,
    pub is_verified: bool,
    pub identity_changed: bool,
    pub loading: bool,
}

#[derive(Clone, Debug)]
pub struct AutoDeleteModalState {
    pub thread_id: String,
    pub is_group: bool,
    pub thread_name: String,
    pub selected_ttl: i64,
}

#[derive(Clone, Debug)]
pub struct ContentProtectionModalState {
    pub thread_id: String,
    pub is_group: bool,
    pub thread_name: String,
    pub current_enabled: bool,
}

#[derive(Clone, Debug)]
pub struct GroupMemberPopover {
    pub group_id: String,
    pub member: crate::chat::SidebarMember,
    pub display_name: String,
    pub anchor: Pos2,
    pub confirm_kick: bool,
    pub opened_this_frame: bool,
}

#[derive(Clone)]
struct ImageViewerModal {
    source: worker::ChatImage,
    is_video: bool,
    navigation_pending: bool,
    thread_id: String,
    message_id: String,
    dimensions: Option<(u32, u32)>,
    tex_key: String,
    hd_tex_key: Option<String>,
    original_url: String,
    title: String,
    sender: String,
    time: String,
    zoom: f32,
    pan: Vec2,
    #[allow(dead_code)]
    video_state: Option<VideoPlayerState>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    AccountSecurity,
    Privacy,
    Sync,
    DataManagement,
    Appearance,
    Notifications,
    Messages,
    Calls,
    Utilities,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct AppSettings {
    /// Phone history window in days; `0` syncs the whole history like Zalo PC.
    phone_sync_range_days: u32,
    // 1. Cài đặt chung
    show_all_contacts: bool,
    language: String,
    autostart_on_boot: bool,
    remember_login: bool,
    download_dir: String,
    // 2. Quyền riêng tư
    show_birthday: String,
    show_online_status: bool,
    show_seen_status: bool,
    allow_message_from: String,
    allow_call_from: String,
    allow_search_by_phone: bool,
    friend_src_qr: bool,
    friend_src_group: bool,
    friend_src_card: bool,
    // 3. Giao diện
    theme_mode: String, // "light" | "dark" | "system"
    appearance: theme::AppearanceSettings,
    use_avatar_as_wallpaper: bool,
    info_drawer_open: bool,
    // 4. Thông báo
    enable_notifications: bool,
    play_notification_sound: bool,
    call_notifications_enabled: bool,
    // 5. Tin nhắn
    quick_messages_enabled: bool,
    /// Show "đang soạn tin" from other people.
    show_typing: bool,
    /// Tell others while this account is typing (at most every 10 s per conversation).
    send_typing: bool,
    /// Send photos as the original file; off sends an HD copy (long side 2560 px).
    send_photo_original: bool,
    split_priority_inbox: bool,
    excel_paste_mode: String, // "image" | "text" | "ask"
    double_click_to_reply: bool,
    show_typing_indicator: bool,
    /// Fetch and render link cards while a message is visible.
    link_suggestions_enabled: bool,
    /// Fetch media thumbnails instead of showing the type fallback icon.
    file_thumbnails_enabled: bool,
    // 6. Tiện ích
    suggest_stickers_while_typing: bool,
    suggest_mentions: bool,
    enable_gif_search: bool,
    enable_sticker_keyword_search: bool,
    pub enable_animated_stickers: bool,
    // 7. Thiết bị cuộc gọi (Mic & Camera)
    selected_mic_device: String,
    selected_speaker_device: String,
    selected_camera_device: String,
    // 8. Tính năng AI & Quyền riêng tư
    ai_translation_consent: bool,
    /// Whether application lock is enabled (defaults to false; MAP-00).
    pub enable_app_lock: bool,
    /// Whether Windows Hello biometric unlock is enabled as an optional unlock method.
    pub use_windows_hello: bool,
    /// Salted SHA-256 hash of the 4-digit app lock PIN.
    pub app_pin_hash: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let default_dl = std::env::var("USERPROFILE")
            .ok()
            .map(|p| format!(r"{p}\Downloads\Zalo Received Files"))
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| format!("{h}/Downloads/Zalo Received Files"))
            })
            .unwrap_or_else(|| "Downloads/Zalo Received Files".into());
        Self {
            phone_sync_range_days: 0,
            show_all_contacts: false,
            language: "Tiếng Việt".into(),
            autostart_on_boot: false,
            remember_login: true,
            download_dir: default_dl,
            show_birthday: "Không hiện".into(),
            show_online_status: true,
            show_seen_status: true,
            allow_message_from: "Tất cả mọi người".into(),
            allow_call_from: "Bạn bè và người lạ từng liên hệ".into(),
            allow_search_by_phone: true,
            friend_src_qr: true,
            friend_src_group: true,
            friend_src_card: true,
            theme_mode: "dark".into(),
            appearance: Default::default(),
            use_avatar_as_wallpaper: false,
            info_drawer_open: true,
            enable_notifications: true,
            play_notification_sound: false,
            call_notifications_enabled: true,
            quick_messages_enabled: true,
            show_typing: true,
            send_typing: true,
            send_photo_original: true,
            split_priority_inbox: false,
            excel_paste_mode: "image".into(),
            double_click_to_reply: true,
            show_typing_indicator: true,
            link_suggestions_enabled: true,
            file_thumbnails_enabled: true,
            suggest_stickers_while_typing: true,
            suggest_mentions: true,
            enable_gif_search: true,
            enable_sticker_keyword_search: true,
            enable_animated_stickers: true,
            selected_mic_device: String::new(),
            selected_speaker_device: String::new(),
            selected_camera_device: String::new(),
            ai_translation_consent: false,
            enable_app_lock: false,
            use_windows_hello: false,
            app_pin_hash: None,
        }
    }
}

impl AppSettings {
    fn load() -> Self {
        std::fs::read_to_string(crate::storage::settings_path())
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .unwrap_or_default()
    }

    pub fn has_pin(&self) -> bool {
        self.app_pin_hash.as_deref().is_some_and(|s| !s.is_empty())
    }

    fn lock_configured(&self) -> bool {
        self.enable_app_lock && (self.has_pin() || self.use_windows_hello)
    }

    /// Slow KDF iteration count for PIN hashing (PBKDF2-HMAC-SHA256).
    /// Provides strong resistance against brute-forcing 4-digit PINs if settings are exposed.
    /// Note: App PIN lock is a UI privacy guard and does not replace Windows account/OS security.
    pub const PIN_PBKDF2_ITERATIONS: u32 = 100_000;

    pub fn set_pin(&mut self, pin: &str) -> Result<(), &'static str> {
        if pin.len() != 4 || !pin.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Mã PIN phải gồm đúng 4 chữ số (0-9).");
        }
        use aws_lc_rs::pbkdf2;
        use aws_lc_rs::rand::{SecureRandom, SystemRandom};
        use std::num::NonZeroU32;

        let mut salt = [0u8; 16];
        SystemRandom::new()
            .fill(&mut salt)
            .map_err(|_| "Không tạo được salt bảo mật cho mã PIN.")?;
        let iterations = NonZeroU32::new(Self::PIN_PBKDF2_ITERATIONS).unwrap();
        let mut key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            iterations,
            &salt,
            pin.as_bytes(),
            &mut key,
        );
        self.app_pin_hash = Some(format!(
            "pbkdf2${}${}${}",
            Self::PIN_PBKDF2_ITERATIONS,
            hex::encode(salt),
            hex::encode(key)
        ));
        Ok(())
    }

    pub fn verify_pin(&self, pin: &str) -> bool {
        if pin.len() != 4 || !pin.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        let Some(stored) = self.app_pin_hash.as_deref() else {
            return false;
        };

        if let Some(rest) = stored.strip_prefix("pbkdf2$") {
            let mut parts = rest.split('$');
            let Some(iter_str) = parts.next() else {
                return false;
            };
            let Some(salt_hex) = parts.next() else {
                return false;
            };
            let Some(hash_hex) = parts.next() else {
                return false;
            };
            if parts.next().is_some() {
                return false;
            }

            let Ok(iterations) = iter_str.parse::<u32>() else {
                return false;
            };
            if !(10_000..=5_000_000).contains(&iterations) {
                return false;
            }
            let Ok(salt) = hex::decode(salt_hex) else {
                return false;
            };
            let Ok(expected) = hex::decode(hash_hex) else {
                return false;
            };
            if salt.len() < 16 || expected.len() != 32 {
                return false;
            }
            use aws_lc_rs::pbkdf2;
            use std::num::NonZeroU32;
            let Some(nonzero_iters) = NonZeroU32::new(iterations) else {
                return false;
            };
            let mut computed = [0u8; 32];
            pbkdf2::derive(
                pbkdf2::PBKDF2_HMAC_SHA256,
                nonzero_iters,
                &salt,
                pin.as_bytes(),
                &mut computed,
            );
            aws_lc_rs::constant_time::verify_slices_are_equal(&computed, &expected).is_ok()
        } else if let Some(rest) = stored.strip_prefix("sha256$") {
            // Backwards-compatibility fallback for existing sha256 hashes
            let Some((salt_hex, hash_hex)) = rest.split_once('$') else {
                return false;
            };
            let Ok(salt) = hex::decode(salt_hex) else {
                return false;
            };
            let Ok(expected) = hex::decode(hash_hex) else {
                return false;
            };
            if salt.is_empty() || expected.len() != 32 {
                return false;
            }
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"zalolite-app-pin-v1:");
            hasher.update(&salt);
            hasher.update(b":");
            hasher.update(pin.as_bytes());
            let computed = hasher.finalize();
            aws_lc_rs::constant_time::verify_slices_are_equal(computed.as_slice(), &expected)
                .is_ok()
        } else {
            false
        }
    }

    /// Verify PIN, and if verified against a legacy sha256 hash, automatically migrate to PBKDF2.
    pub fn verify_and_migrate_pin(&mut self, pin: &str) -> bool {
        if !self.verify_pin(pin) {
            return false;
        }
        if self
            .app_pin_hash
            .as_deref()
            .is_some_and(|h| h.starts_with("sha256$"))
        {
            let _ = self.set_pin(pin);
        }
        true
    }
}

/// A loaded-message jump stays bound to its conversation and exact reference.
struct QuoteJump {
    thread_id: String,
    reference: String,
    highlight_until: std::time::Instant,
    scroll_pending: bool,
}

impl QuoteJump {
    fn target_index(&self, thread_id: &str, messages: &[ChatMessage]) -> Option<usize> {
        if self.thread_id != thread_id || std::time::Instant::now() >= self.highlight_until {
            return None;
        }
        crate::message_navigation::unique_reference_index(messages, &self.reference)
    }
}

struct UiSmoke {
    path: std::path::PathBuf,
    frames: u32,
    lifecycle_started: bool,
    /// Capture this many consecutive frames (`<path>-<n>.png`) instead of one at frame 5.
    sequence: u32,
    shots: u32,
    /// Open the fixture group at this frame, then deliver older history two frames later.
    switch_at: u32,
}

type SidebarSortKey = (usize, bool, usize, i64, String);
type SidebarOrderCache = (Vec<SidebarSortKey>, Vec<usize>);

#[derive(Clone, Debug, PartialEq, Eq)]
struct VoiceDraftBinding {
    owner: String,
    runtime_id: usize,
    thread_id: String,
    is_group: bool,
    generation: u64,
}

impl VoiceDraftBinding {
    fn matches(
        &self,
        owner: &str,
        runtime_id: usize,
        thread_id: &str,
        is_group: bool,
        generation: u64,
    ) -> bool {
        self.owner == owner
            && self.runtime_id == runtime_id
            && self.thread_id == thread_id
            && self.is_group == is_group
            && self.generation == generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MediaTab {
    #[default]
    Photos,
    Files,
    Links,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DrawerView {
    #[default]
    Overview,
    Members,
    Board,
    MediaStorage {
        tab: MediaTab,
    },
    SearchInThread,
    SecuritySettings,
}

pub struct ZaloGuiApp {
    tools: tools_panel::ToolsPanel,
    history_search: search_panel::SearchPanel,
    server_pins: crate::server_pins::Snapshot,
    server_board: board_panel::BoardPanel,
    message_pins: board_panel::MessagePins,
    message_details: Option<(String, ChatMessage)>,
    cloud_usage: workspace::CloudUsageState,
    groups_panel: group_panel::GroupPanel,
    group_entry_panel: group_entry_panel::GroupEntryPanel,
    contact_panel: contact_panel::ContactPanel,
    privacy_panel: privacy_panel::PrivacyPanel,
    profile_panel: profile_panel::ProfilePanel,
    avatar_panel: avatar_panel::AvatarPanel,
    hidden_panel: hidden_panel::HiddenPanel,
    inbox_policy: crate::inbox::Policy,
    own_cloud_id: String,
    server_aliases: HashMap<String, String>,
    server_aliases_ready: bool,
    metadata_status: String,
    group_policy_revision: u64,
    labels_panel: labels_panel::LabelsPanel,
    manual_mark_pending: bool,
    manual_mark_uncertain: bool,
    manual_mark_generation: u64,
    archive_pending: bool,
    archive_uncertain: bool,
    archive_generation: u64,
    policy_panel: policy_panel::PolicyPanel,
    group_content: content_panel::ContentPanel,
    reactor_profiles: HashMap<String, crate::chat::SidebarMember>,
    reactor_profile_pending: HashSet<String>,
    backup_panel: backup_panel::BackupPanel,
    #[cfg(windows)]
    tray: Option<crate::tray::Tray>,
    smoke: Option<UiSmoke>,
    // Theme state
    dark_mode: bool,
    theme_dirty: bool,

    // Session state
    is_logged_in: bool,
    user_name: String,
    user_uin: String,
    user_avatar: Option<String>,
    account_runtime_id: usize,

    // App lock state (WP16-4 / MAP-00)
    lock_guard: auth_guard::AppLockGuard,
    auth_in_progress: bool,
    auth_request_generation: u64,
    lock_pin_input: zeroize::Zeroizing<String>,
    lock_pin_error: Option<String>,
    pin_setup_modal_open: bool,
    pin_setup_enable_after: bool,
    pin_setup_first: zeroize::Zeroizing<String>,
    pin_setup_confirm: zeroize::Zeroizing<String>,
    pin_setup_error: Option<String>,

    // Account switcher state (WP16-3)
    #[allow(dead_code)]
    account_switcher: account_switcher::AccountSwitcher,

    // Login screen state
    login_tab: LoginTab,
    phone_input: String,
    password_input: String,
    login_in_progress: bool,
    login_status_msg: String,

    // QR state
    qr_texture: Option<TextureHandle>,
    qr_status: String,

    // Chat screen state
    threads: Vec<Thread>,
    selected_thread_index: usize,
    chat_render_window: (String, usize),
    thread_filter: ThreadFilter,
    unread_only: bool,
    /// Contacts page replaces the conversation list and chat while open.
    contacts_view_open: bool,
    /// An input method is composing text in the chat composer (Telex/IME preedit).
    composer_ime_active: bool,
    /// Opens the composer toolbar; metadata follows the official PC contract (re-notes/46).
    composer_format_mode: bool,
    /// Last rendered chat-list identity; a change discards one stale layout pass.
    chat_layout_key: u64,
    chat_geometry_revision: u64,
    chat_anchor: Option<(String, String, f32)>,
    /// `(thread without prefix, is_group)` → (who is typing, when it was reported).
    typing_map: HashMap<(String, bool), (String, std::time::Instant)>,
    /// Group rosters loaded for the info drawer, by thread ID.
    group_members: HashMap<String, (Vec<crate::chat::SidebarMember>, Option<u64>)>,
    /// Groups whose roster is loading, or failed (with the error).
    group_members_state: HashMap<String, Option<String>>,
    group_member_search: String,
    /// A region capture is waiting for the user.
    screen_capture_pending: bool,
    /// Last typing signal sent per conversation.
    typing_sent: HashMap<String, std::time::Instant>,
    contacts_tab: contacts::ContactsTab,
    contacts_query: String,
    search_query: String,
    input_message: String,
    composer_draft: composer_drafts::ComposerDraft,
    composer_mentions: crate::mentions::Draft,
    attachment_drafts: file_drop::Drafts,
    #[cfg(windows)]
    native_hwnd: Option<isize>,
    deleted_message_ids: HashSet<(String, String)>,
    status_msg: String,

    // Reply / forward state
    replying_to: Option<ChatMessage>,
    forward_msg: Option<ChatMessage>,
    forward_batch: Vec<ChatMessage>,
    forward_cloud_only: bool,
    pub(super) forward_session: Option<modals::ForwardSessionState>,
    forward_filter: String,
    forward_targets: HashSet<String>,
    forward_note: String,
    filter_starred_only: bool,
    active_call: Option<ActiveCallOverlay>,
    call_terminals: crate::call_lifecycle::TerminalCalls,
    image_viewer: Option<ImageViewerModal>,
    doc_viewer: Option<doc_viewer::DocViewerModal>,
    safety_number_modal: Option<SafetyNumberModalState>,
    auto_delete_modal: Option<AutoDeleteModalState>,
    content_protection_modal: Option<ContentProtectionModalState>,
    trusted_peers: HashSet<String>,
    trial_peers: HashSet<String>,
    pub(super) reset_e2ee_confirm_peer: Option<String>,
    pending_reaction_actions: HashMap<(String, String), u64>,
    next_reaction_action_id: u64,
    profiles: Vec<crate::profiles::Profile>,
    profiles_requested: bool,
    sidebar_order: Option<SidebarOrderCache>,

    info_drawer_open: bool,
    pub drawer_view: DrawerView,
    pub drawer_search_query: String,
    pub drawer_media_query: String,
    pub drawer_media_sender: String,
    pub drawer_media_type: Option<crate::message_view::FileCategory>,
    pub drawer_media_from: String,
    pub drawer_media_to: String,
    pub drawer_media_thread_id: String,
    pub drawer_media_active_tab: MediaTab,
    pub drawer_media_filter_key: String,
    pub drawer_media_limit: usize,
    pub drawer_media_selection_mode: bool,
    pub drawer_media_selected: HashSet<String>,
    archive_local_paths: HashMap<String, std::path::PathBuf>,
    archive_mutation_confirm: Option<workspace::ArchiveMutation>,
    archive_mutation_batch: Option<u64>,
    archive_mutation_results: Vec<String>,
    archive_history: workspace::ArchiveHistory,
    drawer_media_batch: Option<u64>,
    drawer_media_batch_status: String,
    // Common groups cache (MAP-07)
    common_groups_cache: HashMap<String, Vec<Thread>>,
    common_groups_state: HashMap<String, Option<String>>,
    common_groups_generations: HashMap<String, u64>,
    common_groups_expanded: bool,
    common_groups_collapsed: bool,

    // Group member popover (MAP-06)
    group_member_popover: Option<GroupMemberPopover>,
    pub(super) pending_drawer_group_actions: HashMap<u64, (String, String)>,
    profile_menu_open: bool,
    sidebar_more_menu_open: bool,
    toast_notice: Option<(String, std::time::Instant)>,
    phone_modal_open: bool,
    phone_modal_input: String,
    phone_lookup_loading: bool,
    phone_lookup_result: Option<crate::chat::PhoneLookupResult>,
    phone_lookup_error: Option<String>,
    // Sticker picker & image cache state
    sticker_picker_open: bool,
    sticker_tab: StickerTab,
    sticker_textures: HashMap<String, TextureHandle>,
    pub sticker_sprites: HashMap<String, StickerSprite>,
    texture_budget: crate::media_resources::ResourceLru,
    loading_stickers: HashSet<String>,
    update_offer: Option<crate::updater::ReleaseUpdate>,
    update_ready: Option<crate::updater::ReadyUpdate>,
    update_busy: bool,
    update_status: String,
    image_retry_at: HashMap<String, std::time::Instant>,
    image_failures: HashMap<String, crate::media_recovery::Retry>,
    downloads: HashMap<String, download_panel::DownloadView>,
    live_mailbox: Option<std::sync::Arc<crate::live_updates::LiveUpdates>>,
    sound_prepared: bool,
    sound_preparing: bool,
    sound_attempts: u8,
    sound_retry_at: Option<std::time::Instant>,
    emoji_textures: HashMap<String, TextureHandle>,
    app_logo_texture: Option<TextureHandle>,
    sticker_store_packs: Vec<crate::chat::StickerPackInfo>,
    sticker_store_loading: bool,
    sticker_store_filter: String,
    sticker_pack_items: HashMap<u64, Vec<crate::chat::StickerItem>>,
    loading_sticker_packs: HashSet<u64>,
    pinned_sticker_packs: Vec<(u64, String)>,
    sticker_search_query: String,
    sticker_search_results: Vec<crate::chat::StickerItem>,
    sticker_search_loading: bool,
    last_sticker_search_dispatched: String,
    last_sticker_search_edit_at: std::time::Instant,
    sticker_suggest_keywords: HashSet<String>,
    composer_sticker_cache: HashMap<String, Vec<crate::chat::StickerItem>>,
    composer_sticker_loading: HashSet<String>,
    dismissed_sticker_suggest_for: String,
    last_composer_text: String,
    last_composer_edit_at: std::time::Instant,
    composer_sticker_selected: Option<usize>,
    composer_focus_pending: bool,
    clipboard_read_pending: bool,
    attachment_picker_pending: bool,
    clipboard_text_fallback: Option<String>,
    clipboard_text_only: bool,
    clipboard_alternative_text: Option<String>,
    pending_clipboard_image: Option<(std::path::PathBuf, u32, u32, TextureHandle)>,
    unread_counts: HashMap<String, usize>,
    conversation_preferences: HashMap<String, crate::store::ConversationPreference>,
    show_hidden_conversations: bool,
    clear_history_confirmation: Option<(String, bool)>,
    link_metadata_cache: HashMap<String, crate::chat::LinkMetadata>,
    loading_link_previews: HashSet<String>,
    dismissed_link_preview_for: String,
    friend_status_map: HashMap<String, crate::chat::FriendshipStatus>,
    friend_requests: HashMap<String, (u64, FriendAction)>,
    friend_check_retry: HashMap<String, std::time::Instant>,
    allow_quit: bool,
    phone_sync_running: bool,
    phone_sync_status: String,
    phone_download_status: String,
    phone_import_status: String,
    history_sync_running: bool,
    history_sync_status: String,
    refresh_sync_running: bool,
    refresh_sync_status: String,
    sync_notice_dismissed: bool,
    sync_success_until: Option<std::time::Instant>,
    sync_cancel_requested: bool,
    last_sync_was_history: bool,
    settings_modal_open: bool,
    settings_devices: modals::SettingsDevices,
    download_picker_pending: bool,
    batch_selection_mode: bool,
    timeline_selection_scope: Option<(String, usize, String, bool)>,
    selected_msg_ids: HashSet<String>,
    send_contact_modal_open: bool,
    send_contact_query: String,
    voice_worker: Option<crate::voice::VoiceWorker>,
    voice_start_pending: bool,
    voice_finish_pending: bool,
    voice_send_after_finish: bool,
    recorded_voice_samples: Option<(std::sync::Arc<[f32]>, u32, std::time::Duration)>,
    voice_binding: Option<VoiceDraftBinding>,
    voice_context_generation: u64,
    settings_tab: SettingsTab,
    settings: AppSettings,

    // AI manual translations: (thread_id, msg_id) -> (translated_text, source_lang, text_hash)
    translations: HashMap<(String, String), (String, Option<String>, u64)>,
    translating_messages: HashSet<(String, String)>,
    // Voice-to-text transcripts: (thread_id, msg_id) -> transcript
    voice_transcripts: HashMap<(String, String), String>,
    transcribing_voices: HashSet<(String, String)>,
    // Measured height cache for virtualization: (thread_id, msg_id, width_bucket) -> height
    measured_msg_heights: HashMap<(String, String, u32), f32>,

    // Dev Inspector state
    dev_mode: bool,
    dev_panel_open: bool,
    dev_tab: DevTab,
    selected_log: Option<LogEntry>,
    scanner_findings: Vec<ScanFinding>,

    // Background channels
    event_rx: Receiver<GuiEvent>,
    #[allow(dead_code)]
    event_tx: Sender<GuiEvent>,
    quote_jump: Option<QuoteJump>,
    quick_schedule: Option<schedule_modal::Draft>,
}

#[derive(Debug, PartialEq, Eq)]
enum LoginTab {
    QrCode,
    Password,
}

#[derive(Debug, PartialEq, Eq)]
enum DevTab {
    Logs,
    Scanner,
    CryptoState,
}

/// Determines whether a status update message represents an actionable error that should trigger a toast notice.
pub fn is_status_error(msg: &str) -> bool {
    msg.starts_with("Không ")
        || msg.starts_with("Lỗi ")
        || msg.starts_with("Thất bại")
        || msg.starts_with("Chưa ")
        || msg.contains("thất bại")
        || msg.contains("không thể")
        || msg.contains("Không thể")
}

impl ZaloGuiApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        session: Session,
        dev_mode: bool,
        initial_threads: Vec<Thread>,
    ) -> Self {
        #[allow(unused_mut)]
        let mut app = Self::from_context(&cc.egui_ctx, session, dev_mode, initial_threads, true);
        #[cfg(windows)]
        if app.smoke.is_none() || std::env::args().any(|a| a == "--tray-smoke-test") {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = cc.window_handle() {
                if let RawWindowHandle::Win32(window) = handle.as_raw() {
                    app.native_hwnd = Some(window.hwnd.get());
                    match crate::tray::Tray::new(cc.egui_ctx.clone(), window.hwnd.get()) {
                        Ok(tray) => app.tray = Some(tray),
                        Err(e) => {
                            app.status_msg =
                                format!("Không tạo được tray: {e}; nút X sẽ thoát ứng dụng")
                        }
                    }
                }
            }
        }
        app
    }

    fn from_context(
        ctx: &egui::Context,
        session: Session,
        dev_mode: bool,
        initial_threads: Vec<Thread>,
        bootstrap: bool,
    ) -> Self {
        setup_custom_fonts(ctx);
        let account_runtime_id = session.runtime_id();
        let loaded_settings = if bootstrap {
            AppSettings::load()
        } else {
            AppSettings::default()
        };
        loaded_settings.appearance.apply_context(ctx);
        crate::media::set_selected_mic_device(&loaded_settings.selected_mic_device);
        crate::media::set_selected_speaker_device(&loaded_settings.selected_speaker_device);
        crate::media::set_selected_camera_device(&loaded_settings.selected_camera_device);
        let dark_mode = match loaded_settings.theme_mode.as_str() {
            "light" => false,
            "system" => ctx.system_theme() != Some(egui::Theme::Light),
            _ => true,
        };
        if dark_mode {
            ThemePalette::dark().apply_to_ctx(ctx);
        } else {
            ThemePalette::light().apply_to_ctx(ctx);
        }

        let (tx, rx) = channel();

        let is_logged_in = session.login.is_some();
        let (user_name, user_uin, user_avatar) = session
            .login
            .as_ref()
            .map(|l| {
                (
                    l.display_name.clone(),
                    l.uin.clone(),
                    if l.avatar.is_empty() {
                        None
                    } else {
                        Some(l.avatar.clone())
                    },
                )
            })
            .unwrap_or_default();

        // The launcher already read the account cache and SQLite before the GUI starts.
        let mut threads = initial_threads;
        for thread in &mut threads {
            crate::message_view::remove_call_logs(thread);
            if bootstrap && thread.is_group {
                thread.presentation.send_policy = None;
                thread.presentation.metadata_verified = false;
            }
        }
        // Self-storage identity is authoritative only when supplied by the server;
        // never rewrite an arbitrary contact ID because its display name is editable.
        crate::chat::deduplicate_threads(&mut threads, None);

        let selected_thread_index = threads
            .iter()
            .enumerate()
            .max_by_key(|(_, t)| t.sidebar_activity_ts())
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        let unread_counts = threads
            .iter()
            .filter(|t| t.unread.count() > 0)
            .map(|t| (t.id.clone(), t.unread.count()))
            .collect();
        // Availability is asynchronous and must not weaken a restored lock policy.
        let lock_guard = auth_guard::AppLockGuard::with_initial_lock(
            is_logged_in && loaded_settings.lock_configured(),
        );
        let mut app = Self {
            #[cfg(windows)]
            tray: None,
            smoke: None,
            dark_mode,
            theme_dirty: false,
            is_logged_in,
            user_name,
            user_uin,
            user_avatar,
            account_runtime_id,
            login_tab: LoginTab::QrCode,
            phone_input: String::new(),
            password_input: String::new(),
            login_in_progress: false,
            login_status_msg: String::new(),
            qr_texture: None,
            qr_status: "Đang tạo mã QR đăng nhập…".into(),
            threads,
            selected_thread_index,
            chat_render_window: (String::new(), 150),
            thread_filter: ThreadFilter::All,
            unread_only: false,
            contacts_view_open: false,
            composer_ime_active: false,
            composer_format_mode: false,
            chat_layout_key: 0,
            chat_geometry_revision: 0,
            chat_anchor: None,
            typing_map: HashMap::new(),
            typing_sent: HashMap::new(),
            screen_capture_pending: false,
            group_members: HashMap::new(),
            group_members_state: HashMap::new(),
            group_member_search: String::new(),
            contacts_tab: contacts::ContactsTab::default(),
            contacts_query: String::new(),
            search_query: String::new(),
            input_message: String::new(),
            composer_draft: Default::default(),
            composer_mentions: Default::default(),
            attachment_drafts: Default::default(),
            #[cfg(windows)]
            native_hwnd: None,
            deleted_message_ids: HashSet::new(),
            replying_to: None,
            forward_msg: None,
            forward_batch: Vec::new(),
            forward_cloud_only: false,
            forward_session: None,
            forward_filter: String::new(),
            forward_targets: HashSet::new(),
            forward_note: String::new(),
            filter_starred_only: false,
            active_call: None,
            call_terminals: Default::default(),
            image_viewer: None,
            doc_viewer: None,
            safety_number_modal: None,
            auto_delete_modal: None,
            content_protection_modal: None,
            trusted_peers: HashSet::new(),
            trial_peers: HashSet::new(),
            reset_e2ee_confirm_peer: None,
            pending_reaction_actions: HashMap::new(),
            next_reaction_action_id: 0,
            profiles: Vec::new(),
            profiles_requested: false,
            sidebar_order: None,
            status_msg: "Đã đăng nhập · Sẵn sàng".into(),
            info_drawer_open: loaded_settings.info_drawer_open,
            drawer_view: DrawerView::Overview,
            drawer_search_query: String::new(),
            drawer_media_query: String::new(),
            drawer_media_sender: String::new(),
            drawer_media_type: None,
            drawer_media_from: String::new(),
            drawer_media_to: String::new(),
            drawer_media_thread_id: String::new(),
            drawer_media_active_tab: MediaTab::Photos,
            drawer_media_filter_key: String::new(),
            drawer_media_limit: 100,
            drawer_media_selection_mode: false,
            drawer_media_selected: HashSet::new(),
            archive_local_paths: HashMap::new(),
            archive_mutation_confirm: None,
            archive_mutation_batch: None,
            archive_mutation_results: Vec::new(),
            archive_history: Default::default(),
            drawer_media_batch: None,
            drawer_media_batch_status: String::new(),
            common_groups_cache: HashMap::new(),
            common_groups_state: HashMap::new(),
            common_groups_generations: HashMap::new(),
            common_groups_expanded: false,
            common_groups_collapsed: false,
            group_member_popover: None,
            pending_drawer_group_actions: HashMap::new(),
            profile_menu_open: false,
            sidebar_more_menu_open: false,
            toast_notice: None,
            phone_modal_open: false,
            phone_modal_input: String::new(),
            phone_lookup_loading: false,
            phone_lookup_result: None,
            phone_lookup_error: None,
            sticker_picker_open: false,
            sticker_tab: StickerTab::Popular,
            sticker_textures: HashMap::new(),
            sticker_sprites: HashMap::new(),
            texture_budget: crate::media_resources::ResourceLru::new(
                crate::media_resources::TEXTURE_BUDGET,
            ),
            loading_stickers: HashSet::new(),
            update_offer: None,
            update_ready: None,
            update_busy: false,
            update_status: String::new(),
            image_retry_at: HashMap::new(),
            image_failures: HashMap::new(),
            downloads: HashMap::new(),
            live_mailbox: None,
            sound_prepared: false,
            sound_preparing: false,
            sound_attempts: 0,
            sound_retry_at: None,
            emoji_textures: load_default_emoji_textures(ctx),
            app_logo_texture: load_app_logo_texture(ctx),
            sticker_store_packs: Vec::new(),
            sticker_store_loading: false,
            sticker_store_filter: String::new(),
            sticker_pack_items: FEATURED_PACK_TABS
                .iter()
                .filter_map(|&(cid, _)| builtin_pack_stickers(cid).map(|items| (cid, items)))
                .collect(),
            loading_sticker_packs: HashSet::new(),
            pinned_sticker_packs: FEATURED_PACK_TABS
                .iter()
                .map(|&(cid, title)| (cid, title.to_string()))
                .collect(),
            sticker_search_query: String::new(),
            sticker_search_results: Vec::new(),
            sticker_search_loading: false,
            last_sticker_search_dispatched: String::new(),
            last_sticker_search_edit_at: std::time::Instant::now(),
            sticker_suggest_keywords: HashSet::new(),
            composer_sticker_cache: HashMap::new(),
            composer_sticker_loading: HashSet::new(),
            dismissed_sticker_suggest_for: String::new(),
            last_composer_text: String::new(),
            last_composer_edit_at: std::time::Instant::now(),
            composer_sticker_selected: None,
            composer_focus_pending: false,
            clipboard_read_pending: false,
            attachment_picker_pending: false,
            clipboard_text_fallback: None,
            clipboard_text_only: false,
            clipboard_alternative_text: None,
            pending_clipboard_image: None,
            unread_counts,
            conversation_preferences: HashMap::new(),
            show_hidden_conversations: false,
            clear_history_confirmation: None,
            link_metadata_cache: HashMap::new(),
            loading_link_previews: HashSet::new(),
            dismissed_link_preview_for: String::new(),
            friend_status_map: HashMap::new(),
            friend_requests: HashMap::new(),
            friend_check_retry: HashMap::new(),
            allow_quit: false,
            phone_sync_running: false,
            phone_sync_status: String::new(),
            phone_download_status: String::new(),
            phone_import_status: String::new(),
            history_sync_running: false,
            history_sync_status: String::new(),
            refresh_sync_running: false,
            refresh_sync_status: String::new(),
            sync_notice_dismissed: false,
            sync_success_until: None,
            sync_cancel_requested: false,
            last_sync_was_history: true,
            settings_modal_open: false,
            settings_devices: Default::default(),
            download_picker_pending: false,
            batch_selection_mode: false,
            timeline_selection_scope: None,
            selected_msg_ids: HashSet::new(),
            send_contact_modal_open: false,
            send_contact_query: String::new(),
            voice_worker: None,
            voice_start_pending: false,
            voice_finish_pending: false,
            voice_send_after_finish: false,
            recorded_voice_samples: None,
            voice_binding: None,
            voice_context_generation: 0,
            settings_tab: SettingsTab::General,
            tools: tools_panel::ToolsPanel::default(),
            history_search: Default::default(),
            server_pins: Default::default(),
            server_board: Default::default(),
            message_pins: Default::default(),
            message_details: None,
            cloud_usage: Default::default(),
            groups_panel: Default::default(),
            group_entry_panel: Default::default(),
            contact_panel: Default::default(),
            privacy_panel: Default::default(),
            profile_panel: Default::default(),
            avatar_panel: Default::default(),
            own_cloud_id: session
                .login
                .as_ref()
                .map(|login| login.send2me_id.clone())
                .unwrap_or_default(),
            inbox_policy: Default::default(),
            server_aliases: Default::default(),
            server_aliases_ready: false,
            metadata_status: String::new(),
            group_policy_revision: 0,
            hidden_panel: hidden_panel::HiddenPanel {
                view: crate::hidden_chat::Snapshot {
                    ready: !is_logged_in,
                    ..Default::default()
                },
                ..Default::default()
            },
            labels_panel: Default::default(),
            manual_mark_pending: false,
            manual_mark_uncertain: false,
            manual_mark_generation: 0,
            archive_pending: false,
            archive_uncertain: false,
            archive_generation: 0,
            policy_panel: Default::default(),
            group_content: Default::default(),
            reactor_profiles: Default::default(),
            reactor_profile_pending: Default::default(),
            backup_panel: Default::default(),
            settings: loaded_settings,
            translations: HashMap::new(),
            translating_messages: HashSet::new(),
            voice_transcripts: HashMap::new(),
            transcribing_voices: HashSet::new(),
            measured_msg_heights: HashMap::new(),
            dev_mode,
            dev_panel_open: false,
            dev_tab: DevTab::Logs,
            selected_log: None,
            scanner_findings: Vec::new(),
            event_rx: rx,
            event_tx: tx.clone(),
            quote_jump: None,
            quick_schedule: None,
            lock_guard,
            auth_in_progress: false,
            auth_request_generation: 0,
            lock_pin_input: Default::default(),
            lock_pin_error: None,
            pin_setup_modal_open: false,
            pin_setup_enable_after: false,
            pin_setup_first: Default::default(),
            pin_setup_confirm: Default::default(),
            pin_setup_error: None,
            account_switcher: account_switcher::AccountSwitcher::default(),
        };

        let startup_owner = app.user_uin.clone();
        std::thread::spawn(move || {
            crate::storage::sweep_orphan_temp_files(Some(&startup_owner));
        });

        if !bootstrap {
            return app;
        }
        let args: Vec<String> = std::env::args().collect();
        if let Some(index) = args.iter().position(|a| a == "--ui-smoke-test") {
            let path = args
                .get(index + 1)
                .expect("--ui-smoke-test requires a PNG path");
            app.smoke = Some(UiSmoke {
                path: path.into(),
                frames: 0,
                lifecycle_started: false,
                sequence: args
                    .iter()
                    .position(|a| a == "--ui-smoke-sequence")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0)
                    .min(30),
                shots: 0,
                switch_at: args
                    .iter()
                    .position(|a| a == "--ui-smoke-switch-at")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0),
            });
            app.is_logged_in = true;
            app.user_name = "ARM64 UI fixture".into();
            app.settings_modal_open = !args
                .iter()
                .any(|a| a == "--ui-smoke-workspace-sync" || a == "--ui-smoke-phone-sync");
            if !app.settings_modal_open {
                app.history_sync_running = true;
                app.history_sync_status =
                    "Lịch sử 2/4: Nhóm thử nghiệm — trang 3, 125 tin mới".into();
                if args.iter().any(|a| a == "--ui-smoke-phone-sync") {
                    app.history_sync_running = false;
                    app.phone_sync_running = true;
                    app.phone_sync_status =
                        "Chờ xác nhận trên điện thoại: mở Zalo và chọn Đồng bộ ngay…".into();
                    app.phone_download_status = "Tải về: lượt 1 • gói 2 • 128 / 512 KiB".into();
                    app.phone_import_status = "Nhập CSDL: 125 tin mới • đang nhập gói 1".into();
                }
                let now = crate::login::unix_now() * 1000;
                app.threads = [
                    (
                        "fixture-direct",
                        "Trò chuyện thử nghiệm",
                        false,
                        now - 120000,
                    ),
                    ("fixture-group", "Nhóm thử nghiệm", true, now),
                    ("fixture-cloud", "Cloud của tôi", false, now - 3600000),
                ]
                .into_iter()
                .map(|(id, name, is_group, ts)| Thread {
                    id: id.into(),
                    name: name.into(),
                    preview: "Dữ liệu giả phục vụ kiểm thử giao diện".into(),
                    time: crate::clock::message_time(ts),
                    avatar: None,
                    is_group,
                    messages: Vec::new(),
                    presentation: crate::chat::SidebarPresentation {
                        last_message_ts: ts,
                        ..Default::default()
                    },
                    unread: Default::default(),
                })
                .collect();
            }
            app.settings_tab = match args
                .iter()
                .position(|a| a == "--ui-smoke-tab")
                .and_then(|i| args.get(i + 1))
                .map(String::as_str)
            {
                Some("appearance") => SettingsTab::Appearance,
                Some("security") | Some("account") => SettingsTab::AccountSecurity,
                Some("sync") => SettingsTab::Sync,
                _ => SettingsTab::General,
            };
            if let Some(scale) = args
                .iter()
                .position(|a| a == "--ui-smoke-scale")
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse::<f32>().ok())
            {
                ctx.set_pixels_per_point(scale.clamp(1.0, 2.0));
            }
            if args.iter().any(|a| a == "--ui-smoke-quote-jump") {
                app.settings_modal_open = false;
                let mut messages: Vec<_> = (0..200)
                    .map(|i| ChatMessage {
                        id: format!("fixture-row-{i}"),
                        client_message_id: format!("fixture-client-{i}"),
                        sender: "Người dùng thử".into(),
                        text: format!("Tin thử nghiệm số {i}"),
                        ts: 1_700_000_000_000 + i * 60_000,
                        time: "10:00".into(),
                        ..Default::default()
                    })
                    .collect();
                messages[8].text = "TIN GỐC ĐƯỢC TRÍCH DẪN — đã cuộn tới đúng ID".into();
                messages[199].reply_to_id = Some("fixture-row-8".into());
                messages[199].reply_to_text = Some(messages[8].text.clone());
                app.threads = vec![Thread {
                    id: "fixture-quote".into(),
                    name: "Kiểm thử trích dẫn".into(),
                    messages,
                    ..Default::default()
                }];
                app.selected_thread_index = 0;
                app.chat_render_window = ("fixture-quote".into(), 150);
                app.quote_jump = Some(QuoteJump {
                    thread_id: "fixture-quote".into(),
                    reference: "fixture-row-8".into(),
                    highlight_until: std::time::Instant::now() + std::time::Duration::from_secs(60),
                    scroll_pending: true,
                });
            }
            if args.iter().any(|a| a == "--ui-smoke-group-long") {
                app.settings_modal_open = false;
                app.history_sync_running = false;
                let senders = ["An Nguyễn", "Bình Trần", "Chi Lê", "Dũng Phạm", "Em Võ"];
                let messages: Vec<_> = (0..150)
                    .map(|i| ChatMessage {
                        id: format!("fixture-long-{i}"),
                        client_message_id: format!("fixture-long-client-{i}"),
                        sender: senders[i % senders.len()].into(),
                        text: if i % 3 == 0 {
                            format!("Tin dài số {i}: {}", "nội dung thảo luận nhóm khá dài để tự xuống nhiều dòng trong bong bóng chat ".repeat(1 + i % 4))
                        } else {
                            format!("Tin ngắn số {i}")
                        },
                        ts: 1_700_000_000_000 + i as i64 * 60_000,
                        time: "10:00".into(),
                        is_outgoing: i % 7 == 0,
                        ..Default::default()
                    })
                    .collect();
                app.threads = vec![
                    Thread {
                        id: "fixture-direct-short".into(),
                        name: "Trò chuyện ngắn".into(),
                        messages: vec![ChatMessage {
                            id: "fixture-short-1".into(),
                            sender: "An".into(),
                            text: "Xin chào".into(),
                            ts: 1_700_000_000_000,
                            time: "10:00".into(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    Thread {
                        id: "fixture-group-long".into(),
                        name: "Nhóm thảo luận dài".into(),
                        is_group: true,
                        messages,
                        ..Default::default()
                    },
                ];
                app.selected_thread_index = if args.iter().any(|a| a == "--ui-smoke-switch-at") {
                    0
                } else {
                    1
                };
            }
            if args.iter().any(|a| a == "--ui-smoke-contacts") {
                app.settings_modal_open = false;
                app.history_sync_running = false;
                app.contacts_view_open = true;
                app.threads.extend(
                    [
                        ("fixture-an", "An Nguyễn"),
                        ("fixture-binh", "Bình Trần"),
                        ("fixture-duc", "Đức Lê"),
                    ]
                    .into_iter()
                    .map(|(id, name)| Thread {
                        id: id.into(),
                        name: name.into(),
                        ..Default::default()
                    }),
                );
            }
            if args.iter().any(|a| a == "--ui-smoke-rich-text") {
                app.settings_modal_open = false;
                app.history_sync_running = false;
                let text = "Chú ý: họp lúc 9h, mang tài liệu cũ và mới";
                let style = |start, len, apply: fn(&mut crate::rich_text::TextStyle)| {
                    let mut s = crate::rich_text::TextStyle {
                        start,
                        len,
                        ..Default::default()
                    };
                    apply(&mut s);
                    s
                };
                app.threads = vec![Thread {
                    id: "fixture-rich".into(),
                    name: "Kiểm thử định dạng".into(),
                    messages: vec![ChatMessage {
                        id: "fixture-rich-1".into(),
                        sender: "Người dùng thử".into(),
                        text: text.into(),
                        ts: 1_700_000_000_000,
                        time: "10:00".into(),
                        text_styles: vec![
                            style(0, 6, |s| s.bold = true),
                            style(15, 2, |s| s.underline = true),
                            style(33, 2, |s| s.strike = true),
                            style(36, 2, |s| s.italic = true),
                        ],
                        ..Default::default()
                    }],
                    ..Default::default()
                }];
                app.selected_thread_index = 0;
                app.input_message = "Dòng 1\nDòng 2\nDòng 3".into();
            }
            if args
                .iter()
                .any(|a| a == "--ui-smoke-photo" || a == "--ui-smoke-media")
            {
                app.settings_modal_open = false;
                app.history_sync_running = false;
                let url = "https://photo-stal-16.zdn.vn/fixture/photo.jpg";
                let image = egui::ColorImage::example();
                let texture = ctx.load_texture("photo-fixture", image, Default::default());
                app.sticker_textures.insert(
                    crate::media_recovery::image_key(
                        "fixture-photo",
                        "fixture-photo-message",
                        0,
                        false,
                    ),
                    texture.clone(),
                );
                app.sticker_textures.insert(
                    crate::media_recovery::image_key("fixture-photo", "fixture-video", 0, false),
                    texture.clone(),
                );
                app.sticker_textures
                    .insert(format!("thumb_{}", crate::crypto::md5_hex(url)), texture);
                app.threads = vec![Thread {
                    id: "fixture-photo".into(),
                    name: "Kiểm thử ảnh nhận được".into(),
                    messages: vec![ChatMessage {
                        id: "fixture-photo-message".into(),
                        kind: crate::message_model::MessageKind::Link,
                        text: url.into(),
                        link_url: Some(url.into()),
                        link_thumb: Some(url.into()),
                        link_title: Some("Ảnh thử nghiệm".into()),
                        ts: crate::login::unix_now() * 1000,
                        ..Default::default()
                    }],
                    ..Default::default()
                }];
                app.selected_thread_index = 0;
                if args.iter().any(|a| a == "--ui-smoke-media") {
                    let t = &mut app.threads[0];
                    t.is_group = true;
                    t.name = "Nhóm kiểm thử media".into();
                    t.messages[0].sender = "Nguyễn An".into();
                    t.messages.push(ChatMessage {
                        id: "fixture-video".into(),
                        kind: crate::message_model::MessageKind::Video,
                        text: "[Video]".into(),
                        sender: "Trần Bình".into(),
                        link_thumb: Some(url.into()),
                        link_url: Some("https://video.zdn.vn/fixture/movie.mp4".into()),
                        link_title: Some("Video thử nghiệm.mp4".into()),
                        ts: crate::login::unix_now() * 1000,
                        ..Default::default()
                    });
                    t.messages.push(ChatMessage {
                        id: "fixture-file".into(),
                        kind: crate::message_model::MessageKind::File,
                        text: "[Tệp]".into(),
                        sender: "Nguyễn An".into(),
                        link_url: Some("https://file.zdn.vn/fixture/report.xlsx".into()),
                        link_title: Some("Báo cáo.xlsx".into()),
                        ts: crate::login::unix_now() * 1000,
                        ..Default::default()
                    });
                }
            }
            if args.iter().any(|a| a == "--ui-smoke-outgoing") {
                app.settings_modal_open = false;
                app.history_sync_running = false;
                let mut failed = crate::outgoing::pending(
                    "Tin nhắn này vẫn được giữ khi mất mạng.".into(),
                    false,
                    crate::outgoing::Payload::Text,
                );
                failed.delivery = crate::message_model::DeliveryState::Failed;
                failed.outgoing_job.as_mut().unwrap().error = "Không có kết nối mạng. Kiểm tra hội thoại trước khi gửi lại để tránh trùng tin.".into();
                let waiting = crate::outgoing::pending(
                    "Tệp tiếp theo đang chờ tải lên.".into(),
                    false,
                    crate::outgoing::Payload::Text,
                );
                app.threads = vec![Thread {
                    id: "outgoing-fixture".into(),
                    name: "Kiểm thử gửi lại".into(),
                    messages: vec![failed, waiting],
                    ..Default::default()
                }];
                app.selected_thread_index = 0;
            }
            if args
                .iter()
                .any(|a| a == "--ui-smoke-schedule" || a == "--ui-smoke-ami")
            {
                app.settings_modal_open = false;
                app.history_sync_running = false;
                app.user_uin = "fixture-owner".into();
                app.threads = vec![Thread {
                    id: "fixture-schedule".into(),
                    name: "Nhóm thử nghiệm".into(),
                    is_group: true,
                    ..Default::default()
                }];
                app.selected_thread_index = 0;
                app.input_message =
                    "Hẹn gửi báo cáo lúc 08:00. Giữ nguyên 2*3*4 và tiếng Việt.".into();
                if args.iter().any(|a| a == "--ui-smoke-schedule") {
                    app.open_quick_schedule(ctx);
                }
                if args.iter().any(|a| a == "--ui-smoke-ami") {
                    app.sticker_picker_open = true;
                    app.sticker_tab = StickerTab::Pack(crate::bundled_stickers::CATEGORY);
                    for item in crate::bundled_stickers::ITEMS {
                        let image = crate::limits::decode_image(item.png)
                            .expect("bundled fixture")
                            .to_rgba8();
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [image.width() as usize, image.height() as usize],
                            image.as_raw(),
                        );
                        let key = format!("stk_{}_{}", crate::bundled_stickers::CATEGORY, item.id);
                        app.sticker_textures.insert(
                            key.clone(),
                            ctx.load_texture(key, image, Default::default()),
                        );
                    }
                }
            }
            if args.iter().any(|a| a == "--ui-smoke-tools") {
                app.settings_modal_open = false;
                app.history_sync_running = false;
                app.user_uin = "fixture-owner".into();
                app.tools = tools_panel::ToolsPanel {
                    open: true,
                    loaded_for: "fixture-owner".into(),
                    tab: args
                        .iter()
                        .position(|a| a == "--ui-smoke-tools-tab")
                        .and_then(|i| args.get(i + 1))
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(0)
                        .min(4),
                    ..Default::default()
                };
                app.tools.config.rules.push(crate::tools::ReplyRule {
                    id: "fixture-rule".into(),
                    keyword: "Xin chào".into(),
                    reply_text: "Chào bạn, tôi sẽ trả lời sớm.".into(),
                    mode: "contains".into(),
                    scope: "direct".into(),
                    enabled: false,
                    enabled_since: 0,
                });
                app.tools.query = "lịch sử".into();
                app.tools.hits.push(crate::tools::SearchHit {
                    thread_id: "fixture-chat".into(),
                    message_id: "fixture-result".into(),
                    thread_name: "Hội thoại thử nghiệm".into(),
                    text: "Nội dung lịch sử từ SQLite, chưa cần nạp cả hội thoại vào RAM.".into(),
                    time: crate::login::unix_now() * 1000,
                });
                app.threads = vec![Thread {
                    id: "fixture-chat".into(),
                    name: "Hội thoại thử nghiệm".into(),
                    ..Default::default()
                }];
                app.selected_thread_index = 0;
                app.tools.tasks.push(crate::tools::Scheduled {
                    id: "fixture-task".into(),
                    owner: "fixture-owner".into(),
                    thread_id: "fixture-chat".into(),
                    deliver_at: crate::login::unix_now() + 900,
                    message: crate::outgoing::pending(
                        "Tin hẹn giờ thử nghiệm".into(),
                        false,
                        crate::outgoing::Payload::Text,
                    ),
                    status: "pending".into(),
                    error: String::new(),
                });
            }
            if args.iter().any(|a| a == "--ui-smoke-light") {
                app.dark_mode = false;
                ThemePalette::light().apply_to_ctx(ctx);
            }
            if args.iter().any(|a| a == "--ui-smoke-viewer") {
                if let Some((thread, message)) = app
                    .threads
                    .first()
                    .and_then(|t| t.messages.first().map(|m| (t.id.clone(), m.clone())))
                {
                    app.open_media_viewer(&thread, &message);
                }
            }
            // No worker, account restore, network calls, or real contacts in smoke mode.
            return app;
        }

        spawn_session_worker(session, tx, ctx.clone());

        if let Some(wtx) = WORKER_TX.get() {
            if is_logged_in {
                let _ = wtx.send(WorkerCmd::RefreshThreads);
            } else {
                let _ = wtx.send(WorkerCmd::RequestQr);
            }
        }

        app
    }

    pub(super) fn can_lock_app(&self) -> bool {
        self.settings.lock_configured()
    }

    pub(super) fn lock_app(&mut self) {
        if !self.is_logged_in || !self.can_lock_app() {
            return;
        }
        self.lock_pin_input.clear();
        self.lock_pin_error = None;
        self.lock_guard.lock();
    }

    fn handle_key_events(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Q)) {
            self.allow_quit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.is_logged_in
            && !self.lock_guard.is_locked()
            && self.can_lock_app()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::L))
        {
            self.lock_app();
            ctx.request_repaint();
        }
    }

    fn friend_action_pending(&self, uid: &str) -> bool {
        self.friend_requests.contains_key(uid)
    }

    fn request_friend_action(&mut self, uid: &str, action: FriendAction) {
        if !self.is_logged_in || uid.is_empty() || self.friend_action_pending(uid) {
            return;
        }
        if action == FriendAction::Check
            && self
                .friend_check_retry
                .get(uid)
                .is_some_and(|at| at.elapsed() < Duration::from_secs(15))
        {
            return;
        }
        let generation = crate::search::generation();
        if WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::FriendAction {
                owner: self.user_uin.clone(),
                runtime_id: self.account_runtime_id,
                generation,
                user_id: uid.to_owned(),
                action,
            })
            .is_ok()
        }) {
            self.friend_requests
                .insert(uid.to_owned(), (generation, action));
        } else {
            self.friend_check_retry
                .insert(uid.to_owned(), std::time::Instant::now());
            if action != FriendAction::Check {
                self.show_toast("Worker chưa sẵn sàng");
            }
        }
    }

    fn save_settings(&mut self) {
        match serde_json::to_value(&self.settings) {
            Ok(settings) => {
                if let Some(tx) = WORKER_TX.get() {
                    if tx
                        .send(WorkerCmd::SaveAppSettings {
                            settings,
                            persist_session: self.settings.remember_login,
                        })
                        .is_err()
                    {
                        self.show_toast("Không gửi được yêu cầu lưu cài đặt");
                    }
                }
            }
            Err(_) => self.show_toast("Không mã hóa được cài đặt"),
        }
    }

    fn palette(&self) -> ThemePalette {
        let mut palette = if self.dark_mode {
            ThemePalette::dark()
        } else {
            ThemePalette::light()
        };
        palette.apply_tools_theme(&self.tools.config.theme);
        self.settings.appearance.apply_palette(&mut palette);
        palette.ensure_readable_text();
        palette
    }

    /// Forget per-account typing, roster and capture state at an account boundary.
    fn clear_presence_state(&mut self) {
        self.backup_panel
            .cancel
            .store(true, std::sync::atomic::Ordering::Release);
        self.backup_panel = Default::default();
        self.call_terminals = Default::default();
        self.message_pins = Default::default();
        self.message_details = None;
        self.server_board = Default::default();
        self.cloud_usage = Default::default();
        self.groups_panel = Default::default();
        self.group_entry_panel = Default::default();
        self.contact_panel = Default::default();
        self.friend_status_map.clear();
        self.friend_requests.clear();
        self.friend_check_retry.clear();
        self.privacy_panel = Default::default();
        self.profile_panel = Default::default();
        self.avatar_panel = Default::default();
        self.hidden_panel = Default::default();
        self.inbox_policy = Default::default();
        self.own_cloud_id.clear();
        self.server_aliases.clear();
        self.server_aliases_ready = false;
        self.metadata_status.clear();
        self.group_policy_revision = 0;
        self.labels_panel = Default::default();
        self.manual_mark_pending = false;
        self.manual_mark_uncertain = false;
        self.manual_mark_generation = crate::search::generation();
        self.archive_pending = false;
        self.archive_uncertain = false;
        self.archive_generation = crate::search::generation();
        self.policy_panel = Default::default();
        self.group_content = Default::default();
        self.reactor_profiles.clear();
        self.reactor_profile_pending.clear();
        self.unread_only = false;
        self.sidebar_order = None;
        self.profiles.clear();
        self.profiles_requested = false;
        self.trusted_peers.clear();
        self.trial_peers.clear();
        self.reset_e2ee_confirm_peer = None;
        self.pending_reaction_actions.clear();
        self.safety_number_modal = None;
        self.auto_delete_modal = None;
        self.content_protection_modal = None;
        self.typing_map.clear();
        self.typing_sent.clear();
        self.group_members.clear();
        self.group_member_popover = None;
        self.common_groups_cache.clear();
        self.common_groups_state.clear();
        self.common_groups_generations.clear();
        self.common_groups_expanded = false;
        self.common_groups_collapsed = false;
        self.sync_success_until = None;
        self.group_members_state.clear();
        self.group_member_search.clear();
        self.screen_capture_pending = false;
        self.clipboard_text_fallback = None;
        self.clipboard_alternative_text = None;
        self.clipboard_text_only = false;
        self.clear_forward_dialog();
        self.clear_timeline_selection();
        self.archive_local_paths.clear();
        self.archive_mutation_confirm = None;
        self.archive_mutation_batch = None;
        self.archive_mutation_results.clear();
        self.archive_history = Default::default();
        self.drawer_media_filter_key.clear();
        self.drawer_media_batch = None;
        self.drawer_media_batch_status.clear();
        self.drawer_media_selected.clear();
        self.drawer_media_selection_mode = false;
    }

    /// Clear all state belonging to the 2-step forward/share dialog.
    pub(super) fn clear_forward_dialog(&mut self) {
        self.forward_msg = None;
        self.forward_batch.clear();
        self.forward_cloud_only = false;
        self.forward_session = None;
        self.forward_filter.clear();
        self.forward_targets.clear();
        self.forward_note.clear();
    }

    /// Return true whenever any modal dialog, blocking overlay, or popover menu is open.
    pub(super) fn is_any_modal_open(&self) -> bool {
        self.settings_modal_open
            || self.forward_msg.is_some()
            || self.forward_session.is_some()
            || self.phone_modal_open
            || self.pin_setup_modal_open
            || self.active_call.is_some()
            || self.image_viewer.is_some()
            || self.doc_viewer.is_some()
            || self.safety_number_modal.is_some()
            || self.auto_delete_modal.is_some()
            || self.content_protection_modal.is_some()
            || self.reset_e2ee_confirm_peer.is_some()
            || self.send_contact_modal_open
            || self.clear_history_confirmation.is_some()
            || self.quick_schedule.is_some()
            || self.tools.open
            || self.history_search.open
            || self.server_board.open
            || self.message_details.is_some()
            || self.archive_mutation_confirm.is_some()
            || self.groups_panel.open
            || self.group_entry_panel.open
            || self.contact_panel.open
            || self.profile_panel.open
            || self.avatar_panel.open
            || self.hidden_panel.open
            || self.labels_panel.open
            || self.policy_panel.target.is_some()
            || self.group_content.open
            || self.backup_panel.open
            || self.profile_menu_open
            || self.sidebar_more_menu_open
            || self.group_member_popover.is_some()
    }

    /// Return true when background chat hover, context menus, and tooltips must be suppressed.
    pub(super) fn is_workspace_input_gated(&self, ctx: &egui::Context) -> bool {
        self.is_any_modal_open()
            || ctx.data(|d| {
                d.get_temp::<(String, String, ChatMessage)>(Id::new("reaction_details"))
                    .is_some()
            })
    }

    fn request_screen_capture(&mut self) {
        if self.screen_capture_pending {
            return;
        }
        let Some(thread) = self.threads.get(self.selected_thread_index) else {
            return;
        };
        if let Some(tx) = WORKER_TX.get() {
            self.screen_capture_pending = tx
                .send(WorkerCmd::CaptureScreen {
                    owner: self.user_uin.clone(),
                    thread_id: thread.id.clone(),
                    is_group: thread.is_group,
                })
                .is_ok();
            if self.screen_capture_pending {
                self.show_toast("Chọn vùng màn hình cần chụp…");
            }
        }
    }

    fn request_clipboard_image(&mut self) {
        if self.clipboard_read_pending {
            return;
        }
        if let Some(tx) = WORKER_TX.get() {
            self.clipboard_read_pending = tx
                .send(WorkerCmd::ReadClipboardImage {
                    owner: self.user_uin.clone(),
                    thread: self
                        .threads
                        .get(self.selected_thread_index)
                        .map(|t| t.id.clone())
                        .unwrap_or_default(),
                })
                .is_ok();
        }
    }

    fn persist_threads_async(&self) {
        let snapshot = self.threads.clone();
        if let Some(wtx) = WORKER_TX.get() {
            let _ = wtx.send(WorkerCmd::PersistThreads {
                owner: self.user_uin.clone(),
                threads: snapshot,
            });
        }
    }

    fn trim_resident_history(&mut self) {
        for (index, thread) in self.threads.iter_mut().enumerate() {
            let keep = if index == self.selected_thread_index {
                self.chat_render_window.1.max(500)
            } else {
                crate::store::MessageStore::PRELOAD_MSGS
            };
            let drop = thread.messages.len().saturating_sub(keep);
            if drop > 0 {
                thread.messages.drain(..drop);
            }
            if thread.messages.capacity() > keep.saturating_mul(2) {
                thread.messages.shrink_to_fit();
            }
        }
    }

    fn store_texture(&mut self, ctx: &egui::Context, key: String, image: egui::ColorImage) {
        if image.size[0] > 8192 || image.size[1] > 8192 {
            crate::logger::log_error(
                "store_texture",
                &format!(
                    "Texture {key} dimensions {:?} exceed 8192 limit",
                    image.size
                ),
            );
            return;
        }
        let bytes = image.pixels.len() as u64 * 4;
        if bytes > crate::media_resources::TEXTURE_BUDGET {
            return;
        }
        for evicted in self.texture_budget.insert(key.clone(), bytes) {
            self.sticker_textures.remove(&evicted);
            self.sticker_sprites.remove(&evicted);
        }
        if let Some(texture) = self.sticker_textures.get_mut(&key) {
            texture.set(image, TextureOptions::LINEAR);
        } else {
            self.sticker_textures.insert(
                key.clone(),
                ctx.load_texture(key, image, TextureOptions::LINEAR),
            );
        }
    }

    fn document_context_alive(
        &self,
        owner: &str,
        runtime_id: usize,
        context: &worker::ChatImage,
    ) -> bool {
        self.is_logged_in
            && owner == self.user_uin
            && runtime_id == self.account_runtime_id
            && self.private_access(&context.thread, context.group)
            && !crate::message_lifecycle::expired(&context.message, crate::login::unix_now() * 1000)
    }

    fn check_events(&mut self, ctx: &egui::Context) {
        for index in 0..64 {
            let Ok(event) = self.event_rx.try_recv() else {
                break;
            };
            if index == 63 {
                ctx.request_repaint();
            }

            // Keep business state current while locked; rendering and input remain gated.
            let server_ack = matches!(&event, GuiEvent::MessageSent { .. });
            let history_status = match &event {
                GuiEvent::ConversationHistoryLoaded { status, .. } => Some(status.clone()),
                _ => None,
            };
            let event = match event {
                GuiEvent::ConversationHistoryLoaded {
                    runtime_id,
                    threads,
                    status,
                } => {
                    if !self.is_logged_in || runtime_id != self.account_runtime_id {
                        continue;
                    }
                    if threads.is_empty() {
                        GuiEvent::StatusUpdate(status)
                    } else {
                        self.status_msg = status;
                        GuiEvent::ThreadsUpdated {
                            threads,
                            source: ThreadUpdateSource::History,
                        }
                    }
                }
                GuiEvent::LiveThreadsReady(updates) => {
                    self.live_mailbox = Some(updates.clone());
                    GuiEvent::ThreadsUpdated {
                        threads: updates.take(),
                        source: ThreadUpdateSource::Live,
                    }
                }
                GuiEvent::LocalCallRecord {
                    outgoing,
                    runtime_id,
                    owner,
                    thread_id,
                    text,
                    msg_id,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || runtime_id != self.account_runtime_id
                    {
                        continue;
                    }
                    let mut inserted = false;
                    if let Some(thread) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        if !thread.messages.iter().any(|m| m.id == msg_id) {
                            thread.messages.push(ChatMessage {
                                id: msg_id,
                                text,
                                ts: crate::login::unix_now() * 1000,
                                is_outgoing: outgoing,
                                sender: if outgoing {
                                    "Bạn".into()
                                } else {
                                    thread.name.clone()
                                },
                                ..Default::default()
                            });
                            thread.messages.sort_by_key(|m| m.ts);
                            thread.refresh_sidebar_presentation();
                            inserted = true;
                        }
                    }
                    if inserted {
                        self.persist_threads_async();
                    }
                    continue;
                }
                event => event,
            };
            match event {
                GuiEvent::MessageTranslated {
                    thread_id,
                    msg_id,
                    translated_text,
                    source_lang,
                    text_hash,
                    generation: _,
                } => {
                    self.measured_msg_heights
                        .retain(|(t, m, _), _| !(t == &thread_id && m == &msg_id));
                    self.translating_messages
                        .remove(&(thread_id.clone(), msg_id.clone()));
                    self.translations.insert(
                        (thread_id, msg_id),
                        (translated_text, source_lang, text_hash),
                    );
                    ctx.request_repaint();
                }
                GuiEvent::MessageTranslateFailed {
                    thread_id,
                    msg_id,
                    error,
                    generation: _,
                } => {
                    self.measured_msg_heights
                        .retain(|(t, m, _), _| !(t == &thread_id && m == &msg_id));
                    self.translating_messages.remove(&(thread_id, msg_id));
                    self.show_toast(format!("Dịch tin nhắn thất bại: {error}"));
                    ctx.request_repaint();
                }
                GuiEvent::VoiceTranscriptLoaded {
                    thread_id,
                    msg_id,
                    transcript,
                    generation: _,
                } => {
                    self.measured_msg_heights
                        .retain(|(t, m, _), _| !(t == &thread_id && m == &msg_id));
                    self.transcribing_voices
                        .remove(&(thread_id.clone(), msg_id.clone()));
                    self.voice_transcripts
                        .insert((thread_id, msg_id), transcript);
                    ctx.request_repaint();
                }
                GuiEvent::VoiceTranscriptFailed {
                    thread_id,
                    msg_id,
                    error,
                    generation: _,
                } => {
                    self.measured_msg_heights
                        .retain(|(t, m, _), _| !(t == &thread_id && m == &msg_id));
                    self.transcribing_voices.remove(&(thread_id, msg_id));
                    self.show_toast(format!("Chuyển giọng nói thất bại: {error}"));
                    ctx.request_repaint();
                }
                GuiEvent::AppSettingsSaved(result) => {
                    self.profiles_requested = false;
                    self.profiles.clear();
                    if let Err(error) = result {
                        self.show_toast(error);
                    }
                }
                GuiEvent::ProfilesLoaded { owner, result } => {
                    if owner == self.user_uin {
                        match result {
                            Ok(profiles) => self.profiles = profiles,
                            Err(error) => self.show_toast(error),
                        }
                    }
                }
                GuiEvent::SafetyNumberLoaded {
                    owner,
                    peer_id,
                    peer_name,
                    safety_number,
                    is_verified,
                    identity_changed,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    if is_verified {
                        self.trusted_peers.insert(peer_id.clone());
                    } else {
                        self.trusted_peers.remove(&peer_id);
                    }
                    self.safety_number_modal = Some(SafetyNumberModalState {
                        peer_id,
                        peer_name,
                        safety_number,
                        is_verified,
                        identity_changed,
                        loading: false,
                    });
                    ctx.request_repaint();
                }
                GuiEvent::IdentityTrusted {
                    owner,
                    peer_id,
                    is_trusted,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    if is_trusted {
                        self.trusted_peers.insert(peer_id.clone());
                        self.status_msg = "Đã xác minh danh tính thiết bị an toàn".into();
                    } else {
                        self.trusted_peers.remove(&peer_id);
                    }
                    if let Some(ref mut m) = self.safety_number_modal {
                        if m.peer_id == peer_id {
                            m.is_verified = is_trusted;
                            m.loading = false;
                        }
                    }
                    ctx.request_repaint();
                }
                GuiEvent::E2eeTrialPrepared {
                    owner,
                    peer_id,
                    result,
                } => {
                    if owner == self.user_uin {
                        match result {
                            Ok(()) => {
                                self.trial_peers.insert(peer_id);
                                self.show_toast("Đã bật thử E2EE. Tin văn bản tiếp theo sẽ thử gửi mã hóa; không tự quay về gửi thường.");
                            }
                            Err(message) => self.show_toast(message),
                        }
                    }
                }
                GuiEvent::E2eePeersHydrated { owner, peers } => {
                    if owner == self.user_uin {
                        self.trial_peers.extend(peers);
                    }
                }
                GuiEvent::E2eePeerRequired { owner, peer_id } => {
                    if owner == self.user_uin {
                        self.trial_peers.insert(peer_id);
                    }
                }
                GuiEvent::SafetyNumberFailed {
                    owner,
                    peer_id,
                    message,
                } => {
                    if owner == self.user_uin {
                        self.trusted_peers.remove(&peer_id);
                        if self
                            .safety_number_modal
                            .as_ref()
                            .is_some_and(|m| m.peer_id == peer_id)
                        {
                            self.safety_number_modal = None;
                        }
                        self.show_toast(message);
                    }
                }
                GuiEvent::DocPreviewLoaded {
                    owner,
                    runtime_id,
                    context,
                    file_name,
                    content,
                } => {
                    if !self.document_context_alive(&owner, runtime_id, &context) {
                        continue;
                    }
                    self.doc_viewer = Some(doc_viewer::DocViewerModal::new(
                        content, file_name, owner, runtime_id, context,
                    ));
                    ctx.request_repaint();
                }
                GuiEvent::BusinessCatalogLoaded(_) => continue,
                GuiEvent::DevicesLoaded(_) => continue,
                GuiEvent::TwoFactorStatusLoaded(_) => continue,
                GuiEvent::DroppedFilePrepared {
                    owner,
                    thread,
                    id,
                    result,
                } => {
                    if self.is_logged_in && owner == self.user_uin {
                        self.attachment_drafts
                            .complete(ctx, &owner, &thread, id, result);
                    }
                }
                GuiEvent::ComposerDraftLoaded {
                    owner,
                    thread,
                    generation,
                    result,
                } => self.composer_draft_loaded(owner, thread, generation, result),
                GuiEvent::ComposerDraftSaved {
                    owner,
                    thread,
                    generation,
                    text,
                    result,
                } => self.composer_draft_saved(ctx, owner, thread, generation, text, result),
                GuiEvent::MediaStarted {
                    owner,
                    thread,
                    id,
                    name,
                    cancel,
                } => {
                    if owner == self.user_uin
                        && self.is_logged_in
                        && thread
                            .as_ref()
                            .is_none_or(|(id, group)| self.private_access(id, *group))
                    {
                        self.downloads.insert(
                            id,
                            download_panel::DownloadView {
                                thread,
                                name,
                                bytes: 0,
                                total: None,
                                cancel,
                                result: None,
                            },
                        );
                    } else {
                        cancel.store(true, std::sync::atomic::Ordering::Release);
                    }
                }
                GuiEvent::MediaProgress {
                    owner,
                    id,
                    bytes,
                    total,
                } => {
                    if owner == self.user_uin {
                        if let Some(d) = self.downloads.get_mut(&id) {
                            d.bytes = bytes;
                            d.total = total;
                        }
                    }
                }
                GuiEvent::MediaTransientClosed { owner, id } => {
                    if owner == self.user_uin {
                        self.downloads.remove(&id);
                    }
                }
                GuiEvent::MediaFinished { owner, id, result } => {
                    if owner == self.user_uin {
                        if let Some(d) = self.downloads.get_mut(&id) {
                            d.result = Some(result);
                        }
                    }
                }
                GuiEvent::ArchiveMutationFinished {
                    owner,
                    runtime_id,
                    batch,
                    thread,
                    recall,
                    results,
                } => {
                    if owner != self.user_uin
                        || runtime_id != self.account_runtime_id
                        || !self.is_logged_in
                        || self.archive_mutation_batch != Some(batch)
                    {
                        continue;
                    }
                    self.archive_mutation_batch = None;
                    self.archive_mutation_results.clear();
                    for (message, result) in results {
                        let label = format!(
                            "{} ({})",
                            crate::message_view::summary(&message),
                            message.id
                        );
                        match result {
                            Ok(()) => {
                                self.archive_mutation_results.push(format!(
                                    "{}: {}",
                                    if recall {
                                        "Đã thu hồi"
                                    } else {
                                        "Đã xóa phía bạn"
                                    },
                                    label
                                ));
                                if recall {
                                    if let Some(t) =
                                        self.threads.iter_mut().find(|t| t.id == thread)
                                    {
                                        for current in &mut t.messages {
                                            if crate::chat::same_message(current, &message) {
                                                current.mark_recalled();
                                            }
                                        }
                                        t.refresh_after_removal();
                                    }
                                    if self.archive_history.thread == thread {
                                        for current in &mut self.archive_history.messages {
                                            if crate::chat::same_message(current, &message) {
                                                current.mark_recalled();
                                            }
                                        }
                                    }
                                } else {
                                    self.deleted_message_ids
                                        .insert((thread.clone(), message.id.clone()));
                                    if let Some(t) =
                                        self.threads.iter_mut().find(|t| t.id == thread)
                                    {
                                        t.messages.retain(|current| {
                                            !crate::chat::same_message(current, &message)
                                        });
                                        t.refresh_after_removal();
                                    }
                                    if self.archive_history.thread == thread {
                                        self.archive_history.messages.retain(|current| {
                                            !crate::chat::same_message(current, &message)
                                        });
                                    }
                                }
                            }
                            Err(error) => self
                                .archive_mutation_results
                                .push(format!("{label}: {error}")),
                        }
                    }
                    self.chat_geometry_revision = self.chat_geometry_revision.wrapping_add(1);
                    self.drawer_media_selected.clear();
                    self.invalidate_history_search();
                }
                GuiEvent::ArchiveHistoryLoaded {
                    owner,
                    runtime_id,
                    thread,
                    generation,
                    result,
                } => {
                    if owner != self.user_uin
                        || runtime_id != self.account_runtime_id
                        || !self.is_logged_in
                        || thread != self.archive_history.thread
                        || generation != self.archive_history.generation
                    {
                        continue;
                    }
                    self.archive_history.loading = false;
                    if !self
                        .threads
                        .iter()
                        .find(|t| t.id == thread)
                        .is_some_and(|t| self.private_access(&t.id, t.is_group))
                    {
                        self.archive_history = Default::default();
                        self.drawer_media_filter_key.clear();
                        continue;
                    }
                    match result {
                        Ok(page) => {
                            self.archive_history.loaded = true;
                            self.archive_history.error.clear();
                            self.archive_history.scanned += page.scanned;
                            self.archive_history.cursor = page.next;
                            for message in page.messages {
                                if !self
                                    .archive_history
                                    .messages
                                    .iter()
                                    .any(|old| crate::chat::same_message(old, &message))
                                {
                                    self.archive_history.messages.push(message);
                                }
                            }
                        }
                        Err(error) => self.archive_history.error = error,
                    }
                }
                GuiEvent::MediaBatchFinished {
                    owner,
                    batch,
                    results,
                } => {
                    if owner == self.user_uin
                        && self.is_logged_in
                        && self.drawer_media_batch == Some(batch)
                    {
                        self.drawer_media_batch = None;
                        for (key, result) in &results {
                            if let Ok(path) = result {
                                self.archive_local_paths.insert(key.clone(), path.clone());
                            }
                        }
                        let succeeded = results.iter().filter(|(_, result)| result.is_ok()).count();
                        self.drawer_media_batch_status = format!(
                            "Đã lưu {succeeded}/{} mục; xem kết quả từng mục trong Tải tệp.",
                            results.len()
                        );
                    }
                }
                GuiEvent::ScreenCaptureFinished {
                    owner,
                    thread_id,
                    is_group,
                    result,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        if let Ok((path, ..)) = result {
                            // Best-effort cleanup of a capture whose account was closed.
                            std::thread::spawn(move || {
                                let _ = std::fs::remove_file(path);
                            });
                        }
                        continue;
                    }
                    self.screen_capture_pending = false;
                    if !self
                        .threads
                        .get(self.selected_thread_index)
                        .is_some_and(|t| t.id == thread_id && t.is_group == is_group)
                    {
                        if let Ok((path, ..)) = result {
                            std::thread::spawn(move || {
                                let _ = std::fs::remove_file(path);
                            });
                        }
                        self.show_toast("Đã đổi hội thoại; hãy chụp lại để đính kèm đúng nơi");
                        continue;
                    }
                    match result {
                        Ok((path, w, h, image)) => {
                            if let Some((old_path, ..)) = self.pending_clipboard_image.take() {
                                std::thread::spawn(move || {
                                    let _ = std::fs::remove_file(old_path);
                                });
                            }
                            let texture = ctx.load_texture(
                                "clipboard-preview",
                                image,
                                TextureOptions::LINEAR,
                            );
                            self.pending_clipboard_image = Some((path, w, h, texture));
                            self.composer_focus_pending = true;
                            self.show_toast(format!(
                                "Đã chụp màn hình ({w}×{h} px) — nhấn Enter để gửi"
                            ));
                        }
                        Err(reason) => self.show_toast(reason),
                    }
                }
                GuiEvent::MediaNeighbor {
                    owner,
                    thread,
                    anchor,
                    result,
                } => {
                    if owner != self.user_uin
                        || !self
                            .image_viewer
                            .as_ref()
                            .is_some_and(|v| v.thread_id == thread && v.message_id == anchor)
                    {
                        continue;
                    }
                    if let Some(viewer) = self.image_viewer.as_mut() {
                        viewer.navigation_pending = false;
                    }
                    match result {
                        Ok(Some(message)) => self.open_media_viewer(&thread, &message),
                        Ok(None) => self
                            .show_toast("Không còn ảnh/video theo hướng này trong lịch sử cục bộ"),
                        Err(reason) => self.show_toast(reason),
                    }
                }
                GuiEvent::ClipboardImageRead {
                    owner,
                    thread,
                    image,
                } => {
                    if owner != self.user_uin {
                        if let Some((path, ..)) = image {
                            std::thread::spawn(move || {
                                let _ = std::fs::remove_file(path);
                            });
                        }
                        continue;
                    }
                    self.clipboard_read_pending = false;
                    if !self
                        .threads
                        .get(self.selected_thread_index)
                        .is_some_and(|t| t.id == thread)
                    {
                        if let Some((path, ..)) = image {
                            std::thread::spawn(move || {
                                let _ = std::fs::remove_file(path);
                            });
                        }
                        self.clipboard_text_fallback = None;
                        continue;
                    }
                    if let Some((path, w, h, image)) = image {
                        if let Some((old_path, ..)) = self.pending_clipboard_image.take() {
                            std::thread::spawn(move || {
                                let _ = std::fs::remove_file(old_path);
                            });
                        }
                        self.clipboard_alternative_text =
                            self.clipboard_text_fallback.take().filter(|text| {
                                self.settings.excel_paste_mode == "ask" && text.contains('\t')
                            });
                        let texture =
                            ctx.load_texture("clipboard-preview", image, TextureOptions::LINEAR);
                        self.pending_clipboard_image = Some((path, w, h, texture));
                        self.composer_focus_pending = true;
                        self.show_toast(format!(
                            "Đã dán ảnh ({w}×{h} px) từ Clipboard — nhấn Enter để gửi"
                        ));
                    } else if let Some(text) = self.clipboard_text_fallback.take() {
                        self.input_message.push_str(&text);
                        self.composer_focus_pending = true;
                        self.show_toast("Không đọc được ảnh; đã dán dạng văn bản");
                    } else {
                        self.show_toast(
                            "Không đọc được ảnh Clipboard hoặc ảnh vượt giới hạn cho phép",
                        );
                    }
                }
                GuiEvent::AttachmentPickerClosed { owner } => {
                    if owner == self.user_uin {
                        self.attachment_picker_pending = false;
                    }
                }
                GuiEvent::AttachmentDraftPicked {
                    owner,
                    runtime_id,
                    thread,
                    paths,
                } => {
                    if owner == self.user_uin && runtime_id == self.account_runtime_id {
                        self.attachment_picker_pending = false;
                        match paths {
                            Ok(paths) if self.is_logged_in => {
                                self.add_attachment_draft_paths(ctx, &thread, paths)
                            }
                            Ok(_) => {}
                            Err(error) => self.status_msg = error,
                        }
                    }
                }
                GuiEvent::LiveThreadsReady(_) => {
                    unreachable!("live mailbox normalized before dispatch")
                }
                GuiEvent::QrReady { qr_image, .. } => {
                    self.qr_texture =
                        Some(ctx.load_texture("qr_code_tex", qr_image, TextureOptions::NEAREST));
                    self.qr_status = "Mở ứng dụng Zalo trên điện thoại và quét mã QR".into();
                    self.login_in_progress = false;
                }
                GuiEvent::QrScanned { name } => {
                    self.qr_status =
                        format!("Đã quét bởi {name} — Vui lòng bấm 'Đăng nhập' trên điện thoại…");
                }
                GuiEvent::LoginSuccess {
                    name,
                    uin,
                    avatar,
                    runtime_id,
                } => {
                    self.profiles_requested = false;
                    self.profiles.clear();
                    if self.account_runtime_id != runtime_id {
                        self.doc_viewer = None;
                        self.tools.devices.clear();
                        self.tools.two_factor_status = None;
                        self.tools.catalog_items.clear();
                    }
                    if self.user_uin != uin || self.account_runtime_id != runtime_id {
                        self.cancel_voice_recording();
                        self.auth_request_generation = self.auth_request_generation.wrapping_add(1);
                        self.auth_in_progress = false;
                    }
                    if self.user_uin != uin {
                        self.clear_presence_state();
                        self.deleted_message_ids.clear();
                        self.reset_sync_status();
                        self.threads.clear();
                        self.selected_thread_index = 0;
                        self.unread_counts.clear();
                        self.conversation_preferences.clear();
                        self.show_hidden_conversations = false;
                        self.clear_history_confirmation = None;
                        self.sticker_textures.clear();
                        self.sticker_sprites.clear();
                        self.texture_budget = crate::media_resources::ResourceLru::new(
                            crate::media_resources::TEXTURE_BUDGET,
                        );
                        self.loading_stickers.clear();
                        self.image_retry_at.clear();
                        self.image_failures.clear();
                    }
                    self.is_logged_in = true;
                    self.account_runtime_id = runtime_id;
                    self.user_name = name;
                    self.user_uin = uin;
                    if avatar.is_some() {
                        self.user_avatar = avatar;
                    }
                    self.login_in_progress = false;
                    self.password_input.clear();
                    self.composer_sticker_cache.retain(|_, v| !v.is_empty());
                    self.status_msg = "Đã đăng nhập · Nhận cập nhật".into();
                }
                GuiEvent::LoginFailed(err) => {
                    self.login_in_progress = false;
                    self.login_status_msg = format!("Lỗi đăng nhập: {err}");
                }
                GuiEvent::ConversationPreferences { owner, values } => {
                    if self.is_logged_in && owner == self.user_uin {
                        self.conversation_preferences = values;
                    }
                }
                GuiEvent::ThreadDeleted { owner, thread } => {
                    if owner == self.user_uin {
                        self.invalidate_history_search();
                        self.discard_deleted_conversation_draft(&owner, &thread);
                        self.threads.retain(|t| t.id != thread);
                        self.selected_thread_index = self
                            .selected_thread_index
                            .min(self.threads.len().saturating_sub(1));
                        self.status_msg = "Đã xóa hội thoại sau khi máy chủ xác nhận".into();
                    }
                }
                GuiEvent::ThreadHistoryCleared { owner, thread_id } => {
                    if self.is_logged_in && owner == self.user_uin {
                        self.invalidate_history_search();
                        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                            thread.unread.read_foreground(&thread.messages);
                            thread.messages.clear();
                            thread.preview.clear();
                            thread.time.clear();
                            thread.presentation = Default::default();
                        }
                        self.unread_counts.remove(&thread_id);
                        self.apply_server_aliases();
                        self.refresh_unread_title(ctx);
                        self.persist_threads_async();
                        self.show_toast(
                            "Đã xóa lịch sử trên máy này; đồng bộ có thể tải lại tin từ máy chủ",
                        );
                    }
                }
                GuiEvent::ThreadsUpdated {
                    threads: mut updates,
                    source,
                } => {
                    for thread in &updates {
                        if !thread.is_group && thread.messages.iter().any(|m| m.is_e2ee) {
                            self.trial_peers.insert(thread.id.clone());
                        }
                        if thread
                            .messages
                            .iter()
                            .any(|message| message.e2ee_identity_changed)
                        {
                            self.trusted_peers.remove(&thread.id);
                            self.safety_number_modal = None;
                            self.status_msg = "Khóa danh tính đối phương đã thay đổi; cần xác minh lại mã an toàn".into();
                        }
                    }

                    if updates.iter().any(|t| {
                        t.messages.iter().any(|m| {
                            matches!(
                                m.kind,
                                crate::message_model::MessageKind::Recall
                                    | crate::message_model::MessageKind::Delete
                            )
                        })
                    }) {
                        self.invalidate_history_search();
                    }
                    if updates.iter().any(|thread| {
                        self.threads
                            .get(self.selected_thread_index)
                            .is_some_and(|selected| selected.id == thread.id)
                    }) {
                        self.chat_geometry_revision = self.chat_geometry_revision.wrapping_add(1);
                    }
                    for thread in &mut updates {
                        let before = thread.messages.len();
                        thread.messages.retain(|m| {
                            !self.deleted_message_ids.contains(&(
                                thread.id.clone(),
                                if m.id.is_empty() {
                                    format!("msg_{}", m.ts)
                                } else {
                                    m.id.clone()
                                },
                            ))
                        });
                        if before != thread.messages.len() {
                            thread.refresh_after_removal();
                        }
                    }
                    if self.is_logged_in && !updates.is_empty() {
                        let prev_selected_id = self
                            .threads
                            .get(self.selected_thread_index)
                            .map(|t| (t.id.clone(), t.is_group));
                        let live_ids: HashSet<String> = updates
                            .iter()
                            .filter(|u| !u.id.is_empty() && u.id != "0")
                            .map(|u| u.id.clone())
                            .collect();

                        let active_tid = self
                            .threads
                            .get(self.selected_thread_index)
                            .map(|t| t.id.clone())
                            .unwrap_or_default();
                        let is_focused = ctx.input(|i| i.focused);
                        let mut new_incoming_alert = false;

                        for mut nt in updates {
                            let mut alert_metadata = self
                                .threads
                                .iter()
                                .find(|thread| thread.id == nt.id)
                                .map(|thread| thread.presentation.inbox.clone())
                                .unwrap_or_default();
                            alert_metadata.merge(&nt.presentation.inbox, true);
                            let alert_allowed = self.private_access(&nt.id, nt.is_group)
                                && self.inbox_placement_metadata(
                                    &nt.id,
                                    nt.is_group,
                                    &alert_metadata,
                                ) == crate::inbox::Placement::Normal;
                            if let Some(existing) = self.threads.iter_mut().find(|t| t.id == nt.id)
                            {
                                let before = existing.unread.unread_ids.clone();
                                let old_last = existing.messages.last().map_or(0, |m| m.ts);
                                let known: HashSet<String> =
                                    existing.messages.iter().map(|m| m.id.clone()).collect();
                                let fallback = if source == ThreadUpdateSource::Live {
                                    nt.messages
                                        .iter()
                                        .filter(|m| {
                                            !m.is_outgoing
                                                && m.unread_policy.counts_unread()
                                                && m.kind
                                                    != crate::message_model::MessageKind::System
                                                && !m.id.is_empty()
                                                && !known.contains(&m.id)
                                                && m.ts >= old_last
                                        })
                                        .count()
                                } else {
                                    0
                                };
                                if nt.unread.last_seen_id.as_deref() == Some("0") {
                                    // A live fallback cursor is not evidence that cached messages are unread.
                                    nt.unread
                                        .unread_ids
                                        .retain(|id| !known.contains(id) || before.contains(id));
                                }
                                let metadata_fresh = nt.unread.fresh;
                                crate::chat::merge_thread_into(existing, nt, true);
                                if source == ThreadUpdateSource::Live
                                    && existing
                                        .unread
                                        .last_seen_id
                                        .as_deref()
                                        .is_some_and(|id| id != "0")
                                {
                                    existing.unread.observe(&existing.messages);
                                }
                                let inc_cnt = if existing.unread.last_seen_id.is_some() {
                                    existing.unread.unread_ids.difference(&before).count()
                                } else {
                                    fallback
                                };
                                if metadata_fresh || existing.unread.last_seen_id.is_some() {
                                    if existing.id == active_tid && is_focused {
                                        existing.unread.read_foreground(&existing.messages);
                                    }
                                    if existing.unread.count() == 0 {
                                        self.unread_counts.remove(&existing.id);
                                    } else {
                                        self.unread_counts
                                            .insert(existing.id.clone(), existing.unread.count());
                                    }
                                }
                                if inc_cnt > 0 && (existing.id != active_tid || !is_focused) {
                                    if existing.unread.last_seen_id.is_none() {
                                        let count = self
                                            .unread_counts
                                            .entry(existing.id.clone())
                                            .or_insert(0);
                                        *count = count.saturating_add(inc_cnt);
                                    }
                                    new_incoming_alert |= alert_allowed
                                        && !existing.unread.is_muted()
                                        && !self
                                            .conversation_preferences
                                            .get(&existing.id)
                                            .is_some_and(|p| p.muted);
                                }
                            } else if let Some(existing) = self.threads.iter_mut().find(|t| {
                                !live_ids.contains(&t.id)
                                    && crate::chat::threads_represent_same_contact(t, &nt)
                            }) {
                                crate::chat::merge_thread_into(existing, nt, true);
                            } else if !nt.id.is_empty() && nt.id != "0" {
                                let mut nt = nt;
                                if source == ThreadUpdateSource::Live {
                                    nt.unread.advance_seen("0");
                                    nt.unread.observe_for_account(&nt.messages, &self.user_uin);
                                    if nt.id == active_tid && is_focused {
                                        nt.unread.read_foreground(&nt.messages);
                                    }
                                    if nt.unread.count() > 0 {
                                        self.unread_counts.insert(nt.id.clone(), nt.unread.count());
                                        new_incoming_alert |= alert_allowed
                                            && !nt.unread.is_muted()
                                            && !self
                                                .conversation_preferences
                                                .get(&nt.id)
                                                .is_some_and(|p| p.muted);
                                    }
                                }
                                if !nt.messages.is_empty() || !nt.name.is_empty() {
                                    self.threads.push(nt);
                                }
                            }
                        }
                        self.refresh_unread_title(ctx);

                        if new_incoming_alert && self.settings.enable_notifications {
                            let total_unread: usize = self.unread_counts.values().sum();
                            let title = if total_unread > 0 {
                                format!("({total_unread}) Zalo")
                            } else {
                                "Zalo".to_string()
                            };
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
                            // Message badges replace flashing the taskbar for chat alerts.
                        }
                        crate::chat::deduplicate_threads(&mut self.threads, Some(&live_ids));
                        self.apply_server_aliases();

                        if let Some((sel_id, sel_grp)) = prev_selected_id {
                            if let Some(idx) = self
                                .threads
                                .iter()
                                .position(|t| t.is_group == sel_grp && t.id == sel_id)
                            {
                                self.selected_thread_index = idx;
                            }
                        }
                        if self.selected_thread_index >= self.threads.len() {
                            self.selected_thread_index = self.threads.len().saturating_sub(1);
                        }
                        // Every worker delivery is already committed before this event.
                        // Rewriting/cloning all histories per downloaded page amplified RAM/IO.
                        self.trim_resident_history();
                    }
                    self.refresh_unread_title(ctx);
                    self.status_msg = history_status
                        .unwrap_or_else(|| format!("Đã cập nhật {}", crate::ui::clock()));
                }
                GuiEvent::AccountMetadataInvalidated {
                    owner,
                    privacy,
                    relations,
                    business,
                } => {
                    if owner == self.user_uin && self.is_logged_in {
                        if privacy {
                            self.inbox_policy.direct = None;
                            self.inbox_policy.show_online = None;
                            if !self.privacy_panel.pending {
                                self.privacy_panel.snapshot = None;
                            }
                        }
                        if relations {
                            self.inbox_policy.friends = None;
                            self.friend_status_map.clear();
                        }
                        if business {
                            self.inbox_policy.business = None;
                        }
                    }
                }
                GuiEvent::ActivityUpdated { owner, uid, status } => {
                    if owner == self.user_uin
                        && self.is_logged_in
                        && self.inbox_policy.show_online == Some(true)
                        && self.contact_panel.target == uid
                    {
                        self.contact_panel.presence = Some(status);
                        self.contact_panel.presence_checked = Some(std::time::Instant::now());
                    }
                }
                GuiEvent::AccountMetadata { owner, snapshot } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    let mut dirty = false;
                    self.own_cloud_id = snapshot.cloud_id;
                    if let Some(privacy) = snapshot.privacy {
                        self.inbox_policy.show_online = privacy
                            .values
                            .get(&crate::privacy::Setting::ShowOnline)
                            .map(|value| *value != 0);
                        self.inbox_policy.direct = privacy
                            .values
                            .get(&crate::privacy::Setting::StrangerDirect)
                            .map(|value| *value != 0);
                        if !self.privacy_panel.pending {
                            self.privacy_panel.uncertain = snapshot.from_cache;
                            self.privacy_panel.snapshot = Some(privacy);
                        }
                    }
                    if let Some(friends) = snapshot.friends {
                        self.inbox_policy.friends = Some(friends);
                    }
                    if let Some(profile) = snapshot.profile {
                        if !self.profile_panel.pending && !self.avatar_panel.pending {
                            self.user_name = profile.name;
                            if let Some(avatar) = profile.avatar {
                                self.user_avatar = Some(avatar);
                            }
                        }
                    }
                    if let Some(business) = snapshot.business {
                        self.inbox_policy.business = Some(business);
                    }
                    if let Some(labels) = snapshot.labels {
                        if !self.labels_panel.pending {
                            self.labels_panel.snapshot = Some(labels);
                        }
                    }
                    if let Some(archive) = snapshot.archive {
                        if !self.archive_pending {
                            let archived = archive
                                .items
                                .into_iter()
                                .filter_map(|item| {
                                    crate::server_pins::identity(&item.id, item.kind == 1).ok()
                                })
                                .collect::<HashSet<_>>();
                            for thread in &mut self.threads {
                                let value = Some(
                                    crate::server_pins::identity(&thread.id, thread.is_group)
                                        .ok()
                                        .is_some_and(|id| archived.contains(&id)),
                                );
                                dirty |= thread.presentation.archived != value;
                                thread.presentation.archived = value;
                            }
                        }
                    }
                    if let Some(items) = snapshot.inbox {
                        for (id, group, metadata) in items {
                            if let Some(thread) = self
                                .threads
                                .iter_mut()
                                .find(|thread| thread.id == id && thread.is_group == group)
                            {
                                let old = thread.presentation.inbox.clone();
                                thread.presentation.inbox.merge(&metadata, true);
                                dirty |= old != thread.presentation.inbox;
                            }
                        }
                    }
                    if let Some(aliases) = snapshot.aliases {
                        if !self.server_aliases_ready || self.server_aliases != aliases {
                            dirty = true;
                            self.server_aliases_ready = true;
                            self.server_aliases = aliases;
                            self.apply_server_aliases();
                        }
                    }
                    self.metadata_status = if snapshot.warnings.is_empty() {
                        String::new()
                    } else {
                        format!("Chưa cập nhật được: {}", snapshot.warnings.join(", "))
                    };
                    if dirty {
                        self.persist_threads_async();
                    }
                    self.refresh_unread_title(ctx);
                }
                GuiEvent::GroupInvalidated {
                    owner,
                    revision,
                    groups,
                    all,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    let affects = |id: &str| {
                        all || crate::server_pins::identity(id, true)
                            .ok()
                            .is_some_and(|id| groups.contains(&id))
                    };
                    self.group_policy_revision = self.group_policy_revision.max(revision);
                    for thread in &mut self.threads {
                        if thread.is_group && affects(&thread.id) {
                            thread.presentation.send_policy = None;
                            thread.presentation.group_kind = None;
                            thread.presentation.metadata_verified = false;
                            self.group_members.remove(&thread.id);
                        }
                    }
                    if affects(&self.groups_panel.target) {
                        self.groups_panel.info = None;
                        self.groups_panel.needs_refresh = true;
                    }
                    if let Some(thread) = self
                        .threads
                        .get(self.selected_thread_index)
                        .filter(|thread| thread.is_group && affects(&thread.id))
                    {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::FetchGroupPolicy {
                                group_id: thread.id.clone(),
                            });
                        }
                    }
                }
                GuiEvent::GroupKindLoaded {
                    revision,
                    owner,
                    group_id,
                    kind,
                } => {
                    if self.is_logged_in
                        && owner == self.user_uin
                        && revision >= self.group_policy_revision
                    {
                        if let Some(thread) = self
                            .threads
                            .iter_mut()
                            .find(|thread| thread.is_group && thread.id == group_id)
                        {
                            thread.presentation.group_kind = kind;
                        }
                    }
                }
                GuiEvent::GroupPolicy {
                    revision,
                    owner,
                    group_id,
                    result,
                } => {
                    if owner == self.user_uin && revision >= self.group_policy_revision {
                        match result {
                            Ok(policy) => {
                                if let Some(thread) = self
                                    .threads
                                    .iter_mut()
                                    .find(|t| t.is_group && t.id == group_id)
                                {
                                    thread.presentation.send_policy = Some(policy);
                                }
                            }
                            Err(error) => self.status_msg = error,
                        }
                    }
                }
                GuiEvent::GroupMembersLoaded { owner, .. } if owner != self.user_uin => {}
                GuiEvent::GroupMembersLoaded {
                    group_id, result, ..
                } => match result {
                    Ok(roster) => {
                        self.group_members_state.remove(&group_id);
                        self.group_members.insert(group_id, roster);
                    }
                    Err(error) => {
                        self.group_members_state.insert(group_id, Some(error));
                    }
                },
                GuiEvent::Typing { owner, .. } if owner != self.user_uin => {}
                GuiEvent::Typing { events, .. } => {
                    let now = std::time::Instant::now();
                    for event in events {
                        self.typing_map
                            .insert((event.thread_id, event.is_group), (event.uid, now));
                    }
                    ctx.request_repaint_after(crate::presence::TYPING_VISIBLE);
                }
                GuiEvent::Receipts { owner, .. } if owner != self.user_uin => {}
                GuiEvent::Receipts { changed, .. } => {
                    for (thread, msg_id, state) in changed {
                        for t in self.threads.iter_mut().filter(|t| t.id == thread) {
                            for m in t.messages.iter_mut().filter(|m| m.id == msg_id) {
                                m.delivery = m.delivery.merge(state);
                            }
                        }
                    }
                }
                GuiEvent::SessionExpired(reason) => {
                    self.cancel_voice_recording();
                    self.auth_request_generation = self.auth_request_generation.wrapping_add(1);
                    self.auth_in_progress = false;
                    self.lock_guard.unlock();
                    self.lock_pin_input.clear();
                    self.lock_pin_error = None;
                    self.clear_presence_state();
                    self.flush_composer_before_logout();
                    self.live_mailbox = None;
                    crate::desktop_notice::account("");
                    self.clear_downloads();
                    self.is_logged_in = false;
                    self.account_runtime_id = 0;
                    self.tools.devices.clear();
                    self.tools.two_factor_status = None;
                    self.tools.catalog_items.clear();
                    self.threads.clear();
                    self.user_uin.clear();
                    self.user_name.clear();
                    self.active_call = None;
                    self.qr_texture = None;
                    self.status_msg = reason;
                    // Expiry is an account boundary too. Restart the runtime/channel before QR.
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::Logout);
                    }
                }
                GuiEvent::CameraUnavailable(reason) => {
                    if let Some(call) = self.active_call.as_mut() {
                        call.cam_enabled = false;
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::SetCallMediaState {
                                is_on_mic: !call.mic_muted,
                                is_on_cam: false,
                            });
                        }
                    }
                    self.status_msg = reason.clone();
                    self.show_toast(reason);
                }
                GuiEvent::DownloadFolderFailed(message) => self.show_toast(message),
                GuiEvent::DownloadFolderPicked(result) => {
                    self.download_picker_pending = false;
                    match result {
                        Some(Ok(path)) => {
                            self.settings.download_dir = path.display().to_string();
                            self.save_settings();
                            self.show_toast("Đã đổi thư mục tải về");
                        }
                        Some(Err(error)) => self.show_toast(error),
                        None => {}
                    }
                }
                GuiEvent::UploadProgress {
                    owner,
                    runtime_id,
                    upload_id,
                    completed,
                    total,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || runtime_id != self.account_runtime_id
                    {
                        continue;
                    }
                    let client_id = upload_id
                        .strip_prefix("asyncfile-")
                        .or_else(|| upload_id.strip_prefix("photo-"));
                    if let Some(client_id) = client_id {
                        for thread in &mut self.threads {
                            for message in &mut thread.messages {
                                if message.is_outgoing
                                    && message.client_message_id == client_id
                                    && message.delivery
                                        == crate::message_model::DeliveryState::Pending
                                    && message.outgoing_job.as_ref().is_some_and(|job| {
                                        matches!(
                                            job.payload,
                                            crate::outgoing::Payload::Attachment { .. }
                                        )
                                    })
                                {
                                    message.upload_progress =
                                        Some((upload_id.clone(), completed.min(total), total));
                                }
                            }
                        }
                    }
                    self.status_msg = format!("Tải lên: {}/{} bytes", completed, total);
                }
                GuiEvent::UploadResumeAvailable { sessions } => {
                    if !sessions.is_empty() {
                        let summary = if sessions.len() == 1 {
                            format!("Có 1 tải lên chưa hoàn thành ({})…", sessions[0].file_name)
                        } else {
                            format!("Có {} tải lên chưa hoàn thành…", sessions.len())
                        };
                        self.show_toast(summary);
                    }
                }
                GuiEvent::LogoutFailed { message } => {
                    self.show_toast(format!("Đăng xuất thất bại: {message}"));
                    self.status_msg = format!("Lỗi đăng xuất: {message}");
                }
                GuiEvent::LoggedOut => {
                    self.cancel_voice_recording();
                    self.auth_request_generation = self.auth_request_generation.wrapping_add(1);
                    self.auth_in_progress = false;
                    self.lock_guard.unlock();
                    self.lock_pin_input.clear();
                    self.lock_pin_error = None;
                    self.clear_presence_state();
                    self.deleted_message_ids.clear();
                    self.quick_schedule = None;
                    self.reset_sync_status();
                    self.live_mailbox = None;
                    crate::desktop_notice::account("");
                    self.clear_downloads();
                    self.is_logged_in = false;
                    self.account_runtime_id = 0;
                    self.user_name.clear();
                    self.user_uin.clear();
                    self.threads.clear();
                    self.selected_thread_index = 0;
                    self.qr_texture = None;
                    self.qr_status = "Đang xin mã QR mới…".into();
                    self.status_msg = "Đã đăng xuất".into();
                    self.active_call = None;
                    self.image_viewer = None;
                    self.doc_viewer = None;
                    self.user_avatar = None;
                    self.input_message.clear();
                    self.password_input.clear();
                    self.phone_input.clear();
                    self.replying_to = None;
                    self.clear_forward_dialog();
                    self.clipboard_read_pending = false;
                    self.attachment_picker_pending = false;
                    if let Some((old_path, ..)) = self.pending_clipboard_image.take() {
                        std::thread::spawn(move || {
                            let _ = std::fs::remove_file(old_path);
                        });
                    }
                    self.search_query.clear();
                    self.phone_lookup_result = None;
                    self.phone_modal_input.clear();
                    self.friend_status_map.clear();
                    self.loading_link_previews.clear();
                    self.dismissed_link_preview_for.clear();
                    self.dismissed_sticker_suggest_for.clear();
                    self.last_composer_text.clear();
                    self.unread_counts.clear();
                    #[cfg(windows)]
                    crate::tray::set_unread(false);
                    self.conversation_preferences.clear();
                    self.show_hidden_conversations = false;
                    self.clear_history_confirmation = None;
                    self.sticker_textures.clear();
                    self.sticker_sprites.clear();
                    self.texture_budget = crate::media_resources::ResourceLru::new(
                        crate::media_resources::TEXTURE_BUDGET,
                    );
                    self.loading_stickers.clear();
                    self.image_retry_at.clear();
                    self.image_failures.clear();
                    self.composer_sticker_cache.clear();
                    self.link_metadata_cache.clear();
                    self.translations.clear();
                    self.translating_messages.clear();
                    self.voice_transcripts.clear();
                    self.transcribing_voices.clear();
                    self.measured_msg_heights.clear();
                    let (tx, rx) = channel();
                    self.event_rx = rx;
                    self.event_tx = tx.clone();
                    match Session::new(self.dev_mode, false, false) {
                        Ok(session) => {
                            spawn_session_worker(session, tx, ctx.clone());
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::RequestQr);
                            }
                        }
                        Err(e) => self.show_toast(format!("Không thể tạo phiên mới: {e}")),
                    }
                }
                GuiEvent::ProfileSwitched { session } => {
                    let uid = session.account_key().to_owned();
                    let runtime_id = session.runtime_id();
                    self.tools.devices.clear();
                    self.tools.two_factor_status = None;
                    self.tools.catalog_items.clear();
                    self.auth_request_generation = self.auth_request_generation.wrapping_add(1);
                    self.auth_in_progress = false;
                    // A20: Profile switched; thoroughly isolate and reset UI state for new profile
                    self.cancel_voice_recording();
                    self.clear_presence_state();
                    self.deleted_message_ids.clear();
                    self.quick_schedule = None;
                    self.reset_sync_status();
                    self.live_mailbox = None;
                    crate::desktop_notice::account(&uid);
                    self.clear_downloads();
                    self.user_uin = uid.clone();
                    self.account_runtime_id = runtime_id;
                    self.user_name.clear();
                    self.threads.clear();
                    self.selected_thread_index = 0;
                    self.active_call = None;
                    self.image_viewer = None;
                    self.doc_viewer = None;
                    self.user_avatar = None;
                    self.input_message.clear();
                    self.replying_to = None;
                    self.clear_forward_dialog();
                    self.clipboard_read_pending = false;
                    self.attachment_picker_pending = false;
                    if let Some((old_path, ..)) = self.pending_clipboard_image.take() {
                        std::thread::spawn(move || {
                            let _ = std::fs::remove_file(old_path);
                        });
                    }
                    self.search_query.clear();
                    self.phone_lookup_result = None;
                    self.phone_modal_input.clear();
                    self.friend_status_map.clear();
                    self.loading_link_previews.clear();
                    self.dismissed_link_preview_for.clear();
                    self.dismissed_sticker_suggest_for.clear();
                    self.last_composer_text.clear();
                    self.unread_counts.clear();
                    #[cfg(windows)]
                    crate::tray::set_unread(false);
                    self.conversation_preferences.clear();
                    self.show_hidden_conversations = false;
                    self.clear_history_confirmation = None;
                    self.sticker_textures.clear();
                    self.sticker_sprites.clear();
                    self.texture_budget = crate::media_resources::ResourceLru::new(
                        crate::media_resources::TEXTURE_BUDGET,
                    );
                    self.loading_stickers.clear();
                    self.image_retry_at.clear();
                    self.image_failures.clear();
                    self.composer_sticker_cache.clear();
                    self.link_metadata_cache.clear();
                    self.translations.clear();
                    self.translating_messages.clear();
                    self.voice_transcripts.clear();
                    self.transcribing_voices.clear();
                    self.measured_msg_heights.clear();
                    self.status_msg = "Đã chuyển tài khoản thành công".into();
                    self.profile_menu_open = false;
                    let (tx, rx) = channel();
                    self.event_rx = rx;
                    self.event_tx = tx.clone();
                    spawn_session_worker(*session, tx, ctx.clone());
                    // The new account receives a fresh worker, mailbox and event stream.
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::RefreshThreads);
                    }
                }
                GuiEvent::AppUnlockCompleted {
                    request_id,
                    runtime_id,
                    verified,
                } => {
                    if request_id != self.auth_request_generation
                        || runtime_id != self.account_runtime_id
                        || !self.lock_guard.is_locked()
                    {
                        continue;
                    }
                    self.auth_in_progress = false;
                    if verified {
                        self.lock_guard.unlock();
                        crate::desktop_notice::privacy_lock(&self.user_uin, false);
                        self.status_msg = "Đã mở khóa ứng dụng".into();
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::RefreshThreads);
                        }
                    } else {
                        self.status_msg = "Xác thực thất bại. Vui lòng thử lại.".into();
                    }
                }
                GuiEvent::AppUnlockFailed {
                    request_id,
                    runtime_id,
                    message,
                } => {
                    if request_id != self.auth_request_generation
                        || runtime_id != self.account_runtime_id
                        || !self.lock_guard.is_locked()
                    {
                        continue;
                    }
                    self.auth_in_progress = false;
                    // Timeout / busy / invalid-window need different user actions.
                    self.status_msg = format!("Xác thực thất bại: {message}");
                }
                GuiEvent::Tools { owner, result } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    let result = match result {
                        Ok(tools_panel::ToolsResult::SearchOpened {
                            generation,
                            thread,
                            id,
                        }) => {
                            if !self.history_search.open
                                || self.history_search.opening.as_ref()
                                    != Some(&(generation, thread.id.clone(), id.clone()))
                                || generation != self.history_search.request.generation
                            {
                                continue;
                            }
                            self.history_search.open = false;
                            Ok(tools_panel::ToolsResult::Hit { thread, id })
                        }
                        other => other,
                    };
                    match result {
                        Ok(tools_panel::ToolsResult::SearchOpened { .. }) => unreachable!(),
                        Ok(tools_panel::ToolsResult::SearchOpenFailed {
                            generation,
                            thread,
                            id,
                            error,
                        }) => {
                            if self.history_search.opening.as_ref()
                                == Some(&(generation, thread, id))
                                && self.history_search.request.generation == generation
                            {
                                self.history_search.opening = None;
                                self.history_search.status = error;
                            }
                        }
                        Ok(tools_panel::ToolsResult::SearchPage {
                            request,
                            result,
                            senders,
                        }) => {
                            if self.history_search.inflight == Some(request.generation) {
                                self.history_search.pending = false;
                                self.history_search.inflight = None;
                            }
                            if self.history_search.open && request == self.history_search.request {
                                self.history_search.senders = senders;
                                match result {
                                    Ok(page) => self.history_search.page = page,
                                    Err(error) => self.history_search.status = error,
                                }
                            }
                        }
                        Ok(tools_panel::ToolsResult::Tasks { tasks, paused }) => {
                            self.tools.tasks = tasks;
                            self.tools.paused = paused;
                        }
                        Ok(tools_panel::ToolsResult::Loaded {
                            config,
                            tasks,
                            paused,
                        }) => {
                            for old in &self.tools.config.stickers {
                                self.sticker_pack_items.remove(&old.cate_id);
                                self.pinned_sticker_packs
                                    .retain(|(id, _)| *id != old.cate_id);
                            }
                            self.tools.config = config;
                            self.tools.tasks = tasks;
                            self.tools.paused = paused;
                            self.theme_dirty = true;
                            for pack in &self.tools.config.stickers {
                                self.sticker_pack_items.insert(
                                    pack.cate_id,
                                    pack.sticker_ids
                                        .iter()
                                        .map(|id| crate::chat::StickerItem {
                                            sticker_id: *id,
                                            cate_id: pack.cate_id,
                                            sticker_type: pack.sticker_type,
                                            label: pack.name.clone(),
                                        })
                                        .collect(),
                                );
                                if !self
                                    .pinned_sticker_packs
                                    .iter()
                                    .any(|(id, _)| *id == pack.cate_id)
                                {
                                    self.pinned_sticker_packs
                                        .push((pack.cate_id, pack.name.clone()));
                                }
                            }
                        }
                        Ok(tools_panel::ToolsResult::Search { query, hits }) => {
                            if query == self.tools.last_query {
                                self.tools.hits = hits
                                    .into_iter()
                                    .filter(|hit| {
                                        self.private_access(
                                            &hit.thread_id,
                                            hit.thread_id.starts_with('g'),
                                        )
                                    })
                                    .collect();
                            }
                        }
                        Ok(tools_panel::ToolsResult::Hit { thread, id }) => {
                            if !self.private_access(&thread.id, thread.is_group) {
                                continue;
                            }
                            self.replying_to = None;
                            self.filter_starred_only = false;
                            let index = match self.threads.iter().position(|t| t.id == thread.id) {
                                Some(idx) => idx,
                                None => {
                                    self.threads.push(Thread {
                                        id: thread.id.clone(),
                                        name: thread.name.clone(),
                                        is_group: thread.is_group,
                                        ..Default::default()
                                    });
                                    self.apply_server_aliases();
                                    self.threads
                                        .iter()
                                        .position(|t| t.id == thread.id)
                                        .unwrap_or_else(|| self.threads.len() - 1)
                                }
                            };
                            self.selected_thread_index = index;
                            self.sync_composer_draft(ctx);
                            let opened_id = thread.id.clone();
                            let opened_group = thread.is_group;
                            self.threads[index].messages = thread.messages;
                            self.chat_render_window = (opened_id.clone(), 150);
                            self.policy_command(crate::conversation_policy::Action::Read);
                            if opened_group {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::FetchGroupPolicy {
                                        group_id: opened_id.clone(),
                                    });
                                }
                            }
                            self.quote_jump = (!id.is_empty()).then(|| QuoteJump {
                                thread_id: opened_id,
                                reference: id,
                                highlight_until: std::time::Instant::now() + Duration::from_secs(3),
                                scroll_pending: true,
                            });
                            self.tools.open = false;
                        }
                        Ok(tools_panel::ToolsResult::ScheduleSaved {
                            request,
                            thread,
                            text,
                            at,
                            result,
                        }) => {
                            let matching = self.quick_schedule.as_ref().is_some_and(|draft| {
                                draft.matches_saved(request, &thread, &text, at)
                            });
                            match result {
                                Ok(()) => {
                                    if matching {
                                        if self
                                            .threads
                                            .get(self.selected_thread_index)
                                            .is_some_and(|t| t.id == thread)
                                            && self.input_message == text
                                        {
                                            self.input_message.clear();
                                        }
                                        self.quick_schedule = None;
                                    }
                                    self.tools.status = "Đã lưu lịch gửi".into();
                                    self.show_toast("Đã lưu lịch gửi");
                                    self.tools_command(tools_panel::ToolsCommand::Load);
                                }
                                Err(error) => {
                                    if matching {
                                        if let Some(draft) = &mut self.quick_schedule {
                                            draft.failed(error.clone());
                                        }
                                    }
                                    self.tools.status = error.clone();
                                    self.show_toast(format!("Không lưu được lịch gửi: {error}"));
                                }
                            }
                        }
                        Ok(tools_panel::ToolsResult::Status(message)) => {
                            self.tools.status = message;
                        }
                        Err(error) => {
                            if self.history_search.open {
                                self.history_search.status = error.clone();
                                self.history_search.opening = None;
                            }
                            self.tools.status = error;
                        }
                    }
                }
                GuiEvent::OutgoingState {
                    owner,
                    runtime_id,
                    thread_id,
                    message,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || runtime_id != self.account_runtime_id
                    {
                        continue;
                    }
                    apply_outgoing_state(self, thread_id, message);
                }
                GuiEvent::VoiceOutgoingState {
                    owner,
                    runtime_id,
                    thread_id,
                    message,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || runtime_id != self.account_runtime_id
                    {
                        continue;
                    }
                    apply_outgoing_state(self, thread_id, message);
                }
                GuiEvent::VoiceSendRejected {
                    owner,
                    runtime_id,
                    message,
                }
                | GuiEvent::VoiceSendStatus {
                    owner,
                    runtime_id,
                    message,
                } => {
                    if self.is_logged_in
                        && owner == self.user_uin
                        && runtime_id == self.account_runtime_id
                    {
                        self.show_toast(message);
                    }
                }
                GuiEvent::MessageSent {
                    client_message_id,
                    thread_id,
                    text,
                    msg_id,
                } => {
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        if server_ack && !t.is_group {
                            t.presentation.inbox.responded_by_me = Some(true);
                        }
                        t.preview = text.clone();
                        let now = crate::ui::clock();
                        t.time = now.clone();
                        let final_id = if msg_id.is_empty() {
                            format!("out_{client_message_id}")
                        } else {
                            msg_id
                        };
                        merge_sent_message(
                            t,
                            ChatMessage {
                                id: final_id,
                                delivery: if server_ack {
                                    crate::message_model::DeliveryState::ServerAccepted
                                } else {
                                    crate::message_model::DeliveryState::Unknown
                                },
                                client_message_id,
                                sender: "Tôi".into(),
                                text,
                                time: now,
                                is_outgoing: true,
                                ts: (crate::login::unix_now() * 1000),
                                ..Default::default()
                            },
                        );
                        self.persist_threads_async();
                    }
                    self.status_msg = if server_ack {
                        format!("Đã gửi tin nhắn lúc {}", crate::ui::clock())
                    } else {
                        "Đã cập nhật nhật ký cuộc gọi".into()
                    };
                }
                GuiEvent::StickerSent {
                    client_message_id,
                    thread_id,
                    msg_id,
                    sticker_id,
                    cate_id,
                    sticker_type,
                } => {
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        if !t.is_group {
                            t.presentation.inbox.responded_by_me = Some(true);
                        }
                        t.preview = "[Nhãn dán]".into();
                        let now = crate::ui::clock();
                        t.time = now.clone();
                        let final_id = if msg_id.is_empty() {
                            format!("out_{client_message_id}")
                        } else {
                            msg_id
                        };
                        merge_sent_message(
                            t,
                            ChatMessage {
                                id: final_id,
                                client_message_id,
                                sender: "Tôi".into(),
                                text: "[Nhãn dán]".into(),
                                time: now,
                                is_outgoing: true,
                                ts: (crate::login::unix_now() * 1000),
                                sticker_id: Some(sticker_id),
                                sticker_cate_id: Some(cate_id),
                                sticker_type: Some(sticker_type),
                                ..Default::default()
                            },
                        );
                        self.persist_threads_async();
                    }
                    self.status_msg = format!("Đã gửi nhãn dán lúc {}", crate::ui::clock());
                }
                GuiEvent::AttachmentSent { thread_id, msg } => {
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        if !t.is_group {
                            t.presentation.inbox.responded_by_me = Some(true);
                        }
                        t.preview = msg.text.clone();
                        t.time = msg.time.clone();
                        merge_sent_message(t, msg);
                        self.persist_threads_async();
                    }
                    self.status_msg = format!("Đã gửi đính kèm lúc {}", crate::ui::clock());
                }
                GuiEvent::QuoteSent {
                    client_message_id,
                    thread_id,
                    text,
                    msg_id,
                    quoted,
                } => {
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        if !t.is_group {
                            t.presentation.inbox.responded_by_me = Some(true);
                        }
                        t.preview = text.clone();
                        let now = crate::ui::clock();
                        t.time = now.clone();
                        let final_id = if msg_id.is_empty() {
                            format!("out_{client_message_id}")
                        } else {
                            msg_id
                        };
                        merge_sent_message(
                            t,
                            ChatMessage {
                                id: final_id,
                                client_message_id,
                                sender: "Tôi".into(),
                                text,
                                time: now,
                                is_outgoing: true,
                                ts: (crate::login::unix_now() * 1000),
                                reply_to_id: Some(quoted.id.clone()),
                                reply_to_sender: Some(quoted.sender.clone()),
                                reply_to_text: Some(quoted.text.clone()),
                                ..Default::default()
                            },
                        );
                        self.persist_threads_async();
                    }
                    self.status_msg = format!("Đã gửi trả lời lúc {}", crate::ui::clock());
                }
                GuiEvent::ReactorProfiles { owner, ids, result } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    for id in &ids {
                        self.reactor_profile_pending.remove(id);
                    }
                    let profiles = result.unwrap_or_default();
                    for id in ids {
                        let profile = profiles.get(&id);
                        let name = profile
                            .and_then(|value| {
                                ["displayName", "zaloName", "dName", "name"]
                                    .iter()
                                    .find_map(|field| {
                                        value.get(field).and_then(serde_json::Value::as_str)
                                    })
                            })
                            .unwrap_or("")
                            .to_owned();
                        let avatar = profile
                            .and_then(|value| value.get("avatar").or_else(|| value.get("avt")))
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned);
                        self.reactor_profiles.insert(
                            id.clone(),
                            crate::chat::SidebarMember {
                                id,
                                name,
                                avatar,
                                ..Default::default()
                            },
                        );
                    }
                }
                GuiEvent::GroupContent {
                    owner,
                    thread,
                    generation,
                    mutation,
                    result,
                } => {
                    if owner != self.user_uin
                        || !self.is_logged_in
                        || thread != self.group_content.thread
                        || generation != self.group_content.generation
                    {
                        continue;
                    }
                    self.group_content.pending = false;
                    self.group_content.needs_refresh = mutation;
                    match result {
                        Ok(reply) => {
                            self.group_content.next = reply.next;
                            if let Some(notes) = reply.notes {
                                if !reply.append {
                                    self.group_content.notes.clear();
                                }
                                self.group_content.notes.extend(notes);
                                self.group_content.needs_refresh = false;
                            }
                            if let Some(polls) = reply.polls {
                                if !reply.append {
                                    self.group_content.polls.clear();
                                }
                                self.group_content.polls.extend(polls);
                                self.group_content.needs_refresh = false;
                            }
                            if let Some(detail) = reply.detail {
                                self.group_content.selected = detail
                                    .options
                                    .iter()
                                    .filter(|option| option.selected)
                                    .map(|option| option.id)
                                    .collect();
                                self.group_content.detail = Some(detail);
                                self.group_content.needs_refresh = false;
                            }
                            self.group_content.status = if reply.changed {
                                "Server đã xác nhận thao tác; tải lại danh sách để xem trạng thái mới".into()
                            } else {
                                "Đã đọc dữ liệu server".into()
                            };
                        }
                        Err(error) => {
                            self.group_content.status = format!(
                                "Không hoàn tất: {error}. Tải lại để kiểm tra trước khi thử tiếp."
                            )
                        }
                    }
                }
                GuiEvent::ConversationPolicy {
                    loading,
                    owner,
                    generation,
                    result,
                } => {
                    if owner != self.user_uin
                        || !self.is_logged_in
                        || generation != self.policy_panel.generation
                    {
                        continue;
                    }
                    self.policy_panel.pending = loading;
                    match result {
                        Ok(snapshot) => {
                            self.policy_panel.status = if snapshot.from_cache {
                                "Đang dùng thiết lập đã lưu; chưa đọc lại server".into()
                            } else {
                                "Đã đọc thiết lập server".into()
                            };
                            self.policy_panel.snapshot = Some(snapshot);
                        }
                        Err(error) => {
                            self.policy_panel.status = format!("Chưa cập nhật thiết lập: {error}")
                        }
                    }
                }
                GuiEvent::Expired { owner, ids } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    for (thread, id) in &ids {
                        if self.image_viewer.as_ref().is_some_and(|viewer| {
                            viewer.thread_id == *thread && viewer.message_id == *id
                        }) {
                            self.image_viewer = None;
                        }
                        if self.doc_viewer.as_ref().is_some_and(|viewer| {
                            viewer.context.thread == *thread && viewer.context.message.id == *id
                        }) {
                            self.doc_viewer = None;
                        }
                        for attachment in 0..32 {
                            for preview in [false, true] {
                                let key = crate::media_recovery::image_key(
                                    thread, id, attachment, preview,
                                );
                                self.sticker_textures.remove(&key);
                                self.texture_budget.remove(&key);
                            }
                        }

                        if let Some(thread) = self
                            .threads
                            .iter_mut()
                            .find(|candidate| candidate.id == *thread)
                        {
                            if thread
                                .presentation
                                .reaction_preview
                                .as_ref()
                                .is_some_and(|preview| preview.target == *id)
                            {
                                thread.presentation.reaction_preview = None;
                                thread.presentation.reaction_preview_fresh = true;
                            }
                            thread.messages.retain(|message| message.id != *id);
                            thread.unread.unread_ids.remove(id);
                            thread.unread.mention_ids.remove(id);
                            thread.refresh_after_removal();
                        }
                    }
                    if !ids.is_empty() {
                        self.invalidate_history_search();
                        self.chat_geometry_revision = self.chat_geometry_revision.wrapping_add(1);
                        self.refresh_unread_title(ctx);
                    }
                }
                GuiEvent::DesktopBackupPath {
                    owner,
                    generation,
                    path,
                } => {
                    if owner == self.user_uin
                        && self.is_logged_in
                        && generation == self.backup_panel.generation
                    {
                        self.backup_panel.pending = false;
                        if let Some(path) = path {
                            self.backup_panel.path = path.to_string_lossy().into();
                            self.backup_panel.report = None;
                        }
                    }
                }
                GuiEvent::DesktopBackup {
                    owner,
                    generation,
                    imported,
                    result,
                } => {
                    if owner != self.user_uin
                        || !self.is_logged_in
                        || generation != self.backup_panel.generation
                    {
                        continue;
                    }
                    self.backup_panel.pending = false;
                    match result {
                        Ok(report) => {
                            self.backup_panel.status = if imported {
                                format!(
                                    "Đã nhập {} tin mới, bỏ qua {} tin đã có/đã xóa",
                                    report.imported, report.skipped
                                )
                            } else {
                                "Đã kiểm tra backup; chưa nhập dữ liệu".into()
                            };
                            self.backup_panel.report = Some(report);
                            if imported {
                                self.invalidate_history_search();
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::RefreshThreads);
                                }
                            }
                        }
                        Err(error) => {
                            self.backup_panel.status=format!("Không hoàn tất: {error}. Các lô đã nhập được giữ lại; có thể kiểm tra rồi chạy lại.");
                            self.backup_panel.report = None;
                        }
                    }
                }
                GuiEvent::Group {
                    owner,
                    target,
                    generation,
                    mutation,
                    result,
                } => {
                    if owner == self.user_uin
                        && self.is_logged_in
                        && self.common_groups_generations.get(&target) == Some(&generation)
                    {
                        self.common_groups_generations.remove(&target);
                        match result {
                            Ok(reply) => {
                                if let Some(groups) = reply.related {
                                    self.common_groups_cache.insert(target.clone(), groups);
                                    self.common_groups_state.remove(&target);
                                } else {
                                    self.common_groups_state.insert(
                                        target,
                                        Some("Máy chủ không trả danh sách nhóm chung".into()),
                                    );
                                }
                            }
                            Err(error) => {
                                self.common_groups_state.insert(target, Some(error));
                            }
                        }
                        continue;
                    }
                    if owner != self.user_uin
                        || !self.is_logged_in
                        || target != self.groups_panel.target
                        || generation != self.groups_panel.generation
                    {
                        continue;
                    }
                    self.groups_panel.pending = false;
                    self.groups_panel.needs_refresh |= mutation;
                    match result {
                        Ok(reply) => {
                            if let Some(info) = reply.info {
                                if let Some(thread) = self
                                    .threads
                                    .iter_mut()
                                    .find(|t| t.id == target && t.is_group)
                                {
                                    thread.apply_group_sidebar_metadata(&info);
                                }
                                self.groups_panel.info = Some(info);
                                self.groups_panel.needs_refresh = false;
                            }
                            if let Some(community) = reply.community {
                                self.groups_panel.community = Some(community);
                                self.groups_panel.status =
                                    "Đã kiểm tra điều kiện cộng đồng từ máy chủ".into();
                            }
                            if let Some(blocked) = reply.blocked {
                                self.groups_panel.blocked_page = blocked.page;
                                self.groups_panel.blocked_more = blocked.more;
                                self.groups_panel.blocked_members = blocked.members;
                                self.groups_panel.status =
                                    format!("Danh sách chặn: trang {}", blocked.page);
                            }
                            if let Some(members) = reply.pending {
                                self.groups_panel.status =
                                    format!("{} người chờ duyệt", members.len());
                                self.groups_panel.pending_members = members;
                            }
                            if let Some((kind, data)) = reply.detail {
                                if kind == "link" {
                                    self.groups_panel.link = Some(data);
                                    self.groups_panel.link_qr = None;
                                    self.groups_panel.status = "Đã tải trạng thái link mời".into();
                                }
                            }
                            if let Some(related) = reply.related {
                                self.groups_panel.status = format!("{} nhóm chung", related.len());
                                self.groups_panel.related = related;
                            }
                            if reply.acknowledged {
                                self.groups_panel.status="Server đã xác nhận. Tải lại trạng thái nhóm trước khi thay đổi tiếp; lời mời có thể còn chờ duyệt.".into();
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::RefreshThreads);
                                }
                            }
                        }
                        Err(error) => {
                            self.groups_panel.status=format!("Không hoàn tất: {error}. Nếu mất xác nhận, hãy kiểm tra trạng thái trước khi thử lại.");
                        }
                    }
                }
                GuiEvent::CloudUsageLoaded {
                    owner,
                    generation,
                    result,
                } => {
                    if self.is_logged_in
                        && owner == self.user_uin
                        && owner == self.cloud_usage.owner
                        && generation == self.cloud_usage.generation
                    {
                        self.cloud_usage.pending = false;
                        self.cloud_usage.result = Some(result);
                    }
                }
                GuiEvent::MessagePinsLoaded {
                    owner,
                    thread,
                    generation,
                    result,
                } => {
                    if owner != self.user_uin
                        || !self.is_logged_in
                        || thread != self.message_pins.thread
                        || generation != self.message_pins.generation
                    {
                        continue;
                    }
                    self.message_pins.pending = false;
                    match result {
                        Ok(mut page) => {
                            let mut seen = std::collections::HashSet::new();
                            page.items.retain(|item| {
                                seen.insert(if item.message_id.is_empty() {
                                    item.id.clone()
                                } else {
                                    item.message_id.clone()
                                })
                            });
                            self.message_pins.page = Some(page);
                            self.message_pins.error.clear();
                        }
                        Err(error) => {
                            self.message_pins.page = None;
                            self.message_pins.error = error;
                        }
                    }
                }
                GuiEvent::Board {
                    owner,
                    thread,
                    generation,
                    mutation,
                    list,
                    result,
                } => {
                    if owner != self.user_uin
                        || !self.is_logged_in
                        || !self.server_board.open
                        || thread != self.server_board.thread
                        || generation != self.server_board.generation
                    {
                        continue;
                    }
                    self.server_board.pending = false;
                    if mutation {
                        self.server_board.needs_refresh = true;
                        if self.server_board.pins {
                            let target = self.server_board.thread.clone();
                            self.refresh_message_pins(&target, self.server_board.group);
                        }
                    }
                    match result {
                        Ok(page) => {
                            if list {
                                self.server_board.needs_refresh = false;
                            }
                            self.server_board.status = if mutation {
                                "Đã được server xác nhận. Bấm Tải lại để xem trạng thái mới.".into()
                            } else {
                                format!("{} mục trong trang này", page.items.len())
                            };
                            if !mutation {
                                self.server_board.page = page;
                            } else {
                                self.server_board.editing = None;
                                self.server_board.title.clear();
                                self.server_board.when.clear();
                            }
                        }
                        Err(error) => {
                            self.server_board.status = if mutation {
                                format!(
                                    "Chưa xác nhận thay đổi; tải lại trước khi thử lại. {error}"
                                )
                            } else {
                                format!("Chưa hoàn tất: {error}")
                            }
                        }
                    }
                }
                GuiEvent::ServerPins { owner, snapshot } => {
                    if owner == self.user_uin && self.is_logged_in {
                        self.server_pins = snapshot;
                    }
                }
                GuiEvent::SettingsDevicesLoaded { generation, result } => {
                    if generation != self.settings_devices.generation {
                        continue;
                    }
                    self.settings_devices.pending = false;
                    match result {
                        Ok(devices) => {
                            self.settings_devices.devices = Some(devices);
                            self.settings_devices.error.clear();
                        }
                        Err(error) => self.settings_devices.error = error,
                    }
                }
                GuiEvent::NotificationSoundPrepared(ready) => {
                    self.sound_prepared = ready;
                    self.sound_preparing = false;
                    self.sound_retry_at = Some(std::time::Instant::now() + Duration::from_secs(60));
                }
                GuiEvent::Reacted {
                    owner,
                    intent_id,
                    action_id,
                    thread_id,
                    msg_id,
                    ts,
                    reaction,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    let message_key = if msg_id.is_empty() {
                        format!("ts:{ts}")
                    } else {
                        msg_id.clone()
                    };
                    let key = (thread_id.clone(), message_key);
                    if self.pending_reaction_actions.get(&key) != Some(&intent_id) {
                        continue;
                    }
                    self.pending_reaction_actions.remove(&key);
                    let clearing = reaction.is_empty();
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        if let Some(m) = t.messages.iter_mut().find(|m| {
                            (!m.id.is_empty() && m.id == msg_id) || (m.id.is_empty() && m.ts == ts)
                        }) {
                            m.apply_reaction(&crate::reactions::ReactionEvent {
                                message_id: msg_id.clone(),
                                thread_id: thread_id.clone(),
                                sender_id: owner,
                                client_event_id: action_id,
                                ts: crate::login::unix_now(),
                                r_type: crate::chat::reaction_code_and_type(&reaction).1,
                                emoji: reaction,
                                ..Default::default()
                            });
                        }
                        self.persist_threads_async();
                    }
                    self.chat_geometry_revision = self.chat_geometry_revision.wrapping_add(1);
                    self.status_msg = if clearing {
                        "Đã bỏ cảm xúc"
                    } else {
                        "Đã thả cảm xúc"
                    }
                    .into();
                }
                GuiEvent::ReactionFailed {
                    owner,
                    intent_id,
                    thread_id,
                    msg_id,
                    ts,
                    message,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    let key_id = if msg_id.is_empty() {
                        format!("ts:{ts}")
                    } else {
                        msg_id
                    };
                    let key = (thread_id, key_id);
                    if self.pending_reaction_actions.get(&key) == Some(&intent_id) {
                        self.pending_reaction_actions.remove(&key);
                        self.show_toast(message);
                    }
                }
                GuiEvent::MessageDeleted {
                    owner,
                    thread_id,
                    msg_id,
                    ts,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    self.chat_geometry_revision = self.chat_geometry_revision.wrapping_add(1);
                    self.invalidate_history_search();
                    self.deleted_message_ids.insert((
                        thread_id.clone(),
                        if msg_id.is_empty() {
                            format!("msg_{ts}")
                        } else {
                            msg_id.clone()
                        },
                    ));
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        t.messages.retain(|m| {
                            !((!msg_id.is_empty() && m.id == msg_id)
                                || (msg_id.is_empty() && m.ts == ts))
                        });
                        t.refresh_after_removal();
                        self.persist_threads_async();
                    }
                    if self.archive_history.thread == thread_id {
                        self.archive_history.messages.retain(|m| {
                            !((!msg_id.is_empty() && m.id == msg_id)
                                || (msg_id.is_empty() && m.ts == ts))
                        });
                    }
                    self.status_msg = "Đã xóa tin nhắn ở phía bạn".into();
                }
                GuiEvent::MessageRecalled {
                    owner,
                    thread_id,
                    msg_id,
                    ts,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    self.chat_geometry_revision = self.chat_geometry_revision.wrapping_add(1);
                    self.invalidate_history_search();
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        if let Some(m) = t.messages.iter_mut().find(|m| {
                            (!msg_id.is_empty() && m.id == msg_id)
                                || (msg_id.is_empty() && m.ts == ts)
                        }) {
                            m.mark_recalled();
                        }
                        if let Some(last) = t.messages.last() {
                            t.preview = last.text.clone();
                        }
                        self.persist_threads_async();
                    }
                    if self.archive_history.thread == thread_id {
                        for m in &mut self.archive_history.messages {
                            if (!msg_id.is_empty() && m.id == msg_id)
                                || (msg_id.is_empty() && m.ts == ts)
                            {
                                m.mark_recalled();
                            }
                        }
                    }
                    self.status_msg = "Đã thu hồi tin nhắn với mọi người".into();
                }
                GuiEvent::UpdateChecked(result) => {
                    self.update_busy = false;
                    match result {
                        Ok(Some(release)) => {
                            self.update_status =
                                format!("Có bản {} cho Windows ARM64", release.tag);
                            self.update_offer = Some(release);
                        }
                        Ok(None) => {
                            self.update_status = "Chưa có bản phát hành ổn định mới hơn".into();
                            self.update_offer = None;
                        }
                        Err(error) => {
                            self.update_status = error;
                        }
                    }
                }
                GuiEvent::UpdatePrepared(result) => {
                    self.update_busy = false;
                    match result {
                        Ok(ready) => {
                            self.update_ready = Some(ready);
                            self.update_status="Bộ cài đã được kiểm tra SHA-256; ứng dụng trong release đúng ARM64".into();
                        }
                        Err(error) => self.update_status = error,
                    }
                }
                GuiEvent::UpdateLaunched(result) => {
                    self.update_busy = false;
                    match result {
                        Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                        Err(error) => self.update_status = error,
                    }
                }
                GuiEvent::MediaRenewed { thread, message } => {
                    self.chat_geometry_revision = self.chat_geometry_revision.wrapping_add(1);
                    for hit in self
                        .history_search
                        .page
                        .hits
                        .iter_mut()
                        .filter(|h| h.thread == thread && h.id == message.id)
                    {
                        hit.message = Some(message.clone());
                    }
                    if let Some(viewer) = self
                        .image_viewer
                        .as_mut()
                        .filter(|v| v.thread_id == thread && v.message_id == message.id)
                    {
                        let media = crate::message_view::media(&message);
                        viewer.original_url = media.original.unwrap_or_default();
                        viewer.dimensions = media.width.zip(media.height);
                        viewer.source.message = (*message).clone();
                    }
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread) {
                        if let Some(m) = t.messages.iter_mut().find(|m| m.id == message.id) {
                            m.link_url = message.link_url;
                            m.link_thumb = message.link_thumb;
                            if let Some(v) = message.wire_ref.fields.get("renewed_media") {
                                m.wire_ref.fields.insert("renewed_media".into(), v.clone());
                            }
                        }
                    }
                }
                GuiEvent::ImageBusy { key } => {
                    if self.image_retry_at.len() < 1024 {
                        self.image_retry_at
                            .insert(key, std::time::Instant::now() + Duration::from_millis(250));
                    } else {
                        self.loading_stickers.remove(&key);
                    }
                    ctx.request_repaint_after(Duration::from_millis(250));
                }
                GuiEvent::ImageUnavailable { key, failure } => {
                    let next =
                        crate::media_recovery::Retry::next(failure, self.image_failures.get(&key));
                    self.status_msg = next.failure.message().into();
                    if let Some(at) = next.at {
                        self.image_retry_at.insert(key.clone(), at);
                        ctx.request_repaint_after(
                            at.saturating_duration_since(std::time::Instant::now()),
                        );
                    }
                    if self.image_failures.len() >= 1024 {
                        if let Some(old) = self.image_failures.keys().next().cloned() {
                            self.image_failures.remove(&old);
                            self.loading_stickers.remove(&old);
                        }
                    }
                    self.image_failures.insert(key, next);
                }
                GuiEvent::VideoFrameReady {
                    generation,
                    key,
                    image,
                } => {
                    crate::video_preview::consumed(generation, &key);
                    if self.active_call.is_some() && crate::video_preview::current(generation) {
                        self.store_texture(ctx, key, image);
                    }
                }
                GuiEvent::PreviewImageReady {
                    owner,
                    expires_at,
                    key,
                    image,
                    permit,
                } => {
                    if owner != self.user_uin
                        || expires_at.is_some_and(|end| end <= crate::login::unix_now() * 1000)
                    {
                        self.loading_stickers.remove(&key);
                        drop(permit);
                        continue;
                    }
                    self.loading_stickers.remove(&key);
                    self.image_retry_at.remove(&key);
                    self.image_failures.remove(&key);
                    self.store_texture(ctx, key, image);
                    drop(permit);
                }
                GuiEvent::StickerImageReady { key, image } => {
                    self.loading_stickers.remove(&key);
                    self.image_retry_at.remove(&key);
                    self.image_failures.remove(&key);
                    if let Some(spec) = self.sticker_sprites.get(&key) {
                        if spec.frames > 1 {
                            // Multi-frame atlas is already active; do not downgrade or squish
                            continue;
                        }
                    }
                    self.store_texture(ctx, key, image);
                }
                GuiEvent::StickerSpriteReady {
                    key,
                    sprite,
                    permit,
                } => {
                    self.loading_stickers.remove(&key);
                    self.image_retry_at.remove(&key);
                    self.image_failures.remove(&key);
                    let frames = sprite.frames;
                    let delays = sprite.delays;
                    self.store_texture(ctx, key.clone(), sprite.image);
                    if let Some(tex) = self.sticker_textures.get(&key) {
                        self.sticker_sprites.insert(
                            key,
                            StickerSprite {
                                texture: tex.clone(),
                                frames,
                                delays,
                            },
                        );
                    }
                    drop(permit);
                }
                GuiEvent::E2eeRequirementResetResult {
                    owner,
                    peer_id,
                    success,
                } => {
                    if owner != self.user_uin {
                        continue;
                    }
                    if success {
                        self.trial_peers.remove(&peer_id);
                        self.show_toast("Đã gỡ yêu cầu E2EE và khôi phục gửi tin nhắn thường");
                    } else {
                        self.show_toast("Không thể gỡ yêu cầu E2EE: lỗi cập nhật cơ sở dữ liệu");
                    }
                }
                GuiEvent::StickerStoreLoaded(packs) => {
                    self.sticker_store_loading = false;
                    let count = packs.len();
                    self.sticker_store_packs = packs;
                    self.status_msg = format!("Đã tải Kho Nhãn Dán Zalo ({count} bộ nhãn)");
                }
                GuiEvent::StickerPackItemsLoaded {
                    cate_id,
                    name,
                    items,
                } => {
                    self.loading_sticker_packs.remove(&cate_id);
                    let count = items.len();
                    if !self
                        .pinned_sticker_packs
                        .iter()
                        .any(|(id, _)| *id == cate_id)
                    {
                        self.pinned_sticker_packs.insert(0, (cate_id, name.clone()));
                    }
                    self.sticker_pack_items.insert(cate_id, items);
                    self.status_msg = format!("Đã mở bộ nhãn \"{name}\" ({count} nhãn dán)");
                }
                GuiEvent::StickerSearchResults { keyword, items } => {
                    self.sticker_search_loading = false;
                    let count = items.len();
                    self.sticker_search_results = items;
                    self.status_msg =
                        format!("Tìm thấy {count} nhãn dán cho từ khóa \"{keyword}\"");
                }
                GuiEvent::StickerSuggestKeywordsLoaded(kws) => {
                    for k in kws {
                        self.sticker_suggest_keywords.insert(k);
                    }
                }
                GuiEvent::ComposerStickerSuggestions { keyword, items } => {
                    self.composer_sticker_loading.remove(&keyword);
                    self.composer_sticker_cache.insert(keyword, items);
                }
                GuiEvent::LinkPreviewReady {
                    owner,
                    runtime_id,
                    url,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || runtime_id != self.account_runtime_id
                    {
                        continue;
                    }
                    self.loading_link_previews.remove(&url);
                    self.loading_stickers
                        .remove(&format!("linkmeta_{}", crate::crypto::md5_hex(&url)));
                    let Ok(meta) = result else { continue };
                    if meta.url != url {
                        continue;
                    }
                    if !meta.thumb.is_empty() {
                        let key = format!("thumb_{}", crate::crypto::md5_hex(&meta.thumb));
                        if !self.sticker_textures.contains_key(&key)
                            && self.loading_stickers.insert(key.clone())
                        {
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::FetchStickerImage {
                                    key,
                                    url: meta.thumb.clone(),
                                });
                            }
                        }
                    }
                    let mut updated_any = false;
                    for t in &mut self.threads {
                        for m in &mut t.messages {
                            let matches_url = m.link_url.as_deref() == Some(meta.url.as_str())
                                || crate::chat::extract_first_url(&m.text).as_deref()
                                    == Some(meta.url.as_str());
                            if matches_url {
                                if m.link_url.is_none() {
                                    m.link_url = Some(meta.url.clone());
                                    updated_any = true;
                                }
                                if (m.link_title.is_none()
                                    || m.link_title.as_deref() == Some(meta.url.as_str()))
                                    && !meta.title.is_empty()
                                {
                                    m.link_title = Some(meta.title.clone());
                                    updated_any = true;
                                }
                                if m.link_desc.is_none() && !meta.desc.is_empty() {
                                    m.link_desc = Some(meta.desc.clone());
                                    updated_any = true;
                                }
                                let old_thumb_expired = m
                                    .link_thumb
                                    .as_deref()
                                    .map(|th| {
                                        th.is_empty()
                                            || (th.contains("zpcloud.zdn.vn")
                                                && !self.sticker_textures.contains_key(&format!(
                                                    "thumb_{}",
                                                    crate::crypto::md5_hex(th)
                                                )))
                                    })
                                    .unwrap_or(true);
                                if old_thumb_expired && !meta.thumb.is_empty() {
                                    m.link_thumb = Some(meta.thumb.clone());
                                    updated_any = true;
                                }
                                if m.link_src.is_none() && !meta.src.is_empty() {
                                    m.link_src = Some(meta.src.clone());
                                    updated_any = true;
                                }
                            }
                        }
                    }
                    self.link_metadata_cache.insert(meta.url.clone(), meta);
                    if updated_any {
                        self.persist_threads_async();
                    }
                }
                GuiEvent::FriendStatusChecked {
                    owner,
                    runtime_id,
                    generation,
                    user_id,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || runtime_id != self.account_runtime_id
                    {
                        continue;
                    }
                    let Some(&(active_generation, action)) = self.friend_requests.get(&user_id)
                    else {
                        continue;
                    };
                    if active_generation != generation {
                        continue;
                    }
                    self.friend_requests.remove(&user_id);
                    match result {
                        Ok(status) => {
                            self.friend_check_retry.remove(&user_id);
                            self.friend_status_map.insert(user_id.clone(), status);
                            if let Some(prof) = self
                                .phone_lookup_result
                                .as_mut()
                                .filter(|prof| prof.user_id == user_id)
                            {
                                prof.friendship = status;
                            }
                        }
                        Err(error) => {
                            self.friend_check_retry
                                .insert(user_id, std::time::Instant::now());
                            if action != FriendAction::Check {
                                self.show_toast(error);
                            }
                        }
                    }
                }
                GuiEvent::PhoneLookupSuccess(res) => {
                    self.phone_lookup_loading = false;
                    self.phone_lookup_error = None;
                    self.phone_modal_open = true;
                    self.show_toast(format!("Đã tìm thấy tài khoản: {}", res.display_name));
                    self.friend_status_map
                        .insert(res.user_id.clone(), res.friendship);
                    self.phone_lookup_result = Some(res);
                }
                GuiEvent::PhoneLookupFailed(err) => {
                    self.phone_lookup_loading = false;
                    self.phone_lookup_result = None;
                    self.phone_modal_open = true;
                    self.phone_lookup_error = Some(err);
                }
                GuiEvent::CallIdentity {
                    owner,
                    thread_id,
                    call_id,
                } => {
                    if !self.is_logged_in || owner != self.user_uin {
                        continue;
                    }
                    if let Some(call) = self
                        .active_call
                        .as_mut()
                        .filter(|c| c.partner_id == thread_id && c.call_id.is_none())
                    {
                        call.call_id = Some(call_id);
                    }
                }
                GuiEvent::GroupEntry {
                    owner,
                    generation,
                    mutation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.group_entry_panel.generation
                    {
                        continue;
                    }
                    self.group_entry_panel.pending = false;
                    match result {
                        Ok(reply) => {
                            if let Some(page) = reply.page {
                                self.group_entry_panel.status =
                                    format!("Đã tải trang {} lời mời", page.page + 1);
                                self.group_entry_panel.page = Some(page);
                                self.group_entry_panel.uncertain = false;
                            }
                            if let Some(preview) = reply.preview {
                                self.group_entry_panel.preview = Some(preview);
                                self.group_entry_panel.status =
                                    "Đã tải thông tin nhóm; chưa gửi yêu cầu tham gia".into();
                            }
                            if reply.pending {
                                self.group_entry_panel.status =
                                    "Yêu cầu tham gia đang chờ quản trị viên duyệt".into();
                                self.group_entry_panel.uncertain = true;
                            }
                            if reply.acknowledged {
                                self.group_entry_panel.status="Máy chủ đã xác nhận yêu cầu. Tải lại lời mời/hội thoại để kiểm tra trạng thái tham gia.".into();
                                self.group_entry_panel.uncertain = true;
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::RefreshThreads);
                                }
                            }
                        }
                        Err(error) => {
                            self.group_entry_panel.uncertain |= mutation;
                            self.group_entry_panel.status = format!("Chưa hoàn tất: {error}");
                        }
                    }
                }
                GuiEvent::Hidden {
                    owner,
                    generation,
                    view,
                    result,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    if view.revision < self.hidden_panel.view.revision {
                        continue;
                    }
                    if generation == Some(self.hidden_panel.generation) {
                        self.hidden_panel.locking = false;
                    }
                    self.apply_hidden_view(view);
                    if generation == Some(self.hidden_panel.generation)
                        || (generation.is_none() && !self.hidden_panel.pending)
                    {
                        self.hidden_panel.pending = false;
                        self.hidden_panel.status = match result {
                            Ok(()) => "Đã cập nhật trạng thái hội thoại ẩn".into(),
                            Err(error) => error,
                        };
                    }
                }
                GuiEvent::AvatarPicked {
                    owner,
                    generation,
                    path,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.avatar_panel.generation
                    {
                        continue;
                    }
                    if let (Some(path), Some(tx)) = (path, WORKER_TX.get()) {
                        if tx
                            .send(WorkerCmd::PrepareAvatar {
                                owner,
                                generation,
                                path,
                            })
                            .is_err()
                        {
                            self.avatar_panel.pending = false;
                            self.avatar_panel.status = "Worker chưa sẵn sàng".into();
                        }
                    } else {
                        self.avatar_panel.pending = false;
                    }
                }
                GuiEvent::AvatarPrepared {
                    owner,
                    generation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.avatar_panel.generation
                    {
                        continue;
                    }
                    self.avatar_panel.pending = false;
                    match result {
                        Ok(draft) => {
                            self.avatar_panel.texture = Some(ctx.load_texture(
                                "avatar-draft",
                                egui::ColorImage::from_rgba_unmultiplied(
                                    [draft.side as usize, draft.side as usize],
                                    &draft.rgba,
                                ),
                                egui::TextureOptions::LINEAR,
                            ));
                            self.avatar_panel.draft = Some(draft);
                            self.avatar_panel.status =
                                "Đã chuẩn bị bản xem trước; chưa tải lên".into();
                        }
                        Err(error) => {
                            self.avatar_panel.draft = None;
                            self.avatar_panel.texture = None;
                            self.avatar_panel.status = format!("Không chuẩn bị được ảnh: {error}");
                        }
                    }
                }
                GuiEvent::AvatarHistory {
                    owner,
                    generation,
                    mutation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.avatar_panel.generation
                    {
                        continue;
                    }
                    self.avatar_panel.pending = false;
                    match result {
                        Ok(reply) => {
                            self.avatar_panel.history_uncertain = false;
                            self.avatar_panel.history_confirm = None;
                            if let Some(page) = reply.page {
                                self.avatar_panel.history_previews_requested.clear();
                                self.avatar_panel.history = Some(page);
                            }
                            if let Some(profile) = reply.profile {
                                if let Some(avatar) = profile.avatar {
                                    self.user_avatar = Some(avatar);
                                }
                                if self.settings.remember_login {
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::PersistSession);
                                    }
                                }
                            }
                            self.avatar_panel.status = if reply.acknowledged {
                                self.avatar_panel.history = None;
                                "Máy chủ đã xác nhận. Tải lại lịch sử để xem kết quả."
                            } else {
                                "Đã tải lịch sử ảnh đại diện"
                            }
                            .into();
                        }
                        Err(error) => {
                            self.avatar_panel.history_uncertain |= mutation;
                            self.avatar_panel.status = format!("Chưa hoàn tất: {error}");
                        }
                    }
                }
                GuiEvent::AvatarUploaded {
                    owner,
                    generation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.avatar_panel.generation
                    {
                        continue;
                    }
                    self.avatar_panel.pending = false;
                    match result {
                        Ok(profile) => {
                            if let Some(profile) = profile {
                                if self.settings.remember_login {
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::PersistSession);
                                    }
                                }
                                self.user_name = profile.name;
                                if let Some(avatar) = profile.avatar {
                                    self.user_avatar = Some(avatar);
                                }
                            }
                            self.avatar_panel.draft = None;
                            self.avatar_panel.texture = None;
                            self.avatar_panel.status="Máy chủ đã xác nhận ảnh đại diện. Ảnh trên các thiết bị có thể cần vài giây để cập nhật.".into();
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::RefreshThreads);
                            }
                        }
                        Err(error) => {
                            self.avatar_panel.uncertain = true;
                            self.avatar_panel.status = format!("Chưa hoàn tất: {error}");
                        }
                    }
                }
                GuiEvent::Profile {
                    owner,
                    generation,
                    mutation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.profile_panel.generation
                    {
                        continue;
                    }
                    self.profile_panel.pending = false;
                    match result {
                        Ok(profile) => {
                            self.user_name = profile.name.clone();
                            if let Some(avatar) = profile.avatar {
                                self.user_avatar = Some(avatar);
                            }
                            self.profile_panel.name = profile.name;
                            self.profile_panel.gender = profile.gender;
                            self.profile_panel.dob = profile.dob.unwrap_or_default();
                            self.profile_panel.loaded = true;
                            self.profile_panel.uncertain = false;
                            self.profile_panel.status = if mutation {
                                "Đã đọc lại và xác nhận hồ sơ từ máy chủ".into()
                            } else {
                                "Đã tải hồ sơ từ máy chủ".into()
                            };
                            if self.settings.remember_login {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::PersistSession);
                                }
                            }
                        }
                        Err(error) => {
                            self.profile_panel.uncertain |= mutation;
                            self.profile_panel.status = format!("Chưa cập nhật hồ sơ: {error}");
                        }
                    }
                }
                GuiEvent::Labels {
                    owner,
                    generation,
                    mutation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.labels_panel.generation
                    {
                        continue;
                    }
                    self.labels_panel.pending = false;
                    match result {
                        Ok(snapshot) => {
                            if self.labels_panel.filter.is_some_and(|id| {
                                !snapshot.labels.iter().any(|label| label.id == id)
                            }) {
                                self.labels_panel.filter = None;
                            }
                            self.labels_panel.status =
                                format!("{} phân loại đã tải từ máy chủ", snapshot.labels.len());
                            self.labels_panel.snapshot = Some(snapshot);
                            self.labels_panel.uncertain = false;
                        }
                        Err(error) => {
                            self.labels_panel.uncertain |= mutation;
                            self.labels_panel.status = format!("Chưa cập nhật phân loại: {error}");
                        }
                    }
                }
                GuiEvent::Privacy {
                    owner,
                    generation,
                    mutation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.privacy_panel.generation
                    {
                        continue;
                    }
                    self.privacy_panel.pending = false;
                    match result {
                        Ok(snapshot) => {
                            self.inbox_policy.direct = snapshot
                                .values
                                .get(&crate::privacy::Setting::StrangerDirect)
                                .map(|value| *value != 0);
                            self.privacy_panel.snapshot = Some(snapshot);
                            self.privacy_panel.uncertain = false;
                            self.privacy_panel.status = if mutation {
                                "Đã đọc lại và xác nhận thay đổi từ máy chủ".into()
                            } else {
                                "Đã tải quyền riêng tư từ máy chủ".into()
                            };
                        }
                        Err(error) => {
                            self.privacy_panel.uncertain |= mutation;
                            self.privacy_panel.status = format!("Chưa cập nhật: {error}");
                        }
                    }
                }
                GuiEvent::ManualMarks {
                    owner,
                    generation,
                    mutation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.manual_mark_generation
                    {
                        continue;
                    }
                    self.manual_mark_pending = false;
                    match result {
                        Ok(marks) => {
                            self.manual_mark_uncertain = false;
                            crate::unread::merge_manual_unread_marks(&mut self.threads, &marks);
                            self.persist_threads_async();
                            self.show_toast("Đã cập nhật dấu chưa đọc từ máy chủ");
                        }
                        Err(error) => {
                            self.manual_mark_uncertain |= mutation;
                            self.show_toast(format!("Chưa cập nhật dấu chưa đọc: {error}"));
                        }
                    }
                }
                GuiEvent::Archive {
                    owner,
                    generation,
                    mutation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || generation != self.archive_generation
                    {
                        continue;
                    }
                    self.archive_pending = false;
                    match result {
                        Ok(snapshot) => {
                            self.archive_uncertain = false;
                            let archived: std::collections::HashSet<_> = snapshot
                                .items
                                .into_iter()
                                .map(|item| (item.id, item.kind))
                                .collect();
                            for thread in &mut self.threads {
                                thread.presentation.archived =
                                    crate::server_pins::identity(&thread.id, thread.is_group)
                                        .ok()
                                        .map(|id| {
                                            let bare = if thread.is_group {
                                                id[1..].to_owned()
                                            } else {
                                                id
                                            };
                                            archived.contains(&(bare, u8::from(thread.is_group)))
                                        });
                            }
                            self.persist_threads_async();
                            self.show_toast("Đã cập nhật danh sách lưu trữ từ máy chủ");
                        }
                        Err(error) => {
                            self.archive_uncertain |= mutation;
                            self.show_toast(format!("Chưa cập nhật lưu trữ: {error}. Tải lại trạng thái trước khi thử tiếp."));
                        }
                    }
                }
                GuiEvent::Contact {
                    owner,
                    target,
                    generation,
                    mutation,
                    result,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || target != self.contact_panel.target
                        || generation != self.contact_panel.generation
                    {
                        continue;
                    }
                    self.contact_panel.pending = false;
                    match result {
                        Ok(reply) => {
                            if let Some(lookup) = reply.lookup {
                                self.contact_panel.lookup = Some(lookup);
                                self.contact_panel.status =
                                    "Đã tìm thấy hồ sơ; chưa tự kết bạn hay mở cuộc trò chuyện"
                                        .into();
                            }
                            if let Some(friends) = reply.close_friends {
                                self.contact_panel.status =
                                    format!("{} bạn thân từ máy chủ", friends.len());
                                self.contact_panel.close_friends = friends;
                            }
                            if let Some(qr) = reply.qr {
                                self.contact_panel.qr_requested = false;
                                self.contact_panel.qr = Some(qr);
                            }
                            if let Some(aliases) = reply.aliases {
                                self.server_aliases_ready = true;
                                self.server_aliases = aliases;
                                self.apply_server_aliases();
                            }
                            if let Some(presence) = reply.presence {
                                self.contact_panel.presence = Some(presence);
                                self.contact_panel.presence_checked =
                                    Some(std::time::Instant::now());
                            }
                            if let Some(status) = reply.friendship {
                                if mutation && status == crate::chat::FriendshipStatus::Unknown {
                                    if let Some(friends) = self.inbox_policy.friends.as_mut() {
                                        friends.remove(&target);
                                    }
                                }
                                self.friend_status_map.insert(target, status);
                                self.contact_panel.status = format!(
                                    "Trạng thái: {}",
                                    match status {
                                        crate::chat::FriendshipStatus::Friend => "Bạn bè",
                                        crate::chat::FriendshipStatus::IncomingPending =>
                                            "Lời mời đến",
                                        crate::chat::FriendshipStatus::OutgoingPending =>
                                            "Đã gửi lời mời",
                                        crate::chat::FriendshipStatus::Stranger => "Chưa kết bạn",
                                        _ => "Chưa xác định",
                                    }
                                );
                            }
                            if let Some(blocked) = reply.blocked {
                                self.contact_panel.blocked_page = blocked.page;
                                self.contact_panel.blocked_more = blocked.more;
                                self.contact_panel.blocked = blocked.users;
                                self.contact_panel.status =
                                    format!("Danh sách chặn: trang {}", blocked.page);
                            }
                            if mutation {
                                self.contact_panel.status =
                                    "Máy chủ đã xác nhận thay đổi. Tải lại trạng thái để kiểm tra."
                                        .into();
                            }
                            if mutation {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::RefreshThreads);
                                }
                            }
                        }
                        Err(error) => {
                            self.contact_panel.uncertain |= mutation;
                            self.contact_panel.status = format!("Không hoàn tất: {error}");
                        }
                    }
                }
                GuiEvent::CallAnsweredElsewhere { owner, call_id } => {
                    if !self.is_logged_in || owner != self.user_uin {
                        continue;
                    }
                    if self.active_call.as_ref().is_some_and(|call| {
                        call.call_id == Some(call_id) && !call.is_incoming_ringing
                    }) {
                        continue;
                    }
                    self.call_terminals
                        .remember(&owner, call_id, std::time::Instant::now());
                    if !self.active_call.as_ref().is_some_and(|call| {
                        call.call_id == Some(call_id) && call.is_incoming_ringing
                    }) {
                        continue;
                    }
                    self.sticker_textures.remove("call_remote_video");
                    self.sticker_textures.remove("call_remote_video_1");
                    self.sticker_textures.remove("call_local_video");
                    self.active_call = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                        egui::UserAttentionType::Reset,
                    ));
                    self.show_toast("Cuộc gọi đã được nhận trên thiết bị khác");
                }
                GuiEvent::CallStateChanged {
                    owner,
                    call_id,
                    state,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || !self
                            .active_call
                            .as_ref()
                            .is_some_and(|call| call.call_id == Some(call_id))
                    {
                        continue;
                    }
                    if state == "free" {
                        self.call_terminals
                            .remember(&owner, call_id, std::time::Instant::now());
                        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                            egui::UserAttentionType::Reset,
                        ));
                        // A terminal call always closes its incoming controls.
                        let _has_error = self
                            .active_call
                            .as_ref()
                            .and_then(|c| c.error_reason.as_ref())
                            .is_some();
                        self.sticker_textures.remove("call_remote_video");
                        self.sticker_textures.remove("call_remote_video_1");
                        self.sticker_textures.remove("call_local_video");
                        self.active_call = None;
                    } else if let Some(ref mut call) = self.active_call {
                        if state == "incall" || state == "connected" {
                            call.is_incoming_ringing = false;
                        }
                        if call.error_reason.is_none() {
                            call.state_label = match state.as_str() {
                                "calling" | "ringing" => "Đang đổ chuông…".into(),
                                "incoming_ringing" => "Cuộc gọi đến đang đổ chuông…".into(),
                                "connecting" => "Đang kết nối luồng âm thanh/video…".into(),
                                "incall" | "connected" => {
                                    "Đang trong cuộc gọi (Pure-Rust VoIP)".into()
                                }
                                _ => format!("Đang kết nối ({state})…"),
                            };
                        }
                    }
                }
                GuiEvent::IncomingCall {
                    owner,
                    call_id,
                    caller_id,
                    caller_name,
                    is_video,
                    is_group,
                } => {
                    if !self.is_logged_in || owner != self.user_uin {
                        continue;
                    }
                    if self
                        .call_terminals
                        .contains(&owner, call_id, std::time::Instant::now())
                        || self.active_call.is_some()
                    {
                        continue;
                    }
                    let fallback_name = self
                        .threads
                        .iter()
                        .find(|t| !t.is_group && t.id == caller_id)
                        .map(|t| self.resolve_thread_name(t).to_owned())
                        .unwrap_or(caller_name);
                    let resolved_name = self
                        .resolve_contact_name(&caller_id, &fallback_name)
                        .to_owned();
                    if self.settings.call_notifications_enabled {
                        self.status_msg = format!(
                            "📲 Cuộc gọi {} đến từ {resolved_name}…",
                            if is_video { "video" } else { "thoại" }
                        );
                    }
                    self.active_call = Some(ActiveCallOverlay {
                        call_id: Some(call_id),
                        partner_id: caller_id,
                        partner_name: resolved_name,
                        is_video,
                        is_group,
                        state_label: "Cuộc gọi đến đang đổ chuông…".into(),
                        started_at: std::time::Instant::now(),
                        mic_muted: false,
                        cam_enabled: is_video,
                        is_incoming_ringing: true,
                        error_code: None,
                        error_reason: None,
                    });
                    if self.settings.call_notifications_enabled {
                        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                            egui::UserAttentionType::Critical,
                        ));
                    }
                }
                GuiEvent::CallRejected {
                    owner,
                    partner_id,
                    call_id,
                    code,
                    reason,
                } => {
                    if !self.is_logged_in
                        || owner != self.user_uin
                        || !self.active_call.as_ref().is_some_and(|call| {
                            call.partner_id == partner_id
                                && match call_id {
                                    Some(id) => call.call_id == Some(id),
                                    None => call.call_id.is_none(),
                                }
                        })
                    {
                        continue;
                    }
                    if let Some(id) = call_id {
                        self.call_terminals
                            .remember(&owner, id, std::time::Instant::now());
                    }

                    self.status_msg = format!("⚠️ Từ chối cuộc gọi (mã {code}): {reason}");
                    if let Some(ref mut call) = self.active_call {
                        call.is_incoming_ringing = false;
                        call.state_label = "Không thể thực hiện cuộc gọi".into();
                        call.error_code = Some(code);
                        call.error_reason = Some(reason);
                    }
                }
                GuiEvent::ConversationRefreshStatus {
                    running,
                    success,
                    message,
                } => {
                    if running && !self.refresh_sync_running {
                        self.sync_notice_dismissed = false;
                    }
                    self.sync_success_until = if !running && success {
                        Some(std::time::Instant::now() + Duration::from_secs(3))
                    } else {
                        None
                    };
                    if !self.phone_sync_running {
                        self.phone_sync_status.clear();
                    }
                    self.refresh_sync_running = running;
                    self.refresh_sync_status = message.clone();
                    self.status_msg = message;
                    self.last_sync_was_history = false;
                }
                GuiEvent::PhoneSyncStatus {
                    running,
                    success,
                    message,
                } => {
                    if running && !self.phone_sync_running {
                        self.sync_notice_dismissed = false;
                    }
                    self.sync_success_until = if !running && success {
                        Some(std::time::Instant::now() + Duration::from_secs(3))
                    } else {
                        None
                    };
                    if !running {
                        self.phone_download_status = message.clone();
                    }
                    if message.starts_with("Tải về:") {
                        self.phone_download_status = message.clone();
                    } else if message.starts_with("Nhập CSDL:") {
                        self.phone_import_status = message.clone();
                    } else if running && !self.phone_sync_running {
                        self.phone_download_status = message.clone();
                        self.phone_import_status = "Nhập CSDL: chờ dữ liệu".into();
                    }

                    if !running || !self.phone_sync_running {
                        self.sync_cancel_requested = false;
                    }
                    self.phone_sync_running = running;
                    self.phone_sync_status = message.clone();
                    self.status_msg = message;
                }
                GuiEvent::HistorySyncStatus {
                    running,
                    success,
                    message,
                } => {
                    if running && !self.history_sync_running {
                        self.sync_notice_dismissed = false;
                    }
                    self.sync_success_until = if !running && success {
                        Some(std::time::Instant::now() + Duration::from_secs(3))
                    } else {
                        None
                    };
                    if !self.phone_sync_running {
                        self.phone_sync_status.clear();
                    }
                    if !running || !self.history_sync_running {
                        self.sync_cancel_requested = false;
                    }
                    self.last_sync_was_history = true;

                    self.history_sync_running = running;
                    self.status_msg = message.clone();
                    self.history_sync_status = message;
                }
                GuiEvent::LocalCallRecord { .. } => {
                    unreachable!("local call record normalized before dispatch")
                }
                GuiEvent::ConversationHistoryLoaded { .. } => {
                    unreachable!("conversation history normalized before dispatch")
                }
                GuiEvent::StatusUpdate(msg) => {
                    if is_status_error(&msg) {
                        self.show_toast(msg);
                    } else {
                        self.status_msg = msg;
                    }
                }
                GuiEvent::ServerMediaLoaded {
                    runtime_id,
                    thread_id,
                    media_type,
                    page_num,
                    page,
                    generation,
                } => {
                    // Only update if generation matches and thread/media_type match current tools state
                    if self.is_logged_in
                        && runtime_id == self.account_runtime_id
                        && self
                            .threads
                            .get(self.selected_thread_index)
                            .is_some_and(|thread| thread.id == thread_id)
                        && generation == self.tools.server_media_generation
                        && thread_id == self.tools.server_media_thread_id
                        && media_type == self.tools.server_media_type
                    {
                        self.tools.server_media_loading = false;
                        self.tools.server_media_page = page_num;
                        self.tools.server_media_has_more = page.has_more;
                        self.tools.server_media_next_cursor = page.next_cursor;
                        if page_num <= 1 {
                            self.tools.server_media_items = page.items;
                        } else {
                            // Deduplicate items by ID (or URL if ID is empty)
                            for new_item in page.items {
                                let exists = self.tools.server_media_items.iter().any(|existing| {
                                    if !new_item.id.is_empty() && !existing.id.is_empty() {
                                        existing.id == new_item.id
                                    } else {
                                        existing.url == new_item.url
                                    }
                                });
                                if !exists {
                                    self.tools.server_media_items.push(new_item);
                                }
                            }
                        }
                        self.status_msg = format!(
                            "Đã tải {} mục từ kho media server",
                            self.tools.server_media_items.len()
                        );
                    }
                }
                GuiEvent::ServerMediaLoadFailed {
                    runtime_id,
                    thread_id,
                    media_type,
                    generation,
                    error,
                } => {
                    if self.is_logged_in
                        && runtime_id == self.account_runtime_id
                        && self
                            .threads
                            .get(self.selected_thread_index)
                            .is_some_and(|thread| thread.id == thread_id)
                        && generation == self.tools.server_media_generation
                        && thread_id == self.tools.server_media_thread_id
                        && media_type == self.tools.server_media_type
                    {
                        self.tools.server_media_loading = false;
                        self.status_msg = error;
                    }
                }
                GuiEvent::CloudViewerKeyLoaded(key) => {
                    self.status_msg =
                        format!("Đã nhận khoá giải mã zCloud cho tệp {}", key.file_id);
                    self.tools
                        .cloud_viewer_keys
                        .insert(key.file_id.clone(), key);
                }
                GuiEvent::GroupTodosLoaded {
                    runtime_id,
                    thread_id,
                    todos,
                } => {
                    if !self.is_logged_in
                        || runtime_id != self.account_runtime_id
                        || !self
                            .threads
                            .get(self.selected_thread_index)
                            .is_some_and(|thread| thread.is_group && thread.id == thread_id)
                    {
                        continue;
                    }
                    if self.tools.todo_poll_thread_id != thread_id {
                        self.tools.group_polls.clear();
                    }
                    self.tools.todo_poll_thread_id = thread_id;
                    self.tools.group_todos = todos;
                    self.status_msg = format!(
                        "Đã tải {} việc cần làm từ nhóm",
                        self.tools.group_todos.len()
                    );
                }
                GuiEvent::GroupPollsLoaded {
                    runtime_id,
                    thread_id,
                    polls,
                } => {
                    if !self.is_logged_in
                        || runtime_id != self.account_runtime_id
                        || !self
                            .threads
                            .get(self.selected_thread_index)
                            .is_some_and(|thread| thread.is_group && thread.id == thread_id)
                    {
                        continue;
                    }
                    if self.tools.todo_poll_thread_id != thread_id {
                        self.tools.group_todos.clear();
                    }
                    self.tools.todo_poll_thread_id = thread_id;
                    self.tools.group_polls = polls;
                    self.status_msg =
                        format!("Đã tải {} cuộc bình chọn", self.tools.group_polls.len());
                }
                GuiEvent::BatchRecallCompleted {
                    thread_id,
                    recalled_ids,
                    count,
                } => {
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        for m in &mut t.messages {
                            if recalled_ids.contains(&m.id) {
                                m.mark_recalled();
                            }
                        }
                    }
                    self.batch_selection_mode = false;
                    self.selected_msg_ids.clear();
                    self.status_msg = format!("Đã thu hồi {count} tin nhắn cho mọi người");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Lightweight Background Session Worker (2-thread Tokio pool, off-UI disk I/O)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Main eframe::App Implementation
// ---------------------------------------------------------------------------

impl eframe::App for ZaloGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.is_logged_in && !self.lock_guard.is_locked() {
            // Sidebar relative-time labels must advance while the window is idle.
            ctx.request_repaint_after(std::time::Duration::from_secs(30));
        }
        self.cancel_voice_if_context_changed();
        self.lock_guard.poll_availability();
        // A21 fix: Check and update lock state BEFORE processing input/events/render
        // This ensures lock transition happens before any sensitive operation can execute.
        if self.is_logged_in
            && self.lock_guard.should_auto_lock(
                self.settings.enable_app_lock,
                self.settings.has_pin(),
                self.settings.use_windows_hello,
            )
        {
            self.lock_app();
        }
        if self.lock_guard.is_locked() && self.voice_binding.is_some() {
            self.cancel_voice_recording();
        }
        self.poll_voice_events(ctx);

        // Update lock guard on user interaction (WP16-4) — but only if not locked
        // A21: Input after idle should not reset deadline if we're already locked
        if !self.lock_guard.is_locked()
            && ctx.input(|i| {
                !i.raw.hovered_files.is_empty()
                    || !i.raw.dropped_files.is_empty()
                    || i.raw.events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::Key { .. } | egui::Event::PointerButton { .. }
                        )
                    })
            })
        {
            self.lock_guard.touch();
        }

        crate::i18n::set_language(&self.settings.language);
        let notice_owner = if self.is_logged_in {
            self.user_uin.as_str()
        } else {
            ""
        };
        let notice_account_changed = crate::desktop_notice::snapshot().owner != notice_owner;
        crate::desktop_notice::account(notice_owner);
        crate::desktop_notice::privacy_lock(notice_owner, self.lock_guard.is_locked());
        if self.is_logged_in && notice_account_changed {
            crate::desktop_notice::count(&self.user_uin, self.unread_counts.values().sum());
        }
        crate::desktop_notice::configure(
            self.settings.enable_notifications,
            self.settings.play_notification_sound,
        );
        if self.settings.enable_notifications
            && self.settings.play_notification_sound
            && self.is_logged_in
        {
            if !self.sound_prepared
                && self.sound_attempts < 3
                && !self.sound_preparing
                && self
                    .sound_retry_at
                    .is_none_or(|at| std::time::Instant::now() >= at)
            {
                if let Some(tx) = WORKER_TX.get() {
                    self.sound_preparing = tx
                        .send(WorkerCmd::PrepareNotificationSound { play: false })
                        .is_ok();
                    if self.sound_preparing {
                        self.sound_attempts += 1;
                    }
                }
            }
        } else {
            self.sound_prepared = false;
            self.sound_attempts = 0;
            self.sound_retry_at = None;
        }

        if let Some(notice) = crate::desktop_notice::take_activation() {
            if self.is_logged_in && notice.owner == self.user_uin {
                if let Some(index) = self.threads.iter().position(|t| t.id == notice.thread_id) {
                    self.selected_thread_index = index;
                    self.filter_starred_only = false;
                } else {
                    self.tools_command(tools_panel::ToolsCommand::OpenHit {
                        thread: notice.thread_id,
                        id: notice.message_id,
                    });
                }
            }
        }
        #[cfg(windows)]
        crate::tray::configure_notifications(
            self.settings.enable_notifications,
            self.settings.play_notification_sound,
        );
        let smoke_switch = self
            .smoke
            .as_ref()
            .filter(|s| s.switch_at > 0)
            .map(|s| (s.frames + 1, s.switch_at));
        if let Some((frame, at)) = smoke_switch {
            if frame == at {
                if let Some(index) = self
                    .threads
                    .iter()
                    .position(|t| t.id == "fixture-group-long")
                {
                    self.open_thread(ctx, index, "fixture-group-long".into(), true);
                }
            } else if frame == at + 2 {
                if let Some(thread) = self
                    .threads
                    .iter_mut()
                    .find(|t| t.id == "fixture-group-long")
                {
                    let older: Vec<_> = (0..120)
                        .map(|i| ChatMessage {
                            id: format!("fixture-older-{i}"),
                            sender: "Giang Hồ".into(),
                            text: format!(
                                "Tin cũ hơn số {i}: {}",
                                "lịch sử nhóm tải về sau ".repeat(1 + i % 3)
                            ),
                            ts: 1_690_000_000_000 + i as i64 * 60_000,
                            time: "09:00".into(),
                            ..Default::default()
                        })
                        .collect();
                    let mut merged = older;
                    merged.append(&mut thread.messages);
                    thread.messages = merged;
                }
            }
        }
        if let Some(smoke) = &mut self.smoke {
            smoke.frames += 1;
            let captured = ctx.input(|input| {
                input.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(captured) = captured {
                let colors: std::collections::HashSet<_> =
                    captured.pixels.iter().map(|p| p.to_array()).collect();
                if colors.len() < 16 {
                    eprintln!("UI smoke failed: blank framebuffer");
                    std::process::exit(3);
                }
                let rgba: Vec<u8> = captured.pixels.iter().flat_map(|p| p.to_array()).collect();
                let target = if smoke.sequence > 0 {
                    smoke.shots += 1;
                    let stem = smoke.path.with_extension("");
                    std::path::PathBuf::from(format!("{}-{:02}.png", stem.display(), smoke.shots))
                } else {
                    smoke.path.clone()
                };
                if let Err(e) = image::save_buffer(
                    &target,
                    &rgba,
                    captured.size[0] as u32,
                    captured.size[1] as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("UI smoke failed: {e}");
                    std::process::exit(3);
                }
                if smoke.sequence > 0 && smoke.shots < smoke.sequence {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                    ctx.request_repaint();
                    return;
                }
                #[cfg(windows)]
                if std::env::args().any(|a| a == "--tray-smoke-test") {
                    if !smoke.lifecycle_started {
                        smoke.lifecycle_started = true;
                        let Some(tray) = &self.tray else {
                            eprintln!("Tray smoke failed: no native tray");
                            std::process::exit(3);
                        };
                        crate::tray::smoke_lifecycle(tray.control());
                    }
                } else {
                    self.allow_quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                #[cfg(not(windows))]
                {
                    self.allow_quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            } else if (smoke.sequence > 0 && smoke.frames == 1)
                || (smoke.frames == 5 && !smoke.lifecycle_started && smoke.sequence == 0)
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            } else if smoke.frames > 300 {
                eprintln!("UI smoke failed: screenshot timeout");
                std::process::exit(3);
            }
            ctx.request_repaint();
        }
        #[cfg(windows)]
        if crate::tray::take_restore_request() && self.smoke.is_none() && self.is_logged_in {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::RefreshThreads);
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    let _ = tx.send(WorkerCmd::LoadGroupHistory {
                        group_id: thread.id.clone(),
                        is_group: thread.is_group,
                        limit: thread.messages.len().max(500),
                    });
                }
            }
        }
        self.handle_key_events(ctx);
        #[cfg(windows)]
        if let Some(tray) = &self.tray {
            if tray.control().quitting() {
                self.allow_quit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                self.check_events(ctx);
                self.sync_composer_draft(ctx);
                self.preserve_draft_before_close(ctx);
                return;
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_quit {
            #[cfg(windows)]
            if let Some(tray) = &self.tray {
                tray.control().hide_on_close();
            } else {
                // A failed/missing shell must never leave an unreachable process.
                self.allow_quit = true;
            }
            #[cfg(not(windows))]
            {
                self.allow_quit = true;
            }
        }
        if self.settings.theme_mode == "system" {
            let dark = ctx.system_theme() != Some(egui::Theme::Light);
            if dark != self.dark_mode {
                self.dark_mode = dark;
                self.theme_dirty = true;
            }
        }
        if self.theme_dirty {
            self.palette().apply_to_ctx(ctx);
            self.settings.appearance.apply_context(ctx);
            self.theme_dirty = false;
        }

        self.attachment_drafts.sync_owner(if self.is_logged_in {
            &self.user_uin
        } else {
            ""
        });
        self.sync_composer_draft(ctx);
        self.check_events(ctx);
        self.attachment_drafts.sync_owner(if self.is_logged_in {
            &self.user_uin
        } else {
            ""
        });
        self.sync_composer_draft(ctx);
        if self.is_logged_in && self.tools.loaded_for != self.user_uin && self.smoke.is_none() {
            self.tools = tools_panel::ToolsPanel {
                loaded_for: self.user_uin.clone(),
                ..Default::default()
            };
            self.tools_command(tools_panel::ToolsCommand::Load);
        }
        if !self.is_logged_in && !self.tools.loaded_for.is_empty() {
            self.tools = Default::default();
            self.theme_dirty = true;
        }
        if self.profile_menu_open && !self.profiles_requested {
            if let Some(tx) = WORKER_TX.get() {
                self.profiles_requested = tx
                    .send(WorkerCmd::LoadProfiles {
                        owner: self.user_uin.clone(),
                    })
                    .is_ok();
            }
        }
        self.trim_resident_history();
        let now = std::time::Instant::now();
        self.image_retry_at.retain(|key, deadline| {
            if *deadline <= now {
                self.loading_stickers.remove(key);
                false
            } else {
                true
            }
        });
        ensure_cjk_font(
            ctx,
            self.threads.iter().map(|t| t.name.as_str()).chain(
                self.threads
                    .get(self.selected_thread_index)
                    .into_iter()
                    .flat_map(|t| t.messages.iter().map(|m| m.text.as_str())),
            ),
        );

        // Network events, input and animation deadlines schedule their own repaints.

        let pal = self.palette();

        // Dev Inspector Panel — A21: gate when locked
        if self.dev_mode && self.dev_panel_open && !self.lock_guard.is_locked() {
            egui::TopBottomPanel::bottom("dev_inspector")
                .resizable(true)
                .default_height(250.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Dev Inspector & Protocol Scanner");
                        ui.separator();
                        ui.selectable_value(
                            &mut self.dev_tab,
                            DevTab::Logs,
                            "Lưu lượng mạng (Logs)",
                        );
                        ui.selectable_value(
                            &mut self.dev_tab,
                            DevTab::Scanner,
                            "Quét lỗi hệ thống (Scanner)",
                        );
                        ui.selectable_value(
                            &mut self.dev_tab,
                            DevTab::CryptoState,
                            "Trạng thái Crypto & Keys",
                        );

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(crate::i18n::tr("Làm mới quét")).clicked() {
                                self.scanner_findings = logger::scan_logs();
                            }
                            if ui.button(crate::i18n::tr("Xóa log")).clicked() {
                                logger::clear_entries();
                                self.selected_log = None;
                                self.scanner_findings.clear();
                            }
                        });
                    });
                    ui.separator();

                    match self.dev_tab {
                        DevTab::Logs => self.render_dev_logs(ui),
                        DevTab::Scanner => self.render_dev_scanner(ui),
                        DevTab::CryptoState => self.render_dev_crypto(ui),
                    }
                });
        }

        // Main Workspace
        if !self.is_logged_in {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(pal.bg_chat_canvas))
                .show(ctx, |ui| {
                    self.render_login_screen(ui, &pal);
                });
        } else if self.lock_guard.is_locked() {
            // Render lock screen (WP16-4 / MAP-00)
            self.render_lock_screen(ctx, &pal);
        } else {
            self.render_chat_workspace(ctx, &pal);
            self.render_reactor_details(ctx);
            self.render_message_details(ctx);
            self.render_archive_confirmation(ctx, &pal);
        }
        self.sync_composer_draft(ctx);
        self.preserve_draft_before_close(ctx);

        if !ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            let screen = ctx.screen_rect();
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                Id::new("window_border_overlay"),
            ))
            .rect_stroke(
                screen.shrink(0.5),
                Rounding::ZERO,
                Stroke::new(1.0_f32, pal.border),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Screen Renderers
// ---------------------------------------------------------------------------

impl ZaloGuiApp {
    fn render_login_screen(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        ui.vertical_centered(|ui| {
            let avail_h = ui.available_height();
            ui.add_space((avail_h * 0.08).max(20.0));
            if let Some(tex) = &self.app_logo_texture {
                ui.image((tex.id(), Vec2::new(64.0, 64.0)));
                ui.add_space(10.0);
            }
            ui.label(
                RichText::new(crate::i18n::tr("Đăng nhập Zalo"))
                    .color(pal.primary)
                    .size(26.0)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(crate::i18n::tr(
                    "Khởi chạy nhẹ, bảo mật và đồng bộ tin nhắn thời gian thực",
                ))
                .color(pal.text_secondary)
                .size(13.5),
            );
            ui.add_space(18.0);

            egui::Frame::none()
                .fill(pal.bg_elevated)
                .rounding(Rounding::same(16.0))
                .stroke(Stroke::new(1.0_f32, pal.border))
                .inner_margin(Margin::symmetric(28.0, 24.0))
                .show(ui, |ui| {
                    ui.set_width(360.0);
                    ui.vertical_centered(|ui| {
                        // Segmented Tab Switcher
                        egui::Frame::none()
                            .fill(pal.bg_input)
                            .rounding(Rounding::same(10.0))
                            .inner_margin(Margin::same(4.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let tab_w = (ui.available_width() - 8.0) / 2.0;
                                    for (tab, title) in [
                                        (LoginTab::QrCode, "Quét mã QR"),
                                        (LoginTab::Password, "Số điện thoại"),
                                    ] {
                                        let active = self.login_tab == tab;
                                        let btn = egui::Button::new(
                                            RichText::new(title)
                                                .size(13.0)
                                                .color(if active {
                                                    Color32::WHITE
                                                } else {
                                                    pal.text_secondary
                                                })
                                                .strong(),
                                        )
                                        .fill(if active {
                                            pal.primary
                                        } else {
                                            Color32::TRANSPARENT
                                        })
                                        .stroke(Stroke::NONE)
                                        .rounding(Rounding::same(7.0))
                                        .min_size(Vec2::new(tab_w, 32.0));

                                        if ui
                                            .add(btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            self.login_tab = tab;
                                        }
                                    }
                                });
                            });

                        ui.add_space(18.0);

                        match self.login_tab {
                            LoginTab::QrCode => {
                                if let Some(tex) = &self.qr_texture {
                                    egui::Frame::none()
                                        .fill(Color32::WHITE)
                                        .rounding(Rounding::same(12.0))
                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                        .inner_margin(Margin::same(14.0))
                                        .show(ui, |ui| {
                                            ui.image((tex.id(), Vec2::new(240.0, 240.0)));
                                        });
                                } else {
                                    let (rect, _) = ui.allocate_exact_size(
                                        Vec2::new(268.0, 268.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        rect,
                                        Rounding::same(12.0),
                                        pal.bg_input,
                                    );
                                    ui.painter().rect_stroke(
                                        rect,
                                        Rounding::same(12.0),
                                        Stroke::new(1.0_f32, pal.border),
                                    );
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "Đang tải mã QR…",
                                        FontId::proportional(14.5),
                                        pal.text_secondary,
                                    );
                                }

                                ui.add_space(14.0);
                                ui.label(
                                    RichText::new(&self.qr_status)
                                        .color(pal.text_primary)
                                        .size(13.5),
                                );
                                ui.add_space(12.0);

                                let refresh_btn = egui::Button::new(
                                    RichText::new(crate::i18n::tr("Tạo / Làm mới mã QR"))
                                        .size(13.5)
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(pal.primary)
                                .rounding(Rounding::same(8.0))
                                .min_size(Vec2::new(200.0, 36.0));

                                if ui
                                    .add(refresh_btn)
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.login_in_progress = true;
                                    self.qr_status = "Đang xin mã QR…".into();
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::RequestQr);
                                    }
                                }
                            }
                            LoginTab::Password => {
                                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                                    ui.label(
                                        RichText::new(crate::i18n::tr("Số điện thoại"))
                                            .size(13.0)
                                            .color(pal.text_secondary)
                                            .strong(),
                                    );
                                    ui.add_space(4.0);
                                    ui.add_sized(
                                        [ui.available_width(), 34.0],
                                        egui::TextEdit::singleline(&mut self.phone_input)
                                            .hint_text(crate::i18n::tr("Ví dụ: 0912345678")),
                                    );
                                    ui.add_space(12.0);

                                    ui.label(
                                        RichText::new(crate::i18n::tr("Mật khẩu"))
                                            .size(13.0)
                                            .color(pal.text_secondary)
                                            .strong(),
                                    );
                                    ui.add_space(4.0);
                                    ui.add_sized(
                                        [ui.available_width(), 34.0],
                                        egui::TextEdit::singleline(&mut self.password_input)
                                            .password(true)
                                            .hint_text(crate::i18n::tr("Nhập mật khẩu Zalo")),
                                    );
                                    ui.add_space(16.0);

                                    if !self.login_status_msg.is_empty() {
                                        ui.label(
                                            RichText::new(&self.login_status_msg)
                                                .color(pal.danger)
                                                .size(12.5),
                                        );
                                        ui.add_space(8.0);
                                    }

                                    let btn = egui::Button::new(
                                        RichText::new(crate::i18n::tr("Đăng nhập"))
                                            .color(Color32::WHITE)
                                            .size(14.0)
                                            .strong(),
                                    )
                                    .fill(pal.primary)
                                    .rounding(Rounding::same(8.0))
                                    .min_size(Vec2::new(ui.available_width(), 38.0));

                                    if ui
                                        .add_enabled(!self.login_in_progress, btn)
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        self.login_in_progress = true;
                                        self.login_status_msg = "Đang kết nối…".into();
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::LoginPassword {
                                                phone: self.phone_input.clone(),
                                                pass: self.password_input.clone(),
                                            });
                                        }
                                    }
                                });
                            }
                        }
                    });
                });
        });
    }

    fn render_lock_screen(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let has_pin = self.settings.has_pin();
        let show_hello =
            self.settings.use_windows_hello && self.lock_guard.is_biometric_available();

        let mut submit_pin = false;
        if has_pin && !self.auth_in_progress {
            ctx.input(|i| {
                let has_text_digits = !i.modifiers.ctrl
                    && !i.modifiers.alt
                    && !i.modifiers.command
                    && i.events.iter().any(|ev| {
                        matches!(
                            ev,
                            egui::Event::Text(txt) if txt.chars().any(|c| c.is_ascii_digit())
                        )
                    });

                for ev in &i.events {
                    match ev {
                        egui::Event::Text(txt)
                            if !i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.command =>
                        {
                            for ch in txt.chars().filter(|c| c.is_ascii_digit()) {
                                if self.lock_pin_input.len() < 4 {
                                    self.lock_pin_input.push(ch);
                                    self.lock_pin_error = None;
                                }
                            }
                        }
                        egui::Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            ..
                        } if !modifiers.ctrl && !modifiers.alt && !modifiers.command => match key {
                            Key::Backspace => {
                                self.lock_pin_input.pop();
                                self.lock_pin_error = None;
                            }
                            Key::Enter if self.lock_pin_input.len() == 4 => {
                                submit_pin = true;
                            }
                            _ if !has_text_digits => {
                                if let Some(ch) = match key {
                                    Key::Num0 => Some('0'),
                                    Key::Num1 => Some('1'),
                                    Key::Num2 => Some('2'),
                                    Key::Num3 => Some('3'),
                                    Key::Num4 => Some('4'),
                                    Key::Num5 => Some('5'),
                                    Key::Num6 => Some('6'),
                                    Key::Num7 => Some('7'),
                                    Key::Num8 => Some('8'),
                                    Key::Num9 => Some('9'),
                                    _ => None,
                                } {
                                    if self.lock_pin_input.len() < 4 {
                                        self.lock_pin_input.push(ch);
                                        self.lock_pin_error = None;
                                    }
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            });
            if self.lock_pin_input.len() == 4 {
                submit_pin = true;
            }
        }

        if submit_pin {
            let was_legacy = self
                .settings
                .app_pin_hash
                .as_deref()
                .is_some_and(|h| h.starts_with("sha256$"));
            if self.settings.verify_and_migrate_pin(&self.lock_pin_input) {
                if was_legacy {
                    self.save_settings();
                }
                self.lock_pin_input.clear();
                self.lock_pin_error = None;
                self.lock_guard.unlock();
                crate::desktop_notice::privacy_lock(&self.user_uin, false);
                self.status_msg = "Đã mở khóa ứng dụng".into();
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::RefreshThreads);
                }
                ctx.request_repaint();
                return;
            } else {
                self.lock_pin_input.clear();
                self.lock_pin_error = Some("Mã PIN không đúng. Vui lòng thử lại.".into());
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg_chat_canvas))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let avail_h = ui.available_height();
                    ui.add_space((avail_h * 0.22).max(56.0));

                    // Circular user avatar & account display name
                    let user_disp = if self.user_name.is_empty() {
                        "Người dùng Zalo".to_string()
                    } else {
                        self.user_name.clone()
                    };
                    let (av_rect, _) = ui.allocate_exact_size(Vec2::splat(78.0), Sense::hover());
                    paint_user_avatar(
                        ui.painter(),
                        av_rect.center(),
                        36.0,
                        &user_disp,
                        false,
                        self.user_avatar.as_deref(),
                        &self.sticker_textures,
                        &mut self.loading_stickers,
                    );
                    ui.painter().circle_stroke(
                        av_rect.center(),
                        37.5,
                        Stroke::new(2.0_f32, pal.primary),
                    );
                    ui.add_space(12.0);

                    ui.label(
                        RichText::new(&user_disp)
                            .color(pal.text_primary)
                            .size(20.0)
                            .strong(),
                    );
                    ui.add_space(4.0);

                    let subtitle = if has_pin && show_hello {
                        "Nhập mã PIN 4 số hoặc dùng Windows Hello để mở khóa"
                    } else if has_pin {
                        "Nhập mã PIN 4 số để mở khóa Zalo"
                    } else if show_hello {
                        "Xác thực bằng Windows Hello để tiếp tục"
                    } else if self.settings.use_windows_hello {
                        "Windows Hello chưa sẵn sàng. Bạn có thể đăng xuất và đăng nhập lại để tiếp tục"
                    } else {
                        "Ứng dụng đang bị khóa"
                    };
                    ui.label(
                        RichText::new(crate::i18n::tr(subtitle))
                            .color(pal.text_secondary)
                            .size(13.5),
                    );
                    ui.add_space(22.0);

                    if has_pin {
                        let box_w = 46.0_f32;
                        let box_h = 52.0_f32;
                        let gap = 10.0_f32;
                        let total_w = box_w * 4.0 + gap * 3.0;
                        let (row_rect, _) =
                            ui.allocate_exact_size(Vec2::new(total_w, box_h), Sense::hover());
                        let entered = self.lock_pin_input.len();
                        for idx in 0..4 {
                            let x0 = row_rect.left() + idx as f32 * (box_w + gap);
                            let box_rect = Rect::from_min_size(
                                Pos2::new(x0, row_rect.top()),
                                Vec2::new(box_w, box_h),
                            );
                            let is_active = idx == entered && self.lock_pin_error.is_none();
                            let stroke_col = if self.lock_pin_error.is_some() {
                                pal.danger
                            } else if is_active {
                                pal.primary
                            } else {
                                pal.border
                            };
                            let stroke_w = if is_active || self.lock_pin_error.is_some() {
                                2.0_f32
                            } else {
                                1.0_f32
                            };
                            ui.painter().rect(
                                box_rect,
                                Rounding::same(10.0),
                                pal.bg_input,
                                Stroke::new(stroke_w, stroke_col),
                            );
                            if idx < entered {
                                ui.painter().circle_filled(
                                    box_rect.center(),
                                    6.0,
                                    pal.text_primary,
                                );
                            }
                        }

                        ui.add_space(8.0);
                        if let Some(ref err) = self.lock_pin_error {
                            ui.label(RichText::new(err).color(pal.danger).size(12.5));
                        } else {
                            ui.label(
                                RichText::new(
                                    "Nhập 4 chữ số trên bàn phím (0-9, Backspace, Enter)",
                                )
                                .color(pal.text_muted)
                                .size(11.8),
                            );
                        }
                        ui.add_space(18.0);
                    }

                    if show_hello {
                        let btn = egui::Button::new(
                            RichText::new(if self.auth_in_progress {
                                "Đang xác thực..."
                            } else {
                                "Mở khóa bằng Windows Hello"
                            })
                            .color(Color32::WHITE)
                            .size(14.0)
                            .strong(),
                        )
                        .fill(pal.primary)
                        .rounding(Rounding::same(8.0))
                        .min_size(Vec2::new(240.0, 42.0));

                        if ui
                            .add_enabled(!self.auth_in_progress, btn)
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            let Some(hwnd) = self.native_hwnd else {
                                self.status_msg =
                                    "Không thể xác thực: chưa có cửa sổ Windows hợp lệ".into();
                                return;
                            };
                            self.auth_in_progress = true;
                            self.auth_request_generation =
                                self.auth_request_generation.wrapping_add(1);
                            let request_id = self.auth_request_generation;
                            let runtime_id = self.account_runtime_id;
                            let ctx_clone = ctx.clone();
                            let event_tx = self.event_tx.clone();

                            tokio::spawn(async move {
                                let event = match auth_guard::request_windows_hello(hwnd).await {
                                    Ok(verified) => GuiEvent::AppUnlockCompleted {
                                        request_id,
                                        runtime_id,
                                        verified,
                                    },
                                    Err(error) => GuiEvent::AppUnlockFailed {
                                        request_id,
                                        runtime_id,
                                        message: error.to_string(),
                                    },
                                };
                                let _ = event_tx.send(event);
                                ctx_clone.request_repaint();
                            });
                        }
                        ui.add_space(12.0);
                    }

                    let logout_btn = egui::Button::new(
                        RichText::new("Đăng xuất tài khoản")
                            .color(pal.text_secondary)
                            .size(13.0),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(8.0))
                    .min_size(Vec2::new(240.0, 38.0));

                    if ui
                        .add(logout_btn)
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.status_msg = "Đang đăng xuất…".into();
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::Logout);
                        }
                    }

                    if !self.status_msg.is_empty() && self.status_msg != "Đã đăng nhập · Sẵn sàng"
                    {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(&self.status_msg)
                                .color(pal.text_secondary)
                                .size(12.5),
                        );
                    }
                });
            });
    }

    fn refresh_unread_title(&self, ctx: &egui::Context) {
        let total: usize = self
            .threads
            .iter()
            .filter(|thread| {
                self.private_access(&thread.id, thread.is_group)
                    && self.inbox_placement(thread) != crate::inbox::Placement::StrangerBox
            })
            .map(|thread| self.unread_counts.get(&thread.id).copied().unwrap_or(0))
            .sum();
        #[cfg(windows)]
        crate::desktop_notice::count(&self.user_uin, total);
        if let Some(mailbox) = &self.live_mailbox {
            mailbox.seed(&self.threads);
        }
        let title = if total > 0 {
            format!("({total}) Zalo Lite — Native Rust Client")
        } else {
            "Zalo Lite — Native Rust Client".into()
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    fn show_toast(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        self.status_msg = text.clone();
        self.toast_notice = Some((text, std::time::Instant::now()));
    }
}

#[cfg(all(test, windows, target_arch = "aarch64"))]
mod renderer_tests {
    #[test]
    fn dx12_backend_is_compiled_into_the_arm64_binary() {
        assert!(
            eframe::wgpu::Instance::enabled_backend_features()
                .contains(eframe::wgpu::Backends::DX12),
            "Selecting DX12 at runtime is insufficient: enable the wgpu/dx12 Cargo feature"
        );
    }
}

#[cfg(test)]
mod settings_layout_tests {
    use super::*;
    #[test]
    fn settings_tabs_fit_small_wide_and_high_dpi_viewports() {
        let ctx = egui::Context::default();
        let session = Session::new(false, false, false).unwrap();
        let mut app = ZaloGuiApp::from_context(&ctx, session, false, Vec::new(), false);
        app.is_logged_in = true;
        app.settings_modal_open = true;
        app.settings.download_dir = r"C:\Users\a-long-windows-user-name\Downloads".into();
        app.smoke = Some(UiSmoke {
            path: Default::default(),
            frames: 0,
            lifecycle_started: false,
            sequence: 0,
            shots: 0,
            switch_at: 0,
        });
        for size in [[480.0, 360.0], [1020.0, 700.0], [320.0, 240.0]] {
            for tab in [
                SettingsTab::General,
                SettingsTab::AccountSecurity,
                SettingsTab::Privacy,
                SettingsTab::Sync,
                SettingsTab::Appearance,
                SettingsTab::Notifications,
                SettingsTab::Messages,
                SettingsTab::Utilities,
            ] {
                app.settings_tab = tab;
                for frame in 0..6 {
                    app.smoke.as_mut().unwrap().frames = frame;
                    let input = egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(size[0], size[1]),
                        )),
                        ..Default::default()
                    };
                    let _ = ctx.run(input, |ctx| app.render_settings_modal(ctx, &app.palette()));
                }
            }
        }
    }
}

#[cfg(test)]
mod history_residency_tests {
    use super::*;
    #[test]
    fn conversation_history_requires_the_current_logged_in_runtime() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![thread("selected", 1, 1)],
            false,
        );
        app.is_logged_in = true;
        let runtime = app.account_runtime_id;
        app.status_msg = "current session".into();
        for threads in [vec![thread("stale", 1, 1)], Vec::new()] {
            app.event_tx
                .send(GuiEvent::ConversationHistoryLoaded {
                    runtime_id: runtime.wrapping_add(1),
                    threads,
                    status: "stale status".into(),
                })
                .unwrap();
        }
        app.check_events(&ctx);
        assert!(app.threads.iter().all(|t| t.id != "stale"));
        assert_eq!(app.status_msg, "current session");

        app.is_logged_in = false;
        app.event_tx
            .send(GuiEvent::ConversationHistoryLoaded {
                runtime_id: runtime,
                threads: vec![thread("logged-out", 1, 1)],
                status: "logged-out status".into(),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.threads.iter().all(|t| t.id != "logged-out"));
        assert_eq!(app.status_msg, "current session");

        app.is_logged_in = true;
        app.event_tx
            .send(GuiEvent::ConversationHistoryLoaded {
                runtime_id: runtime,
                threads: vec![thread("current", 1, 1)],
                status: "current history".into(),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.threads.iter().any(|t| t.id == "current"));
        assert_eq!(app.status_msg, "current history");
    }

    fn thread(id: &str, start: i64, count: usize) -> Thread {
        Thread {
            id: id.into(),
            name: id.into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: true,
            presentation: Default::default(),
            unread: Default::default(),
            messages: (0..count)
                .map(|i| ChatMessage {
                    id: format!("m{}", start + i as i64),
                    ts: start + i as i64,
                    text: "Synthetic history".into(),
                    ..Default::default()
                })
                .collect(),
        }
    }
    #[test]
    fn history_pages_do_not_create_unread_and_resident_eviction_preserves_sqlite() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![thread("selected", 1, 1)],
            false,
        );
        app.is_logged_in = true;
        let mut store = crate::store::MessageStore::open(":memory:").unwrap();
        let page = thread("background", 1, 2000);
        crate::store::persist_threads(&mut store, std::slice::from_ref(&page)).unwrap();
        app.event_tx
            .send(GuiEvent::ThreadsUpdated {
                threads: vec![page],
                source: ThreadUpdateSource::History,
            })
            .unwrap();
        app.check_events(&ctx);
        let background = app.threads.iter().find(|t| t.id == "background").unwrap();
        assert_eq!(
            background.messages.len(),
            crate::store::MessageStore::PRELOAD_MSGS
        );
        assert!(background.messages.capacity() <= 40);
        assert!(
            app.unread_counts.is_empty(),
            "Downloading old pages is not live delivery"
        );
        assert_eq!(
            store.messages_for("background", 3000).unwrap().len(),
            2000,
            "Resident eviction must not delete stored history"
        );
        app.event_tx
            .send(GuiEvent::ThreadsUpdated {
                threads: vec![thread("background", 2001, 1)],
                source: ThreadUpdateSource::Live,
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.unread_counts.get("background"), Some(&1));
        app.event_tx
            .send(GuiEvent::ThreadsUpdated {
                threads: vec![thread("background", 2001, 1)],
                source: ThreadUpdateSource::Live,
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(
            app.unread_counts.get("background"),
            Some(&1),
            "Replay must not count twice"
        );
    }
    #[tokio::test]
    async fn queued_previews_cannot_reappear_after_expiry_or_account_change() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        let queue = crate::media_resources::MediaJobQueue::default();
        for (owner, expires_at, key) in [
            (app.user_uin.clone(), Some(1), "expired"),
            ("another-account".into(), None, "foreign"),
        ] {
            app.event_tx
                .send(GuiEvent::PreviewImageReady {
                    owner,
                    expires_at,
                    key: key.into(),
                    image: egui::ColorImage::new([4, 4], Color32::WHITE),
                    permit: queue.acquire().await.unwrap(),
                })
                .unwrap();
        }
        app.check_events(&ctx);
        assert!(!app.sticker_textures.contains_key("expired"));
        assert!(!app.sticker_textures.contains_key("foreign"));
        assert!(
            tokio::time::timeout(Duration::from_secs(1), queue.acquire())
                .await
                .is_ok()
        );
    }
    #[tokio::test]
    async fn paused_gui_keeps_image_decode_permits_until_consumed() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        let queue = crate::media_resources::MediaJobQueue::default();
        for index in 0..crate::media_resources::ACTIVE_JOBS {
            app.event_tx
                .send(GuiEvent::PreviewImageReady {
                    owner: app.user_uin.clone(),
                    expires_at: None,
                    key: format!("image-{index}"),
                    image: egui::ColorImage::new([4, 4], Color32::WHITE),
                    permit: queue.acquire().await.unwrap(),
                })
                .unwrap();
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(10), queue.acquire())
                .await
                .is_err()
        );
        app.check_events(&ctx);
        assert!(
            tokio::time::timeout(Duration::from_secs(1), queue.acquire())
                .await
                .is_ok()
        );
    }
    #[test]
    fn texture_evictions_release_actual_gui_handles() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.texture_budget = crate::media_resources::ResourceLru::new(1024);
        for id in ["first", "second", "third"] {
            app.store_texture(
                &ctx,
                id.into(),
                egui::ColorImage::new([16, 16], Color32::WHITE),
            );
        }
        assert_eq!(app.texture_budget.used(), 1024);
        assert_eq!(app.sticker_textures.len(), 1);
        assert!(app.sticker_textures.contains_key("third"));
    }
    #[test]
    fn selected_history_window_is_bounded_and_can_be_expanded() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![thread("selected", 1, 2000)],
            false,
        );
        app.trim_resident_history();
        assert_eq!(app.threads[0].messages.len(), 500);
        app.threads[0] = thread("selected", 1, 2000);
        app.chat_render_window.1 = 800;
        app.trim_resident_history();
        assert_eq!(app.threads[0].messages.len(), 800);
        assert_eq!(app.threads[0].messages[0].ts, 1201);
    }
}

#[cfg(test)]
mod history_residency_benchmark {
    use super::*;
    #[test]
    #[ignore = "manual synthetic performance measurement"]
    fn measure_history_event_processing() {
        let mut samples = Vec::new();
        let mut resident = 0;
        for _ in 0..20 {
            let ctx = egui::Context::default();
            let selected = Thread {
                id: "selected".into(),
                name: "Selected".into(),
                preview: String::new(),
                time: String::new(),
                is_group: false,
                avatar: None,
                presentation: Default::default(),
                unread: Default::default(),
                messages: Vec::new(),
            };
            let mut app = ZaloGuiApp::from_context(
                &ctx,
                Session::new(false, false, false).unwrap(),
                false,
                vec![selected],
                false,
            );
            app.is_logged_in = true;
            let start = std::time::Instant::now();
            for page in 0..100 {
                let thread = Thread {
                    id: "background".into(),
                    name: "Background".into(),
                    preview: String::new(),
                    time: String::new(),
                    is_group: true,
                    avatar: None,
                    presentation: Default::default(),
                    unread: Default::default(),
                    messages: (0..50)
                        .map(|i| {
                            let n = 5000 - page * 50 - i;
                            ChatMessage {
                                id: format!("msg-{n}"),
                                text: "Synthetic history".into(),
                                ts: n,
                                ..Default::default()
                            }
                        })
                        .collect(),
                };
                app.event_tx
                    .send(GuiEvent::ThreadsUpdated {
                        threads: vec![thread],
                        source: ThreadUpdateSource::History,
                    })
                    .unwrap();
                app.check_events(&ctx);
            }
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            resident = app
                .threads
                .iter()
                .find(|t| t.id == "background")
                .unwrap()
                .messages
                .len();
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({"task":"100_history_pages_50_messages","iterations":samples.len(),"min_ms":samples[0],"avg_ms":samples.iter().sum::<f64>()/samples.len() as f64,"p50_ms":samples[10],"p95_ms":samples[18],"p99_ms":samples[19],"resident_messages":resident,"arch":std::env::consts::ARCH,"os":std::env::consts::OS,"profile":"debug"})
        );
    }
}

#[cfg(test)]
mod conversation_menu_event_tests {
    use super::*;
    #[test]
    fn preferences_reject_stale_account_events_and_survive_history_refresh() {
        let ctx = egui::Context::default();
        let session = Session::new(false, false, false).unwrap();
        let mut app = ZaloGuiApp::from_context(&ctx, session, false, Vec::new(), false);
        app.is_logged_in = true;
        app.user_uin = "fixture-owner".into();
        let (sender, receiver) = channel();
        app.event_rx = receiver;
        let values = HashMap::from([(
            "fixture-thread".into(),
            crate::store::ConversationPreference {
                pinned: true,
                muted: true,
                hidden: true,
            },
        )]);
        sender
            .send(GuiEvent::ConversationPreferences {
                owner: "different-owner".into(),
                values: values.clone(),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.conversation_preferences.is_empty());
        sender
            .send(GuiEvent::ConversationPreferences {
                owner: "fixture-owner".into(),
                values: values.clone(),
            })
            .unwrap();
        sender
            .send(GuiEvent::ThreadsUpdated {
                threads: Vec::new(),
                source: ThreadUpdateSource::History,
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.conversation_preferences, values);
    }
}

#[cfg(test)]
mod send_ack_order_tests {
    use super::*;
    #[test]
    fn sent_ack_keeps_server_clock_and_distinct_identical_sends() {
        let mut thread = Thread {
            messages: vec![
                ChatMessage {
                    id: "server-a".into(),
                    client_message_id: "client-a".into(),
                    text: "ok".into(),
                    is_outgoing: true,
                    ts: 1000,
                    ..Default::default()
                },
                ChatMessage {
                    id: "server-b".into(),
                    client_message_id: "client-b".into(),
                    text: "ok".into(),
                    is_outgoing: true,
                    ts: 2000,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        for id in ["out_client-a", "server-a"] {
            merge_sent_message(
                &mut thread,
                ChatMessage {
                    id: id.into(),
                    client_message_id: "client-a".into(),
                    text: "ok".into(),
                    is_outgoing: true,
                    ts: 3000,
                    ..Default::default()
                },
            );
            assert_eq!(thread.messages.len(), 2);
            assert_eq!(thread.messages[0].id, "server-a");
            assert_eq!(thread.messages[0].ts, 1000);
        }
    }
}

#[cfg(test)]
mod audit_unread_tests {
    use super::*;
    fn app() -> (egui::Context, ZaloGuiApp) {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "active".into(),
                ..Default::default()
            }],
            false,
        );
        app.is_logged_in = true;
        (ctx, app)
    }
    #[test]
    fn first_message_in_new_thread_counts_and_remote_read_clears_its_badge() {
        let (ctx, mut app) = app();
        let queue = std::sync::Arc::new(crate::live_updates::LiveUpdates::default());
        queue.publish(vec![Thread {
            id: "456".into(),
            name: "Fixture".into(),
            messages: vec![ChatMessage {
                id: "11".into(),
                ts: 1700000000000,
                ..Default::default()
            }],
            ..Default::default()
        }]);
        app.event_tx
            .send(GuiEvent::LiveThreadsReady(queue.clone()))
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.unread_counts.get("456"), Some(&1));
        let mut read = Thread {
            id: "456".into(),
            ..Default::default()
        };
        read.unread.advance_seen("11");
        read.unread.fresh = true;
        queue.publish(vec![read]);
        app.event_tx
            .send(GuiEvent::LiveThreadsReady(queue))
            .unwrap();
        app.check_events(&ctx);
        assert!(!app.unread_counts.contains_key("456"));
    }
    #[test]
    fn silent_messages_do_not_increase_existing_thread_unread() {
        let (ctx, mut app) = app();
        app.threads.push(Thread {
            id: "456".into(),
            ..Default::default()
        });
        let mut message = ChatMessage {
            id: "11".into(),
            ts: 1700000000000,
            ..Default::default()
        };
        message.unread_policy = crate::message_model::UnreadPolicy::from_raw(
            &serde_json::json!({"paramsExt":{"countUnread":0}}),
        );
        app.event_tx
            .send(GuiEvent::ThreadsUpdated {
                threads: vec![Thread {
                    id: "456".into(),
                    messages: vec![message],
                    ..Default::default()
                }],
                source: ThreadUpdateSource::Live,
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(!app.unread_counts.contains_key("456"));
    }
    #[test]
    fn coalesced_replay_does_not_turn_cached_messages_into_unread() {
        let (ctx, mut app) = app();
        let message = ChatMessage {
            id: "11".into(),
            ts: 1700000000000,
            ..Default::default()
        };
        app.threads.push(Thread {
            id: "456".into(),
            messages: vec![message.clone()],
            ..Default::default()
        });
        let queue = std::sync::Arc::new(crate::live_updates::LiveUpdates::default());
        for _ in 0..2 {
            queue.publish(vec![Thread {
                id: "456".into(),
                messages: vec![message.clone()],
                ..Default::default()
            }]);
            app.event_tx
                .send(GuiEvent::LiveThreadsReady(queue.clone()))
                .unwrap();
            app.check_events(&ctx);
            assert!(!app.unread_counts.contains_key("456"));
        }
        queue.publish(vec![Thread {
            id: "456".into(),
            messages: vec![ChatMessage {
                id: "12".into(),
                ts: 1700000000001,
                ..Default::default()
            }],
            ..Default::default()
        }]);
        app.event_tx
            .send(GuiEvent::LiveThreadsReady(queue))
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.unread_counts.get("456"), Some(&1));
    }
}

#[cfg(test)]
mod outgoing_ui_tests {
    use super::*;
    #[test]
    fn stale_ordinary_send_cannot_update_relogged_same_account() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "recipient".into(),
                ..Default::default()
            }],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "owner".into();
        app.event_tx
            .send(GuiEvent::OutgoingState {
                owner: "owner".into(),
                runtime_id: app.account_runtime_id.wrapping_add(1),
                thread_id: "recipient".into(),
                message: crate::outgoing::pending(
                    "stale".into(),
                    false,
                    crate::outgoing::Payload::Text,
                ),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.threads[0].messages.is_empty());
    }
    #[test]
    fn failed_bubble_stays_in_its_thread_and_success_removes_retry() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![
                Thread {
                    id: "other".into(),
                    ..Default::default()
                },
                Thread {
                    id: "recipient".into(),
                    ..Default::default()
                },
            ],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "owner".into();
        app.input_message = "new draft in another chat".into();
        let mut failed = crate::outgoing::pending(
            "retained text".into(),
            false,
            crate::outgoing::Payload::Text,
        );
        failed.delivery = crate::message_model::DeliveryState::Failed;
        app.event_tx
            .send(GuiEvent::OutgoingState {
                runtime_id: app.account_runtime_id,
                owner: "owner".into(),
                thread_id: "recipient".into(),
                message: failed.clone(),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.threads[0].messages.is_empty());
        assert_eq!(app.threads[1].messages.len(), 1);
        assert_eq!(app.input_message, "new draft in another chat");
        failed.delivery = crate::message_model::DeliveryState::ServerAccepted;
        failed.id = "server-ack".into();
        failed.outgoing_job = None;
        app.event_tx
            .send(GuiEvent::OutgoingState {
                runtime_id: app.account_runtime_id,
                owner: "owner".into(),
                thread_id: "recipient".into(),
                message: failed,
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.threads[1].messages.len(), 1);
        assert!(app.threads[1].messages[0].outgoing_job.is_none());
        assert_eq!(app.threads[1].messages[0].id, "server-ack");
    }
}

#[cfg(test)]
mod capture_boundary_tests {
    use super::*;

    #[test]
    fn capture_does_not_attach_to_a_different_conversation() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "current-thread".into(),
                ..Default::default()
            }],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "test-account".into();
        app.screen_capture_pending = true;
        app.event_tx
            .send(GuiEvent::ScreenCaptureFinished {
                owner: "test-account".into(),
                thread_id: "previous-thread".into(),
                is_group: false,
                result: Ok((
                    std::env::temp_dir().join(format!(
                        "absent-capture-{}.png",
                        crate::chat::next_client_message_id()
                    )),
                    1,
                    1,
                    egui::ColorImage::new([1, 1], Color32::WHITE),
                )),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.pending_clipboard_image.is_none());
        assert!(!app.screen_capture_pending);
        app.screen_capture_pending = true;
        app.clear_presence_state();
        assert!(!app.screen_capture_pending);
    }
}

#[cfg(test)]
mod v003_viewer_tests {
    use super::*;
    fn text_rects(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    text_rects(shape, out);
                }
            }
            egui::Shape::Text(text) => out.push((
                text.galley.job.text.clone(),
                Rect::from_min_size(text.pos, text.galley.size()),
            )),
            _ => {}
        }
    }
    #[test]
    fn viewer_actions_fit_small_and_large_light_and_dark_windows() {
        for dark in [false, true] {
            for size in [Vec2::new(480.0, 360.0), Vec2::new(1020.0, 700.0)] {
                let ctx = egui::Context::default();
                let message = ChatMessage {
                    id: "photo-1".into(),
                    sender: "Một tên người gửi rất dài để kiểm tra".into(),
                    kind: crate::message_model::MessageKind::Photo,
                    link_url: Some("https://example.invalid/photo.jpg".into()),
                    link_title: Some("Bức ảnh kiểm tra bố cục với tên tệp dài.jpg".into()),
                    ..Default::default()
                };
                let mut app = ZaloGuiApp::from_context(
                    &ctx,
                    Session::new(false, false, false).unwrap(),
                    false,
                    vec![Thread {
                        id: "thread-1".into(),
                        messages: vec![message.clone()],
                        ..Default::default()
                    }],
                    false,
                );
                app.dark_mode = dark;
                app.is_logged_in = true;
                let pal = app.palette();
                pal.apply_to_ctx(&ctx);
                let key = crate::media_recovery::image_key("thread-1", "photo-1", 0, true);
                app.store_texture(&ctx, key, egui::ColorImage::new([80, 60], Color32::BLUE));
                app.open_media_viewer("thread-1", &message);
                let mut last = Vec::new();
                for _ in 0..3 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                            ..Default::default()
                        },
                        |ctx| app.render_image_viewer_modal(ctx, &pal),
                    );
                    last.clear();
                    for shape in output.shapes {
                        text_rects(&shape.shape, &mut last);
                    }
                }

                // WP15-4: Document viewer modal (Task 4c)
                if app.doc_viewer.is_some() {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                            ..Default::default()
                        },
                        |ctx| app.render_doc_viewer_modal(ctx, &pal),
                    );
                    last.clear();
                    for shape in output.shapes {
                        text_rects(&shape.shape, &mut last);
                    }
                }
                for label in [
                    "Trước",
                    "Tiếp",
                    "Lưu thành…",
                    "Sao chép ảnh",
                    "Sao chép link",
                    "Đóng ×",
                    "Vừa cửa sổ",
                ] {
                    let (_, rect) = last
                        .iter()
                        .find(|(text, _)| text == label)
                        .unwrap_or_else(|| panic!("missing {label}"));
                    assert!(
                        rect.left() >= 0.0
                            && rect.right() <= size.x + 1.0
                            && rect.top() >= 0.0
                            && rect.bottom() < size.y,
                        "{label}: {rect:?} in {size:?}"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod multidevice_call_tests {
    use super::*;
    #[test]
    fn remote_answer_dismisses_only_the_matching_pending_account_call() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "account-a".into();
        app.event_tx
            .send(GuiEvent::IncomingCall {
                owner: "account-a".into(),
                call_id: 12,
                caller_id: "123".into(),
                caller_name: "Test".into(),
                is_video: false,
                is_group: false,
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.active_call.as_ref().unwrap().call_id, Some(12));
        for (owner, id) in [("account-b", 12), ("account-a", 13)] {
            app.event_tx
                .send(GuiEvent::CallAnsweredElsewhere {
                    owner: owner.into(),
                    call_id: id,
                })
                .unwrap();
            app.check_events(&ctx);
            assert!(app.active_call.is_some());
        }
        app.event_tx
            .send(GuiEvent::CallAnsweredElsewhere {
                owner: "account-a".into(),
                call_id: 12,
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.active_call.is_none());
        app.event_tx
            .send(GuiEvent::IncomingCall {
                owner: "account-b".into(),
                call_id: 14,
                caller_id: "123".into(),
                caller_name: "Old account".into(),
                is_video: false,
                is_group: false,
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.active_call.is_none());
    }
    #[test]
    fn remote_answer_does_not_close_a_locally_accepted_call() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "account-a".into();
        app.event_tx
            .send(GuiEvent::IncomingCall {
                owner: "account-a".into(),
                call_id: 12,
                caller_id: "123".into(),
                caller_name: "Test".into(),
                is_video: false,
                is_group: false,
            })
            .unwrap();
        app.check_events(&ctx);
        app.active_call.as_mut().unwrap().is_incoming_ringing = false;
        app.event_tx
            .send(GuiEvent::CallAnsweredElsewhere {
                owner: "account-a".into(),
                call_id: 12,
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.active_call.is_some());
    }
}

#[cfg(test)]
mod v008_gui_boundary_tests {
    use super::*;
    fn app(ctx: &egui::Context) -> ZaloGuiApp {
        let mut app = ZaloGuiApp::from_context(
            ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "account-a".into();
        app
    }
    fn incoming(id: i64) -> GuiEvent {
        GuiEvent::IncomingCall {
            owner: "account-a".into(),
            call_id: id,
            caller_id: "123".into(),
            caller_name: "Test".into(),
            is_video: false,
            is_group: false,
        }
    }
    #[test]
    fn terminal_before_invite_cannot_reopen_the_same_call_but_a_new_id_can_ring() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.event_tx
            .send(GuiEvent::CallAnsweredElsewhere {
                owner: "account-a".into(),
                call_id: 12,
            })
            .unwrap();
        app.event_tx.send(incoming(12)).unwrap();
        app.check_events(&ctx);
        assert!(app.active_call.is_none());
        app.event_tx.send(incoming(13)).unwrap();
        app.check_events(&ctx);
        assert_eq!(app.active_call.as_ref().unwrap().call_id, Some(13));
        app.event_tx
            .send(GuiEvent::CallStateChanged {
                owner: "account-a".into(),
                call_id: 12,
                state: "free".into(),
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.active_call.as_ref().unwrap().call_id, Some(13));
    }
    #[test]
    fn duplicate_invite_does_not_reset_a_connected_overlay() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.event_tx.send(incoming(12)).unwrap();
        app.check_events(&ctx);
        app.event_tx
            .send(GuiEvent::CallStateChanged {
                owner: "account-a".into(),
                call_id: 12,
                state: "connected".into(),
            })
            .unwrap();
        app.check_events(&ctx);
        app.event_tx.send(incoming(12)).unwrap();
        app.check_events(&ctx);
        assert!(!app.active_call.as_ref().unwrap().is_incoming_ringing);
    }
    #[test]
    fn cancelling_folder_picker_preserves_the_previous_configuration() {
        let ctx = egui::Context::default();
        let mut app = app(&ctx);
        app.settings.download_dir = "old-directory".into();
        app.download_picker_pending = true;
        app.event_tx
            .send(GuiEvent::DownloadFolderPicked(None))
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.settings.download_dir, "old-directory");
        assert!(!app.download_picker_pending);
    }
}

#[cfg(test)]
mod v008_dialog_layout_tests {
    use super::*;
    #[test]
    fn avatar_history_results_are_account_scoped_and_do_not_clear_upload_uncertainty() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "123".into();
        app.avatar_panel.generation = 9;
        app.avatar_panel.pending = true;
        app.avatar_panel.uncertain = true;
        app.avatar_panel.history_uncertain = true;
        let reply = || crate::avatar_history::Reply {
            page: Some(
                crate::avatar_history::parse_page(
                    serde_json::json!({"albumId":"0","hasMore":false,"photos":[]}),
                    1,
                    "",
                )
                .unwrap(),
            ),
            ..Default::default()
        };
        for (owner, generation) in [("456", 9), ("123", 8)] {
            app.event_tx
                .send(GuiEvent::AvatarHistory {
                    owner: owner.into(),
                    generation,
                    mutation: false,
                    result: Ok(reply()),
                })
                .unwrap();
            app.check_events(&ctx);
            assert!(app.avatar_panel.pending);
            assert!(app.avatar_panel.history.is_none());
        }
        app.event_tx
            .send(GuiEvent::AvatarHistory {
                owner: "123".into(),
                generation: 9,
                mutation: false,
                result: Ok(reply()),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(!app.avatar_panel.pending);
        assert!(app.avatar_panel.history.is_some());
        assert!(app.avatar_panel.uncertain);
        assert!(!app.avatar_panel.history_uncertain);
    }

    #[test]
    fn new_management_dialogs_stay_inside_small_light_and_dark_viewports() {
        for dark in [false, true] {
            for size in [Vec2::new(480.0, 360.0), Vec2::new(320.0, 240.0)] {
                for dialog in [
                    "group-panel",
                    "contact-actions",
                    "server-labels",
                    "own-profile",
                    "avatar-review",
                    "group-entry",
                ] {
                    let ctx = egui::Context::default();
                    let mut app = ZaloGuiApp::from_context(
                        &ctx,
                        Session::new(false, false, false).unwrap(),
                        false,
                        vec![],
                        false,
                    );
                    app.is_logged_in = true;
                    app.user_uin = "123".into();
                    app.dark_mode = dark;
                    app.palette().apply_to_ctx(&ctx);
                    app.groups_panel.open = true;
                    app.groups_panel.create = false;
                    app.groups_panel.group = true;
                    app.groups_panel.target = "g999".into();
                    app.groups_panel.info = Some(
                        serde_json::json!({"creatorId":"123","type":1,"setting":{"blockName":0,"lockCreatePost":0,"lockSendMsg":0,"lockCreatePoll":0,"joinAppr":0}}),
                    );
                    app.groups_panel.link = Some(
                        serde_json::json!({"enabled":true,"link":"https://zalo.me/g/fixture123","expiration_date":4102444800000_i64}),
                    );
                    app.groups_panel.link_qr = Some(ctx.load_texture(
                        "fixture-qr",
                        egui::ColorImage::new([16, 16], Color32::WHITE),
                        Default::default(),
                    ));
                    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
                    app.contact_panel.open = true;
                    app.contact_panel.target = "456".into();
                    app.labels_panel.open = true;
                    app.profile_panel.open = true;
                    app.profile_panel.loaded = true;
                    app.profile_panel.name = "Tên hồ sơ kiểm thử".into();
                    app.profile_panel.dob = "2000-01-01".into();
                    app.profile_panel.gender = Some(0);
                    app.profile_panel.confirm = true;
                    app.avatar_panel.open = true;
                    app.avatar_panel.target = Some(crate::avatar::Target::Own);
                    app.avatar_panel.history=Some(crate::avatar_history::parse_page(serde_json::json!({"albumId":"0","hasMore":false,"photos":[{"photoId":"321"}]}),1,"").unwrap());

                    app.group_entry_panel.open = true;
                    app.avatar_panel.texture = Some(ctx.load_texture(
                        "fixture-avatar",
                        egui::ColorImage::new([16, 16], Color32::WHITE),
                        Default::default(),
                    ));
                    for _ in 0..5 {
                        let _ = ctx.run(
                            egui::RawInput {
                                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                                ..Default::default()
                            },
                            |ctx| match dialog {
                                "group-panel" => app.render_group_panel(ctx),
                                "contact-actions" => app.render_contact_panel(ctx),
                                "server-labels" => app.render_labels_panel(ctx),
                                "own-profile" => app.render_own_profile(ctx),
                                "avatar-review" => app.render_avatar_panel(ctx),
                                "group-entry" => app.render_group_entry(ctx),
                                _ => unreachable!(),
                            },
                        );
                    }
                    let rect = ctx
                        .memory(|memory| memory.area_rect(Id::new(dialog)))
                        .unwrap();
                    assert!(
                        Rect::from_min_size(Pos2::ZERO, size)
                            .expand(1.0)
                            .contains_rect(rect),
                        "{dialog} outside {size:?}: {rect:?}"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod v008_account_boundary_tests {
    use super::*;
    fn app() -> (egui::Context, ZaloGuiApp) {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![
                Thread {
                    id: "12".into(),
                    name: "Tên gốc".into(),
                    ..Default::default()
                },
                Thread {
                    id: "group_34".into(),
                    is_group: true,
                    presentation: crate::chat::SidebarPresentation {
                        send_policy: Some(crate::chat::GroupSendPolicy {
                            locked: false,
                            managers: vec![],
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "99".into();
        (ctx, app)
    }
    #[test]
    fn hidden_relock_invalidates_search_and_rejects_an_older_unlock_result() {
        let (ctx, mut app) = app();
        app.history_search.open = true;
        let generation = app.history_search.request.generation;
        app.hidden_panel.view = crate::hidden_chat::Snapshot {
            revision: 10,
            ready: true,
            unlocked: true,
            ids: std::collections::HashSet::from(["u12".into()]),
            ..Default::default()
        };
        app.event_tx
            .send(GuiEvent::Hidden {
                owner: "99".into(),
                generation: None,
                view: crate::hidden_chat::Snapshot {
                    revision: 11,
                    ready: true,
                    ids: std::collections::HashSet::from(["u12".into()]),
                    ..Default::default()
                },
                result: Ok(()),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(!app.private_access("12", false));
        assert!(!app.history_search.open);
        assert_ne!(generation, app.history_search.request.generation);
        app.event_tx
            .send(GuiEvent::Hidden {
                owner: "99".into(),
                generation: None,
                view: crate::hidden_chat::Snapshot {
                    revision: 10,
                    ready: true,
                    unlocked: true,
                    ..Default::default()
                },
                result: Ok(()),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(!app.private_access("12", false));
    }
    #[test]
    fn group_control_aliases_clear_permissions_and_ignore_an_old_policy_response() {
        let (ctx, mut app) = app();
        app.event_tx
            .send(GuiEvent::GroupInvalidated {
                owner: "99".into(),
                revision: 7,
                groups: HashSet::from(["g34".into()]),
                all: false,
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.threads[1].presentation.send_policy.is_none());
        app.event_tx
            .send(GuiEvent::GroupPolicy {
                owner: "99".into(),
                group_id: "group_34".into(),
                revision: 6,
                result: Ok(crate::chat::GroupSendPolicy {
                    locked: false,
                    managers: vec![],
                }),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(app.threads[1].presentation.send_policy.is_none());
        app.event_tx
            .send(GuiEvent::GroupPolicy {
                owner: "99".into(),
                group_id: "group_34".into(),
                revision: 7,
                result: Ok(crate::chat::GroupSendPolicy {
                    locked: true,
                    managers: vec![],
                }),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(
            app.threads[1]
                .presentation
                .send_policy
                .as_ref()
                .unwrap()
                .locked
        );
    }
    #[test]
    fn local_call_records_reject_logged_out_and_other_accounts() {
        let (ctx, mut app) = app();
        let thread_id = app.threads[0].id.clone();
        let initial = app.threads[0].messages.len();
        for (logged_in, owner) in [(true, "88"), (false, "99")] {
            app.is_logged_in = logged_in;
            app.event_tx
                .send(GuiEvent::LocalCallRecord {
                    runtime_id: app.account_runtime_id,
                    outgoing: true,
                    owner: owner.into(),
                    thread_id: thread_id.clone(),
                    text: "Cuộc gọi".into(),
                    msg_id: "call_fixture".into(),
                })
                .unwrap();
            app.check_events(&ctx);
            assert_eq!(app.threads[0].messages.len(), initial);
        }
    }

    #[test]
    fn message_pin_snapshots_reject_stale_thread_generation_and_account() {
        let (ctx, mut app) = app();
        app.message_pins = board_panel::MessagePins {
            thread: "group_34".into(),
            generation: 20,
            pending: true,
            ..Default::default()
        };
        for (owner, thread, generation) in [
            ("88", "group_34", 20),
            ("99", "group_35", 20),
            ("99", "group_34", 19),
        ] {
            app.event_tx
                .send(GuiEvent::MessagePinsLoaded {
                    owner: owner.into(),
                    thread: thread.into(),
                    generation,
                    result: Ok(Default::default()),
                })
                .unwrap();
            app.check_events(&ctx);
            assert!(app.message_pins.pending);
            assert!(app.message_pins.page.is_none());
        }
        app.event_tx
            .send(GuiEvent::MessagePinsLoaded {
                owner: "99".into(),
                thread: "group_34".into(),
                generation: 20,
                result: Ok(crate::board::Page {
                    items: vec![
                        crate::board::Item {
                            id: "topic1".into(),
                            title: "One".into(),
                            start: 0,
                            creator: String::new(),
                            raw: serde_json::json!({}),
                            message_id: "msg1".into(),
                            repeating: false,
                        },
                        crate::board::Item {
                            id: "topic2".into(),
                            title: "Duplicate".into(),
                            start: 0,
                            creator: String::new(),
                            raw: serde_json::json!({}),
                            message_id: "msg1".into(),
                            repeating: false,
                        },
                    ],
                    ..Default::default()
                }),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(!app.message_pins.pending);
        assert_eq!(app.message_pins.page.as_ref().unwrap().items.len(), 1);
    }

    #[test]
    fn alias_updates_and_removal_restore_the_original_name_without_changing_identity() {
        let (_, mut app) = app();
        app.server_aliases_ready = true;
        app.server_aliases
            .insert("12".into(), "Biệt danh 🎉".into());
        app.apply_server_aliases();
        assert_eq!(app.threads[0].name, "Biệt danh 🎉");
        app.server_aliases.insert("12".into(), "Tên mới".into());
        app.apply_server_aliases();
        app.server_aliases.clear();
        app.apply_server_aliases();
        assert_eq!(app.threads[0].name, "Tên gốc");
        assert_eq!(app.threads[0].id, "12");
    }
    #[test]
    fn a_contact_alias_cannot_impersonate_the_cloud_exception() {
        let (_, mut app) = app();
        app.inbox_policy = crate::inbox::Policy {
            friends: Some(Default::default()),
            business: Some(false),
            direct: Some(false),
            ..Default::default()
        };
        app.own_cloud_id = "55".into();
        let thread = &mut app.threads[0];
        thread.name = "Cloud của tôi".into();
        thread.presentation.inbox = crate::inbox::Metadata {
            responded_by_me: Some(false),
            outside: Some(0),
            official_account: Some(false),
        };
        assert_eq!(
            app.inbox_placement(&app.threads[0]),
            crate::inbox::Placement::StrangerBox
        );
    }
    #[test]
    fn a_rejection_for_an_old_call_cannot_relabel_a_new_incoming_call() {
        let (ctx, mut app) = app();
        app.event_tx
            .send(GuiEvent::IncomingCall {
                owner: "99".into(),
                call_id: 100,
                caller_id: "12".into(),
                caller_name: "Tên".into(),
                is_video: false,
                is_group: false,
            })
            .unwrap();
        app.check_events(&ctx);
        let old = app.active_call.as_ref().unwrap().state_label.clone();
        for (owner, id) in [("wrong", 100), ("99", 99)] {
            app.event_tx
                .send(GuiEvent::CallRejected {
                    owner: owner.into(),
                    partner_id: "12".into(),
                    call_id: Some(id),
                    code: 1,
                    reason: "Bận".into(),
                })
                .unwrap();
            app.check_events(&ctx);
            assert_eq!(app.active_call.as_ref().unwrap().state_label, old);
        }
        app.event_tx
            .send(GuiEvent::CallRejected {
                owner: "99".into(),
                partner_id: "12".into(),
                call_id: Some(100),
                code: 1,
                reason: "Bận".into(),
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.active_call.as_ref().unwrap().error_code, Some(1));
    }
}

#[cfg(test)]
mod v008_relock_regression_tests {
    use super::*;
    #[test]
    fn a_normal_refresh_of_a_locked_list_does_not_close_unrelated_search() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "99".into();
        app.hidden_panel.view = crate::hidden_chat::Snapshot {
            ready: true,
            revision: 4,
            ids: HashSet::from(["u12".into()]),
            ..Default::default()
        };
        app.history_search.open = true;
        let generation = app.history_search.request.generation;
        let mut view = app.hidden_panel.view.clone();
        view.revision = 5;
        app.apply_hidden_view(view);
        assert!(app.history_search.open);
        assert_eq!(app.history_search.request.generation, generation);
    }
    #[test]
    fn cached_aliases_are_not_removed_before_a_complete_alias_snapshot_is_available() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "12".into(),
                name: "Biệt danh 🎉".into(),
                presentation: crate::chat::SidebarPresentation {
                    server_alias: Some("Biệt danh 🎉".into()),
                    base_name: Some("Tên gốc".into()),
                    ..Default::default()
                },
                ..Default::default()
            }],
            false,
        );
        app.apply_server_aliases();
        assert_eq!(app.threads[0].name, "Biệt danh 🎉");
        app.server_aliases_ready = true;
        app.apply_server_aliases();
        assert_eq!(app.threads[0].name, "Tên gốc");
    }
    #[test]
    fn resolve_contact_name_and_threads_updated_prioritize_server_aliases() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "12".into(),
                name: "Tên Zalo".into(),
                ..Default::default()
            }],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "99".into();
        app.server_aliases_ready = true;
        app.server_aliases.insert("12".into(), "Mẹ Yêu".into());
        app.server_aliases.insert("13".into(), "   ".into());
        assert_eq!(app.resolve_contact_name("12", "Tên Zalo"), "Mẹ Yêu");
        assert_eq!(app.resolve_contact_name("13", "Fallback"), "Fallback");
        assert_eq!(app.resolve_thread_name(&app.threads[0]), "Mẹ Yêu");

        let msg = ChatMessage {
            id: "m1".into(),
            sender: "Tên Zalo".into(),
            wire_ref: crate::message_model::WireMessageRef {
                sender: Some(serde_json::json!("12")),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            app.resolve_message_sender_name(&msg, "g1", true, "Nhóm"),
            "Mẹ Yêu"
        );

        app.event_tx
            .send(GuiEvent::ThreadsUpdated {
                threads: vec![Thread {
                    id: "12".into(),
                    name: "Tên Zalo Mới".into(),
                    ..Default::default()
                }],
                source: ThreadUpdateSource::Live,
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.threads[0].name, "Mẹ Yêu");
        assert_eq!(
            app.threads[0].presentation.base_name.as_deref(),
            Some("Tên Zalo Mới")
        );
    }
}

#[cfg(test)]
mod audit_lock_tests {
    use super::*;
    #[test]
    fn hidden_relock_drops_message_details_and_destructive_snapshot() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![],
            false,
        );
        app.hidden_panel.view = crate::hidden_chat::Snapshot {
            ready: true,
            unlocked: true,
            ..Default::default()
        };
        app.message_details = Some((
            "12".into(),
            ChatMessage {
                text: "private".into(),
                ..Default::default()
            },
        ));
        app.archive_mutation_confirm = Some(workspace::ArchiveMutation {
            thread: "12".into(),
            group: false,
            recall: false,
            messages: vec![],
        });
        app.apply_hidden_view(crate::hidden_chat::Snapshot {
            ready: true,
            unlocked: false,
            ..Default::default()
        });
        assert!(app.message_details.is_none());
        assert!(app.archive_mutation_confirm.is_none());
    }
    #[test]
    fn upload_progress_targets_pending_attachment_and_rejects_stale_scope() {
        let ctx = egui::Context::default();
        let mut first = crate::outgoing::pending(
            "[Tệp] one.pdf".into(),
            false,
            crate::outgoing::Payload::Attachment {
                path: "one.pdf".into(),
                caption: String::new(),
                stamp: None,
                hd_source: None,
            },
        );
        first.client_message_id = "101".into();
        let mut second = first.clone();
        second.client_message_id = "102".into();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "thread-a".into(),
                messages: vec![first, second],
                ..Default::default()
            }],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "owner-a".into();
        let runtime = app.account_runtime_id;
        for (owner, runtime_id, completed) in [
            ("other", runtime, 10),
            ("owner-a", runtime.wrapping_add(1), 20),
            ("owner-a", runtime, 150),
        ] {
            app.event_tx
                .send(GuiEvent::UploadProgress {
                    owner: owner.into(),
                    runtime_id,
                    upload_id: "asyncfile-101".into(),
                    completed,
                    total: 100,
                })
                .unwrap();
            app.check_events(&ctx);
            if owner != "owner-a" || runtime_id != runtime {
                assert!(app.threads[0].messages[0].upload_progress.is_none());
            }
        }
        assert_eq!(
            app.threads[0].messages[0].upload_progress,
            Some(("asyncfile-101".into(), 100, 100))
        );
        assert!(app.threads[0].messages[1].upload_progress.is_none());
        assert_eq!(
            app.threads[0].messages[0].delivery,
            crate::message_model::DeliveryState::Pending
        );
        let encoded = serde_json::to_value(&app.threads[0].messages[0]).unwrap();
        assert!(encoded.get("upload_progress").is_none());
        app.threads[0].messages[0].delivery = crate::message_model::DeliveryState::ServerAccepted;
        app.event_tx
            .send(GuiEvent::UploadProgress {
                owner: "owner-a".into(),
                runtime_id: runtime,
                upload_id: "asyncfile-101".into(),
                completed: 1,
                total: 100,
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(
            app.threads[0].messages[0]
                .upload_progress
                .as_ref()
                .unwrap()
                .1,
            100
        );
    }

    #[test]
    fn locked_app_updates_business_state_without_unlocking() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        let (tx, rx) = channel();
        app.event_rx = rx;
        app.lock_guard.lock();
        app.is_logged_in = true;
        tx.send(GuiEvent::UploadProgress {
            owner: app.user_uin.clone(),
            runtime_id: app.account_runtime_id,
            upload_id: "upload".into(),
            completed: 42,
            total: 100,
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.lock_guard.is_locked());
        assert_eq!(app.status_msg, "Tải lên: 42/100 bytes");
    }
    #[test]
    fn safety_result_clears_stale_verification_and_rejects_another_owner() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.user_uin = "owner-a".into();
        app.is_logged_in = true;
        let (tx, rx) = channel();
        app.event_rx = rx;
        app.trusted_peers.insert("peer".into());
        tx.send(GuiEvent::SafetyNumberLoaded {
            owner: "owner-a".into(),
            peer_id: "peer".into(),
            peer_name: "Peer".into(),
            safety_number: "number".into(),
            is_verified: false,
            identity_changed: true,
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(!app.trusted_peers.contains("peer"));
        tx.send(GuiEvent::IdentityTrusted {
            owner: "owner-b".into(),
            peer_id: "peer".into(),
            is_trusted: true,
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(!app.trusted_peers.contains("peer"));
    }

    #[test]
    fn drawer_view_default_is_overview() {
        let ctx = egui::Context::default();
        let app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        assert_eq!(DrawerView::default(), DrawerView::Overview);
        assert_eq!(MediaTab::default(), MediaTab::Photos);
        assert_eq!(app.drawer_view, DrawerView::Overview);
        assert!(app.drawer_search_query.is_empty());
    }

    #[test]
    fn drawer_view_navigation_transitions() {
        let mut view = DrawerView::Overview;
        assert_eq!(view, DrawerView::Overview);

        view = DrawerView::Members;
        assert_eq!(view, DrawerView::Members);

        view = DrawerView::Board;
        assert_eq!(view, DrawerView::Board);

        view = DrawerView::MediaStorage {
            tab: MediaTab::Photos,
        };
        assert_eq!(
            view,
            DrawerView::MediaStorage {
                tab: MediaTab::Photos
            }
        );

        view = DrawerView::MediaStorage {
            tab: MediaTab::Files,
        };
        assert_eq!(
            view,
            DrawerView::MediaStorage {
                tab: MediaTab::Files
            }
        );

        view = DrawerView::MediaStorage {
            tab: MediaTab::Links,
        };
        assert_eq!(
            view,
            DrawerView::MediaStorage {
                tab: MediaTab::Links
            }
        );

        view = DrawerView::SearchInThread;
        assert_eq!(view, DrawerView::SearchInThread);

        view = DrawerView::SecuritySettings;
        assert_eq!(view, DrawerView::SecuritySettings);

        view = DrawerView::Overview;
        assert_eq!(view, DrawerView::Overview);
    }

    #[test]
    fn open_thread_resets_drawer_view_to_overview_and_clears_search() {
        let ctx = egui::Context::default();
        let thread_a = Thread {
            id: "thread-a".into(),
            name: "Hội thoại A".into(),
            is_group: false,
            ..Default::default()
        };
        let thread_b = Thread {
            id: "thread-b".into(),
            name: "Hội thoại B".into(),
            is_group: false,
            ..Default::default()
        };
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![thread_a, thread_b],
            false,
        );

        app.selected_thread_index = 0;
        app.drawer_view = DrawerView::MediaStorage {
            tab: MediaTab::Files,
        };
        app.drawer_search_query = "tài liệu kế toán".into();
        app.group_member_search = "thành viên ABC".into();

        // Switch to thread 1
        app.open_thread(&ctx, 1, "thread-b".into(), false);

        assert_eq!(app.selected_thread_index, 1);
        assert_eq!(app.drawer_view, DrawerView::Overview);
        assert!(app.drawer_search_query.is_empty());
        assert!(app.group_member_search.is_empty());
    }

    #[test]
    fn drawer_view_headless_render_smoke_test() {
        let ctx = egui::Context::default();
        let thread_msg = ChatMessage {
            id: "msg-1".into(),
            sender: "User".into(),
            text: "Xin chào bạn".into(),
            time: "10:30".into(),
            is_outgoing: false,
            ts: 1700000000000,
            ..Default::default()
        };
        let thread_group = Thread {
            id: "group-1".into(),
            name: "Nhóm dự án".into(),
            is_group: true,
            messages: vec![thread_msg],
            ..Default::default()
        };
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![thread_group],
            false,
        );
        app.is_logged_in = true;
        app.info_drawer_open = true;

        for view in [
            DrawerView::Overview,
            DrawerView::Members,
            DrawerView::Board,
            DrawerView::MediaStorage {
                tab: MediaTab::Photos,
            },
            DrawerView::MediaStorage {
                tab: MediaTab::Files,
            },
            DrawerView::MediaStorage {
                tab: MediaTab::Links,
            },
            DrawerView::SearchInThread,
            DrawerView::SecuritySettings,
        ] {
            app.drawer_view = view;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1024.0, 768.0))),
                    ..Default::default()
                },
                |ctx| {
                    app.render_chat_workspace(ctx, &app.palette());
                },
            );
        }
    }
}

#[cfg(test)]
mod voice_draft_boundary_tests {
    use super::*;

    fn app_with_voice_draft() -> ZaloGuiApp {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "thread-x".into(),
                is_group: false,
                ..Default::default()
            }],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "owner-a".into();
        let runtime_id = app.account_runtime_id;
        app.voice_binding = Some(VoiceDraftBinding {
            owner: "owner-a".into(),
            runtime_id,
            thread_id: "thread-x".into(),
            is_group: false,
            generation: 0,
        });
        app.recorded_voice_samples = Some((
            std::sync::Arc::from(vec![0.25, -0.25]),
            16_000,
            Duration::from_secs(1),
        ));
        app
    }

    #[test]
    fn voice_draft_is_bound_to_owner_thread_kind_and_generation() {
        let app = app_with_voice_draft();

        let runtime_id = app.account_runtime_id;
        assert!(app.voice_draft_matches("owner-a", runtime_id, "thread-x", false));
        assert!(!app.voice_draft_matches("owner-b", runtime_id, "thread-x", false));
        assert!(!app.voice_draft_matches("owner-a", runtime_id, "thread-y", false));
        assert!(!app.voice_draft_matches("owner-a", runtime_id, "thread-x", true));
        assert!(!app.voice_draft_matches("owner-a", runtime_id.wrapping_add(1), "thread-x", false,));
        assert!(!app
            .voice_binding
            .as_ref()
            .unwrap()
            .matches("owner-a", runtime_id, "thread-x", false, 1,));
        assert!(!app
            .voice_binding
            .as_ref()
            .unwrap()
            .matches("owner-a", runtime_id, "thread-x", false, 2));
    }

    #[test]
    fn changing_thread_cancels_synthetic_audio_before_it_can_be_sent() {
        let mut app = app_with_voice_draft();
        app.threads.push(Thread {
            id: "thread-y".into(),
            is_group: false,
            ..Default::default()
        });
        app.selected_thread_index = 1;
        app.cancel_voice_if_context_changed();

        assert!(app.recorded_voice_samples.is_none());
        assert!(app.voice_binding.is_none());
        assert_eq!(app.voice_context_generation, 1);

        app.voice_binding = Some(VoiceDraftBinding {
            owner: app.user_uin.clone(),
            runtime_id: app.account_runtime_id,
            thread_id: "thread-x".into(),
            is_group: false,
            generation: app.voice_context_generation,
        });
        app.recorded_voice_samples = Some((
            std::sync::Arc::from(vec![0.25, -0.25]),
            16_000,
            Duration::from_secs(1),
        ));
        app.send_voice_recording(&egui::Context::default(), "thread-y", false);

        assert!(app.recorded_voice_samples.is_none());
        assert!(app.voice_binding.is_none());
        assert_eq!(app.voice_context_generation, 2);
        assert_eq!(
            app.status_msg,
            "Bản ghi âm đã bị hủy do tài khoản hoặc hội thoại thay đổi"
        );
    }

    #[test]
    fn logged_out_owner_invalidates_an_existing_audio_draft() {
        let mut app = app_with_voice_draft();
        app.cancel_voice_recording();
        app.is_logged_in = false;
        app.user_uin.clear();

        assert!(app.recorded_voice_samples.is_none());
        assert!(app.voice_binding.is_none());
        assert_eq!(app.voice_context_generation, 1);
    }

    #[test]
    fn stale_voice_worker_events_do_not_update_a_relogged_same_uid() {
        let ctx = egui::Context::default();
        let mut app = app_with_voice_draft();
        let old_runtime = app.account_runtime_id;
        app.account_runtime_id = old_runtime.wrapping_add(1);
        app.status_msg = "current session".into();
        let (tx, rx) = channel();
        app.event_rx = rx;
        tx.send(GuiEvent::VoiceOutgoingState {
            owner: "owner-a".into(),
            runtime_id: old_runtime,
            thread_id: "thread-x".into(),
            message: ChatMessage {
                client_message_id: "old-voice".into(),
                text: "[Tin nhắn thoại]".into(),
                is_outgoing: true,
                ..Default::default()
            },
        })
        .unwrap();
        tx.send(GuiEvent::VoiceSendStatus {
            owner: "owner-a".into(),
            runtime_id: old_runtime,
            message: "Đã gửi tin nhắn thoại".into(),
        })
        .unwrap();

        app.check_events(&ctx);

        assert!(app.threads[0].messages.is_empty());
        assert_eq!(app.status_msg, "current session");
    }

    #[test]
    fn stale_or_cross_runtime_hello_completion_cannot_unlock_app() {
        let ctx = egui::Context::default();
        let mut app = app_with_voice_draft();
        let runtime_id = app.account_runtime_id;
        app.lock_guard.lock();
        app.auth_in_progress = true;
        app.auth_request_generation = 9;
        let (tx, rx) = channel();
        app.event_rx = rx;

        tx.send(GuiEvent::AppUnlockCompleted {
            request_id: 8,
            runtime_id,
            verified: true,
        })
        .unwrap();
        tx.send(GuiEvent::AppUnlockCompleted {
            request_id: 9,
            runtime_id: runtime_id.wrapping_add(1),
            verified: true,
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.lock_guard.is_locked());
        assert!(app.auth_in_progress);

        tx.send(GuiEvent::AppUnlockCompleted {
            request_id: 9,
            runtime_id,
            verified: true,
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(!app.lock_guard.is_locked());
        assert!(!app.auth_in_progress);
    }

    #[test]
    fn app_settings_default_disables_lock_and_verifies_salted_pin() {
        let mut settings = AppSettings::default();
        assert!(settings.call_notifications_enabled);
        assert!(settings.selected_speaker_device.is_empty());
        assert!(!settings.enable_app_lock);
        assert!(!settings.use_windows_hello);
        assert!(settings.app_pin_hash.is_none());
        assert!(!settings.has_pin());
        assert!(!settings.lock_configured());
        assert!(!settings.verify_pin("1234"));

        assert!(settings.set_pin("12a4").is_err());
        assert!(settings.set_pin("123").is_err());
        assert!(settings.set_pin("12345").is_err());

        settings.set_pin("2580").unwrap();
        assert!(settings.has_pin());
        assert!(settings
            .app_pin_hash
            .as_deref()
            .is_some_and(|h| h.starts_with("pbkdf2$100000$")));
        assert!(settings.verify_pin("2580"));
        assert!(!settings.verify_pin("0000"));
    }

    #[test]
    fn legacy_hello_only_settings_keep_the_restored_session_locked() {
        let settings: AppSettings = serde_json::from_str(
            r#"{"enable_app_lock":true,"use_windows_hello":true,"app_pin_hash":null}"#,
        )
        .unwrap();
        assert!(settings.lock_configured());
        let mut guard = auth_guard::AppLockGuard::with_initial_lock(settings.lock_configured());
        assert!(!guard.is_biometric_available());
        assert!(guard.is_locked());
        guard.poll_availability();
        assert!(!guard.is_biometric_available());
        assert!(guard.is_locked());
    }

    #[test]
    fn call_settings_round_trip_and_load_legacy_settings_defaults() {
        let settings = AppSettings {
            call_notifications_enabled: false,
            selected_speaker_device: "USB Headset".into(),
            ..Default::default()
        };
        let serialized = serde_json::to_string(&settings).unwrap();
        let restored: AppSettings = serde_json::from_str(&serialized).unwrap();
        assert!(!restored.call_notifications_enabled);
        assert_eq!(restored.selected_speaker_device, "USB Headset");

        let legacy: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(legacy.call_notifications_enabled);
        assert!(legacy.selected_speaker_device.is_empty());
    }

    #[test]
    fn pin_pbkdf2_hash_verification_error_handling_and_migration() {
        let mut settings = AppSettings::default();

        // 1. Correct vs wrong PIN with new slow PBKDF2 KDF
        settings.set_pin("4321").unwrap();
        assert!(settings.verify_pin("4321"));
        assert!(!settings.verify_pin("1234"));
        assert!(!settings.verify_pin("432a"));
        assert!(!settings.verify_pin("432"));
        assert!(!settings.verify_pin("43210"));

        // 2. Malformed hash strings must fail gracefully without panic
        settings.app_pin_hash = Some("corrupted".into());
        assert!(!settings.verify_pin("4321"));

        settings.app_pin_hash = Some("pbkdf2$not_a_number$abcd$1234".into());
        assert!(!settings.verify_pin("4321"));

        // Iteration count out of bounds (< 10_000)
        settings.app_pin_hash = Some("pbkdf2$100$0102030405060708090a0b0c0d0e0f10$0102030405060708090a0b0c0d0e0f100102030405060708090a0b0c0d0e0f10".into());
        assert!(!settings.verify_pin("4321"));

        // Salt too short (< 16 bytes)
        settings.app_pin_hash = Some(
            "pbkdf2$100000$0102$0102030405060708090a0b0c0d0e0f100102030405060708090a0b0c0d0e0f10"
                .into(),
        );
        assert!(!settings.verify_pin("4321"));

        // Hash length wrong (!= 32 bytes)
        settings.app_pin_hash = Some("pbkdf2$100000$0102030405060708090a0b0c0d0e0f10$abcd".into());
        assert!(!settings.verify_pin("4321"));

        // Invalid hex
        settings.app_pin_hash = Some("pbkdf2$100000$not_hex_salt!zzzzzzzzzzzzzzzzzz$0102030405060708090a0b0c0d0e0f100102030405060708090a0b0c0d0e0f10".into());
        assert!(!settings.verify_pin("4321"));

        // 3. Backwards-compatibility and automatic migration from legacy sha256$...
        use sha2::{Digest, Sha256};
        let salt = [7u8; 16];
        let mut hasher = Sha256::new();
        hasher.update(b"zalolite-app-pin-v1:");
        hasher.update(salt);
        hasher.update(b":");
        hasher.update(b"7890");
        let digest = hasher.finalize();
        let legacy_hash = format!("sha256${}${}", hex::encode(salt), hex::encode(digest));

        settings.app_pin_hash = Some(legacy_hash.clone());
        // Read-only verify works on legacy hash
        assert!(settings.verify_pin("7890"));
        assert!(!settings.verify_pin("0000"));
        assert_eq!(settings.app_pin_hash.as_deref(), Some(legacy_hash.as_str()));

        // verify_and_migrate_pin upgrades legacy hash to pbkdf2
        assert!(settings.verify_and_migrate_pin("7890"));
        let new_hash = settings.app_pin_hash.as_ref().unwrap();
        assert!(new_hash.starts_with("pbkdf2$100000$"));
        assert_ne!(new_hash, &legacy_hash);

        // Subsequent verification works with the migrated hash
        assert!(settings.verify_pin("7890"));
        assert!(!settings.verify_pin("1111"));
    }

    #[test]
    fn ctrl_l_and_pin_lock_screen_unlock_flow() {
        let ctx = egui::Context::default();
        let mut app = app_with_voice_draft();
        app.user_name = "Tester".into();
        assert!(!app.lock_guard.is_locked());

        // Ctrl+L must NOT lock when enable_app_lock is false.
        let ctrl_l_input = egui::RawInput {
            modifiers: egui::Modifiers::COMMAND,
            events: vec![egui::Event::Key {
                key: Key::L,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            ..Default::default()
        };
        let _ = ctx.run(ctrl_l_input.clone(), |ctx| app.handle_key_events(ctx));
        assert!(!app.lock_guard.is_locked());

        // Enable app lock with a 4-digit PIN -> Ctrl+L locks.
        app.settings.enable_app_lock = true;
        app.settings.set_pin("1357").unwrap();
        let _ = ctx.run(ctrl_l_input, |ctx| app.handle_key_events(ctx));
        assert!(app.lock_guard.is_locked());

        // Wrong PIN (9999) keeps locked and records error.
        let wrong_pin_input = egui::RawInput {
            events: vec![
                egui::Event::Text("9".into()),
                egui::Event::Text("9".into()),
                egui::Event::Text("9".into()),
                egui::Event::Text("9".into()),
            ],
            ..Default::default()
        };
        let _ = ctx.run(wrong_pin_input, |ctx| {
            let pal = app.palette();
            app.render_lock_screen(ctx, &pal);
        });
        assert!(app.lock_guard.is_locked());
        assert!(app.lock_pin_error.is_some());

        // Typing with Backspace correction via Key::Num* unlocks on 1357.
        let right_pin_input = egui::RawInput {
            events: vec![
                egui::Event::Key {
                    key: Key::Num1,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Key {
                    key: Key::Num9,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Key {
                    key: Key::Backspace,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Key {
                    key: Key::Num3,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Key {
                    key: Key::Num5,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Key {
                    key: Key::Num7,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(right_pin_input, |ctx| {
            let pal = app.palette();
            app.render_lock_screen(ctx, &pal);
        });
        assert!(!app.lock_guard.is_locked());
        assert!(app.lock_pin_error.is_none());
    }
}

#[cfg(test)]
mod phase3_completion_tests {
    use super::*;
    fn app() -> (egui::Context, ZaloGuiApp, Sender<GuiEvent>) {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "fixture-owner".into();
        let (tx, rx) = channel();
        app.event_rx = rx;
        (ctx, app, tx)
    }
    #[test]
    fn group_tool_results_reject_old_runtime_and_other_conversation() {
        let (ctx, mut app, tx) = app();
        app.threads = vec![Thread {
            id: "group-b".into(),
            is_group: true,
            ..Default::default()
        }];
        app.selected_thread_index = 0;
        app.status_msg = "unchanged".into();
        for (runtime_id, thread_id) in [
            (app.account_runtime_id.wrapping_add(1), "group-b"),
            (app.account_runtime_id, "group-a"),
        ] {
            tx.send(GuiEvent::GroupTodosLoaded {
                runtime_id,
                thread_id: thread_id.into(),
                todos: Vec::new(),
            })
            .unwrap();
            tx.send(GuiEvent::GroupPollsLoaded {
                runtime_id,
                thread_id: thread_id.into(),
                polls: Vec::new(),
            })
            .unwrap();
        }
        app.check_events(&ctx);
        assert_eq!(app.status_msg, "unchanged");
        assert!(app.tools.todo_poll_thread_id.is_empty());
        tx.send(GuiEvent::GroupTodosLoaded {
            runtime_id: app.account_runtime_id,
            thread_id: "group-b".into(),
            todos: Vec::new(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.tools.todo_poll_thread_id, "group-b");
        assert!(app.status_msg.contains("0"));
        tx.send(GuiEvent::GroupPollsLoaded {
            runtime_id: app.account_runtime_id,
            thread_id: "group-b".into(),
            polls: Vec::new(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.status_msg.contains("bình chọn"));
    }
    #[test]
    fn only_typed_success_starts_sync_banner_expiry() {
        let (ctx, mut app, tx) = app();
        tx.send(GuiEvent::ConversationRefreshStatus {
            running: false,
            success: true,
            message: "finished".into(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.sync_success_until.is_some());
        tx.send(GuiEvent::PhoneSyncStatus {
            running: false,
            success: false,
            message: "partial".into(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.sync_success_until.is_none());
        assert_eq!(app.phone_sync_status, "partial");
    }
    #[test]
    fn stale_preview_does_not_clear_current_runtime_pending_or_mutate_cache() {
        let (ctx, mut app, tx) = app();
        let url = "https://example.invalid/fixture".to_owned();
        app.loading_link_previews.insert(url.clone());
        tx.send(GuiEvent::LinkPreviewReady {
            owner: app.user_uin.clone(),
            runtime_id: app.account_runtime_id.wrapping_add(1),
            url: url.clone(),
            result: Err("stale".into()),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.loading_link_previews.contains(&url));
        assert!(app.link_metadata_cache.is_empty());
        tx.send(GuiEvent::LinkPreviewReady {
            owner: app.user_uin.clone(),
            runtime_id: app.account_runtime_id,
            url: url.clone(),
            result: Err("failed".into()),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(!app.loading_link_previews.contains(&url));
    }
}

#[cfg(test)]
mod phase5_friend_event_tests {
    use super::*;

    #[test]
    fn friend_results_release_pending_preserve_errors_and_reject_stale_scopes() {
        use crate::chat::FriendshipStatus::{Friend, IncomingPending};
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "owner".into();
        app.account_runtime_id = 11;
        let (tx, rx) = channel();
        app.event_rx = rx;
        app.friend_status_map.insert("peer".into(), IncomingPending);
        app.friend_requests
            .insert("peer".into(), (7, FriendAction::Accept));
        for (owner, runtime_id, generation) in
            [("other", 11, 7), ("owner", 10, 7), ("owner", 11, 6)]
        {
            tx.send(GuiEvent::FriendStatusChecked {
                owner: owner.into(),
                runtime_id,
                generation,
                user_id: "peer".into(),
                result: Ok(Friend),
            })
            .unwrap();
        }
        app.check_events(&ctx);
        assert!(app.friend_action_pending("peer"));
        assert_eq!(app.friend_status_map["peer"], IncomingPending);
        // A duplicate action cannot replace the active generation.
        app.request_friend_action("peer", FriendAction::Accept);
        assert_eq!(app.friend_requests["peer"], (7, FriendAction::Accept));
        tx.send(GuiEvent::FriendStatusChecked {
            owner: "owner".into(),
            runtime_id: 11,
            generation: 7,
            user_id: "peer".into(),
            result: Err("fixture refusal".into()),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(!app.friend_action_pending("peer"));
        assert_eq!(app.friend_status_map["peer"], IncomingPending);
        assert!(app.friend_check_retry.contains_key("peer"));
        app.friend_requests
            .insert("peer".into(), (8, FriendAction::Check));
        tx.send(GuiEvent::FriendStatusChecked {
            owner: "owner".into(),
            runtime_id: 11,
            generation: 8,
            user_id: "peer".into(),
            result: Ok(Friend),
        })
        .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.friend_status_map["peer"], Friend);
        assert!(!app.friend_action_pending("peer"));
        assert!(!app.friend_check_retry.contains_key("peer"));
        app.clear_presence_state();
        assert!(app.friend_status_map.is_empty());
        assert!(app.friend_requests.is_empty());
    }
}
