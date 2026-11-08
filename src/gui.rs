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
mod download_panel;
mod doc_viewer;
mod file_drop;
mod group_entry_panel;
mod group_panel;
mod hidden_panel;
mod labels_panel;
mod mention_composer;
mod messages;
mod modals;
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

pub enum GuiEvent {
    SafetyNumberLoaded {
        peer_id: String,
        peer_name: String,
        safety_number: String,
        is_verified: bool,
        identity_changed: bool,
    },
    IdentityTrusted {
        peer_id: String,
        is_trusted: bool,
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
        message: String,
    },
    PhoneSyncStatus {
        running: bool,
        message: String,
    },
    HistorySyncStatus {
        running: bool,
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
    },
    LoginFailed(String),
    ThreadsUpdated {
        threads: Vec<Thread>,
        source: ThreadUpdateSource,
    },
    LocalCallRecord {
        thread_id: String,
        text: String,
        msg_id: String,
    },
    OutgoingState {
        owner: String,
        thread_id: String,
        message: ChatMessage,
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
    LinkPreviewReady(crate::chat::LinkMetadata),
    PhoneLookupSuccess(crate::chat::PhoneLookupResult),
    PhoneLookupFailed(String),
    FriendStatusChecked {
        user_id: String,
        status: crate::chat::FriendshipStatus,
    },
    QuoteSent {
        client_message_id: String,
        thread_id: String,
        text: String,
        msg_id: String,
        quoted: ChatMessage,
    },
    NotificationSoundPrepared(bool),
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
        action_id: String,
        thread_id: String,
        msg_id: String,
        ts: i64,
        reaction: String,
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
        items: Vec<crate::server_media::ServerMediaItem>,
        media_type: u8,
    },
    CloudViewerKeyLoaded(crate::zcloud::CloudViewerKey),
    BusinessCatalogLoaded(Vec<crate::business::ProductCatalogItem>),
    DevicesLoaded(Vec<crate::device::DeviceSession>),
    TwoFactorStatusLoaded(crate::account_recovery::TwoFactorStatus),
    GroupTodosLoaded(Vec<crate::todo::TodoItem>),
    GroupPollsLoaded(Vec<crate::poll::PollItem>),
    LanPeersDiscovered(Vec<crate::lan_transfer::LanPeer>),
    BatchRecallCompleted {
        thread_id: String,
        recalled_ids: Vec<String>,
        count: usize,
    },
    BatchForwardCompleted {
        target_id: String,
        count: usize,
    },
    LoggedOut,
    DownloadFolderPicked(Option<Result<std::path::PathBuf, String>>),
    DownloadFolderFailed(String),
    CameraUnavailable(String),
    UploadProgress {
        upload_id: String,
        completed: u64,
        total: u64,
    },
    UploadResumeAvailable {
        sessions: Vec<crate::upload::ResumableUploadState>,
    },
    ProfileSwitched {
        uid: String,
    },
    AppUnlocked,
    AppUnlockFailed,
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
    Privacy,
    Sync,
    Appearance,
    Notifications,
    Messages,
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
    // 6. Tiện ích
    suggest_stickers_while_typing: bool,
    suggest_mentions: bool,
    enable_gif_search: bool,
    enable_sticker_keyword_search: bool,
    // 7. Thiết bị cuộc gọi (Mic & Camera)
    selected_mic_device: String,
    selected_camera_device: String,
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
            quick_messages_enabled: true,
            show_typing: true,
            send_typing: true,
            send_photo_original: true,
            split_priority_inbox: false,
            excel_paste_mode: "image".into(),
            double_click_to_reply: true,
            show_typing_indicator: true,
            suggest_stickers_while_typing: true,
            suggest_mentions: true,
            enable_gif_search: true,
            enable_sticker_keyword_search: true,
            selected_mic_device: String::new(),
            selected_camera_device: String::new(),
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

    fn save(&self) -> anyhow::Result<()> {
        crate::storage::save_settings(self)
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

pub struct ZaloGuiApp {
    tools: tools_panel::ToolsPanel,
    history_search: search_panel::SearchPanel,
    server_pins: crate::server_pins::Snapshot,
    server_board: board_panel::BoardPanel,
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

    // App lock state (WP16-4)
    lock_guard: auth_guard::AppLockGuard,
    auth_in_progress: bool,

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
    forward_filter: String,
    filter_starred_only: bool,
    active_call: Option<ActiveCallOverlay>,
    call_terminals: crate::call_lifecycle::TerminalCalls,
    image_viewer: Option<ImageViewerModal>,
    #[allow(dead_code)] // TODO(WP15-4): Wire "Preview" button in messages.rs
    doc_viewer: Option<doc_viewer::DocViewerModal>,
    safety_number_modal: Option<SafetyNumberModalState>,
    trusted_peers: HashSet<String>,

    info_drawer_open: bool,
    profile_menu_open: bool,
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
    checking_friend_status: HashSet<String>,
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
    sync_cancel_requested: bool,
    last_sync_was_history: bool,
    settings_modal_open: bool,
    download_picker_pending: bool,
    batch_selection_mode: bool,
    selected_msg_ids: HashSet<String>,
    send_contact_modal_open: bool,
    send_contact_query: String,
    batch_forward_modal_open: bool,
    voice_recorder: Option<crate::voice::VoiceRecorder>,
    voice_preview: Option<crate::voice::VoicePreviewPlayer>,
    recorded_voice_samples: Option<(Vec<f32>, u32, std::time::Duration)>,
    settings_tab: SettingsTab,
    settings: AppSettings,

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
        let loaded_settings = if bootstrap {
            AppSettings::load()
        } else {
            AppSettings::default()
        };
        loaded_settings.appearance.apply_context(ctx);
        crate::media::set_selected_mic_device(&loaded_settings.selected_mic_device);
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
        if let Some(li) = session.login.as_ref() {
            if !li.send2me_id.is_empty() {
                for t in threads.iter_mut() {
                    if !t.is_group && t.name.contains("Cloud của tôi") {
                        t.id = li.send2me_id.clone();
                    }
                }
            }
        }
        crate::chat::deduplicate_threads(&mut threads, None);
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
            forward_filter: String::new(),
            filter_starred_only: false,
            active_call: None,
            call_terminals: Default::default(),
            image_viewer: None,
            doc_viewer: None,
            safety_number_modal: None,
            trusted_peers: HashSet::new(),
            status_msg: "Đã đăng nhập · Sẵn sàng".into(),
            info_drawer_open: loaded_settings.info_drawer_open,
            profile_menu_open: false,
            toast_notice: None,
            phone_modal_open: false,
            phone_modal_input: String::new(),
            phone_lookup_loading: false,
            phone_lookup_result: None,
            phone_lookup_error: None,
            sticker_picker_open: false,
            sticker_tab: StickerTab::Popular,
            sticker_textures: HashMap::new(),
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
            checking_friend_status: HashSet::new(),
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
            sync_cancel_requested: false,
            last_sync_was_history: true,
            settings_modal_open: false,
            download_picker_pending: false,
            batch_selection_mode: false,
            selected_msg_ids: HashSet::new(),
            send_contact_modal_open: false,
            send_contact_query: String::new(),
            batch_forward_modal_open: false,
            voice_recorder: None,
            voice_preview: None,
            recorded_voice_samples: None,
            settings_tab: SettingsTab::General,
            tools: tools_panel::ToolsPanel::default(),
            history_search: Default::default(),
            server_pins: Default::default(),
            server_board: Default::default(),
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
            dev_mode,
            dev_panel_open: false,
            dev_tab: DevTab::Logs,
            selected_log: None,
            scanner_findings: Vec::new(),
            event_rx: rx,
            event_tx: tx.clone(),
            quote_jump: None,
            quick_schedule: None,
            lock_guard: auth_guard::AppLockGuard::new(),
            auth_in_progress: false,
            account_switcher: account_switcher::AccountSwitcher::default(),
        };

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
                    app.open_quick_schedule();
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

    fn save_settings(&mut self) {
        match self.settings.save() {
            Ok(()) => {
                if self.settings.remember_login {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::PersistSession);
                    }
                }
            }
            Err(e) => self.show_toast(format!("Không thể lưu cài đặt: {e}")),
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
        self.groups_panel = Default::default();
        self.group_entry_panel = Default::default();
        self.contact_panel = Default::default();
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
        self.typing_map.clear();
        self.typing_sent.clear();
        self.group_members.clear();
        self.group_members_state.clear();
        self.group_member_search.clear();
        self.screen_capture_pending = false;
        self.clipboard_text_fallback = None;
        self.clipboard_alternative_text = None;
        self.clipboard_text_only = false;
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
        let bytes = image.pixels.len() as u64 * 4;
        if bytes > crate::media_resources::TEXTURE_BUDGET {
            return;
        }
        for evicted in self.texture_budget.insert(key.clone(), bytes) {
            self.sticker_textures.remove(&evicted);
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

    fn check_events(&mut self, ctx: &egui::Context) {
        for index in 0..64 {
            let Ok(event) = self.event_rx.try_recv() else {
                break;
            };
            if index == 63 {
                ctx.request_repaint();
            }

            // A21 fix: When locked, gate event processing but allow unlock lifecycle events
            if self.lock_guard.is_locked() {
                match event {
                    GuiEvent::AppUnlocked => {
                        self.lock_guard.unlock();
                        self.auth_in_progress = false;
                        self.status_msg = "Đã mở khóa ứng dụng".into();
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::RefreshThreads);
                        }
                        ctx.request_repaint();
                    }
                    GuiEvent::AppUnlockFailed => {
                        self.auth_in_progress = false;
                        self.status_msg = "Xác thực thất bại".into();
                        ctx.request_repaint();
                    }
                    _ => {}
                }
                continue;
            }

            let server_ack = matches!(&event, GuiEvent::MessageSent { .. });
            let event = match event {
                GuiEvent::LiveThreadsReady(updates) => {
                    self.live_mailbox = Some(updates.clone());
                    GuiEvent::ThreadsUpdated {
                        threads: updates.take(),
                        source: ThreadUpdateSource::Live,
                    }
                }
                GuiEvent::LocalCallRecord {
                    thread_id,
                    text,
                    msg_id,
                } => {
                    let _ = (thread_id, text, msg_id);
                    continue;
                }
                event => event,
            };
            match event {
                GuiEvent::SafetyNumberLoaded {
                    peer_id,
                    peer_name,
                    safety_number,
                    is_verified,
                    identity_changed,
                } => {
                    if is_verified {
                        self.trusted_peers.insert(peer_id.clone());
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
                    peer_id,
                    is_trusted,
                } => {
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
                        continue;
                    }
                    self.clipboard_read_pending = false;
                    if !self
                        .threads
                        .get(self.selected_thread_index)
                        .is_some_and(|t| t.id == thread)
                    {
                        self.clipboard_text_fallback = None;
                        continue;
                    }
                    if let Some((path, w, h, image)) = image {
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
                GuiEvent::LoginSuccess { name, uin, avatar } => {
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
                        self.texture_budget = crate::media_resources::ResourceLru::new(
                            crate::media_resources::TEXTURE_BUDGET,
                        );
                        self.loading_stickers.clear();
                        self.image_retry_at.clear();
                        self.image_failures.clear();
                    }
                    self.is_logged_in = true;
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
                    self.status_msg = format!("Đã cập nhật {}", crate::ui::clock());
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
                    self.clear_presence_state();
                    self.flush_composer_before_logout();
                    self.live_mailbox = None;
                    crate::desktop_notice::account("");
                    self.clear_downloads();
                    self.is_logged_in = false;
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
                GuiEvent::UploadProgress { upload_id: _, completed, total } => {
                    self.status_msg = format!("Tải lên: {}/{} bytes", completed, total);
                }
                GuiEvent::UploadResumeAvailable { sessions } => {
                    if !sessions.is_empty() {
                        let summary = if sessions.len() == 1 {
                            format!(
                                "Có 1 tải lên chưa hoàn thành ({})…",
                                sessions[0].file_name
                            )
                        } else {
                            format!("Có {} tải lên chưa hoàn thành…", sessions.len())
                        };
                        self.show_toast(summary);
                    }
                }
                GuiEvent::LoggedOut => {
                    self.clear_presence_state();
                    self.deleted_message_ids.clear();
                    self.quick_schedule = None;
                    self.reset_sync_status();
                    self.live_mailbox = None;
                    crate::desktop_notice::account("");
                    self.clear_downloads();
                    self.is_logged_in = false;
                    self.user_name.clear();
                    self.user_uin.clear();
                    self.threads.clear();
                    self.selected_thread_index = 0;
                    self.qr_texture = None;
                    self.qr_status = "Đang xin mã QR mới…".into();
                    self.status_msg = "Đã đăng xuất".into();
                    self.active_call = None;
                    self.image_viewer = None;
                    self.user_avatar = None;
                    self.input_message.clear();
                    self.password_input.clear();
                    self.phone_input.clear();
                    self.replying_to = None;
                    self.forward_msg = None;
                    self.forward_filter.clear();
                    self.clipboard_read_pending = false;
                    self.pending_clipboard_image = None;
                    self.search_query.clear();
                    self.phone_lookup_result = None;
                    self.phone_modal_input.clear();
                    self.friend_status_map.clear();
                    self.checking_friend_status.clear();
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
                    self.texture_budget = crate::media_resources::ResourceLru::new(
                        crate::media_resources::TEXTURE_BUDGET,
                    );
                    self.loading_stickers.clear();
                    self.image_retry_at.clear();
                    self.image_failures.clear();
                    self.composer_sticker_cache.clear();
                    self.link_metadata_cache.clear();
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
                GuiEvent::ProfileSwitched { uid: _ } => {
                    // WP16-3: Profile switched; refresh UI state for new profile
                    self.status_msg = "Đã chuyển tài khoản thành công".into();
                    self.profile_menu_open = false;
                    // Request refresh of threads and state for new profile
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::RefreshThreads);
                    }
                }
                GuiEvent::AppUnlocked => {
                    // WP16-4: Windows Hello authentication succeeded
                    self.lock_guard.unlock();
                    self.auth_in_progress = false;
                    self.status_msg = "Đã mở khóa ứng dụng".into();
                    // Refresh UI state after unlock
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::RefreshThreads);
                    }
                }
                GuiEvent::AppUnlockFailed => {
                    // WP16-4: Windows Hello authentication failed
                    self.auth_in_progress = false;
                    self.status_msg = "Xác thực thất bại. Vui lòng thử lại.".into();
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
                            let index = self
                                .threads
                                .iter()
                                .position(|t| t.id == thread.id)
                                .unwrap_or_else(|| {
                                    self.threads.push(Thread {
                                        id: thread.id.clone(),
                                        name: thread.name.clone(),
                                        is_group: thread.is_group,
                                        ..Default::default()
                                    });
                                    self.threads.len() - 1
                                });
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
                    thread_id,
                    message,
                } => {
                    if !self.is_logged_in || owner != self.user_uin {
                        continue;
                    }
                    if let Some(thread) = self.threads.iter_mut().find(|t| t.id == thread_id) {
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
                        self.persist_threads_async();
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
                GuiEvent::NotificationSoundPrepared(ready) => {
                    self.sound_prepared = ready;
                    self.sound_preparing = false;
                    self.sound_retry_at = Some(std::time::Instant::now() + Duration::from_secs(60));
                }
                GuiEvent::Reacted {
                    owner,
                    action_id,
                    thread_id,
                    msg_id,
                    ts,
                    reaction,
                } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
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
                    self.store_texture(ctx, key, image);
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
                GuiEvent::LinkPreviewReady(meta) => {
                    self.loading_link_previews.remove(&meta.url);
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
                GuiEvent::FriendStatusChecked { user_id, status } => {
                    self.checking_friend_status.remove(&user_id);
                    self.friend_status_map.insert(user_id.clone(), status);
                    if let Some(ref mut prof) = self.phone_lookup_result {
                        if prof.user_id == user_id {
                            prof.friendship = status;
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
                    let resolved_name = self
                        .threads
                        .iter()
                        .find(|t| t.id == caller_id)
                        .map(|t| t.name.clone())
                        .unwrap_or(caller_name);
                    self.status_msg = format!(
                        "📲 Cuộc gọi {} đến từ {resolved_name}…",
                        if is_video { "video" } else { "thoại" }
                    );
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
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                        egui::UserAttentionType::Critical,
                    ));
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
                GuiEvent::ConversationRefreshStatus { running, message } => {
                    if !self.phone_sync_running {
                        self.phone_sync_status.clear();
                    }
                    self.refresh_sync_running = running;
                    self.refresh_sync_status = message.clone();
                    self.status_msg = message;
                    self.last_sync_was_history = false;
                    self.sync_notice_dismissed = false;
                }
                GuiEvent::PhoneSyncStatus { running, message } => {
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
                    self.sync_notice_dismissed = false;
                }
                GuiEvent::HistorySyncStatus { running, message } => {
                    if !self.phone_sync_running {
                        self.phone_sync_status.clear();
                    }
                    if !running || !self.history_sync_running {
                        self.sync_cancel_requested = false;
                    }
                    self.last_sync_was_history = true;
                    self.sync_notice_dismissed = false;
                    self.history_sync_running = running;
                    self.status_msg = message.clone();
                    self.history_sync_status = message;
                }
                GuiEvent::LocalCallRecord { .. } => {
                    unreachable!("local call record normalized before dispatch")
                }
                GuiEvent::StatusUpdate(msg) => {
                    self.status_msg = msg;
                }
                GuiEvent::ServerMediaLoaded { items, media_type: _ } => {
                    self.tools.server_media_items = items;
                    self.status_msg = format!("Đã tải {} mục từ kho media server", self.tools.server_media_items.len());
                }
                GuiEvent::CloudViewerKeyLoaded(key) => {
                    self.status_msg = format!("Đã nhận khoá giải mã zCloud cho tệp {}", key.file_id);
                    self.tools.cloud_viewer_keys.insert(key.file_id.clone(), key);
                }
                GuiEvent::BusinessCatalogLoaded(items) => {
                    self.tools.catalog_items = items;
                    self.status_msg = format!("Đã tải {} sản phẩm doanh nghiệp", self.tools.catalog_items.len());
                }
                GuiEvent::DevicesLoaded(devices) => {
                    self.tools.devices = devices;
                    self.status_msg = format!("Đã cập nhật {} thiết bị đăng nhập", self.tools.devices.len());
                }
                GuiEvent::TwoFactorStatusLoaded(status) => {
                    let st_str = if status.is_enabled { "Đã kích hoạt" } else { "Chưa kích hoạt" };
                    self.status_msg = format!("Xác thực 2FA: {} (SĐT: {})", st_str, status.phone_masked);
                    self.tools.two_factor_status = Some(status);
                }
                GuiEvent::GroupTodosLoaded(todos) => {
                    self.tools.group_todos = todos;
                    self.status_msg = format!("Đã tải {} việc cần làm từ nhóm", self.tools.group_todos.len());
                }
                GuiEvent::GroupPollsLoaded(polls) => {
                    self.tools.group_polls = polls;
                    self.status_msg = format!("Đã tải {} cuộc bình chọn", self.tools.group_polls.len());
                }
                GuiEvent::LanPeersDiscovered(peers) => {
                    self.tools.lan_peers = peers;
                    self.status_msg = format!("Đã phát hiện {} thiết bị Zalo trong mạng LAN", self.tools.lan_peers.len());
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
                GuiEvent::BatchForwardCompleted { target_id: _, count } => {
                    self.batch_selection_mode = false;
                    self.selected_msg_ids.clear();
                    self.status_msg = format!("Đã chuyển tiếp {count} tin nhắn");
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
        // A21 fix: Check and update lock state BEFORE processing input/events/render
        // This ensures lock transition happens before any sensitive operation can execute.
        if self.is_logged_in && self.lock_guard.should_lock() {
            self.lock_guard.lock();
        }

        // Update lock guard on user interaction (WP16-4) — but only if not locked
        // A21: Input after idle should not reset deadline if we're already locked
        if !self.lock_guard.is_locked()
            && ctx.input(|i| !i.raw.hovered_files.is_empty()
            || !i.raw.dropped_files.is_empty()
            || i.raw.events.iter().any(|e| matches!(e, egui::Event::Key { .. } | egui::Event::PointerButton { .. }))) {
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
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Q)) {
            self.allow_quit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
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
            // Render lock overlay (WP16-4)
            self.render_lock_overlay(ctx, &pal);
        } else {
            self.render_sync_status_bar(ctx, &pal);
            self.render_chat_workspace(ctx, &pal);
            self.render_reactor_details(ctx);
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

    fn render_lock_overlay(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg_chat_canvas))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let avail_h = ui.available_height();
                    ui.add_space((avail_h * 0.35).max(100.0));

                    // Lock icon
                    ui.label(
                        RichText::new("🔒")
                            .size(64.0),
                    );
                    ui.add_space(16.0);

                    ui.label(
                        RichText::new("Ứng dụng đã bị khóa")
                            .color(pal.text_primary)
                            .size(24.0)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    ui.label(
                        RichText::new("Xác thực bằng Windows Hello để tiếp tục")
                            .color(pal.text_secondary)
                            .size(14.0),
                    );
                    ui.add_space(24.0);

                    let btn = egui::Button::new(
                        RichText::new(if self.auth_in_progress {
                            "Đang xác thực..."
                        } else {
                            "Mở khóa với Windows Hello"
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
                        // Trigger authentication via worker task
                        self.auth_in_progress = true;
                        let ctx_clone = ctx.clone();
                        let event_tx = self.event_tx.clone();

                        tokio::spawn(async move {
                            match auth_guard::request_windows_hello().await {
                                Ok(true) => {
                                    let _ = event_tx.send(GuiEvent::AppUnlocked);
                                }
                                _ => {
                                    let _ = event_tx.send(GuiEvent::AppUnlockFailed);
                                }
                            }
                            ctx_clone.request_repaint();
                        });
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
}
