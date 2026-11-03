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

mod composer_drafts;
mod contacts;
mod download_panel;
mod messages;
mod modals;
mod schedule_modal;
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
    /// Group roster for the info drawer: members and server total.
    GroupMembersLoaded {
        group_id: String,
        result: Result<(Vec<crate::chat::SidebarMember>, Option<u64>), String>,
    },
    /// People typing now (`presence::typing`).
    Typing(Vec<crate::presence::Typing>),
    /// Outgoing messages whose delivery state rose: `(thread, msg_id, state)`.
    Receipts(Vec<(String, String, crate::message_model::DeliveryState)>),
    ComposerDraftLoaded {
        owner: String,
        thread: String,
        generation: u64,
        result: Result<String, String>,
    },
    ComposerDraftSaved {
        owner: String,
        thread: String,
        generation: u64,
        text: String,
        result: Result<(), String>,
    },
    MediaStarted {
        owner: String,
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
    ClipboardImageRead(Option<(std::path::PathBuf, u32, u32, egui::ColorImage)>),
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
    Reacted {
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
        thread_id: String,
        msg_id: String,
        ts: i64,
    },
    SessionExpired(String),
    CallStateChanged(String),
    CallIdentity {
        thread_id: String,
        call_id: i64,
    },
    IncomingCall {
        call_id: i64,
        caller_id: String,
        caller_name: String,
        is_video: bool,
        is_group: bool,
    },
    CallRejected {
        code: i64,
        reason: String,
    },
    StatusUpdate(String),
    LoggedOut,
    DownloadFolderPicked(std::path::PathBuf),
    CameraUnavailable(String),
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

#[derive(Debug, Clone)]
struct ImageViewerModal {
    tex_key: String,
    hd_tex_key: Option<String>,
    full_url: String,
    original_url: String,
    title: String,
    sender: String,
    time: String,
    zoom: f32,
    pan: Vec2,
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
    hidden_chat_pin_enabled: bool,
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
            use_avatar_as_wallpaper: false,
            info_drawer_open: true,
            enable_notifications: true,
            play_notification_sound: false,
            quick_messages_enabled: true,
            show_typing: true,
            send_typing: true,
            send_photo_original: true,
            hidden_chat_pin_enabled: true,
            split_priority_inbox: false,
            excel_paste_mode: "ask".into(),
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
    /// Contacts page replaces the conversation list and chat while open.
    contacts_view_open: bool,
    /// An input method is composing text in the chat composer (Telex/IME preedit).
    composer_ime_active: bool,
    /// Last rendered chat-list identity; a change discards one stale layout pass.
    chat_layout_key: u64,
    /// `(thread without prefix, is_group)` → (who is typing, when it was reported).
    typing_map: HashMap<(String, bool), (String, std::time::Instant)>,
    /// Group rosters loaded for the info drawer, by thread ID.
    group_members: HashMap<String, (Vec<crate::chat::SidebarMember>, Option<u64>)>,
    /// Groups whose roster is loading, or failed (with the error).
    group_members_state: HashMap<String, Option<String>>,
    group_member_search: String,
    /// Last typing signal sent per conversation.
    typing_sent: HashMap<String, std::time::Instant>,
    contacts_tab: contacts::ContactsTab,
    contacts_query: String,
    search_query: String,
    input_message: String,
    composer_draft: composer_drafts::ComposerDraft,
    deleted_message_ids: HashSet<(String, String)>,
    status_msg: String,

    // Reply / forward state
    replying_to: Option<ChatMessage>,
    forward_msg: Option<ChatMessage>,
    forward_filter: String,
    filter_starred_only: bool,
    active_call: Option<ActiveCallOverlay>,
    image_viewer: Option<ImageViewerModal>,

    info_drawer_open: bool,
    profile_menu_open: bool,
    toast_notice: Option<(String, std::time::Instant)>,
    phone_modal_open: bool,
    phone_modal_input: String,
    phone_lookup_loading: bool,
    phone_lookup_result: Option<crate::chat::PhoneLookupResult>,
    phone_lookup_error: Option<String>,
    in_chat_search_open: bool,
    in_chat_search_query: String,
    in_chat_search_focus_pending: bool,
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
            contacts_view_open: false,
            composer_ime_active: false,
            chat_layout_key: 0,
            typing_map: HashMap::new(),
            typing_sent: HashMap::new(),
            group_members: HashMap::new(),
            group_members_state: HashMap::new(),
            group_member_search: String::new(),
            contacts_tab: contacts::ContactsTab::default(),
            contacts_query: String::new(),
            search_query: String::new(),
            input_message: String::new(),
            composer_draft: Default::default(),
            deleted_message_ids: HashSet::new(),
            replying_to: None,
            forward_msg: None,
            forward_filter: String::new(),
            filter_starred_only: false,
            active_call: None,
            image_viewer: None,
            status_msg: "Đã đăng nhập · Sẵn sàng".into(),
            info_drawer_open: loaded_settings.info_drawer_open,
            profile_menu_open: false,
            toast_notice: None,
            phone_modal_open: false,
            phone_modal_input: String::new(),
            phone_lookup_loading: false,
            phone_lookup_result: None,
            phone_lookup_error: None,
            in_chat_search_open: false,
            in_chat_search_query: String::new(),
            in_chat_search_focus_pending: false,
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
            settings_tab: SettingsTab::General,
            tools: tools_panel::ToolsPanel::default(),
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
        palette
    }

    fn request_clipboard_image(&mut self) {
        if self.clipboard_read_pending {
            return;
        }
        if let Some(tx) = WORKER_TX.get() {
            self.clipboard_read_pending = tx.send(WorkerCmd::ReadClipboardImage).is_ok();
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
                    id,
                    name,
                    cancel,
                } => {
                    if owner == self.user_uin && self.is_logged_in {
                        self.downloads.insert(
                            id,
                            download_panel::DownloadView {
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
                GuiEvent::ClipboardImageRead(image) => {
                    self.clipboard_read_pending = false;
                    if let Some((path, w, h, image)) = image {
                        let texture =
                            ctx.load_texture("clipboard-preview", image, TextureOptions::LINEAR);
                        self.pending_clipboard_image = Some((path, w, h, texture));
                        self.composer_focus_pending = true;
                        self.show_toast(format!(
                            "Đã dán ảnh ({w}×{h} px) từ Clipboard — nhấn Enter để gửi"
                        ));
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
                        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                            thread.unread.read_visible(&thread.messages);
                            thread.messages.clear();
                            thread.preview.clear();
                            thread.time.clear();
                            thread.presentation = Default::default();
                        }
                        self.unread_counts.remove(&thread_id);
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
                                        existing.unread.read_visible(&existing.messages);
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
                                    new_incoming_alert |= !existing.unread.is_muted()
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
                                        nt.unread.read_visible(&nt.messages);
                                    }
                                    if nt.unread.count() > 0 {
                                        self.unread_counts.insert(nt.id.clone(), nt.unread.count());
                                        new_incoming_alert |= !nt.unread.is_muted()
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
                GuiEvent::GroupMembersLoaded { group_id, result } => match result {
                    Ok(roster) => {
                        self.group_members_state.remove(&group_id);
                        self.group_members.insert(group_id, roster);
                    }
                    Err(error) => {
                        self.group_members_state.insert(group_id, Some(error));
                    }
                },
                GuiEvent::Typing(events) => {
                    let now = std::time::Instant::now();
                    for event in events {
                        self.typing_map
                            .insert((event.thread_id, event.is_group), (event.uid, now));
                    }
                    ctx.request_repaint_after(crate::presence::TYPING_VISIBLE);
                }
                GuiEvent::Receipts(changed) => {
                    for (thread, msg_id, state) in changed {
                        for t in self.threads.iter_mut().filter(|t| t.id == thread) {
                            for m in t.messages.iter_mut().filter(|m| m.id == msg_id) {
                                m.delivery = m.delivery.merge(state);
                            }
                        }
                    }
                }
                GuiEvent::SessionExpired(reason) => {
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
                GuiEvent::DownloadFolderPicked(path) => {
                    self.settings.download_dir = path.display().to_string();
                    self.save_settings();
                    self.show_toast("Đã đổi thư mục tải về");
                }
                GuiEvent::LoggedOut => {
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
                GuiEvent::Tools { owner, result } => {
                    if owner != self.user_uin || !self.is_logged_in {
                        continue;
                    }
                    match result {
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
                                self.tools.hits = hits;
                            }
                        }
                        Ok(tools_panel::ToolsResult::Hit { thread, id }) => {
                            self.replying_to = None;
                            self.filter_starred_only = false;
                            self.in_chat_search_query.clear();
                            self.in_chat_search_open = false;
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
                            self.threads[index].messages = thread.messages;
                            self.chat_render_window = (thread.id.clone(), 150);
                            self.quote_jump = Some(QuoteJump {
                                thread_id: thread.id,
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
                GuiEvent::Reacted {
                    thread_id,
                    msg_id,
                    ts,
                    reaction,
                } => {
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        if let Some(m) = t.messages.iter_mut().find(|m| {
                            (!m.id.is_empty() && m.id == msg_id) || (m.id.is_empty() && m.ts == ts)
                        }) {
                            m.reactions = vec![reaction];
                        }
                        self.persist_threads_async();
                    }
                    self.status_msg = "Đã thả cảm xúc".into();
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
                    thread_id,
                    msg_id,
                    ts,
                } => {
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
                GuiEvent::PreviewImageReady { key, image, permit } => {
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
                GuiEvent::CallIdentity { thread_id, call_id } => {
                    if let Some(call) = self
                        .active_call
                        .as_mut()
                        .filter(|c| c.partner_id == thread_id && c.call_id.is_none())
                    {
                        call.call_id = Some(call_id);
                    }
                }
                GuiEvent::CallStateChanged(state) => {
                    if state == "free" {
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
                    call_id,
                    caller_id,
                    caller_name,
                    is_video,
                    is_group,
                } => {
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
                GuiEvent::CallRejected { code, reason } => {
                    self.status_msg = format!("⚠️ Từ chối cuộc gọi (mã {code}): {reason}");
                    if let Some(ref mut call) = self.active_call {
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
            if !self.sound_prepared {
                if let Some(tx) = WORKER_TX.get() {
                    self.sound_prepared = tx.send(WorkerCmd::PrepareNotificationSound).is_ok();
                }
            }
        } else {
            self.sound_prepared = false;
        }

        if let Some(notice) = crate::desktop_notice::take_activation() {
            if self.is_logged_in && notice.owner == self.user_uin {
                if let Some(index) = self.threads.iter().position(|t| t.id == notice.thread_id) {
                    self.selected_thread_index = index;
                    self.filter_starred_only = false;
                    self.in_chat_search_open = false;
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
            self.theme_dirty = false;
        }

        self.sync_composer_draft(ctx);
        self.check_events(ctx);
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

        // Dev Inspector Panel
        if self.dev_mode && self.dev_panel_open {
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
                            if ui.button("Làm mới quét").clicked() {
                                self.scanner_findings = logger::scan_logs();
                            }
                            if ui.button("Xóa log").clicked() {
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
        } else {
            self.render_sync_status_bar(ctx, &pal);
            self.render_chat_workspace(ctx, &pal);
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
                RichText::new("Đăng nhập Zalo")
                    .color(pal.primary)
                    .size(26.0)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Khởi chạy nhẹ, bảo mật và đồng bộ tin nhắn thời gian thực")
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
                                    RichText::new("Tạo / Làm mới mã QR")
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
                                        RichText::new("Số điện thoại")
                                            .size(13.0)
                                            .color(pal.text_secondary)
                                            .strong(),
                                    );
                                    ui.add_space(4.0);
                                    ui.add_sized(
                                        [ui.available_width(), 34.0],
                                        egui::TextEdit::singleline(&mut self.phone_input)
                                            .hint_text("Ví dụ: 0912345678"),
                                    );
                                    ui.add_space(12.0);

                                    ui.label(
                                        RichText::new("Mật khẩu")
                                            .size(13.0)
                                            .color(pal.text_secondary)
                                            .strong(),
                                    );
                                    ui.add_space(4.0);
                                    ui.add_sized(
                                        [ui.available_width(), 34.0],
                                        egui::TextEdit::singleline(&mut self.password_input)
                                            .password(true)
                                            .hint_text("Nhập mật khẩu Zalo"),
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
                                        RichText::new("Đăng nhập")
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

    fn refresh_unread_title(&self, ctx: &egui::Context) {
        let total: usize = self.unread_counts.values().sum();
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
