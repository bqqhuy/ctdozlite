use super::*;

pub enum WorkerCmd {
    Hidden {
        owner: String,
        generation: u64,
        action: crate::hidden_chat::Action,
    },
    AvatarHistory {
        owner: String,
        generation: u64,
        action: crate::avatar_history::Action,
    },
    GroupEntry {
        owner: String,
        generation: u64,
        action: crate::group_entry::Action,
    },
    PrepareAvatar {
        owner: String,
        generation: u64,
        path: std::path::PathBuf,
    },
    UploadAvatar {
        owner: String,
        generation: u64,
        target: crate::avatar::Target,
        draft: std::sync::Arc<crate::avatar::Draft>,
        client: String,
    },
    Profile {
        owner: String,
        generation: u64,
        action: crate::profile::Action,
    },
    Labels {
        owner: String,
        generation: u64,
        action: crate::labels::Action,
    },
    Privacy {
        owner: String,
        generation: u64,
        action: crate::privacy::Action,
    },
    ManualMark {
        owner: String,
        generation: u64,
        target: String,
        group: bool,
        action: crate::unread::MarkAction,
    },
    Archive {
        owner: String,
        generation: u64,
        target: String,
        group: bool,
        archived: Option<bool>,
    },
    Contact {
        owner: String,
        target: String,
        generation: u64,
        action: crate::contact_actions::Action,
    },
    PrepareDroppedFile {
        owner: String,
        thread: String,
        id: u64,
        path: std::path::PathBuf,
    },
    SendDroppedFiles {
        owner: String,
        batch: super::file_drop::Batch,
    },
    CopyViewerImage {
        owner: String,
        context: ChatImage,
    },
    MediaNeighbor {
        owner: String,
        thread: String,
        anchor: String,
        next: bool,
    },
    LoadComposerDraft {
        owner: String,
        thread: String,
        generation: u64,
    },
    SaveComposerDraft {
        owner: String,
        thread: String,
        generation: u64,
        text: crate::mentions::Draft,
    },
    CheckUpdate,
    PrepareUpdate(crate::updater::ReleaseUpdate),
    LaunchUpdate(crate::updater::ReadyUpdate),
    PersistInfoDrawer {
        owner: String,
        open: bool,
    },
    PrepareNotificationSound {
        play: bool,
    },
    OpenLibraryLink {
        owner: String,
        url: String,
    },
    DownloadMedia {
        url: String,
        name: String,
        play: bool,
        open: bool,
        context: Option<ChatImage>,
    },
    OpenDownloaded {
        owner: String,
        thread: Option<(String, bool)>,
        path: std::path::PathBuf,
        folder: bool,
    },
    Tools {
        owner: String,
        command: super::tools_panel::ToolsCommand,
    },
    RunAutomation,
    PruneExpired,
    FetchReactorProfiles {
        owner: String,
        ids: Vec<String>,
    },
    GroupContent {
        owner: String,
        thread: String,
        generation: u64,
        action: crate::group_content::Action,
    },
    ConversationPolicy {
        owner: String,
        generation: u64,
        action: crate::conversation_policy::Action,
    },
    RetryOutgoing {
        thread_id: String,
        message: ChatMessage,
    },
    ReadClipboardImage {
        owner: String,
        thread: String,
    },
    /// Open the Windows region picker and attach the capture like a pasted image.
    CaptureScreen {
        owner: String,
        thread_id: String,
        is_group: bool,
    },
    RequestQr,
    PersistSession,
    LoginPassword {
        phone: String,
        pass: String,
    },
    RefreshThreads,
    PickDesktopBackup {
        owner: String,
        generation: u64,
    },
    DesktopBackup {
        owner: String,
        generation: u64,
        path: std::path::PathBuf,
        key: zeroize::Zeroizing<String>,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        expected: Option<crate::desktop_backup::Report>,
    },

    FetchGroupPolicy {
        group_id: String,
    },
    Group {
        owner: String,
        target: String,
        generation: u64,
        action: crate::groups::Action,
    },
    Board {
        owner: String,
        thread: String,
        group: bool,
        generation: u64,
        action: crate::board::Action,
    },
    SetServerPin {
        owner: String,
        thread: String,
        group: bool,
        pinned: bool,
    },
    SetConversationPreference {
        owner: String,
        thread_id: String,
        /// Addresses the account-wide mute change (`muteType` 1 peer, 2 group).
        is_group: bool,
        value: crate::store::ConversationPreference,
    },
    DeleteConversation {
        owner: String,
        thread: Box<Thread>,
    },
    ClearThreadHistory {
        owner: String,
        thread_id: String,
    },
    SyncHistory {
        threads: Vec<Thread>,
        /// Ask again for conversations the server limited in the last 24 hours.
        retry_filtered: bool,
    },
    CancelHistorySync,
    /// Load a group's member roster for the info drawer.
    FetchGroupMembers {
        group_id: String,
    },
    /// Best-effort typing signal; the UI rate-limits it per conversation.
    SendTyping {
        thread_id: String,
        is_group: bool,
    },
    /// Explicit phone-approved history transfer; never started by login/refresh.
    /// `range_days == 0` requests the whole phone history.
    SyncPhoneHistory {
        range_days: u32,
    },
    RescanPhoneHistory {
        range_days: u32,
    },
    SyncPhoneRecent,
    SyncPhoneLegacy,
    CancelPhoneHistory,
    PollMessages {
        known_threads: Vec<(String, String, bool)>,
    },
    LoadGroupHistory {
        group_id: String,
        is_group: bool,
        limit: usize,
    },
    SendMentioned {
        to_id: String,
        draft: crate::mentions::Draft,
        quoted: Option<ChatMessage>,
    },
    SendMessage {
        to_id: String,
        text: String,
        is_group: bool,
    },
    SendQuote {
        to_id: String,
        text: String,
        quoted: ChatMessage,
        is_group: bool,
    },
    Forward {
        target_id: String,
        target_is_group: bool,
        msg: ChatMessage,
    },
    React {
        to_id: String,
        is_group: bool,
        msg: ChatMessage,
        reaction: String,
    },
    DeleteForMe {
        to_id: String,
        is_group: bool,
        msg: ChatMessage,
    },
    RecallMessage {
        to_id: String,
        is_group: bool,
        msg: ChatMessage,
    },
    SendContactCard {
        to_id: String,
        is_group: bool,
        contact_id: String,
        contact_name: String,
        phone: String,
        avatar: String,
    },
    BatchRecall {
        to_id: String,
        is_group: bool,
        messages: Vec<ChatMessage>,
    },
    BatchForward {
        to_id: String,
        is_group: bool,
        messages: Vec<ChatMessage>,
    },
    SendVoiceMessage {
        to_id: String,
        is_group: bool,
        wav_bytes: Vec<u8>,
        duration_secs: u32,
    },
    SendSticker {
        to_id: String,
        sticker_id: u64,
        cate_id: u64,
        sticker_type: u32,
        is_group: bool,
    },
    PickAndSendAttachment {
        to_id: String,
        is_group: bool,
        images_only: bool,
        caption: String,
        preset_path: Option<std::path::PathBuf>,
        /// `false` sends photos as an HD copy (long side 2560 px) like Zalo PC's default.
        photo_original: bool,
    },
    FetchChatImage {
        key: String,
        url: String,
        thread: String,
        group: bool,
        message: Box<ChatMessage>,
        preview: bool,
    },
    FetchStickerImage {
        key: String,
        url: String,
    },
    FetchStickerStore,
    FetchStickerPack {
        cate_id: u64,
        name: String,
    },
    SearchStickers {
        keyword: String,
    },
    SuggestComposerStickers {
        keyword: String,
    },
    ParseLinkPreview {
        url: String,
    },
    LookupPhone {
        phone: String,
    },
    StartCall {
        partner_id: String,
        partner_name: String,
        partner_avatar: Option<String>,
        is_video: bool,
        is_group: bool,
    },
    AnswerCall {
        call_id: Option<i64>,
        accept: bool,
    },
    SetCallMediaState {
        is_on_mic: bool,
        is_on_cam: bool,
    },
    RequestShareScreen {
        call_id: i64,
        partner_id: String,
        enable: bool,
        is_group: bool,
    },
    KickDevice {
        device_id: String,
    },
    FetchServerMedia {
        thread_id: String,
        is_group: bool,
        media_type: u8,
    },
    GetCloudViewerKey {
        file_id: String,
    },
    GetBusinessCatalog,
    DiscoverLanPeers,
    CreateTodo {
        thread_id: String,
        content: String,
        assignee_id: String,
        deadline_ts: i64,
    },
    FetchTodos {
        thread_id: String,
    },
    CreatePoll {
        thread_id: String,
        question: String,
        options: Vec<String>,
    },
    FetchPolls {
        thread_id: String,
    },
    FetchDevices,
    CheckTwoFactor,
    EndCall {
        call_id: Option<i64>,
    },
    AddFriend {
        user_id: String,
        user_name: String,
    },
    CheckFriendStatus {
        user_id: String,
    },
    AcceptFriend {
        user_id: String,
        user_name: String,
    },
    RejectFriend {
        user_id: String,
        user_name: String,
    },
    UndoFriend {
        user_id: String,
        user_name: String,
    },
    PersistThreads {
        owner: String,
        threads: Vec<Thread>,
    },
    ResumeUpload {
        upload_id: String,
    },
    CancelUpload {
        upload_id: String,
    },
    SwitchProfile {
        owner: String,
        target_uid: String,
    },
    Logout,
}

// Replaceable mailbox: each logout tears down the old worker runtime and event channel.
#[derive(Clone)]
pub(super) struct WorkerSender {
    commands: tokio::sync::mpsc::UnboundedSender<WorkerCmd>,
    images: tokio::sync::mpsc::Sender<(String, String, Option<ChatImage>)>,
    events: Sender<GuiEvent>,
    ctx: egui::Context,
}
impl WorkerSender {
    pub(super) fn send(&self, command: WorkerCmd) -> Result<(), ()> {
        let image = match command {
            WorkerCmd::FetchStickerImage { key, url } => Some((key, url, None)),
            WorkerCmd::FetchChatImage {
                key,
                url,
                thread,
                group,
                message,
                preview,
            } => Some((
                key,
                url,
                Some(ChatImage {
                    thread,
                    group,
                    message: *message,
                    preview,
                }),
            )),
            other => {
                return self.commands.send(other).map_err(|_| ());
            }
        };
        if let Some((key, url, context)) = image {
            if self.images.try_send((key.clone(), url, context)).is_err() {
                let _ = self.events.send(GuiEvent::ImageBusy { key });
                self.ctx.request_repaint();
            }
            Ok(())
        } else {
            Ok(())
        }
    }
}
pub(super) struct WorkerMailbox(std::sync::Mutex<Option<WorkerSender>>);
impl WorkerMailbox {
    pub(super) fn get(&self) -> Option<WorkerSender> {
        self.0.lock().ok()?.clone()
    }
    fn set(&self, sender: WorkerSender) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(sender);
    }
}
pub(super) static WORKER_TX: WorkerMailbox = WorkerMailbox(std::sync::Mutex::new(None));

struct PollGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for PollGuard {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

pub(super) fn resp_error_code(resp: &serde_json::Value) -> Option<i64> {
    resp.get("error_code")
        .and_then(serde_json::Value::as_i64)
        .filter(|c| *c != 0)
}

// A terminal refresh event is required on every exit, including transport errors
// and cancelled tasks. The guard owns no locks or blocking resources.
struct RefreshNotice {
    sender: Sender<GuiEvent>,
    ctx: egui::Context,
    message: String,
}
impl Drop for RefreshNotice {
    fn drop(&mut self) {
        let _ = self.sender.send(GuiEvent::ConversationRefreshStatus {
            running: false,
            message: std::mem::take(&mut self.message),
        });
        self.ctx.request_repaint();
    }
}

pub(super) fn is_session_expired_error(err_str: &str) -> bool {
    crate::session::is_expired_error(err_str)
}

/// Immediately decode any disk-cached sticker thumbnails (`stickers_cache/stk_{cid}_{sid}.png`)
/// and push `GuiEvent::StickerImageReady` *before* sending sticker list metadata, returning any
/// missing `(key, url, cache_path)` entries for concurrent network prefetching.
pub(super) async fn preload_disk_sticker_thumbnails(
    items: &[crate::chat::StickerItem],
    gui_tx: &Sender<GuiEvent>,
    limit: usize,
) -> Vec<(String, String, std::path::PathBuf)> {
    let items = items.to_vec();
    let tx = gui_tx.clone();
    match tokio::task::spawn_blocking(move || preload_disk_sticker_inner(&items, &tx, limit)).await
    {
        Ok(missing) => missing,
        Err(_) => {
            crate::logger::log_error("sticker cache", "Thumbnail cache worker failed");
            Vec::new()
        }
    }
}
fn preload_disk_sticker_inner(
    items: &[crate::chat::StickerItem],
    gui_tx: &Sender<GuiEvent>,
    limit: usize,
) -> Vec<(String, String, std::path::PathBuf)> {
    let cache_dir = crate::storage::cache_dir();
    let _ = std::fs::create_dir_all(&cache_dir);

    let mut missing = Vec::new();
    for item in items.iter().take(limit) {
        let key = format!("stk_{}_{}", item.cate_id, item.sticker_id);
        let cache_path = cache_dir.join(format!("{key}.png"));
        if let Ok(disk_bytes) = crate::limits::read_file(&cache_path, crate::limits::IMAGE_BYTES) {
            if let Ok(dyn_img) = crate::limits::decode_image(&disk_bytes) {
                let rgba = dyn_img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.as_flat_samples();
                let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                let _ = gui_tx.send(GuiEvent::StickerImageReady {
                    key,
                    image: color_img,
                });
                continue;
            } else {
                let _ = std::fs::remove_file(&cache_path);
            }
        }
        let url = if item.cate_id == 0 && (1..=30).contains(&item.sticker_id) {
            format!(
                "https://stc-chat.zdn.vn/images/stickers/default/thumb/{}.png",
                item.sticker_id
            )
        } else {
            format!(
                "https://api.zaloapp.com/api/emoticon/sticker/webpc?eid={}&size=130",
                item.sticker_id
            )
        };
        missing.push((key, url, cache_path));
    }
    missing
}

/// Concurrently download, cache, and decode missing sticker thumbnails so the popover and search
/// grid render smoothly without waiting for frame-by-frame lazy requests.
async fn prefetch_missing_sticker_thumbnails(
    requests: &tokio::sync::mpsc::Sender<(String, String, Option<ChatImage>)>,
    missing: Vec<(String, String, std::path::PathBuf)>,
    gui_tx: &Sender<GuiEvent>,
    ctx: &egui::Context,
) {
    for (key, url, _) in missing {
        if requests.send((key.clone(), url, None)).await.is_err() {
            let _ = gui_tx.send(GuiEvent::ImageUnavailable {
                key,
                failure: crate::media_recovery::Failure::Local,
            });
            ctx.request_repaint();
        }
    }
}

// Hidden viewports may stop rendering. Persist worker deltas before notifying UI
// so delivery and history do not depend on a visible application window.
fn deliver_thread_updates() -> bool {
    #[cfg(windows)]
    {
        !crate::tray::hidden()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Store a received batch. Returns `true` only when it reached SQLite, which is the
/// condition for advancing persisted queue cursors.
async fn persist_received(session: &Session, threads: &mut Vec<Thread>) -> bool {
    persist_received_since(session, threads, None).await
}

async fn persist_received_since(
    session: &Session,
    threads: &mut Vec<Thread>,
    baseline: Option<crate::chat::MessageFingerprints>,
) -> bool {
    let session = session.clone();
    let mut batch = threads.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<(Vec<Thread>, bool)> {
        anyhow::ensure!(session.is_active(), "Account changed");
        let mut store = crate::store::MessageStore::open(session.history_path())?;
        store.filter_deleted(&mut batch)?;
        // Once deletion filtering succeeds, a cache-write or automation failure must
        // not suppress otherwise valid live delivery (history persistence is fail-open).
        let changed = baseline
            .as_ref()
            .map(|marks| crate::chat::changed_since(&batch, marks));
        let offered = changed.as_deref().unwrap_or(&batch);
        let stored = match crate::store::persist_threads(&mut store, offered) {
            Ok(_) => {
                if crate::tools::auto_reply(&store, session.account_key(), offered).is_err() {
                    eprintln!("Cannot evaluate auto reply for received batch");
                }
                true
            }
            Err(_) => {
                eprintln!("Cannot persist received message batch; delivering filtered messages");
                false
            }
        };
        if baseline.is_some() {
            // Recheck the entire view, including unchanged rows deleted during the refresh.
            store.filter_deleted(&mut batch)?;
            if session.is_active() {
                crate::chat::save_threads_cache(&session, &batch);
            }
        }
        Ok((batch, stored))
    })
    .await;
    match result {
        Ok(Ok((batch, stored))) => {
            *threads = batch;
            stored
        }
        _ => {
            eprintln!("Cannot persist or filter received message batch");
            threads.clear();
            false
        }
    }
}

async fn fetch_gui_image(
    s: &Session,
    root: &std::path::Path,
    key: &str,
    url: &str,
) -> anyhow::Result<egui::ColorImage> {
    use crate::media_resources::{MediaKey, MediaQuality, PrivateMediaCache};
    anyhow::ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "No active media account"
    );
    if let Some(bytes) = crate::bundled_stickers::thumbnail(key) {
        let key = key.to_owned();
        return tokio::task::spawn_blocking(move || {
            let image = crate::limits::preview_image(bytes, &key)?;
            Ok(egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            ))
        })
        .await?;
    }
    let root = root.to_owned();
    let account = s.account_key().to_owned();
    let key = key.to_owned();
    let url = url.to_owned();
    let media_key = MediaKey {
        account: account.clone(),
        thread: "ui-preview".into(),
        message: key.clone(),
        quality: if key.starts_with("full_") {
            MediaQuality::Preview
        } else {
            MediaQuality::Thumbnail
        },
    };
    let directory = root.join("private-media");
    let (cache, path) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let cache = PrivateMediaCache::new(&directory, &account)?;
        let path = cache.path_for(&media_key)?;
        Ok((cache, path))
    })
    .await??;
    let cached_path = path.clone();
    let cached_key = key.clone();
    if let Ok(Ok(image)) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let bytes = crate::limits::read_file(&cached_path, crate::limits::IMAGE_BYTES)?;
        crate::limits::preview_image(&bytes, &cached_key)
    })
    .await
    {
        return Ok(egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ));
    }
    let bytes = if url.starts_with("https://") || url.starts_with("http://") {
        let _permit =
            crate::limits::image_network_permit(key.starts_with("ava_") || key.starts_with("stk_"))
                .await?;
        crate::crash_report::phase("media-network");
        let compatible = crate::media_recovery::compatible_image_url(&url, true)?;
        let response =
            crate::media_download::response(&compatible, Duration::from_secs(20)).await?;
        crate::media_resources::read_bounded_response(response, crate::limits::IMAGE_BYTES)
            .await
            .map_err(|error| match error {
                crate::media_resources::MediaError::Timeout => {
                    crate::media_recovery::Failure::Timeout
                }
                crate::media_resources::MediaError::Offline => {
                    crate::media_recovery::Failure::Network
                }
                _ => crate::media_recovery::Failure::Content,
            })?
    } else {
        let root = root.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let path = std::path::Path::new(&url).canonicalize()?;
            anyhow::ensure!(
                path.starts_with(root.canonicalize()?),
                "Local preview is outside application data"
            );
            crate::limits::read_file(&path, crate::limits::IMAGE_BYTES)
        })
        .await??
    };
    crate::media_recovery::validate_image_body(&bytes)?;
    let rgba = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let image = crate::limits::preview_image(&bytes, &key)
            .map_err(|_| crate::media_recovery::Failure::Decode)?;
        if crate::storage::atomic_write(&path, &bytes).is_err() {
            // Rendering may proceed when a full/read-only cache cannot be updated.
            crate::logger::log_error("image cache", "Cannot save/prune preview cache");
        }
        // Cache pruning is coalesced and detached from image delivery.
        tokio_cache_prune(cache);
        Ok(image)
    })
    .await??;
    anyhow::ensure!(s.is_active(), "Media account was logged out");
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

pub(super) fn spawn_session_worker(
    session: Session,
    gui_tx: Sender<GuiEvent>,
    egui_ctx: egui::Context,
) {
    session.hidden.enable();
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<WorkerCmd>();
    let live_updates = std::sync::Arc::new(crate::live_updates::LiveUpdates::default());
    let image_jobs = crate::media_resources::MediaJobQueue::default();
    let commands = cmd_tx.downgrade();
    let (image_tx, mut image_rx) =
        tokio::sync::mpsc::channel::<(String, String, Option<ChatImage>)>(128);
    WORKER_TX.set(WorkerSender {
        commands: cmd_tx,
        images: image_tx.clone(),
        events: gui_tx.clone(),
        ctx: egui_ctx.clone(),
    });

    let refresh_in_flight = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Serialize page receipt through durable cursor commit, not just the HTTP request.
    let poll_delivery = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
    let mute_sync = MuteSync::default();
    let metadata_notify = std::sync::Arc::new(tokio::sync::Notify::new());
    let metadata_cache_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let metadata_gate = std::sync::Arc::new(tokio::sync::Mutex::new(()));
    let pin_gate = std::sync::Arc::new(tokio::sync::Mutex::new(()));
    let search_gate = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
    let search_latest = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let history_sync_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let history_sync_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let phone_sync_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let phone_sync_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (phone_transport_tx, phone_transport_rx) =
        tokio::sync::watch::channel::<Option<crate::socket::SyncTransport>>(None);
    let session = std::sync::Arc::new(tokio::sync::Mutex::new(session));
    let poll_state =
        std::sync::Arc::new(tokio::sync::Mutex::new(crate::chat::PollState::default()));
    let ctrl_poll_state = std::sync::Arc::new(tokio::sync::Mutex::new(
        crate::call::CtrlPollState::default(),
    ));

    std::thread::Builder::new()
        .name("zalo-worker".into())
        .spawn(move || {
            // Lightweight 2-thread async runtime to keep memory & CPU footprint minimal
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async move {
                {
                    let shared=session.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();let notify=metadata_notify.clone();let gate=metadata_gate.clone();let cache_started=metadata_cache_started.clone();let metadata_live=live_updates.clone();
                    tokio::spawn(async move {let mut tick=tokio::time::interval(Duration::from_secs(3));let mut previous=String::new();let mut at=std::time::Instant::now()-Duration::from_secs(121);
                        loop {
                            let awakened=tokio::select! {_=tick.tick()=>false,_=notify.notified()=>true};let requested=metadata_live.take_metadata_request()||awakened;
                            let account=shared.lock().await.clone();if !account.is_active(){break;}let owner=account.account_key().to_owned();if owner.is_empty(){continue;}
                            if !requested && owner==previous && at.elapsed()<Duration::from_secs(900){continue;}
                            let _gate=gate.lock().await;
                            if previous!=owner {
                                if !cache_started.swap(true,std::sync::atomic::Ordering::SeqCst) {if let Ok(Some(snapshot))=crate::account_metadata::restore_cached(&account).await {let _=tx.send(GuiEvent::AccountMetadata {owner:owner.clone(),snapshot:Box::new(snapshot)});}}
                                if let Ok(Some(view))=crate::hidden_chat::restore_cached(&account).await {let _=tx.send(GuiEvent::Hidden {owner:owner.clone(),generation:None,view,result:Ok(())});ctx.request_repaint();}
                            }
                            previous=owner.clone();at=std::time::Instant::now();
                            let hidden=async {let result=crate::hidden_chat::execute(&account,&crate::hidden_chat::Action::Read).await.map(|_|()).map_err(|error|error.to_string());let _=tx.send(GuiEvent::Hidden {owner:owner.clone(),generation:None,view:account.hidden.view(&owner),result});ctx.request_repaint();};
                            let (_,snapshot)=tokio::join!(hidden,crate::account_metadata::read(&account));
                            if !account.is_active(){break;}
                            if let Ok(mut policy)=account.inbox_policy.write(){if let Some(friends)=&snapshot.friends{policy.friends=Some(friends.clone());}
                                if let Some(business)=snapshot.business{policy.business=Some(business);}
                                if let Some(privacy)=&snapshot.privacy{policy.direct=privacy.values.get(&crate::privacy::Setting::StrangerDirect).map(|value|*value!=0);policy.show_online=privacy.values.get(&crate::privacy::Setting::ShowOnline).map(|value|*value!=0);}}

                            crate::account_metadata::save_cached(&account,&snapshot).await;
                            let _=tx.send(GuiEvent::AccountMetadata {owner,snapshot:Box::new(snapshot)});ctx.request_repaint();
                        }
                    });
                }
                // Old HD photo copies left by failed sends; pending retries rebuild theirs.
                tokio::task::spawn_blocking(crate::photo_quality::prune_all);
                {
                    let shared = session.clone(); let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                    let visible = crate::media_resources::MediaJobQueue::default();
                    let flights=std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
                    tokio::spawn(async move {
                        while let Some((key,url,context)) = image_rx.recv().await {
                            let s = shared.lock().await.clone();
                            let Some(flight)=ImageFlight::start(flights.clone(),s.account_key(),&key)else{continue;};
                            let queue = if key.starts_with("ava_") || key.starts_with("stk_") { &image_jobs } else { &visible };
                            let Some(admission) = queue.try_admit() else {
                                let _ = tx.send(GuiEvent::ImageBusy { key }); ctx.request_repaint(); continue;
                            };
                            let tx = tx.clone(); let ctx = ctx.clone();
                            tokio::spawn(async move {
                                let _flight=flight;
                                let background = key.starts_with("ava_") || key.starts_with("stk_");
                                let Some(job) = admission.start().await else { return; };
                                let _background = background;
                                let owner = s.account_key().to_owned();
                                let expires_at = context.as_ref().and_then(|source| crate::message_lifecycle::expires_at(&source.message));
                                match fetch_chat_image(&s,&key,&url,context,&tx).await {
                                    Ok(image) => { let _ = tx.send(GuiEvent::PreviewImageReady { owner,expires_at,key,image,permit:job }); },
                                    Err(error) => { let failure=error.downcast_ref::<crate::media_recovery::Failure>().cloned().unwrap_or(crate::media_recovery::Failure::Local);crate::transfer_diagnostics::media(&failure);let _ = tx.send(GuiEvent::ImageUnavailable { key,failure }); }
                                }
                                ctx.request_repaint();
                            });
                        }
                    });
                }
                {
                    let s = session.lock().await.clone();
                    let cached_session = s.clone();
                    match tokio::task::spawn_blocking(move || crate::chat::load_threads_cache(&cached_session)).await {
                        Ok(cached) => live_updates.seed(&cached),
                        Err(_) => crate::logger::log_error("live baseline", "Cannot prime cached message identities"),
                    }
                    let _ = crate::login::get_server_info(&s).await;
                }
                {
                    let s_kw = session.clone();
                    let gui_tx_kw = gui_tx.clone();
                    let ctx_kw = egui_ctx.clone();
                    tokio::spawn(async move {
                        let s = s_kw.lock().await.clone();
                        if s.login.is_some() {
                            if let Ok(kws) = crate::chat::fetch_sticker_suggest_keywords(&s).await {
                                if !kws.is_empty() {
                                    let _ = gui_tx_kw
                                        .send(GuiEvent::StickerSuggestKeywordsLoaded(kws));
                                    ctx_kw.request_repaint();
                                }
                            }
                        }
                    });
                }

                let call_bridge = std::sync::Arc::new(crate::call::CallBridge::spawn(
                    session.clone(),
                    gui_tx.clone(),
                    egui_ctx.clone(),
                ));

                let is_ws_connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let is_ws_connected_poll = is_ws_connected.clone();

                // 1) Real-time `zpw_ws` WebSocket loop for instant chat messages + VoIP events
                {
                    let live_updates = live_updates.clone();
                    let metadata_notify=metadata_notify.clone();
                    let mute_sync = mute_sync.clone();
                    let s_ws = session.clone();
                    let cb = call_bridge.clone();
                    let gui_tx_ws = gui_tx.clone();
                    let ctx_ws = egui_ctx.clone();
                    tokio::spawn(async move {
                        let mut ws_rx: Option<
                            tokio::sync::mpsc::Receiver<crate::socket::WsEvent>,
                        > = None;
                        let mut ws_sek = String::new();
                        loop {
                            let s_snap = s_ws.lock().await.clone();
                            if s_snap.login.is_some() {
                                let cur_sek = s_snap.zpw_sek.clone().unwrap_or_default();
                                if !cur_sek.is_empty() && (ws_rx.is_none() || ws_sek != cur_sek) {
                                    ws_sek = cur_sek;
                                    let (events,transport)=crate::socket::spawn_with_sync(s_snap.clone());
                                    ws_rx = Some(events);
                                    tokio::spawn(crate::phone_sync::clean_after_connect(s_snap.clone(),transport.clone()));
                                    let account=s_snap.clone();let notify=metadata_notify.clone();let mut epochs=transport.epochs();let epoch_tx=gui_tx_ws.clone();let epoch_ctx=ctx_ws.clone();
                                    tokio::spawn(async move {while epochs.changed().await.is_ok(){if !account.is_active(){break;}
                                        if epochs.borrow().is_some(){account.hidden.lock();let _=epoch_tx.send(GuiEvent::Hidden {owner:account.account_key().into(),generation:None,view:account.hidden.view(account.account_key()),result:Ok(())});apply_metadata_impact(&account,crate::account_controls::Impact {all_groups:true,..Default::default()},&notify,&epoch_tx,&epoch_ctx);notify.notify_one();}}});
                                    phone_transport_tx.send_replace(Some(transport.clone()));
                                    // WP15.1-D: liveness thật từ transport epoch (Some = đã auth),
                                    // thay cho detection theo "có nhận event" (sai khi idle/half-dead).
                                    let liveness_flag = is_ws_connected.clone();
                                    let mut epoch_watch = transport.epochs();
                                    tokio::spawn(async move {
                                        loop {
                                            liveness_flag.store(epoch_watch.borrow().is_some(), std::sync::atomic::Ordering::SeqCst);
                                            if epoch_watch.changed().await.is_err() { break; }
                                        }
                                    });
                                }
                                if let Some(rx) = ws_rx.as_mut() {
                                    match tokio::time::timeout(
                                        Duration::from_millis(1500),
                                        rx.recv(),
                                    )
                                    .await
                                    {
                                        Ok(Some(ev)) => {
                                            // WP15.1-D: liveness lấy từ epoch watch (spawn ở trên),
                                            // không set theo activity event — idle-but-connected vẫn connected.
                                            let data_node = ev
                                                .payload
                                                .get("data")
                                                .unwrap_or(&ev.payload);
                                            for voip_ev in
                                                crate::call::extract_voip_controls(data_node)
                                            {
                                                cb.handle_control_event(voip_ev);
                                            }
                                            apply_metadata_impact(&s_snap,crate::account_controls::from_frame(&ev),&metadata_notify,&gui_tx_ws,&ctx_ws);
                                            deliver_presence(&s_snap, &ev.payload, &gui_tx_ws, &ctx_ws).await;
                                            let reaction_events=crate::reaction_pipeline::events(&ev.payload);
                                            let has_reactions=!reaction_events.is_empty();
                                            let reactions_stored=if reaction_events.is_empty(){true}else{
                                                let account=s_snap.clone();
                                                match tokio::task::spawn_blocking(move||->anyhow::Result<_>{let store=crate::store::MessageStore::open(account.history_path())?;crate::reaction_pipeline::enqueue(&store,&reaction_events)?;crate::reaction_pipeline::drain(&store,&account)}).await {
                                                    Ok(Ok(threads))=>{if !threads.is_empty(){let _=gui_tx_ws.send(GuiEvent::ThreadsUpdated{threads,source:ThreadUpdateSource::History});ctx_ws.request_repaint();}true},
                                                    _=>{crate::logger::log_error("reaction","Cannot persist incoming reaction page");false}
                                                }
                                            };

                                            if ev.is_voip_ish() {
                                                let mutes = crate::mute_sync::parse_mute_controls(data_node, crate::login::unix_now());
                                                apply_server_mutes(&s_snap, mutes, &mute_sync, &gui_tx_ws, &live_updates, &ctx_ws).await;
                                            }
                                            if ev.is_chat_ish() || has_reactions || data_node.get("clearUnreads").is_some()
                                                || data_node.get("clearUnreadInfo").is_some() || data_node.get("chatEntries").is_some() || data_node.get("statusMap").is_some() {
                                                let mut ws_threads = Vec::new();
                                                if let Some(li) = s_snap.login.as_ref() {
                                                    if !li.send2me_id.is_empty() {
                                                        ws_threads.push(Thread {
                                                            presentation: Default::default(),
                                                            unread: Default::default(),
                                                            id: li.send2me_id.clone(),
                                                            name: "Cloud của tôi (Lưu trữ)"
                                                                .into(),
                                                            preview: String::new(),
                                                            time: String::new(),
                                                            avatar: None,
                                                            is_group: false,
                                                            messages: Vec::new(),
                                                        });
                                                    }
                                                }
                                                crate::chat::merge_previews(
                                                    &mut ws_threads,
                                                    &ev.payload,
                                                );
                                                ws_threads.retain(|t| !t.messages.is_empty() || t.unread.fresh);
                                                let mut stored = true;
                                                if !ws_threads.is_empty() {
                                                    crate::chat::resolve_thread_names(
                                                        &s_snap,
                                                        &mut ws_threads,
                                                    )
                                                    .await;
                                                    stored = persist_received(&s_snap, &mut ws_threads).await;
                                                    if publish_live(&s_snap, &live_updates, ws_threads) {
                                                        let _ = gui_tx_ws.send(GuiEvent::LiveThreadsReady(live_updates.clone()));
                                                    }
                                                    ctx_ws.request_repaint();
                                                }
                                                // Like Zalo PC, a queue cursor advances only after its page is stored.
                                                crate::queue_cursors::commit_socket(&s_snap, ev.cmd, ev.sub_cmd, &ev.payload, stored && reactions_stored).await;
                                            }
                                        }
                                        Ok(None) => {
                                            ws_rx = None;
                                            tokio::time::sleep(Duration::from_secs(2)).await;
                                        }
                                        Err(_) => {}
                                    }
                                } else {
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                }
                            } else {
                                ws_rx = None;
                                ws_sek.clear();
                                tokio::time::sleep(Duration::from_secs(3)).await;
                            }
                        }
                    });
                }

                // 2) Background HTTP long-poll fallback for VoIP and mute control events (`lp_control`)
                {
                    let s_ctrl = session.clone();
                    let metadata_notify=metadata_notify.clone();
                    let cps = ctrl_poll_state.clone();
                    let cb = call_bridge.clone();
                    let mute_sync = mute_sync.clone();
                    let live_ctrl = live_updates.clone();
                    let gui_tx_ctrl = gui_tx.clone();
                    let ctx_ctrl = egui_ctx.clone();
                    tokio::spawn(async move {
                        loop {
                            let s_snap = s_ctrl.lock().await.clone();
                            if s_snap.login.is_some() {
                                let voip_res = {
                                    let mut st = cps.lock().await;
                                    crate::call::poll_control_events(&s_snap, &mut st).await
                                };
                                if let Ok((events, mutes,impact)) = voip_res {
                                    apply_metadata_impact(&s_snap,impact,&metadata_notify,&gui_tx_ctrl,&ctx_ctrl);
                                    for ev in events {
                                        cb.handle_control_event(ev);
                                    }
                                    apply_server_mutes(&s_snap, mutes, &mute_sync, &gui_tx_ctrl, &live_ctrl, &ctx_ctrl).await;
                                } else {
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                }
                            } else {
                                tokio::time::sleep(Duration::from_secs(3)).await;
                            }
                        }
                    });
                }

                // A19 fix: auth-attempt generation/cancel to prevent stale login response override
                let current_auth_attempt = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
                let mut current_qr_task: Option<tokio::task::JoinHandle<()>> = None;
                let is_polling = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let known_threads = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<(String, String, bool)>::new()));
                {
                    let commands = commands.clone();
                    let session = session.clone();
                    let known = known_threads.clone();
                    let polling = is_polling.clone();
                    let is_ws_connected = is_ws_connected_poll.clone();
                    tokio::spawn(async move {
                        let mut timer = tokio::time::interval(Duration::from_millis(1200));
                        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                        loop {
                            timer.tick().await;
                            if commands.strong_count() == 0 { break; }
                            if polling.load(std::sync::atomic::Ordering::SeqCst) { continue; }
                            let s = session.lock().await;
                            if s.login.is_none() || !s.is_active() { continue; }
                            drop(s);

                            // WP13-2 fulfilled: polling eliminated when WebSocket epoch is Some.
                            // HTTP polling runs ONLY as fallback when the WebSocket is down.
                            // The is_ws_connected flag is driven by transport.epochs() watch in
                            // the liveness task above; it goes false the moment ready_tx sends None
                            // (socket epoch ends or reconnect backoff). No polling skew allowed.
                            if is_ws_connected.load(std::sync::atomic::Ordering::SeqCst) {
                                continue;
                            }

                            let snapshot = known.lock().await.clone();
                            if let Some(tx) = commands.upgrade() {
                                if tx.send(WorkerCmd::PollMessages {known_threads: snapshot}).is_err() { break; }
                            } else { break; }
                        }
                    });
                }

                {
                    let commands=commands.clone();
                    tokio::spawn(async move {let mut tick=tokio::time::interval(Duration::from_secs(2));loop{tick.tick().await;let Some(tx)=commands.upgrade()else{break;};if tx.send(WorkerCmd::RunAutomation).is_err(){break;}
if tx.send(WorkerCmd::PruneExpired).is_err(){break;}}});
                }
                let mut recovered_accounts=HashSet::new();
                let automation_running=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                while let Some(cmd) = cmd_rx.recv().await {
                    if let Some((thread_id, mut message)) = outgoing_command(&cmd) {
                        let Some(active) = crate::outgoing::Active::start(&message.client_message_id) else { continue; };
                        let tx = gui_tx.clone(); let ctx = egui_ctx.clone(); let shared = session.clone();
                        tokio::spawn(async move {
                            let active = active;
                            let s = shared.lock().await.clone();
                            let owner = s.account_key().to_owned();
                            message.delivery = crate::message_model::DeliveryState::Pending;
                            if let Some(job) = message.outgoing_job.as_mut() { job.error.clear(); }
                            persist_outgoing(&s, &thread_id, &message).await;
                            let _ = tx.send(GuiEvent::OutgoingState { owner: owner.clone(), thread_id: thread_id.clone(), message: message.clone() });
                            ctx.request_repaint();
                            let result = crate::outgoing::send(&s, &thread_id, &message).await;
                            let expired = result.as_ref().err().is_some_and(|error| is_session_expired_error(&error.to_string()));
                            match result {
                                Ok(sent) => message = sent,
                                Err(error) => {
                                    message.delivery = crate::message_model::DeliveryState::Failed;
                                    if let Some(job) = message.outgoing_job.as_mut() { job.error = crate::outgoing::failure_message(&error); }
                                }
                            }
                            persist_outgoing(&s, &thread_id, &message).await;
                            drop(active);
                            let _ = tx.send(GuiEvent::OutgoingState { owner, thread_id, message });
                            if expired {
                                let _ = tokio::task::spawn_blocking(crate::storage::remove_session).await;
                                shared.lock().await.logout();
                                let _ = tx.send(GuiEvent::SessionExpired("Không thể gửi vì phiên đăng nhập đã hết hạn".into()));
                            }
                            ctx.request_repaint();
                        });
                        continue;
                    }
                    match cmd {
                        WorkerCmd::LoadComposerDraft{owner,thread,generation}=>{
                            let s=session.lock().await.clone();
                            let id=thread.clone();let expected=owner.clone();
                            let result=tokio::task::spawn_blocking(move||->anyhow::Result<crate::mentions::Draft>{
                                anyhow::ensure!(s.is_active() && !expected.is_empty() && s.account_key()==expected,"Tài khoản đã thay đổi");
                                crate::store::MessageStore::open(s.history_path())?.composer_document(&id)
                            }).await.map_err(|_|"Không đọc được bản nháp".to_owned()).and_then(|r|r.map_err(|_|"Không đọc được bản nháp".into()));
                            let _=gui_tx.send(GuiEvent::ComposerDraftLoaded{owner,thread,generation,result});egui_ctx.request_repaint();
                        }
                        WorkerCmd::SaveComposerDraft{owner,thread,generation,text}=>{
                            let s=session.lock().await.clone();
                            let (id,expected,value)=(thread.clone(),owner.clone(),text.clone());
                            let result=tokio::task::spawn_blocking(move||->anyhow::Result<()>{
                                anyhow::ensure!(s.is_active() && !expected.is_empty() && s.account_key()==expected,"Tài khoản đã thay đổi");
                                crate::store::MessageStore::open(s.history_path())?.save_composer_document(&id,&value)
                            }).await.map_err(|_|"Không lưu được bản nháp".to_owned()).and_then(|r|r.map_err(|_|"Không lưu được bản nháp".into()));
                            let _=gui_tx.send(GuiEvent::ComposerDraftSaved{owner,thread,generation,text,result});egui_ctx.request_repaint();
                        }
                        WorkerCmd::Tools {owner,command}=>{
                            let s=session.lock().await.clone();if owner!=s.account_key() || owner.is_empty() || !s.is_active(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let open_request=match &command {super::tools_panel::ToolsCommand::OpenSearchHit{generation,thread,id}=>Some((*generation,thread.clone(),id.clone())),_=>None};
                            let search_request=match &command {super::tools_panel::ToolsCommand::SearchPage(request)=>Some(request.clone()),_=>None};
                            let search_gate=search_gate.clone();let latest=search_latest.clone();
                            if let Some(request)=&search_request{latest.store(request.generation,std::sync::atomic::Ordering::Release);}
                            let schedule=match &command {super::tools_panel::ToolsCommand::Schedule{request,thread,text,at,..}=>Some((*request,thread.clone(),text.clone(),*at)),_=>None};
                            tokio::spawn(async move{
                                let permit=if search_request.is_some(){search_gate.acquire_owned().await.ok()}else{None};
                                let stale=search_request.as_ref().is_some_and(|r|latest.load(std::sync::atomic::Ordering::Acquire)!=r.generation);
                                let mut result=if stale{Err("Đã thay bằng truy vấn mới".into())}else{account_tools(s,command).await.map_err(|e|e.to_string())};
                                drop(permit);
                                if let (Err(error),Some((generation,thread,id)))=(&result,open_request){result=Ok(super::tools_panel::ToolsResult::SearchOpenFailed{generation,thread,id,error:error.clone()});}
                                if let (Err(error),Some(request))=(&result,search_request){result=Ok(super::tools_panel::ToolsResult::SearchPage{request,result:Err(error.clone()),senders:Vec::new()});}
                                if let (Err(error),Some((request,thread,text,at)))=(&result,schedule){result=Ok(super::tools_panel::ToolsResult::ScheduleSaved{request,thread,text,at,result:Err(error.clone())});}
                                let _=tx.send(GuiEvent::Tools{owner,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::FetchReactorProfiles{owner,ids}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{
                                let result=if owner!=account.account_key()||!account.is_active()||ids.len()>50{Err("Yêu cầu hồ sơ không hợp lệ".into())}else{crate::chat::get_mini_profiles(&account,&ids).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::ReactorProfiles{owner,ids,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::GroupContent{owner,thread,generation,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{let mutation=action.mutation();let result=if owner!=account.account_key()||!account.is_active(){Err("Phiên đã thay đổi".into())}else{crate::group_content::execute(&account,&thread,&action).await.map_err(|error|error.to_string())};let _=tx.send(GuiEvent::GroupContent{owner,thread,generation,mutation,result});ctx.request_repaint();});
                        }
                        WorkerCmd::ConversationPolicy{owner,generation,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{
                                if matches!(action,crate::conversation_policy::Action::Read)&&owner==account.account_key()&&account.is_active(){if let Ok(Some(snapshot))=crate::conversation_policy::cached(&account).await{let _=tx.send(GuiEvent::ConversationPolicy{loading:true,owner:owner.clone(),generation,result:Ok(snapshot)});ctx.request_repaint();}}
                                let result=if owner!=account.account_key()||!account.is_active(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::conversation_policy::execute(&account,&action).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::ConversationPolicy{loading:false,owner,generation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::PruneExpired=>{
                            static PRUNER:tokio::sync::Semaphore=tokio::sync::Semaphore::const_new(1);
                            let Ok(permit)=PRUNER.try_acquire()else{continue;};
                            let account=session.lock().await.clone();if !account.is_active()||account.account_key().is_empty(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();let owner=account.account_key().to_owned();
                            tokio::spawn(async move{
                                let _permit=permit;
                                let result=tokio::task::spawn_blocking(move||->anyhow::Result<_>{
                                    anyhow::ensure!(account.is_active(),"Phiên đã đóng");
                                    let store=crate::store::MessageStore::open(account.history_path())?;
                                    let ids=crate::message_lifecycle::purge(&store.conn,crate::login::unix_now()*1000)?;
                                    crate::message_lifecycle::clean_restored(&account,&store.conn)?;let updates=crate::reaction_pipeline::drain(&store,&account)?;Ok((ids,updates))
                                }).await;
                                if let Ok(Ok((ids,threads)))=result {if !ids.is_empty(){let _=tx.send(GuiEvent::Expired{owner:owner.clone(),ids});ctx.request_repaint();}
if !threads.is_empty(){let _=tx.send(GuiEvent::ThreadsUpdated{threads,source:ThreadUpdateSource::History});ctx.request_repaint();}}
                            });
                        }
                        WorkerCmd::RunAutomation=>{
                            let s=session.lock().await.clone();if !s.is_active() || s.account_key().is_empty(){continue;}
                            if automation_running.swap(true,std::sync::atomic::Ordering::AcqRel){continue;}
                            let automation_guard=PollGuard(automation_running.clone());
                            let recover=!recovered_accounts.contains(s.account_key());
                            let account=s.clone();let owner=s.account_key().to_owned();
                            let due=tokio::task::spawn_blocking(move ||->anyhow::Result<_>{
                                if !account.is_active(){return Ok(None);}
                                let store=crate::store::MessageStore::open(account.history_path())?;
                                if recover{crate::tools::recover(&store,&owner)?;}
                                crate::tools::claim(&store,&owner,crate::login::unix_now())
                            }).await;
                            if matches!(&due,Ok(Ok(_))){recovered_accounts.insert(s.account_key().to_owned());}
                            else {crate::logger::log_error("scheduler","Cannot read due jobs");}
                            if let Ok(Ok(Some(task)))=due {
                                let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                                tokio::spawn(async move{
                                    let _automation=automation_guard;
                                    let Some(active)=crate::outgoing::Active::start(&task.message.client_message_id)else{
                                        let account=s.clone();let id=task.id.clone();let owner=task.owner.clone();
                                        let _=tokio::task::spawn_blocking(move||->anyhow::Result<()>{let store=crate::store::MessageStore::open(account.history_path())?;crate::tools::finish(&store,&owner,&id,"uncertain","Một lần gửi khác đang xử lý; kiểm tra hội thoại trước khi thử lại")}).await;
                                        return;
                                    };
                                    persist_outgoing(&s,&task.thread_id,&task.message).await;
                                    let _=tx.send(GuiEvent::OutgoingState{owner:task.owner.clone(),thread_id:task.thread_id.clone(),message:task.message.clone()});ctx.request_repaint();
                                    let (message,status,error)=match crate::outgoing::send(&s,&task.thread_id,&task.message).await {
                                        Ok(message)=>(message,"sent",String::new()),
                                        Err(error)=>{let error=crate::outgoing::failure_message(&error);let mut message=task.message.clone();message.delivery=crate::message_model::DeliveryState::Failed;if let Some(job)=message.outgoing_job.as_mut(){job.error=error.clone();}(message,"uncertain",error)}
                                    };
                                    persist_outgoing(&s,&task.thread_id,&message).await;
                                    let account=s.clone();let id=task.id.clone();let owner=task.owner.clone();
                                    let result=tokio::task::spawn_blocking(move ||->anyhow::Result<_>{anyhow::ensure!(account.is_active(),"Đã đăng xuất");let store=crate::store::MessageStore::open(account.history_path())?;crate::tools::finish(&store,&owner,&id,status,&error)?;Ok(super::tools_panel::ToolsResult::Tasks{tasks:crate::tools::scheduled(&store,&owner)?,paused:crate::tools::paused(&store)?})}).await;
                                    match result {Ok(Ok(result))=>{let _=tx.send(GuiEvent::Tools{owner:task.owner.clone(),result:Ok(result)});},_=>crate::logger::log_error("scheduler","Cannot finalize scheduled delivery")};
                                    drop(active);let _=tx.send(GuiEvent::OutgoingState{owner:task.owner,thread_id:task.thread_id,message});ctx.request_repaint();
                                });
                            }
                        }
                        WorkerCmd::RetryOutgoing { .. } => {},
                        WorkerCmd::CheckUpdate=>{
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();tokio::spawn(async move{let result=crate::updater::discover().await;let _=tx.send(GuiEvent::UpdateChecked(result));ctx.request_repaint();});
                        }
                        WorkerCmd::PrepareUpdate(release)=>{
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();tokio::spawn(async move{let result=crate::updater::prepare_update(release,|_,_|{}).await;let _=tx.send(GuiEvent::UpdatePrepared(result));ctx.request_repaint();});
                        }
                        WorkerCmd::LaunchUpdate(ready)=>{
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();tokio::spawn(async move{let result=tokio::task::spawn_blocking(move||crate::updater::launch_update(ready)).await.unwrap_or_else(|_|Err("Không chạy được bộ cài".into()));let _=tx.send(GuiEvent::UpdateLaunched(result));ctx.request_repaint();});
                        }
                        WorkerCmd::CopyViewerImage{owner,context}=>{
                            static COPY:tokio::sync::Semaphore=tokio::sync::Semaphore::const_new(1);
                            let Ok(slot)=COPY.try_acquire() else{let _=gui_tx.send(GuiEvent::StatusUpdate("Đang sao chép ảnh…".into()));continue;};
                            let account=session.lock().await.clone();if owner!=account.account_key()||!account.is_active(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{
                                let _slot=slot;
                                let media=crate::message_view::media(&context.message);
                                let key=crate::media_recovery::image_key(&context.thread,&context.message.id,0,true);
                                let url=media.preview.or(media.original).or(media.thumbnail).unwrap_or_default();
                                if let Err(error)=crate::conversation_policy::require_shareable(&account,&context.thread,context.group).await{let _=tx.send(GuiEvent::StatusUpdate(error.to_string()));ctx.request_repaint();return;}
                                let result=fetch_chat_image(&account,&key,&url,Some(context),&tx).await;
                                let status=match result {
                                    Ok(image)=>{let active=account.clone();let result=tokio::task::spawn_blocking(move||->anyhow::Result<()>{
                                        anyhow::ensure!(active.is_active(),"Đã đăng xuất");
                                        #[cfg(windows)]{crate::clipboard_image::write_windows(&image)}
                                        #[cfg(not(windows))]{let _=image;anyhow::bail!("Sao chép ảnh yêu cầu Windows")}
                                    }).await;
                                        if matches!(result,Ok(Ok(()))){"Đã sao chép ảnh đang xem"}else{"Không sao chép được ảnh; clipboard có thể đang bận"}
                                    },
                                    Err(_)=>"Không tải được ảnh để sao chép",
                                };
                                if account.is_active(){let _=tx.send(GuiEvent::StatusUpdate(status.into()));ctx.request_repaint();}
                            });
                        }
                        WorkerCmd::MediaNeighbor{owner,thread,anchor,next}=>{
                            let account=session.lock().await.clone();if owner!=account.account_key()||!account.is_active(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let t=thread.clone();let a=anchor.clone();let disk=account.clone();
                                let result=tokio::task::spawn_blocking(move||->anyhow::Result<_>{
                                    anyhow::ensure!(disk.is_active(),"Đã đăng xuất");
                                    let store=crate::store::MessageStore::open(disk.history_path())?;
                                    crate::tools::media_neighbor(&store,&t,&a,next)
                                }).await.map_err(|_|"Không đọc được lịch sử media".to_owned()).and_then(|r|r.map_err(|_|"Không đọc được lịch sử media".into()));
                                if account.is_active(){let _=tx.send(GuiEvent::MediaNeighbor{owner,thread,anchor,result});ctx.request_repaint();}
                            });
                        }
                        WorkerCmd::OpenDownloaded{owner,thread,path,folder}=>{
                            let account=session.lock().await.clone();
                            tokio::task::spawn_blocking(move||{if !account.is_active()||account.account_key()!=owner||thread.as_ref().is_some_and(|(id,group)|!account.hidden.view(&owner).allows(id,*group)){return;}
                                if folder{if let Some(parent)=path.parent(){crate::platform::open_folder(parent);}}else{crate::platform::open_file(&path);}});
                        }
                        WorkerCmd::DownloadMedia{mut url,name,play,open,context}=>{
                            static REQUESTS:tokio::sync::Semaphore=tokio::sync::Semaphore::const_new(4);
                            let Ok(request_slot)=REQUESTS.try_acquire() else {let _=gui_tx.send(GuiEvent::StatusUpdate("Đang có nhiều yêu cầu tải; vui lòng chờ hoặc hủy bớt".into()));continue;};
                            let account=session.lock().await.clone();if !account.is_active() || account.account_key().is_empty(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{
                                let _request_slot=request_slot;
                                if !media_context_alive(&account,context.as_ref()) { let _=tx.send(GuiEvent::StatusUpdate("Tin nhắn đã hết hạn hoặc phiên đã đóng".into()));ctx.request_repaint();return; }
                                if !play {if let Some(source)=&context {if let Err(error)=crate::conversation_policy::require_shareable(&account,&source.thread,source.group).await{let _=tx.send(GuiEvent::StatusUpdate(error.to_string()));ctx.request_repaint();return;}}}

                                let name=crate::media_download::safe_name(&name);
                                if let Some(source)=&context {
                                    if let Some(restored)=crate::desktop_backup::restored_path(&account,&source.message,false){
                                        if tokio::fs::symlink_metadata(&restored).await.is_ok_and(|meta|meta.is_file()){
                                            let account=account.clone();let tx=tx.clone();let ctx=ctx.clone();let name=name.clone();let context=context.clone();
                                            let result=tokio::task::spawn_blocking(move||->anyhow::Result<()>{
                                                anyhow::ensure!(account.is_active(),"Phiên đã đóng");
                                                let destination=if play||open{
                                                    let root=account.cache_path().parent().ok_or_else(||anyhow::anyhow!("Missing cache"))?.join("restored-open").join(crate::crypto::md5_hex(&restored.to_string_lossy()));
                                                    crate::storage::private_dir(&root)?;root.join(&name)
                                                }else{
                                                    #[cfg(windows)]{let Some(path)=rfd::FileDialog::new().set_title("Lưu tệp backup").set_file_name(&name).save_file()else{return Ok(());};path}
                                                    #[cfg(not(windows))]{anyhow::bail!("Chọn nơi lưu tệp hiện hỗ trợ Windows");}
                                                };
                                                std::fs::copy(&restored,&destination)?;
                                                if play{crate::video_player::play(&destination,&name,||media_context_alive(&account,context.as_ref()))?;}else if open && media_context_alive(&account,context.as_ref()){crate::platform::open_file(&destination);}
                                                Ok(())
                                            }).await;
                                            if !matches!(result,Ok(Ok(()))){let _=tx.send(GuiEvent::StatusUpdate("Không mở/lưu được tệp đã phục hồi từ backup".into()));ctx.request_repaint();}
                                            return;
                                        }
                                    }
                                }

                                let id=crate::chat::next_client_message_id().to_string();
                                let owner=account.account_key().to_owned();
                                let video_lease=if play {
                                    let root=account.history_path().with_extension("video-cache");let name=name.clone();let id=id.clone();
                                    tokio::task::spawn_blocking(move||crate::video_player::CachedVideo::reserve(&root,&format!("{id}-{name}"))).await.ok().and_then(Result::ok)
                                }else{None};
                                let file_entry=if open {
                                    let Some(source)=context.as_ref() else {let _=tx.send(GuiEvent::StatusUpdate("Thiếu định danh tệp đính kèm".into()));return;};
                                    let root=account.history_path().with_extension("attachment-cache");
                                    let thread=source.thread.clone();let message=source.message.id.clone();let name=name.clone();
                                    match tokio::task::spawn_blocking(move||crate::attachment_cache::Entry::reserve(&root,&thread,&message,&name)).await {
                                        Ok(Ok(Some(entry)))=>Some(entry),
                                        Ok(Ok(None))=>{let _=tx.send(GuiEvent::StatusUpdate("Cache đang xử lý tệp; vui lòng thử lại sau".into()));ctx.request_repaint();return;},
                                        _=>{let _=tx.send(GuiEvent::StatusUpdate("Không tạo được cache tệp".into()));ctx.request_repaint();return;}
                                    }
                                }else{None};
                                let (file_entry,ready)=if let Some(entry)=file_entry {
                                    match tokio::task::spawn_blocking(move||{let ready=entry.ready();(entry,ready)}).await {
                                        Ok((entry,ready))=>(Some(entry),ready),
                                        Err(_)=>{let _=tx.send(GuiEvent::StatusUpdate("Không đọc được cache tệp".into()));ctx.request_repaint();return;}
                                    }
                                }else{(None,false)};
                                if let Some(entry)=file_entry.as_ref().filter(|_|ready) {
                                    let path=entry.path().to_owned();
                                    tokio::task::spawn_blocking(move||{if account.is_active(){crate::platform::open_file(&path);}}).await.ok();
                                    return;
                                }
                                let destination=if open {file_entry.as_ref().map(|e|e.path().to_owned())}else if play {video_lease.as_ref().map(|v|v.path().to_owned())}else{
                                    let name=name.clone();tokio::task::spawn_blocking(move||{
                                        #[cfg(windows)]{rfd::FileDialog::new().set_title("Lưu tệp về máy").set_file_name(&name).save_file()}
                                        #[cfg(not(windows))]{let _=name;None}
                                    }).await.ok().flatten()
                                };
                                let Some(path)=destination else{return;};
                                let cancel=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                                let _=tx.send(GuiEvent::MediaStarted{owner:owner.clone(),thread:context.as_ref().map(|source|(source.thread.clone(),source.group)),id:id.clone(),name:name.clone(),cancel:cancel.clone()});ctx.request_repaint();
                                let visual=play||context.as_ref().is_some_and(|c|matches!(crate::message_view::media(&c.message).kind,crate::message_view::MediaKind::Photo|crate::message_view::MediaKind::Video));
                                let mut result=save_media_target(&url,&path,cancel.clone(),|bytes,total|{
                                    if !media_context_alive(&account,context.as_ref()){cancel.store(true,std::sync::atomic::Ordering::Release);}
                                    let _=tx.send(GuiEvent::MediaProgress{owner:owner.clone(),id:id.clone(),bytes,total});ctx.request_repaint();
                                },visual,video_lease.as_ref(),open).await;
                                if result.as_ref().err().and_then(|e|e.downcast_ref::<crate::media_recovery::Failure>()).is_some_and(|e|e.renew_candidate()) {
                                    if let Some(context)=context.as_ref(){
                                        if let Ok(media)=crate::media_renew::renew(&account,&context.thread,context.group,&context.message).await {
                                            if let Some(next)=media.original.clone() {let mut renewed=context.clone();if persist_media_projection(&account,&mut renewed,&media,&tx).await.is_err(){crate::logger::log_error("media renewal","Cannot persist renewed file metadata");}url=next;result=save_media_target(&url,&path,cancel.clone(),|bytes,total|{if !account.is_active(){cancel.store(true,std::sync::atomic::Ordering::Release);}let _=tx.send(GuiEvent::MediaProgress{owner:owner.clone(),id:id.clone(),bytes,total});ctx.request_repaint();},visual,video_lease.as_ref(),open).await;}
                                        }
                                    }
                                }
                                let result=result.map(|_|path.clone()).map_err(|e|e.to_string());
                                let success=result.is_ok();let _=tx.send(GuiEvent::MediaFinished{owner:owner.clone(),id:id.clone(),result});ctx.request_repaint();
                                if success && open && media_context_alive(&account,context.as_ref()){
                                    let path=path.clone();let account=account.clone();let open_context=context.clone();
                                    tokio::task::spawn_blocking(move||{if media_context_alive(&account,open_context.as_ref()){crate::platform::open_file(&path);}}).await.ok();
                                }
                                if let Some(entry)=file_entry {tokio::task::spawn_blocking(move||entry.prune()).await.ok();}
                                if success && play && account.is_active(){
                                    let result=tokio::task::spawn_blocking(move||crate::video_player::play(&path,&name,||media_context_alive(&account,context.as_ref()))).await;
                                    let _=tx.send(GuiEvent::MediaTransientClosed{owner,id});ctx.request_repaint();
                                    if !matches!(result,Ok(Ok(()))){let _=tx.send(GuiEvent::StatusUpdate("Không phát được video; kiểm tra codec hoặc thử lưu tệp".into()));ctx.request_repaint();}
                                }
                            });
                        }
                        WorkerCmd::PersistInfoDrawer {owner,open}=>{
                            let s=session.lock().await.clone();
                            if owner.is_empty() || owner!=s.account_key() || !s.is_active(){continue;}
                            // Await off-thread I/O so rapid toggles persist in command order.
                            let result=tokio::task::spawn_blocking(move||->anyhow::Result<()> {
                                anyhow::ensure!(s.is_active(),"Tài khoản đã đăng xuất");
                                let mut settings=AppSettings::load();settings.info_drawer_open=open;
                                settings.save()
                            }).await;
                            if !matches!(result,Ok(Ok(()))){let _=gui_tx.send(GuiEvent::StatusUpdate("Không lưu được trạng thái bảng thông tin hội thoại".into()));egui_ctx.request_repaint();}
                        }
                        WorkerCmd::OpenLibraryLink{owner,url}=>{
                            let s=session.lock().await.clone();if !s.is_active()||owner!=s.account_key(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{
                                let result=tokio::task::spawn_blocking(move||crate::platform::open_url(&url)).await;
                                if !matches!(result,Ok(true)){let _=tx.send(GuiEvent::StatusUpdate("Không mở được liên kết".into()));ctx.request_repaint();}
                            });
                        }
                        WorkerCmd::PrepareNotificationSound{play}=>{
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();tokio::spawn(async move{
                                let ready = crate::notification_sound::prepare().await.is_ok(); if ready&&play{crate::notification_sound::play();} let _ = tx.send(GuiEvent::NotificationSoundPrepared(ready)); ctx.request_repaint();
                                if !ready {let _=tx.send(GuiEvent::StatusUpdate("Chưa tải được âm báo Zalo; kiểm tra mạng rồi bật lại âm thanh".into()));ctx.request_repaint();}
                            });
                        }
                        WorkerCmd::CaptureScreen { owner, thread_id, is_group } => {
                            let s = session.lock().await.clone();
                            if owner != s.account_key() || !s.is_active() { continue; }
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let result = async {
                                    let mut before = crate::screen_capture::clipboard_sequence();
                                    tokio::task::spawn_blocking(crate::screen_capture::launch).await
                                        .map_err(|_| "Không mở được công cụ chụp màn hình".to_owned())?
                                        .map_err(|e| e.to_string())?;
                                    let deadline = tokio::time::Instant::now() + crate::screen_capture::WAIT;
                                    while tokio::time::Instant::now() < deadline {
                                        tokio::time::sleep(crate::screen_capture::POLL).await;
                                        if !s.is_active() { return Err("Đã dừng chụp màn hình".to_owned()); }
                                        let now = crate::screen_capture::clipboard_sequence();
                                        if crate::screen_capture::changed(before, now) {
                                            before = now;
                                            // The clipboard may change before all image formats are ready.
                                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                            if let Some(image) = tokio::task::spawn_blocking(super::messages::read_clipboard_image_to_png).await.ok().flatten() {
                                                return Ok(image);
                                            }
                                        }
                                    }
                                    Err("Không nhận được ảnh chụp trong 60 giây".to_owned())
                                }.await;
                                let _ = tx.send(GuiEvent::ScreenCaptureFinished { owner, thread_id, is_group, result });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::ReadClipboardImage{owner,thread} => {
                            let account=session.lock().await.clone();if owner!=account.account_key()||!account.is_active(){continue;}
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let image = tokio::task::spawn_blocking(super::messages::read_clipboard_image_to_png).await.ok().flatten();
                                if account.is_active(){let _ = tx.send(GuiEvent::ClipboardImageRead{owner,thread,image});ctx.request_repaint();}
                            });
                        }
                        WorkerCmd::SetConversationPreference { owner, thread_id, is_group, value } => {
                            let s = session.lock().await.clone();
                            if owner.is_empty() || owner != s.account_key() || !s.is_active() { continue; }
                            let owner = owner.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            let live = live_updates.clone(); let mute_sync = mute_sync.clone();
                            // Runs off the command loop: the mute flag is account-wide and waits on the server.
                            tokio::spawn(async move {
                                let account = s.clone(); let id = thread_id.clone();
                                let current = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                    Ok(crate::store::MessageStore::open(account.history_path())?.conversation_preferences()?.get(&id).copied().unwrap_or_default())
                                }).await;
                                let Ok(Ok(current)) = current else {
                                    let _ = tx.send(GuiEvent::StatusUpdate("Không thể lưu tùy chọn hội thoại".into())); ctx.request_repaint(); return;
                                };
                                if current.muted != value.muted {
                                    mute_sync.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                    // Fail closed: never show a mute state the server did not accept.
                                    if crate::mute_sync::set_remote_mute(&s, &thread_id, is_group, value.muted).await.is_err() {
                                        let _ = tx.send(GuiEvent::StatusUpdate("Không thể đổi thông báo trên máy chủ Zalo; giữ nguyên trạng thái".into()));
                                        ctx.request_repaint(); return;
                                    }
                                }
                                let result=tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                    anyhow::ensure!(s.is_active(), "Phiên đã kết thúc");
                                    let store=crate::store::MessageStore::open(s.history_path())?;
                                    store.set_conversation_preference(&thread_id,value)?;
                                    store.conversation_preferences()
                                }).await;
                                match result {
                                    Ok(Ok(values)) => { live.set_muted(&values); let _ = tx.send(GuiEvent::ConversationPreferences { owner, values }); }
                                    _ => { let _ = tx.send(GuiEvent::StatusUpdate("Không thể lưu tùy chọn hội thoại".into())); }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::DeleteConversation{owner,thread}=>{
                            let account=session.lock().await.clone();
                            if owner!=account.account_key()||!account.is_active(){continue;}
                            let result=async {
                                crate::chat::delete_conversation(&account,&thread).await?;
                                let id=thread.id.clone();let disk=account.clone();let visible=thread.messages.clone();
                                tokio::task::spawn_blocking(move||->anyhow::Result<()>{
                                    anyhow::ensure!(disk.is_active(),"Đã đăng xuất");
                                    let mut store=crate::store::MessageStore::open(disk.history_path())?;
                                    store.delete_conversation(&id,&visible)?;
                                    let mut cache=crate::chat::load_threads_cache(&disk);cache.retain(|t|t.id!=id);crate::chat::save_threads_cache(&disk,&cache);Ok(())
                                }).await??;
                                Ok::<_,anyhow::Error>(())
                            }.await;
                            match result{Ok(())=>{let _=gui_tx.send(GuiEvent::ThreadDeleted{owner,thread:thread.id});},Err(error)=>{let _=gui_tx.send(GuiEvent::StatusUpdate(error.to_string()));}}
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::ClearThreadHistory { owner, thread_id } => {
                            let s = session.lock().await.clone();
                            if owner.is_empty() || owner != s.account_key() || !s.is_active() { continue; }
                            let id=thread_id.clone();
                            let result=tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                                anyhow::ensure!(s.is_active(), "Phiên đã kết thúc");
                                let mut store=crate::store::MessageStore::open(s.history_path())?;
                                store.clear_thread_history(&id)?;
                                let mut cache=crate::chat::load_threads_cache(&s);
                                if let Some(thread)=cache.iter_mut().find(|t|t.id==id) {
                                    thread.unread.read_visible(&thread.messages);
                                    thread.messages.clear(); thread.preview.clear(); thread.time.clear(); thread.presentation=Default::default();
                                }
                                crate::chat::save_threads_cache(&s,&cache);
                                Ok(())
                            }).await;
                            match result {
                                Ok(Ok(())) => { let _ = gui_tx.send(GuiEvent::ThreadHistoryCleared { owner, thread_id }); }
                                _ => { let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể xóa lịch sử trên máy này".into())); }
                            }
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::PersistThreads { owner, threads } => {
                            let s = session.lock().await.clone();
                            if owner.is_empty() || owner != s.account_key() || !s.is_active() { continue; }
                            live_updates.seed(&threads);
                            // Await writes in command order: logout cannot race a detached snapshot write.
                            let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                                if !s.is_active() { return Ok(()); }
                                let mut store = crate::store::MessageStore::open(s.history_path())?;
                                crate::store::persist_local_threads(&mut store, &threads)?;
                                crate::chat::save_threads_cache(&s, &threads);
                                Ok(())
                            }).await;
                            if !matches!(result, Ok(Ok(()))) {
                                let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể lưu lịch sử hội thoại".into()));
                            }
                        }
                        WorkerCmd::ResumeUpload { upload_id } => {
                            let s = session.lock().await.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                // Load upload state from store
                                let result = tokio::task::spawn_blocking({
                                    let s = s.clone();
                                    move || -> anyhow::Result<Option<crate::upload::ResumableUploadState>> {
                                        if !s.is_active() {
                                            anyhow::bail!("Phiên đã hết hạn");
                                        }
                                        crate::store::MessageStore::open(s.history_path())?
                                            .load_upload_state(&upload_id)
                                    }
                                })
                                .await
                                .map_err(|_| "Không thể đọc trạng thái tải lên".to_string())
                                .and_then(|r| r.map_err(|_| "Không thể đọc trạng thái tải lên".to_string()));

                                if let Ok(Some(_state)) = result {
                                    let _ = tx.send(GuiEvent::StatusUpdate("Tiếp tục tải lên (v0.016)".into()));
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::CancelUpload { upload_id } => {
                            let s = session.lock().await.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                // Delete upload state from store
                                let result = tokio::task::spawn_blocking({
                                    let s = s.clone();
                                    let id = upload_id.clone();
                                    move || -> anyhow::Result<()> {
                                        if !s.is_active() {
                                            anyhow::bail!("Phiên đã hết hạn");
                                        }
                                        crate::store::MessageStore::open(s.history_path())?
                                            .delete_upload_state(&id)
                                    }
                                })
                                .await
                                .map_err(|_| "Không thể hủy tải lên".to_string())
                                .and_then(|r| r.map_err(|_| "Không thể hủy tải lên".to_string()));

                                if result.is_ok() {
                                    let _ = tx.send(GuiEvent::StatusUpdate("Đã hủy tải lên".into()));
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Logout => {
                            // A19: Cancel QR task and bump auth attempt to invalidate all pending auth responses
                            if let Some(task) = current_qr_task.take() { task.abort(); }
                            current_auth_attempt.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                            call_bridge.end_call();
                            crate::queue_cursors::flush_all().await;
                            let account = session.lock().await.account_key().to_owned();
                            if !account.is_empty() {
                                let _ = tokio::task::spawn_blocking(move || crate::photo_quality::remove_account_copies(&account)).await;
                            }
                            session.lock().await.logout();
                            if let Err(e) = crate::storage::remove_session() {
                                let _ = gui_tx.send(GuiEvent::StatusUpdate(format!("Không thể xóa phiên: {e}")));
                                continue;
                            }
                            let _ = gui_tx.send(GuiEvent::LoggedOut);
                            egui_ctx.request_repaint();
                            // Dropping this runtime cancels all old network tasks. A fresh UI channel
                            // prevents queued events from the previous account reaching the next one.
                            break;
                        }
                        WorkerCmd::SwitchProfile { owner, target_uid } => {
                            // WP16-3: Profile switching integration
                            let current_account = session.lock().await.account_key().to_owned();
                            if current_account != owner {
                                let _ = gui_tx.send(GuiEvent::StatusUpdate("Tài khoản đã thay đổi".into()));
                                continue;
                            }

                            let target_uid_clone = target_uid.clone();
                            // Load the target profile from ProfileRegistry
                            let profile_result = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                let registry = crate::profiles::ProfileRegistry::load()?;
                                let profile = registry.profiles.values()
                                    .find(|p| p.uid == target_uid)
                                    .ok_or_else(|| anyhow::anyhow!("Profile not found"))?
                                    .clone();
                                Ok(profile)
                            }).await;

                            match profile_result {
                                Ok(Ok(profile)) => {
                                    // Logout current session
                                    session.lock().await.logout();

                                    // Load new session from profile
                                    let load_result = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                        let mut new_session = crate::session::Session::new(false, false, false)?;
                                        let session_path = profile.session_file.to_string_lossy().to_string();
                                        new_session.load_from_file(&session_path)?;
                                        Ok(new_session)
                                    }).await;

                                    match load_result {
                                        Ok(Ok(new_session)) => {
                                            *session.lock().await = new_session;
                                            let _ = gui_tx.send(GuiEvent::ProfileSwitched { uid: target_uid_clone });
                                            egui_ctx.request_repaint();
                                            let _ = gui_tx.send(GuiEvent::StatusUpdate("Đã chuyển tài khoản".into()));
                                        }
                                        Ok(Err(e)) => {
                                            let _ = gui_tx.send(GuiEvent::StatusUpdate(format!("Không thể tải phiên: {e}")));
                                        }
                                        Err(_) => {
                                            let _ = gui_tx.send(GuiEvent::StatusUpdate("Lỗi tải phiên".into()));
                                        }
                                    }
                                }
                                Ok(Err(e)) => {
                                    let _ = gui_tx.send(GuiEvent::StatusUpdate(format!("Không tìm thấy tài khoản: {e}")));
                                }
                                Err(_) => {
                                    let _ = gui_tx.send(GuiEvent::StatusUpdate("Lỗi đọc danh sách tài khoản".into()));
                                }
                            }
                        }
                        WorkerCmd::PersistSession => {
                            let s = session.lock().await;
                            if s.login.is_some() {
                                if let Err(e) = s.save_to_file("session.json") {
                                    let _ = gui_tx.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                }
                            }
                        }
                        WorkerCmd::RequestQr => {
                            // A19: Cancel current QR task and bump auth attempt generation
                            if let Some(task) = current_qr_task.take() {
                                task.abort();
                            }
                            let attempt_id = current_auth_attempt.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

                            let session_clone = session.clone();
                            let poll_clone = poll_state.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            let commands = commands.clone();
                            let auth_attempt_clone = current_auth_attempt.clone();

                            let handle = tokio::spawn(async move {
                                {
                                    let mut s = session_clone.lock().await;
                                    s.logout();
                                }
                                {
                                    let mut ps = poll_clone.lock().await;
                                    *ps = crate::chat::PollState::default();
                                }
                                let _ = gui_tx_clone
                                    .send(GuiEvent::StatusUpdate("Đang xin mã QR mới…".into()));
                                ctx_clone.request_repaint();

                                let s_snap = { session_clone.lock().await.clone() };
                                let q_res = crate::qr::request_qr(&s_snap, "vi").await;

                                match q_res {
                                    Ok(q) => {
                                        if let Some(payload) = q.payload() {
                                            if let Ok(code) = qrcode::QrCode::new(payload.as_bytes())
                                            {
                                                let w = code.width();
                                                let quiet_zone = 4;
                                                let total_w = w + quiet_zone * 2;
                                                let mut pixels =
                                                    Vec::with_capacity(total_w * total_w * 4);
                                                for y in 0..total_w {
                                                    for x in 0..total_w {
                                                        let in_qr = x >= quiet_zone
                                                            && x < quiet_zone + w
                                                            && y >= quiet_zone
                                                            && y < quiet_zone + w;
                                                        let dark = in_qr
                                                            && (code[(
                                                                x - quiet_zone,
                                                                y - quiet_zone,
                                                            )] == qrcode::Color::Dark);
                                                        let c = if dark {
                                                            [15, 23, 42, 255]
                                                        } else {
                                                            [255, 255, 255, 255]
                                                        };
                                                        pixels.extend_from_slice(&c);
                                                    }
                                                }
                                                let img = egui::ColorImage::from_rgba_unmultiplied(
                                                    [total_w, total_w],
                                                    &pixels,
                                                );
                                                let _ = gui_tx_clone.send(GuiEvent::QrReady {
                                                    qr_login: q.clone(),
                                                    qr_image: img,
                                                });
                                                ctx_clone.request_repaint();

                                                let scan_res =
                                                    crate::qr::wait_for_scan(&s_snap, &q).await;

                                                if let Ok(scanned) = scan_res {
                                                    let name = scanned
                                                        .get("display_name")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("Zalo User");
                                                    let _ =
                                                        gui_tx_clone.send(GuiEvent::QrScanned {
                                                            name: name.to_string(),
                                                        });
                                                    ctx_clone.request_repaint();

                                                    let confirm_res =
                                                        crate::qr::confirm(&s_snap, &q).await;

                                                    if let Ok(resp) = confirm_res {
                                                        // A19: Validate auth attempt before committing QR login result
                                                        let current = auth_attempt_clone.load(std::sync::atomic::Ordering::SeqCst);
                                                        if current != attempt_id {
                                                            // Stale response — another auth started, discard this result
                                                            return;
                                                        }

                                                        let mut s = session_clone.lock().await;
                                                        if let Some(sek) =
                                                            crate::qr::extract_zpw_sek(&resp)
                                                        {
                                                            s.set_zpw_sek(sek);
                                                        }
                                                        if let Some(data) = resp.get("data") {
                                                            crate::login::apply_login_config(
                                                                &mut s, data,
                                                            );
                                                        }
                                                        if let Err(e) = s.save_to_file("session.json") {
                                                            let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                                        }

                                                        if let Ok(info) =
                                                            crate::login::get_login_info(&s).await
                                                        {
                                                            if let Some(cfg) = info.get("data") {
                                                                crate::login::apply_login_config(
                                                                    &mut s, cfg,
                                                                );
                                                                s.apply_server_info(cfg);
                                                            }
                                                        }
                                                        let _ = s.sync_profile_me().await;
                                                        if let Err(e) = s.save_to_file("session.json") {
                                                            let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                                        }

                                                        let uin = s
                                                            .login
                                                            .as_ref()
                                                            .map(|l| l.uin.clone())
                                                            .unwrap_or_default();
                                                        let dname = s
                                                            .login
                                                            .as_ref()
                                                            .map(|l| l.display_name.clone())
                                                            .unwrap_or_default();
                                                        let u_ava = s
                                                            .login
                                                            .as_ref()
                                                            .map(|l| l.avatar.clone())
                                                            .filter(|a| !a.is_empty());
                                                        let _ = gui_tx_clone.send(
                                                            GuiEvent::LoginSuccess {
                                                                name: dname,
                                                                uin,
                                                                avatar: u_ava,
                                                            },
                                                        );
                                                        ctx_clone.request_repaint();

                                                        if let Some(tx) = commands.upgrade() {
                                                            let _ = tx.send(WorkerCmd::RefreshThreads);
                                                        }

                                                        let s_kw = s.clone();
                                                        let tx_kw = gui_tx_clone.clone();
                                                        let ctx_kw = ctx_clone.clone();
                                                        tokio::spawn(async move {
                                                            if let Ok(kws) = crate::chat::fetch_sticker_suggest_keywords(&s_kw).await {
                                                                if !kws.is_empty() {
                                                                    let _ = tx_kw.send(GuiEvent::StickerSuggestKeywordsLoaded(kws));
                                                                    ctx_kw.request_repaint();
                                                                }
                                                            }
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // A19: Validate auth attempt before showing QR error
                                        let current = auth_attempt_clone.load(std::sync::atomic::Ordering::SeqCst);
                                        if current != attempt_id {
                                            // Stale response — discard
                                            return;
                                        }

                                        let _ = gui_tx_clone
                                            .send(GuiEvent::LoginFailed(e.to_string()));
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                            current_qr_task = Some(handle);
                        }
                        WorkerCmd::CancelPhoneHistory => {
                            phone_sync_cancel.store(true,std::sync::atomic::Ordering::Relaxed);
                            if phone_sync_running.load(std::sync::atomic::Ordering::SeqCst){let _=gui_tx.send(GuiEvent::PhoneSyncStatus{running:true,message:"Đang dừng đồng bộ và dọn tác vụ; giữ dữ liệu đã lưu…".into()});egui_ctx.request_repaint();}
                        }
                        command @ (WorkerCmd::SyncPhoneHistory{..} | WorkerCmd::RescanPhoneHistory{..} | WorkerCmd::SyncPhoneRecent | WorkerCmd::SyncPhoneLegacy) => {
                            let legacy=matches!(command,WorkerCmd::SyncPhoneLegacy);
                            let full_rescan=matches!(command,WorkerCmd::RescanPhoneHistory{..});
                            let recent_only=matches!(command,WorkerCmd::SyncPhoneRecent);
                            let range_days=match command {WorkerCmd::SyncPhoneHistory{range_days}|WorkerCmd::RescanPhoneHistory{range_days}=>range_days,_=>0};
                            if phone_sync_running.swap(true,std::sync::atomic::Ordering::SeqCst) {
                                let _=gui_tx.send(GuiEvent::PhoneSyncStatus{running:true,message:"Phiên trước chưa kết thúc; chờ dọn tác vụ xong rồi bấm đồng bộ lại".into()});egui_ctx.request_repaint();continue;
                            }
                            phone_sync_cancel.store(false,std::sync::atomic::Ordering::Relaxed);
                            let cancel=phone_sync_cancel.clone();let running=phone_sync_running.clone();
                            let transport=phone_transport_rx.borrow().clone();
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let _=tx.send(GuiEvent::PhoneSyncStatus{running:true,message:"Đang chuẩn bị đồng bộ từ điện thoại…".into()});
                            ctx.request_repaint();
                            tokio::spawn(async move {
                                let _guard=PollGuard(running);
                                let progress_tx=tx.clone();let progress_ctx=ctx.clone();let progress_cancel=cancel.clone();
                                let result=if let Some(transport)=transport {
                                    if legacy {
                                        crate::phone_sync::legacy::download(&account,transport,cancel,
                                            move |message|{if !progress_cancel.load(std::sync::atomic::Ordering::Acquire){let _=progress_tx.send(GuiEvent::PhoneSyncStatus{running:true,message});progress_ctx.request_repaint();}},
                                            |thread|{if deliver_thread_updates() {let _=tx.send(GuiEvent::ThreadsUpdated{threads:vec![thread],source:ThreadUpdateSource::History});ctx.request_repaint();}}).await
                                    } else {
                                    crate::phone_sync::download(&account,transport,cancel,full_rescan,recent_only,range_days,
                                        move |message|{if !progress_cancel.load(std::sync::atomic::Ordering::Acquire){let _=progress_tx.send(GuiEvent::PhoneSyncStatus{running:true,message});progress_ctx.request_repaint();}},
                                        |thread|{if deliver_thread_updates() {let _=tx.send(GuiEvent::ThreadsUpdated{threads:vec![thread],source:ThreadUpdateSource::History});ctx.request_repaint();}}).await
                                    }
                                }else{Err(anyhow::anyhow!("WebSocket chưa sẵn sàng; chờ kết nối rồi thử lại"))};
                                let message=result.unwrap_or_else(|e|format!("Đồng bộ điện thoại đã dừng: {e}"));
                                let _=tx.send(GuiEvent::PhoneSyncStatus{running:false,message});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::PickDesktopBackup{owner,generation}=>{
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{let path=tokio::task::spawn_blocking(||tools_file(false,"Chọn backup Desktop V2",&["zip"])).await.unwrap_or(None);let _=tx.send(GuiEvent::DesktopBackupPath{owner,generation,path});ctx.request_repaint();});
                        }
                        WorkerCmd::DesktopBackup{owner,generation,path,key,cancel,expected}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{
                                let imported=expected.is_some();let wanted=owner.clone();
                                let result=tokio::task::spawn_blocking(move||->anyhow::Result<_>{
                                    anyhow::ensure!(account.is_active() && account.account_key()==wanted,"Phiên tài khoản đã thay đổi");
                                    crate::desktop_backup::execute(&path,&key,&account,cancel,expected.as_ref())
                                }).await.map_err(|_|"Không xử lý được backup".into()).and_then(|result|result.map_err(|error|error.to_string()));
                                let _=tx.send(GuiEvent::DesktopBackup{owner,generation,imported,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Hidden {owner,generation,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let result=if owner!=account.account_key(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::hidden_chat::execute(&account,&action).await.map(|_|()).map_err(|error|error.to_string())};
                                let view=account.hidden.view(account.account_key());let _=tx.send(GuiEvent::Hidden {owner,generation:Some(generation),view,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::GroupEntry {owner,generation,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let mutation=action.mutation();
                                let result=if owner!=account.account_key(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::group_entry::execute(&account,&action).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::GroupEntry {owner,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::AvatarHistory {owner,generation,action}=>{
                            let shared=session.clone();let account=shared.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_gate.lock().await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                let mutation=action.mutation();
                                let result=if owner!=account.account_key(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::avatar_history::execute(&account,&action).await.map_err(|error|error.to_string())};
                                if let Ok(reply)=&result {if let Some(profile)=&reply.profile {let mut current=shared.lock().await;if current.is_active() && current.account_key()==owner {if let Some(login)=current.login.as_mut(){if let Some(avatar)=&profile.avatar{login.avatar=avatar.clone();}}}}}
                                let _=tx.send(GuiEvent::AvatarHistory {owner,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::PrepareAvatar {owner,generation,path}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let result=if owner!=account.account_key() || !account.is_active(){Err("Phiên tài khoản đã thay đổi".into())}else{tokio::task::spawn_blocking(move ||crate::avatar::prepare(&path)).await.map_err(|_|"Không xử lý được ảnh đại diện".to_owned()).and_then(|result|result.map(std::sync::Arc::new).map_err(|error|error.to_string()))};
                                let _=tx.send(GuiEvent::AvatarPrepared {owner,generation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::UploadAvatar {owner,generation,target,draft,client}=>{
                            let shared=session.clone();let account=shared.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_gate.lock().await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                let result=if owner!=account.account_key(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::avatar::upload(&account,&target,&draft,&client).await.map_err(|error|error.to_string())};
                                if let Ok(Some(profile))=&result {
                                    let mut current=shared.lock().await;if current.is_active() && current.account_key()==owner {if let Some(login)=current.login.as_mut(){login.display_name=profile.name.clone();if let Some(avatar)=&profile.avatar{login.avatar=avatar.clone();}}}
                                }
                                let _=tx.send(GuiEvent::AvatarUploaded {owner,generation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Profile {owner,generation,action}=>{
                            let shared=session.clone();let account=shared.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_gate.lock().await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                let mutation=!matches!(action,crate::profile::Action::Read);
                                let result=if owner!=account.account_key(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::profile::execute(&account,&action).await.map_err(|error|error.to_string())};
                                if let Ok(profile)=&result {
                                    let mut current=shared.lock().await;
                                    if current.is_active() && current.account_key()==owner {if let Some(login)=current.login.as_mut(){login.display_name=profile.name.clone();if let Some(avatar)=&profile.avatar{login.avatar=avatar.clone();}}}
                                }
                                let _=tx.send(GuiEvent::Profile {owner,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Labels {owner,generation,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_gate.lock().await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);impl Drop for Refresh {fn drop(&mut self){self.0.notify_one();}}let _refresh=(!matches!(action,crate::labels::Action::Read)).then(||Refresh(metadata_notify));
                                let mutation=!matches!(action,crate::labels::Action::Read);
                                let result=if owner!=account.account_key(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::labels::execute(&account,&action).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::Labels {owner,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Privacy {owner,generation,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_gate.lock().await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);impl Drop for Refresh {fn drop(&mut self){self.0.notify_one();}}let _refresh=(matches!(action,crate::privacy::Action::Set {..})).then(||Refresh(metadata_notify));
                                let mutation=!matches!(action,crate::privacy::Action::Read);
                                let result=if owner!=account.account_key() {Err("Phiên tài khoản đã thay đổi".into())}else{crate::privacy::execute(&account,&action).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::Privacy {owner,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::ManualMark {owner,generation,target,group,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let mutation=!matches!(action,crate::unread::MarkAction::Refresh);
                                let result=if owner!=account.account_key() {Err("Phiên tài khoản đã thay đổi".into())}else{crate::unread::update_manual_mark(&account,&target,group,&action).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::ManualMarks {owner,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Archive {owner,generation,target,group,archived}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_gate.lock().await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);impl Drop for Refresh {fn drop(&mut self){self.0.notify_one();}}let _refresh=(archived.is_some()).then(||Refresh(metadata_notify));
                                let result=if owner!=account.account_key() {Err("Phiên tài khoản đã thay đổi".into())}else {crate::archive::execute(&account,&target,group,archived).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::Archive {owner,generation,mutation:archived.is_some(),result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Contact {owner,target,generation,action} => {
                            let account=session.lock().await.clone(); let tx=gui_tx.clone(); let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_gate.lock().await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);impl Drop for Refresh {fn drop(&mut self){self.0.notify_one();}}let _refresh=(action.mutation()).then(||Refresh(metadata_notify));
                                let mutation=action.mutation();
                                let result=if owner!=account.account_key() { Err("Phiên tài khoản đã thay đổi".into()) } else { crate::contact_actions::execute(&account,&target,&action).await.map_err(|error|error.to_string()) };
                                let _=tx.send(GuiEvent::Contact {owner,target,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Group {owner,target,generation,action} => {
                            let s=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let mutation=action.mutation();let revision=s.group_revision.load(std::sync::atomic::Ordering::SeqCst);
                                let mut result=if owner!=s.account_key() || !s.is_active(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::groups::execute(&s,&target,&action).await.map_err(|error|error.to_string())};
                                if !mutation && revision!=s.group_revision.load(std::sync::atomic::Ordering::SeqCst){result=Err("Quyền nhóm đã thay đổi trong lúc tải; hãy tải lại".into());}
                                let _=tx.send(GuiEvent::Group{owner,target,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::FetchGroupPolicy { group_id } => {
                            let account = session.lock().await.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let owner = account.account_key().to_owned();
                                let revision=account.group_revision.load(std::sync::atomic::Ordering::SeqCst);
                                let result = async {
                                    let snapshot = crate::group_state::snapshot(&account, &group_id, false).await?;
                                    if snapshot.removed { return Ok(crate::chat::GroupSendPolicy { locked: true, managers: Vec::new() }); }
                                    crate::chat::GroupSendPolicy::parse(&snapshot.info).ok_or_else(|| anyhow::anyhow!("Chưa xác minh được quyền nhóm"))
                                }.await.map_err(|error: anyhow::Error| error.to_string());
                                if account.is_active() { let _ = tx.send(GuiEvent::GroupPolicy { owner, group_id, revision, result }); ctx.request_repaint(); }
                            });
                        }
                        WorkerCmd::FetchGroupMembers { group_id } => {
                            let s = session.lock().await.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let owner = s.account_key().to_owned();
                                let revision=s.group_revision.load(std::sync::atomic::Ordering::SeqCst);
                                let mut result = crate::chat::group_members(&s, &group_id).await.map_err(|e| e.to_string());
                                if revision!=s.group_revision.load(std::sync::atomic::Ordering::SeqCst){result=Err("Thành viên/quyền nhóm đã thay đổi; tải lại danh sách".into());}
                                let _ = tx.send(GuiEvent::GroupMembersLoaded { owner, group_id, result });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::SendTyping { thread_id, is_group } => {
                            let s = session.lock().await.clone();
                            tokio::spawn(async move {
                                if !s.hidden.view(s.account_key()).allows(&thread_id,is_group) || !s.inbox_policy.read().map(|policy|policy.permits_typing(&thread_id,is_group)).unwrap_or(false){return;}
                                if let Err(e) = crate::presence::send_typing(&s, &thread_id, is_group).await {
                                    crate::logger::log_error("presence::typing", &e.to_string());
                                }
                            });
                        }
                        WorkerCmd::CancelHistorySync => {
                            history_sync_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        WorkerCmd::SyncHistory { threads, retry_filtered } => {
                            if history_sync_running.swap(true, std::sync::atomic::Ordering::SeqCst) { continue; }
                            history_sync_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
                            let running = history_sync_running.clone();
                            let cancel = history_sync_cancel.clone();
                            let s = session.lock().await.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            let _ = tx.send(GuiEvent::HistorySyncStatus { running: true, message: "Đang tải dữ liệu máy chủ còn cung cấp (nhóm và Cloud)…".into() });
                            ctx.request_repaint();
                            tokio::spawn(async move {
                                let result = crate::history_sync::download(&s, threads, cancel, retry_filtered,
                                    |message| { if deliver_thread_updates() { let _ = tx.send(GuiEvent::HistorySyncStatus { running: true, message }); ctx.request_repaint(); } },
                                    |thread| { if deliver_thread_updates() { let _ = tx.send(GuiEvent::ThreadsUpdated {threads:vec![thread],source:ThreadUpdateSource::History}); ctx.request_repaint(); } }).await;
                                running.store(false, std::sync::atomic::Ordering::SeqCst);
                                let message = result.unwrap_or_else(|e| format!("Lỗi tải lịch sử: {e}"));
                                let _ = tx.send(GuiEvent::HistorySyncStatus { running: false, message });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Board{owner,thread,group,generation,action} => {
                            let s=session.lock().await.clone();if !s.is_active()||owner!=s.account_key(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{
                                let list=matches!(action,crate::board::Action::List{..});
                                let mutation=matches!(action,crate::board::Action::Save{..}|crate::board::Action::Remove{..}|crate::board::Action::PinMessage{..}|crate::board::Action::Unpin{..});
                                let result=crate::board::execute(&s,&thread,group,&action).await.map_err(|e|e.to_string());
                                if s.is_active(){let _=tx.send(GuiEvent::Board{owner,thread,generation,mutation,list,result});ctx.request_repaint();}
                            });
                        }
                        WorkerCmd::SetServerPin{owner,thread,group,pinned} => {
                            let s=session.lock().await.clone();if !s.is_active()||owner!=s.account_key(){continue;}
                            let gate=pin_gate.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let _guard=gate.lock().await;
                                match crate::server_pins::set(&s,&thread,group,pinned).await {
                                    Ok(snapshot)=>publish_pins(&s,snapshot,&tx).await,
                                    Err(_)=>{
                                        if let Ok(snapshot)=crate::server_pins::fetch(&s).await {publish_pins(&s,snapshot,&tx).await;}
                                        let _=tx.send(GuiEvent::StatusUpdate("Chưa xác nhận thay đổi ghim; đã thử tải lại trạng thái server".into()));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::RefreshThreads => {
                            let s=session.lock().await.clone();let gate=pin_gate.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let _guard=gate.lock().await;
                                if !s.is_active()||s.account_key().is_empty(){return;}
                                let cached=s.clone();
                                if let Ok(Ok(Some(snapshot)))=tokio::task::spawn_blocking(move||->anyhow::Result<Option<crate::server_pins::Snapshot>>{
                                    use rusqlite::OptionalExtension;
                                    let store=crate::store::MessageStore::open(cached.history_path())?;
                                    let raw:Option<String>=store.conn.query_row("SELECT value FROM tool_config WHERE key='server_pins'",[],|r|r.get(0)).optional()?;
                                    Ok(raw.and_then(|s|serde_json::from_str(&s).ok()))
                                }).await {let _=tx.send(GuiEvent::ServerPins{owner:s.account_key().into(),snapshot});}
                                match crate::server_pins::fetch(&s).await {
                                    Ok(snapshot)=>publish_pins(&s,snapshot,&tx).await,
                                    Err(_)=>{let _=tx.send(GuiEvent::StatusUpdate("Chưa tải được danh sách ghim server; giữ bản đã lưu".into()));}
                                }
                                ctx.request_repaint();
                            });
                            let s_pref=session.lock().await.clone();
                            if !s_pref.account_key().is_empty() && s_pref.is_active() {
                                let owner=s_pref.account_key().to_owned();
                                match tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                    let preferences = crate::store::MessageStore::open(s_pref.history_path())?.conversation_preferences()?;
                                    Ok((preferences, crate::chat::load_threads_cache(&s_pref)))
                                }).await {
                                    Ok(Ok((values,cached))) => { live_updates.seed(&cached); live_updates.set_muted(&values); let _ = gui_tx.send(GuiEvent::ConversationPreferences {owner,values}); }
                                    _ => { let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể đọc tùy chọn hội thoại".into())); }
                                }
                                let s_mute = session.lock().await.clone();
                                tokio::spawn(sync_mute_snapshot(s_mute, mute_sync.clone(), gui_tx.clone(), live_updates.clone(), egui_ctx.clone()));
                            }

                            if refresh_in_flight.swap(true, std::sync::atomic::Ordering::SeqCst) { continue; }
                            let refresh_guard = PollGuard(refresh_in_flight.clone());
                            let session_clone = session.clone();
                            let poll_clone = poll_state.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            let known = known_threads.clone();
                            let poll_delivery = poll_delivery.clone();
                            tokio::spawn(async move {
                                let _refresh_guard = refresh_guard;
                                let mut notice=RefreshNotice {sender:gui_tx_clone.clone(),ctx:ctx_clone.clone(),message:"Chưa hoàn tất cập nhật hội thoại".into()};
                                let _=gui_tx_clone.send(GuiEvent::ConversationRefreshStatus {running:true,message:"Đang cập nhật hội thoại và tin nhắn gần đây…".into()});
                                ctx_clone.request_repaint();
                                let mut s = session_clone.lock().await.clone();
                                if s.login.is_some() {
                                    let status = s.validate_session().await;
                                    if let crate::session::SessionStatus::Unreachable(reason) = &status {
                                        notice.message=format!("Chưa thể kết nối; giữ phiên và lịch sử cục bộ: {reason}");
                                        ctx_clone.request_repaint();
                                        return;
                                    }
                                    if status == crate::session::SessionStatus::Expired {
                                        notice.message="Phiên đã hết hạn; cần đăng nhập lại để đồng bộ".into();
                                        if let Err(e) = crate::storage::remove_session() { eprintln!("Cannot remove session: {e}"); }
                                        {
                                            let mut sl = session_clone.lock().await;
                                            sl.logout();
                                        }
                                        let _ = gui_tx_clone.send(GuiEvent::SessionExpired(
                                            "Phiên đăng nhập (zpw_sek) đã hết hạn trên máy chủ Zalo"
                                                .into(),
                                        ));
                                        ctx_clone.request_repaint();
                                        return;
                                    }
                                    if let Some((uid, name)) = s.sync_profile_me().await {
                                        {
                                            let mut sl = session_clone.lock().await;
                                            if let Some(ref mut li) = sl.login {
                                                li.uin = uid.clone();
                                                if !name.is_empty() {
                                                    li.display_name = name.clone();
                                                }
                                            }
                                            if let Err(e) = sl.save_to_file("session.json") {
                                                let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                            }
                                        }
                                        let u_ava = s
                                            .login
                                            .as_ref()
                                            .map(|l| l.avatar.clone())
                                            .filter(|a| !a.is_empty());
                                        let _ = gui_tx_clone.send(GuiEvent::LoginSuccess {
                                            name,
                                            uin: uid,
                                            avatar: u_ava,
                                        });
                                    }
                                    let mut list = match crate::chat::get_friends(&s).await {
                                        Ok(list) => list,
                                        Err(e) => {
                                            notice.message=format!("Cập nhật hội thoại thất bại: {e}");
                                            ctx_clone.request_repaint();
                                            return;
                                        }
                                    };
                                    let account = s.clone();
                                    let (hydrated, preloaded) = match tokio::task::spawn_blocking(move || {
                                        let mut list = list;
                                        let marks = crate::store::MessageStore::open(account.history_path())
                                            .and_then(|store| store.hydrate_refresh(&mut list, crate::store::MessageStore::PRELOAD_MSGS));
                                        (list, marks.unwrap_or_default())
                                    }).await {
                                        Ok(snapshot) => snapshot,
                                        Err(_) => { notice.message = "Không đọc được lịch sử cục bộ; thử cập nhật lại".into(); return; }
                                    };
                                    list = hydrated;
                                    let Ok(_cycle) = poll_delivery.acquire_owned().await else { return; };
                                    let (recent_result, processed_state, before_state)={
                                        let mut ps = poll_clone.lock().await;
                                        // Resume lp_chat where this account stopped (Zalo PC `init`).
                                        if *ps == crate::chat::PollState::default() {
                                            if let Some(saved) = crate::queue_cursors::poll_start(&s).await {
                                                *ps = saved;
                                            }
                                        }
                                        let before = ps.clone();
                                        let result = crate::chat::receive_new_messages(&s, &mut ps, &mut list).await;
                                        (result, ps.clone(), before)
                                    };
                                    let stored = persist_received_since(&s, &mut list, Some(preloaded)).await;
                                    if stored {
                                        crate::queue_cursors::commit_poll(&s, &processed_state).await;
                                    } else {
                                        *poll_clone.lock().await = before_state;
                                    }
                                    *known.lock().await = list.iter().map(|t| (t.id.clone(), t.name.clone(), t.is_group)).collect();
                                    // Startup refreshes live/offline queues; historical backfill is explicit.
                                    let count = list.len();
                                    let _ = gui_tx_clone.send(GuiEvent::ThreadsUpdated {threads:list,source:ThreadUpdateSource::History});
                                    notice.message=if !stored {
                                        "Chưa lưu được cập nhật vào CSDL; giữ cursor cũ để lấy bù lần sau".into()
                                    } else { match recent_result {
                                        Ok(_) => format!("Đã cập nhật {count} hội thoại và tin gần đây; không phải toàn bộ lịch sử điện thoại"),
                                        Err(error) => format!("Đã cập nhật {count} hội thoại; chưa lấy được tin gần đây: {error}"),
                                    }};
                                    ctx_clone.request_repaint();
                                }
                            });
                        }
                        WorkerCmd::PollMessages { known_threads } => {
                            if is_polling.swap(true, std::sync::atomic::Ordering::SeqCst) {
                                continue;
                            }

                            let live_updates = live_updates.clone();
                            let session_clone = session.clone();
                            let poll_clone = poll_state.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            let poll_guard = PollGuard(is_polling.clone());
                            let poll_delivery = poll_delivery.clone();
                            tokio::spawn(async move {
                                let _poll_guard = poll_guard;
                                let Ok(_cycle) = poll_delivery.acquire_owned().await else { return; };
                                let s = session_clone.lock().await.clone();
                                if s.login.is_some() {
                                    let before_state = {
                                        let mut state = poll_clone.lock().await;
                                        if *state == crate::chat::PollState::default() {
                                            if let Some(saved) = crate::queue_cursors::poll_start(&s).await { *state = saved; }
                                        }
                                        state.clone()
                                    };
                                    let mut temp_threads: Vec<Thread> = known_threads
                                        .into_iter()
                                        .map(|(id, name, is_group)| Thread {
                                            presentation: Default::default(),
                                            unread: Default::default(),
                                            id,
                                            name,
                                            preview: String::new(),
                                            time: String::new(),
                                            avatar: None,
                                            is_group,
                                            messages: Vec::new(),
                                        })
                                        .collect();

                                    let mut poll_err: Option<String> = None;
                                    for _ in 0..3 {
                                        let poll_res = {
                                            let mut ps = poll_clone.lock().await;
                                            crate::chat::get_chat_from_polling(&s, &mut ps).await
                                        };
                                        match poll_res {
                                            Ok(lp_resp) => {
                                                deliver_presence(&s, &lp_resp, &gui_tx_clone, &ctx_clone).await;
                                                crate::chat::merge_previews(
                                                    &mut temp_threads,
                                                    &lp_resp,
                                                );
                                                let data_node =
                                                    lp_resp.get("data").unwrap_or(&lp_resp);
                                                let more = data_node
                                                    .get("more")
                                                    .and_then(serde_json::Value::as_i64)
                                                    .unwrap_or(0);
                                                let has_reset = data_node
                                                    .get("resetLastActionId")
                                                    .and_then(serde_json::Value::as_str)
                                                    .map(|r| r != "0" && !r.is_empty())
                                                    .unwrap_or(false);
                                                if (more == 0 && !has_reset) || !poll_clone.lock().await.catchup_ready() {
                                                    break;
                                                }
                                            }
                                            Err(e) => {
                                                poll_err = Some(e.to_string());
                                                break;
                                            }
                                        }
                                    }

                                    let expired = poll_err.as_deref().is_some_and(is_session_expired_error);
                                    if expired {
                                        if let Err(e) = crate::storage::remove_session() { eprintln!("Cannot remove session: {e}"); }
                                        {
                                            let mut sl = session_clone.lock().await;
                                            sl.logout();
                                        }
                                        let _ = gui_tx_clone.send(GuiEvent::SessionExpired(
                                            "Phiên đăng nhập (zpw_sek) đã hết hạn".into(),
                                        ));
                                        ctx_clone.request_repaint();
                                    } else {
                                        // Pages received before a later poll failed are stored too: the
                                        // in-memory cursor has already moved past them.
                                        temp_threads.retain(|t| !t.messages.is_empty() || t.unread.fresh);
                                        let mut stored = true;
                                        if !temp_threads.is_empty() {
                                            crate::chat::resolve_thread_names(
                                                &s,
                                                &mut temp_threads,
                                            )
                                            .await;
                                            stored = persist_received(&s, &mut temp_threads).await;
                                            if publish_live(&s, &live_updates, temp_threads) {
                                                let _ = gui_tx_clone.send(GuiEvent::LiveThreadsReady(live_updates.clone()));
                                            }
                                            ctx_clone.request_repaint();
                                        }
                                        if stored {
                                            let state = poll_clone.lock().await.clone();
                                            crate::queue_cursors::commit_poll(&s, &state).await;
                                        } else {
                                            *poll_clone.lock().await = before_state;
                                        }
                                    }
                                }
                            });
                        }
                        WorkerCmd::LoadGroupHistory { group_id, is_group, limit } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                if s.login.is_some() {
                                    let is_send2me = s
                                        .login
                                        .as_ref()
                                        .map(|l| !l.send2me_id.is_empty() && l.send2me_id == group_id)
                                        .unwrap_or(false);
                                    let t = Thread {
                                        presentation: Default::default(),
                                        unread: Default::default(),
                                        id: group_id.clone(),
                                        name: String::new(),
                                        preview: String::new(),
                                        time: String::new(),
                                        avatar: None,
                                        is_group: is_group && !is_send2me,
                                        messages: Vec::new(),
                                    };
                                    let path = s.history_path();
                                    let loaded = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Thread>> {
                                        let store = crate::store::MessageStore::open(path)?;
                                        let mut list = vec![t];
                                        store.hydrate_selected_threads(&mut list, limit)?;
                                        Ok(list)
                                    }).await;
                                    let mut t = match loaded {
                                        Ok(Ok(mut list)) => list.remove(0),
                                        _ => {
                                            let _ = gui_tx_clone.send(GuiEvent::StatusUpdate("Không thể đọc lịch sử cục bộ".into()));
                                            ctx_clone.request_repaint(); return;
                                        }
                                    };
                                    let head=if is_group||is_send2me {
                                        crate::history_open::fetch(&s,&group_id,is_group).await.unwrap_or_default()
                                    }else{None};
                                    if let Some(head)=&head{crate::chat::merge_group_history(&mut t,&head.response);}
                                    if !t.messages.is_empty() {
                                        let cnt = t.messages.len();
                                        let mut filtered=vec![t];
                                        let stored=persist_received(&s, &mut filtered).await;
                                        if stored{if let Some(head)=head{if head.commit().await.is_err(){crate::logger::log_error("history head","Cannot persist head freshness");}}}
                                        let _ = gui_tx_clone.send(GuiEvent::ThreadsUpdated {threads:filtered,source:ThreadUpdateSource::History});
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Đã đọc {cnt} tin từ lịch sử cục bộ và tin gần đây; lịch sử server theo dõi trong Đồng bộ"
                                        )));
                                        ctx_clone.request_repaint();
                                    } else if let Some(head)=head {if head.commit().await.is_err(){crate::logger::log_error("history head","Cannot persist empty head freshness");}}
                                }
                            });
                        }
                        WorkerCmd::SendMentioned { .. } | WorkerCmd::SendMessage { .. } | WorkerCmd::SendQuote { .. } | WorkerCmd::Forward { .. } | WorkerCmd::SendSticker { .. } => {
                            let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể gửi loại tin nhắn này".into()));
                        }
                        WorkerCmd::React {
                            to_id,
                            is_group,
                            msg,
                            reaction,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let res = crate::chat::send_reaction(
                                    &s, &to_id, is_group, &msg, &reaction,
                                )
                                .await;
                                match res {
                                    Ok(resp) if resp_error_code(&resp).is_none() => {
                                        let _ = gui_tx_clone.send(GuiEvent::Reacted {
                                            owner: s.account_key().to_owned(),
                                            action_id: resp.get("client_action_id").and_then(serde_json::Value::as_str).unwrap_or_default().to_owned(),
                                            thread_id: to_id,
                                            msg_id: msg.id.clone(),
                                            ts: msg.ts,
                                            reaction,
                                        });
                                    }
                                    Ok(resp) => {
                                        let err_s = format!(
                                            "server error {}",
                                            resp_error_code(&resp).unwrap_or(0)
                                        );
                                        crate::logger::log_error("gui::react", &err_s);
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Thả cảm xúc thất bại: {err_s}"
                                        )));
                                    }
                                    Err(e) => {
                                        crate::logger::log_error("gui::react", &e.to_string());
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Thả cảm xúc thất bại: {e}"
                                        )));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::DeleteForMe {
                            to_id,
                            is_group,
                            msg,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let local_only = msg.id.is_empty()
                                    || msg.id.starts_with("out_")
                                    || msg.id.starts_with("local_");
                                let res: anyhow::Result<serde_json::Value> = if local_only {
                                    Ok(serde_json::json!({}))
                                } else {
                                    crate::chat::delete_message_for_me(&s, &to_id, is_group, &msg)
                                        .await
                                };
                                match res {
                                    Ok(resp) if resp_error_code(&resp).is_none() => {
                                        let tid=to_id.clone();let snapshot=msg.clone();let disk=s.clone();
                                        let result=tokio::task::spawn_blocking(move||->anyhow::Result<()> {
                                            anyhow::ensure!(disk.is_active(),"Account changed");
                                            let mut store=crate::store::MessageStore::open(disk.history_path())?;
                                            store.delete_chat_message(&tid,&snapshot)?;
                                            let cache=crate::chat::load_threads_cache(&disk);
                                            crate::chat::save_threads_cache(&disk,&cache);
                                            Ok(())
                                        }).await;
                                        if matches!(result,Ok(Ok(()))) && s.is_active() {
                                            let _ = gui_tx_clone.send(GuiEvent::MessageDeleted {
                                                owner:s.account_key().to_owned(),thread_id: to_id,msg_id: msg.id.clone(),ts: msg.ts,
                                            });
                                        }else{
                                            let _=gui_tx_clone.send(GuiEvent::StatusUpdate("Máy chủ đã nhận yêu cầu xóa nhưng chưa lưu được dấu xóa trên máy này; hãy thử lại".into()));
                                        }
                                    }
                                    Ok(resp) => {
                                        let err_s = format!(
                                            "server error {}",
                                            resp_error_code(&resp).unwrap_or(0)
                                        );
                                        crate::logger::log_error("gui::delete", &err_s);
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Xóa tin nhắn thất bại: {err_s}"
                                        )));
                                    }
                                    Err(e) => {
                                        crate::logger::log_error("gui::delete", &e.to_string());
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Xóa tin nhắn thất bại: {e}"
                                        )));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::RecallMessage {
                            to_id,
                            is_group,
                            msg,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let local_only = msg.id.is_empty()
                                    || msg.id.starts_with("out_")
                                    || msg.id.starts_with("local_");
                                let res: anyhow::Result<serde_json::Value> = if local_only {
                                    Err(anyhow::anyhow!("Tin chưa có ID máy chủ; chưa thể xác nhận thu hồi cho mọi người"))
                                } else {
                                    crate::chat::recall_message(&s, &to_id, is_group, &msg).await
                                };
                                match res {
                                    Ok(resp) if resp_error_code(&resp).is_none() => {
                                        let mut recalled=msg.clone();recalled.mark_recalled();persist_outgoing(&s,&to_id,&recalled).await;
                                        let _ = gui_tx_clone.send(GuiEvent::MessageRecalled {
                                            owner:s.account_key().into(),
                                            thread_id: to_id,
                                            msg_id: msg.id.clone(),
                                            ts: msg.ts,
                                        });
                                    }
                                    Ok(resp) => {
                                        let err_s = format!(
                                            "server error {}",
                                            resp_error_code(&resp).unwrap_or(0)
                                        );
                                        crate::logger::log_error("gui::recall", &err_s);
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Máy chủ từ chối thu hồi: {err_s}"
                                        )));
                                    }
                                    Err(e) => {
                                        crate::logger::log_error("gui::recall", &e.to_string());
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Không thể thu hồi trên máy chủ: {e}"
                                        )));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::SendContactCard {
                            to_id,
                            is_group,
                            contact_id,
                            contact_name,
                            phone,
                            avatar,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let client_id = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0);
                                match crate::chat::send_contact_card(&s, &to_id, is_group, &contact_id, &contact_name, &phone, &avatar, client_id).await {
                                    Ok(_) => {
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!("Đã gửi danh thiếp: {contact_name}")));
                                    }
                                    Err(e) => {
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!("Gửi danh thiếp thất bại: {e}")));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::BatchRecall {
                            to_id,
                            is_group,
                            messages,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let mut targets = Vec::new();
                                let mut recalled_ids = Vec::new();
                                for msg in &messages {
                                    let cli_id = crate::chat::client_message_id(msg).unwrap_or_default();
                                    let srv_id = crate::chat::server_message_id(msg).unwrap_or_default();
                                    if !srv_id.is_empty() {
                                        targets.push(crate::chat::UndoTargetItem {
                                            cli_msg_id: std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .map(|d| d.as_millis() as u64)
                                                .unwrap_or(0),
                                            cli_msg_id_undo: cli_id,
                                            global_msg_id: srv_id,
                                        });
                                        recalled_ids.push(msg.id.clone());
                                    }
                                }
                                if !targets.is_empty() {
                                    match crate::chat::multi_undo_messages(&s, &to_id, is_group, &targets).await {
                                        Ok(_) => {
                                            for msg in &messages {
                                                let mut r = msg.clone();
                                                r.mark_recalled();
                                                persist_outgoing(&s, &to_id, &r).await;
                                            }
                                            let count = targets.len();
                                            let _ = gui_tx_clone.send(GuiEvent::BatchRecallCompleted {
                                                thread_id: to_id,
                                                recalled_ids,
                                                count,
                                            });
                                        }
                                        Err(e) => {
                                            let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!("Thu hồi hàng loạt thất bại: {e}")));
                                        }
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::BatchForward {
                            to_id,
                            is_group,
                            messages,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let mut count = 0;
                                for msg in messages {
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|d| d.as_millis() as u64)
                                        .unwrap_or(0);
                                    if crate::chat::forward_message_with_client_id(&s, &to_id, is_group, &msg, now).await.is_ok() {
                                        count += 1;
                                    }
                                }
                                let _ = gui_tx_clone.send(GuiEvent::BatchForwardCompleted {
                                    target_id: to_id,
                                    count,
                                });
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::FetchStickerImage { .. } | WorkerCmd::FetchChatImage {..} => {},
                        WorkerCmd::FetchStickerStore => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                match crate::chat::fetch_sticker_store_packs(&s).await {
                                    Ok(packs) => {
                                        let _ = gui_tx_clone
                                            .send(GuiEvent::StickerStoreLoaded(packs));
                                    }
                                    Err(e) => {
                                        crate::logger::log_error(
                                            "gui::fetch_sticker_store",
                                            &e.to_string(),
                                        );
                                        let _ = gui_tx_clone
                                            .send(GuiEvent::StickerStoreLoaded(Vec::new()));
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Không tải được kho nhãn dán: {e}"
                                        )));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::FetchStickerPack { cate_id, name } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                match crate::chat::fetch_sticker_pack_items(&s, cate_id, &name)
                                    .await
                                {
                                    Ok(items) => {
                                        let _ = gui_tx_clone.send(
                                            GuiEvent::StickerPackItemsLoaded {
                                                cate_id,
                                                name,
                                                items,
                                            },
                                        );
                                    }
                                    Err(e) => {
                                        crate::logger::log_error(
                                            "gui::fetch_sticker_pack",
                                            &e.to_string(),
                                        );
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Không tải được bộ nhãn \"{name}\": {e}"
                                        )));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::SearchStickers { keyword } => {
                            let image_requests=image_tx.clone();
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                match crate::chat::search_stickers_by_keyword(&s, &keyword).await {
                                    Ok(items) => {
                                        let missing = preload_disk_sticker_thumbnails(
                                            &items,
                                            &gui_tx_clone,
                                            24,
                                        ).await;
                                        let _ = gui_tx_clone.send(GuiEvent::StickerSearchResults {
                                            keyword,
                                            items,
                                        });
                                        ctx_clone.request_repaint();
                                        prefetch_missing_sticker_thumbnails(
                                            &image_requests,
                                            missing,
                                            &gui_tx_clone,
                                            &ctx_clone,
                                        )
                                        .await;
                                    }
                                    Err(e) => {
                                        crate::logger::log_error(
                                            "gui::search_stickers",
                                            &e.to_string(),
                                        );
                                        let _ = gui_tx_clone.send(GuiEvent::StickerSearchResults {
                                            keyword,
                                            items: Vec::new(),
                                        });
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                        }
                        WorkerCmd::SuggestComposerStickers { keyword } => {
                            let image_requests=image_tx.clone();
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let items = crate::chat::search_stickers_by_keyword(&s, &keyword)
                                    .await
                                    .unwrap_or_default();
                                let missing =
                                    preload_disk_sticker_thumbnails(&items, &gui_tx_clone, 16).await;
                                let _ =
                                    gui_tx_clone.send(GuiEvent::ComposerStickerSuggestions {
                                        keyword,
                                        items,
                                    });
                                ctx_clone.request_repaint();
                                prefetch_missing_sticker_thumbnails(
                                    &image_requests,
                                    missing,
                                    &gui_tx_clone,
                                    &ctx_clone,
                                )
                                .await;
                            });
                        }
                        WorkerCmd::ParseLinkPreview { url } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                if let Ok(meta) = crate::chat::parse_link_metadata(&s, &url).await {
                                    let _ = gui_tx_clone.send(GuiEvent::LinkPreviewReady(meta));
                                    ctx_clone.request_repaint();
                                }
                            });
                        }
                        WorkerCmd::PrepareDroppedFile {owner,thread,id,path} => {
                            let s=session.lock().await.clone();let events=gui_tx.clone();let ctx=egui_ctx.clone();
                            static PREVIEW_QUEUE:tokio::sync::Semaphore=tokio::sync::Semaphore::const_new(64);
                            let Ok(queued)=PREVIEW_QUEUE.try_acquire() else {
                                let _=events.send(GuiEvent::DroppedFilePrepared {owner,thread,id,result:Err("Danh sách xử lý đang đầy; bỏ tệp và thả lại để thử".into())});ctx.request_repaint();continue;
                            };
                            tokio::spawn(async move {
                                let _queued=queued;
                                static PREVIEWS:tokio::sync::Semaphore=tokio::sync::Semaphore::const_new(2);
                                let result=async {
                                    anyhow::ensure!(s.is_active() && s.account_key()==owner,"Tài khoản đã thay đổi");
                                    let _permit=PREVIEWS.acquire().await?;
                                    anyhow::ensure!(s.is_active(),"Tài khoản đã thay đổi");
                                    super::file_drop::prepare(path).await
                                }.await.map_err(|_|"Không đọc được tệp; hãy bỏ tệp và thả lại để thử".to_owned());
                                let _=events.send(GuiEvent::DroppedFilePrepared {owner,thread,id,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::SendDroppedFiles {owner,batch} => {
                            let s=session.lock().await.clone();
                            if !s.is_active() || s.account_key()!=owner || batch.files.len()>16 {continue;}
                            tokio::spawn(send_attachment_batch(s,batch,gui_tx.clone(),egui_ctx.clone()));
                        }
                        WorkerCmd::SendVoiceMessage {
                            to_id,
                            is_group,
                            wav_bytes,
                            duration_secs,
                        } => {
                            let session_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                if !s.is_active() {
                                    return;
                                }
                                let owner = s.account_key().to_owned();
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0);
                                let voice_dir = std::env::temp_dir().join("ctdozlite_voice");
                                let _ = tokio::fs::create_dir_all(&voice_dir).await;
                                let temp_file = voice_dir.join(format!("voice_{now}.wav"));
                                if let Err(e) = tokio::fs::write(&temp_file, &wav_bytes).await {
                                    let _ = events.send(GuiEvent::StatusUpdate(format!("Lỗi lưu tệp âm thanh: {e}")));
                                    return;
                                }
                                let stamp = crate::outgoing::FileStamp::read(&temp_file).await.ok();
                                let caption = format!("[Tin nhắn thoại] ({duration_secs}s)");
                                let mut message = crate::outgoing::pending(
                                    caption.clone(),
                                    is_group,
                                    crate::outgoing::Payload::Attachment {
                                        path: temp_file.clone(),
                                        caption: caption.clone(),
                                        stamp,
                                        hd_source: None,
                                    },
                                );
                                message.kind = crate::message_model::MessageKind::Voice;
                                let Some(active) = crate::outgoing::Active::start(&message.client_message_id) else {
                                    return;
                                };
                                persist_outgoing(&s, &to_id, &message).await;
                                let _ = events.send(GuiEvent::OutgoingState {
                                    owner: owner.clone(),
                                    thread_id: to_id.clone(),
                                    message: message.clone(),
                                });
                                ctx.request_repaint();

                                match crate::outgoing::send(&s, &to_id, &message).await {
                                    Ok(mut sent) => {
                                        sent.kind = crate::message_model::MessageKind::Voice;
                                        message = sent;
                                        let _ = events.send(GuiEvent::StatusUpdate("Đã gửi tin nhắn thoại".into()));
                                    }
                                    Err(error) => {
                                        message.delivery = crate::message_model::DeliveryState::Failed;
                                        if let Some(job) = message.outgoing_job.as_mut() {
                                            job.error = crate::outgoing::failure_message(&error);
                                        }
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Gửi tin nhắn thoại thất bại: {error}")));
                                    }
                                }
                                persist_outgoing(&s, &to_id, &message).await;
                                drop(active);
                                let _ = events.send(GuiEvent::OutgoingState {
                                    owner,
                                    thread_id: to_id,
                                    message,
                                });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::PickAndSendAttachment {to_id,is_group,images_only,caption,preset_path,photo_original} => {
                            let s=session.lock().await.clone();let events=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let paths=if let Some(path)=preset_path {vec![path]} else {
                                    tokio::task::spawn_blocking(move||super::messages::pick_native_files_dialog(images_only)).await.ok().flatten().unwrap_or_default()
                                };
                                send_attachment_batch(s,super::file_drop::Batch {thread:to_id,group:is_group,caption,original:photo_original,files:paths.into_iter().map(|p|(p,None)).collect()},events,ctx).await;
                            });
                        }
                        WorkerCmd::StartCall {
                            partner_id,
                            partner_name,
                            partner_avatar,
                            is_video,
                            is_group,
                        } => {
                            let kind_label = if is_video { "Gọi video" } else { "Gọi thoại" };
                            call_bridge.make_call(
                                &partner_id,
                                &partner_name,
                                partner_avatar.as_deref(),
                                is_video,
                                is_group,
                            );
                            let status_msg = format!(
                                "📞 Đang khởi tạo {kind_label} trực tiếp (Pure-Rust) tới {partner_name} ({partner_id})…"
                            );
                            let _ = gui_tx.send(GuiEvent::StatusUpdate(status_msg));
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::AnswerCall { accept, call_id } => {
                            call_bridge.answer_call_for(accept,call_id);
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::SetCallMediaState {
                            is_on_mic,
                            is_on_cam,
                        } => {
                            call_bridge.set_media_state(is_on_mic, is_on_cam);
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::RequestShareScreen {
                            call_id,
                            partner_id,
                            enable,
                            is_group,
                        } => {
                            call_bridge.request_share_screen(call_id, &partner_id, enable, is_group);
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::EndCall { call_id } => {
                            call_bridge.end_call_for(call_id);

                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::KickDevice { device_id } => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::device::kick_remote_device(&s, &device_id).await {
                                    Ok(_) => {
                                        let _ = events.send(GuiEvent::StatusUpdate("Đã đăng xuất thiết bị từ xa".into()));
                                        if let Ok(devs) = crate::device::get_logged_in_devices(&s).await {
                                            let _ = events.send(GuiEvent::DevicesLoaded(devs));
                                        }
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Lỗi đăng xuất thiết bị: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::FetchServerMedia { thread_id, is_group, media_type } => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                let kind = match media_type {
                                    1 => crate::server_media::ServerMediaType::Photo,
                                    2 => crate::server_media::ServerMediaType::Video,
                                    3 => crate::server_media::ServerMediaType::File,
                                    _ => crate::server_media::ServerMediaType::Link,
                                };
                                let res = if is_group {
                                    crate::server_media::fetch_group_media_store(&s, &thread_id, kind, 1, 30).await
                                } else {
                                    crate::server_media::fetch_direct_media_store(&s, &thread_id, kind, 1, 30).await
                                };
                                match res {
                                    Ok(page) => {
                                        let _ = events.send(GuiEvent::ServerMediaLoaded { items: page.items, media_type });
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Không thể tải kho media server: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::GetCloudViewerKey { file_id } => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::zcloud::get_cloud_viewer_key(&s, &file_id).await {
                                    Ok(key) => {
                                        let _ = events.send(GuiEvent::CloudViewerKeyLoaded(key));
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Lấy khoá zCloud thất bại: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::GetBusinessCatalog => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::business::fetch_product_catalog(&s).await {
                                    Ok(items) => {
                                        let _ = events.send(GuiEvent::BusinessCatalogLoaded(items));
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Không thể tải danh mục sản phẩm: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::DiscoverLanPeers => {
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let mut peers = Vec::new();
                                if let Ok(sock) = tokio::net::UdpSocket::bind("0.0.0.0:0").await {
                                    let _ = sock.set_broadcast(true);
                                    let dev_name = format!("Windows 11 ({})", std::env::consts::ARCH);
                                    let pkt = crate::lan_transfer::format_lan_discovery_packet("current_device", &dev_name, 8998);
                                    let _ = sock.send_to(pkt.as_bytes(), "255.255.255.255:8998").await;
                                    let mut buf = [0u8; 1024];
                                    let start = tokio::time::Instant::now();
                                    let deadline = std::time::Duration::from_millis(1500);
                                    while tokio::time::Instant::now() - start < deadline {
                                        let remaining = deadline.saturating_sub(tokio::time::Instant::now() - start);
                                        if remaining.is_zero() {
                                            break;
                                        }
                                        if let Ok(Ok((len, addr))) = tokio::time::timeout(remaining, sock.recv_from(&mut buf)).await {
                                            let now = crate::login::unix_now();
                                            if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                                                if let Some(peer) = crate::lan_transfer::parse_lan_discovery_packet(msg, &addr.ip().to_string(), now) {
                                                    if !peers.iter().any(|p: &crate::lan_transfer::LanPeer| p.peer_id == peer.peer_id) {
                                                        peers.push(peer);
                                                    }
                                                }
                                            }
                                        } else {
                                            break;
                                        }
                                    }
                                }
                                let count = peers.len();
                                let _ = events.send(GuiEvent::LanPeersDiscovered(peers));
                                let _ = events.send(GuiEvent::StatusUpdate(format!("Quét hoàn tất: tìm thấy {count} thiết bị cùng mạng LAN")));
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::CreateTodo { thread_id, content, assignee_id, deadline_ts } => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::todo::create_group_todo(&s, &thread_id, &content, &assignee_id, deadline_ts).await {
                                    Ok(_) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Đã tạo việc thành công: {content}")));
                                        if let Ok(todos) = crate::todo::list_group_todos(&s, &thread_id).await {
                                            let _ = events.send(GuiEvent::GroupTodosLoaded(todos));
                                        }
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Tạo việc thất bại: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::FetchTodos { thread_id } => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::todo::list_group_todos(&s, &thread_id).await {
                                    Ok(todos) => {
                                        let _ = events.send(GuiEvent::GroupTodosLoaded(todos));
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Không thể tải danh sách to-do: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::CreatePoll { thread_id, question, options } => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::poll::create_group_poll(&s, &thread_id, &question, &options, false, false, false).await {
                                    Ok(_) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Đã tạo cuộc bình chọn: {question}")));
                                        if let Ok(reply) = crate::group_content::execute(&s, &thread_id, &crate::group_content::Action::ListPolls { page: 1, last: String::new(), last_type: 0 }).await {
                                            let poll_items: Vec<crate::poll::PollItem> = reply.polls.unwrap_or_default().into_iter().map(|p| {
                                                let total: u32 = p.options.iter().map(|o| o.votes.unwrap_or(0) as u32).sum();
                                                crate::poll::PollItem {
                                                    id: p.id.parse::<i64>().unwrap_or(0),
                                                    grid: thread_id.clone(),
                                                    question: p.question,
                                                    creator_id: p.creator,
                                                    options: p.options.into_iter().map(|o| crate::poll::PollOption {
                                                        id: o.id,
                                                        text: o.text,
                                                        vote_count: o.votes.unwrap_or(0) as u32,
                                                        is_voted_by_me: o.selected,
                                                    }).collect(),
                                                    multi_choice: p.multi,
                                                    is_closed: p.closed,
                                                    total_votes: total,
                                                }
                                            }).collect();
                                            let _ = events.send(GuiEvent::GroupPollsLoaded(poll_items));
                                        }
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Tạo bình chọn thất bại: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::FetchPolls { thread_id } => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::group_content::execute(&s, &thread_id, &crate::group_content::Action::ListPolls { page: 1, last: String::new(), last_type: 0 }).await {
                                    Ok(reply) => {
                                        let poll_items: Vec<crate::poll::PollItem> = reply.polls.unwrap_or_default().into_iter().map(|p| {
                                            let total: u32 = p.options.iter().map(|o| o.votes.unwrap_or(0) as u32).sum();
                                            crate::poll::PollItem {
                                                id: p.id.parse::<i64>().unwrap_or(0),
                                                grid: thread_id.clone(),
                                                question: p.question,
                                                creator_id: p.creator,
                                                options: p.options.into_iter().map(|o| crate::poll::PollOption {
                                                    id: o.id,
                                                    text: o.text,
                                                    vote_count: o.votes.unwrap_or(0) as u32,
                                                    is_voted_by_me: o.selected,
                                                }).collect(),
                                                multi_choice: p.multi,
                                                is_closed: p.closed,
                                                total_votes: total,
                                            }
                                        }).collect();
                                        let count = poll_items.len();
                                        let _ = events.send(GuiEvent::GroupPollsLoaded(poll_items));
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Đã tải {count} cuộc bình chọn")));
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Không thể tải danh sách bình chọn: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::FetchDevices => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::device::get_logged_in_devices(&s).await {
                                    Ok(devs) => {
                                        let _ = events.send(GuiEvent::DevicesLoaded(devs));
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Không thể tải danh sách thiết bị: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::CheckTwoFactor => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                match crate::account_recovery::check_two_factor_status(&s).await {
                                    Ok(status) => {
                                        let _ = events.send(GuiEvent::TwoFactorStatusLoaded(status));
                                    }
                                    Err(e) => {
                                        let _ = events.send(GuiEvent::StatusUpdate(format!("Kiểm tra 2FA thất bại: {e}")));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::CheckFriendStatus { user_id } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                if let Ok(status) =
                                    crate::chat::check_friendship_status(&s, &user_id).await
                                {
                                    let _ = gui_tx_clone
                                        .send(GuiEvent::FriendStatusChecked { user_id, status });
                                    ctx_clone.request_repaint();
                                }
                            });
                        }
                        WorkerCmd::AcceptFriend { user_id, user_name } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                match crate::chat::accept_friend_request(&s, &user_id).await {
                                    Ok(_) => {
                                        let _ = gui_tx_clone.send(GuiEvent::FriendStatusChecked {
                                            user_id,
                                            status: crate::chat::FriendshipStatus::Friend,
                                        });
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Đã đồng ý kết bạn với {user_name}!"
                                        )));
                                    }
                                    Err(e) => {
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Không thể đồng ý kết bạn với {user_name}: {e}"
                                        )));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::RejectFriend { user_id, user_name } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                if crate::chat::reject_friend_request(&s, &user_id)
                                    .await
                                    .is_ok()
                                {
                                    let _ = gui_tx_clone.send(GuiEvent::FriendStatusChecked {
                                        user_id,
                                        status: crate::chat::FriendshipStatus::Stranger,
                                    });
                                    let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                        "Đã từ chối lời mời kết bạn của {user_name}."
                                    )));
                                    ctx_clone.request_repaint();
                                }
                            });
                        }
                        WorkerCmd::UndoFriend { user_id, user_name } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                if crate::chat::undo_friend_request(&s, &user_id).await.is_ok() {
                                    let _ = gui_tx_clone.send(GuiEvent::FriendStatusChecked {
                                        user_id,
                                        status: crate::chat::FriendshipStatus::Stranger,
                                    });
                                    let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                        "Đã hủy lời mời kết bạn tới {user_name}."
                                    )));
                                    ctx_clone.request_repaint();
                                }
                            });
                        }
                        WorkerCmd::AddFriend { user_id, user_name } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                // Check if there is an incoming request we can accept directly
                                if crate::chat::accept_friend_request(&s, &user_id).await.is_ok() {
                                    let _ = gui_tx_clone.send(GuiEvent::FriendStatusChecked {
                                        user_id,
                                        status: crate::chat::FriendshipStatus::Friend,
                                    });
                                    let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                        "Đã đồng ý kết bạn với {user_name}! Bạn có thể gọi lại ngay."
                                    )));
                                    ctx_clone.request_repaint();
                                    return;
                                }
                                match crate::chat::send_friend_request(
                                    &s,
                                    &user_id,
                                    "Xin chào, kết bạn với mình nhé!",
                                )
                                .await
                                {
                                    Ok(_) => {
                                        let _ = gui_tx_clone.send(GuiEvent::FriendStatusChecked {
                                            user_id: user_id.clone(),
                                            status: crate::chat::FriendshipStatus::OutgoingPending,
                                        });
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Đã gửi lời mời kết bạn tới {user_name} ({user_id})."
                                        )));
                                    }
                                    Err(e) => {
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Không thể gửi lời mời kết bạn tới {user_name}: {e}"
                                        )));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::LookupPhone { phone } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                match crate::chat::find_user_by_phone(&s, &phone).await {
                                    Ok(profile) => {
                                        let _ = gui_tx_clone
                                            .send(GuiEvent::PhoneLookupSuccess(profile));
                                    }
                                    Err(e) => {
                                        crate::logger::log_error(
                                            "gui::lookup_phone",
                                            &e.to_string(),
                                        );
                                        let _ = gui_tx_clone
                                            .send(GuiEvent::PhoneLookupFailed(e.to_string()));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::LoginPassword { phone, pass } => {
                            // A19: Cancel QR task and bump auth attempt generation
                            if let Some(task) = current_qr_task.take() { task.abort(); }
                            let attempt_id = current_auth_attempt.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

                            session.lock().await.logout();
                            let session_clone = session.clone();
                            let commands = commands.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            let auth_attempt_clone = current_auth_attempt.clone();
                            tokio::spawn(async move {
                                let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(
                                    "Đang xác thực tài khoản…".into(),
                                ));
                                ctx_clone.request_repaint();
                                let mut s = session_clone.lock().await;
                                match crate::login::login_by_password(&mut s, &phone, &pass, "VN")
                                    .await
                                {
                                    Ok(_) => {
                                        // A19: Validate auth attempt before committing result
                                        let current = auth_attempt_clone.load(std::sync::atomic::Ordering::SeqCst);
                                        if current != attempt_id {
                                            // Stale response — another auth started, discard this result
                                            return;
                                        }

                                        if let Ok(info) = crate::login::get_login_info(&s).await {
                                            if let Some(cfg) = info.get("data") {
                                                crate::login::apply_login_config(&mut s, cfg);
                                                s.apply_server_info(cfg);
                                                if let Err(e) = s.save_to_file("session.json") {
                                                    let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                                }

                                                let uin = s
                                                    .login
                                                    .as_ref()
                                                    .map(|l| l.uin.clone())
                                                    .unwrap_or_default();
                                                let name = s
                                                    .login
                                                    .as_ref()
                                                    .map(|l| l.display_name.clone())
                                                    .unwrap_or_default();
                                                let u_ava = s
                                                    .login
                                                    .as_ref()
                                                    .map(|l| l.avatar.clone())
                                                    .filter(|a| !a.is_empty());
                                                let _ = gui_tx_clone.send(GuiEvent::LoginSuccess {
                                                    name,
                                                    uin,
                                                    avatar: u_ava,
                                                });
                                                ctx_clone.request_repaint();

                                                if let Some(tx) = commands.upgrade() {
                                                    let _ = tx.send(WorkerCmd::RefreshThreads);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // A19: Validate auth attempt before showing error
                                        let current = auth_attempt_clone.load(std::sync::atomic::Ordering::SeqCst);
                                        if current != attempt_id {
                                            // Stale response — discard
                                            return;
                                        }

                                        let _ = gui_tx_clone
                                            .send(GuiEvent::LoginFailed(e.to_string()));
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                        }
                    }
                }
            });
        })
        .expect("Failed to spawn zalo-worker thread");
}

#[cfg(test)]
mod lifecycle_tests {
    use super::PollGuard;
    #[tokio::test]
    async fn cancelling_poll_releases_inflight_flag() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let flag = Arc::new(AtomicBool::new(true));
        let guard = PollGuard(flag.clone());
        let task = tokio::spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });
        tokio::task::yield_now().await;
        task.abort();
        let _ = task.await;
        assert!(!flag.load(Ordering::SeqCst));
    }
}

#[cfg(test)]
mod image_pipeline_tests {
    use super::*;
    #[tokio::test]
    async fn bundled_thumbnail_never_uses_network_or_private_disk_cache() {
        let mut session = Session::new(false, false, false).unwrap();
        session.login = Some(crate::session::LoginInfo {
            uin: "bundled-thumbnail-fixture".into(),
            ..Default::default()
        });
        let root = std::env::temp_dir().join(format!(
            "ctdoz-no-cache-{}",
            crate::param_cipher::random_hex(16)
        ));
        let image = fetch_gui_image(
            &session,
            &root,
            "stk_99901_999001",
            "https://untrusted.invalid/not-contacted",
        )
        .await
        .unwrap();
        assert!(image.size[0] > 0 && image.size[1] > 0);
        assert!(!root.exists());
    }

    #[tokio::test]
    async fn corrupt_cached_preview_is_replaced_and_account_cache_is_reused() {
        use std::io::{Cursor, Read, Write};
        let root = std::env::temp_dir().join(format!(
            "ctdoz-image-test-{}",
            crate::param_cipher::random_hex(20)
        ));
        let mut image = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1024, 512)
            .write_to(&mut image, image::ImageFormat::Png)
            .unwrap();
        let bytes = image.into_inner();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            stream.write_all(&bytes).unwrap();
        });
        let mut session = Session::new(false, false, false).unwrap();
        session.login = Some(crate::session::LoginInfo {
            uin: "account-fixture".into(),
            ..Default::default()
        });
        session.http = reqwest::Client::builder().no_proxy().build().unwrap();
        let cache = crate::media_resources::PrivateMediaCache::new(
            &root.join("private-media"),
            session.account_key(),
        )
        .unwrap();
        let key = crate::media_resources::MediaKey {
            account: session.account_key().into(),
            thread: "ui-preview".into(),
            message: "fixture-image".into(),
            quality: crate::media_resources::MediaQuality::Thumbnail,
        };
        let cache_path = cache.path_for(&key).unwrap();
        std::fs::write(&cache_path, b"broken cache").unwrap();
        let first = fetch_gui_image(
            &session,
            &root,
            "fixture-image",
            &format!("http://{address}/fixture"),
        )
        .await
        .unwrap();
        assert_eq!(first.size, [512, 256]);
        server.join().unwrap();
        // The fixture server is closed: success now proves a valid cache hit.
        let second = fetch_gui_image(
            &session,
            &root,
            "fixture-image",
            &format!("http://{address}/fixture"),
        )
        .await
        .unwrap();
        assert_eq!(second.size, [512, 256]);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod refresh_notice_tests {
    use super::*;
    #[test]
    fn refresh_guard_always_reports_terminal_failure_when_work_exits_early() {
        let (tx, rx) = channel();
        {
            let mut notice = RefreshNotice {
                sender: tx,
                ctx: egui::Context::default(),
                message: "Chưa hoàn tất".into(),
            };
            notice.message = "Không thể kết nối máy chủ".into();
        }
        assert!(
            matches!(rx.try_recv().unwrap(),GuiEvent::ConversationRefreshStatus {running:false,message} if message=="Không thể kết nối máy chủ")
        );
    }
}

/// Orders server mute reconciliation against newer local or realtime mute changes.
#[derive(Clone, Default)]
struct MuteSync {
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Most local-only mutes published during the first reconciliation of one refresh.
const MUTE_SEED_LIMIT: usize = 50;

async fn persist_mutes(
    s: &Session,
    changes: Vec<crate::mute_sync::MuteChange>,
    mark_seeded: bool,
) -> Option<HashMap<String, crate::store::ConversationPreference>> {
    let account = s.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        anyhow::ensure!(account.is_active(), "Session ended");
        crate::store::MessageStore::open(account.history_path())?
            .apply_mute_changes(&changes, mark_seeded)
    })
    .await;
    match result {
        Ok(Ok(values)) => Some(values),
        _ => {
            crate::logger::log_error("mute sync", "Cannot persist server mute state");
            None
        }
    }
}

fn publish_preferences(
    s: &Session,
    values: HashMap<String, crate::store::ConversationPreference>,
    gui_tx: &Sender<GuiEvent>,
    live: &crate::live_updates::LiveUpdates,
    ctx: &egui::Context,
) {
    live.set_muted(&values);
    let _ = gui_tx.send(GuiEvent::ConversationPreferences {
        owner: s.account_key().to_owned(),
        values,
    });
    ctx.request_repaint();
}

/// Forward typing actions and persist delivery/read receipts from a realtime payload.
async fn deliver_presence(
    s: &Session,
    payload: &serde_json::Value,
    gui_tx: &Sender<GuiEvent>,
    ctx: &egui::Context,
) {
    let self_id = s.account_key().to_owned();
    let typing = crate::presence::typing(payload, &self_id);
    if !typing.is_empty() {
        let _ = gui_tx.send(GuiEvent::Typing {
            owner: self_id.clone(),
            events: typing,
        });
        ctx.request_repaint();
    }
    let receipts = crate::presence::receipts(payload);
    if receipts.is_empty() || !s.is_active() {
        return;
    }
    let account = s.clone();
    let changed = tokio::task::spawn_blocking(move || {
        crate::store::MessageStore::open(account.history_path())?.apply_receipts(&receipts)
    })
    .await;
    match changed {
        Ok(Ok(changed)) if !changed.is_empty() => {
            let _ = gui_tx.send(GuiEvent::Receipts {
                owner: s.account_key().to_owned(),
                changed,
            });
            ctx.request_repaint();
        }
        Ok(Err(e)) => crate::logger::log_error("presence::receipts", &e.to_string()),
        _ => {}
    }
}

/// Apply realtime `act_type: "mute"` controls; newer than any snapshot already in flight.
async fn apply_server_mutes(
    s: &Session,
    changes: Vec<crate::mute_sync::MuteChange>,
    sync: &MuteSync,
    gui_tx: &Sender<GuiEvent>,
    live: &crate::live_updates::LiveUpdates,
    ctx: &egui::Context,
) {
    if changes.is_empty() || !s.is_active() || s.account_key().is_empty() {
        return;
    }
    sync.generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Some(values) = persist_mutes(s, changes, false).await {
        publish_preferences(s, values, gui_tx, live, ctx);
    }
}

/// Reconcile local mute flags with the complete server snapshot (ADR-0013).
async fn sync_mute_snapshot(
    s: Session,
    sync: MuteSync,
    gui_tx: Sender<GuiEvent>,
    live: std::sync::Arc<crate::live_updates::LiveUpdates>,
    ctx: egui::Context,
) {
    if s.login.is_none() || !s.is_active() || s.account_key().is_empty() {
        return;
    }
    if sync.running.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let _running = PollGuard(sync.running.clone());
    let generation = sync.generation.load(std::sync::atomic::Ordering::SeqCst);
    let server = match crate::mute_sync::fetch_mute_snapshot(&s).await {
        Ok(server) => server,
        Err(_) => {
            crate::logger::log_error(
                "mute sync",
                "Mute snapshot unavailable; keeping local state",
            );
            return;
        }
    };
    let account = s.clone();
    let plan = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        anyhow::ensure!(account.is_active(), "Session ended");
        let store = crate::store::MessageStore::open(account.history_path())?;
        let changes = crate::mute_sync::reconcile(&store.conversation_preferences()?, &server);
        let seeded = store.mute_sync_seeded()?;
        let mut local_only = Vec::new();
        if !seeded {
            for change in changes.iter().filter(|c| !c.muted) {
                local_only.push((
                    change.thread_id.clone(),
                    store.thread_is_group(&change.thread_id)?,
                ));
            }
        }
        Ok((changes, seeded, local_only))
    })
    .await;
    let Ok(Ok((mut changes, seeded, local_only))) = plan else {
        crate::logger::log_error("mute sync", "Cannot read local mute state");
        return;
    };
    let mut complete = true;
    if !seeded {
        // Before this sync existed mutes were device-only; publish them once instead of
        // letting the first authoritative snapshot silently turn them back on.
        complete = local_only.len() <= MUTE_SEED_LIMIT;
        for (id, is_group) in local_only.iter().take(MUTE_SEED_LIMIT) {
            let published = match is_group {
                Some(is_group) => crate::mute_sync::set_remote_mute(&s, id, *is_group, true)
                    .await
                    .is_ok(),
                None => false,
            };
            complete &= published;
        }
        changes.retain(|change| change.muted);
    }
    if sync.generation.load(std::sync::atomic::Ordering::SeqCst) != generation {
        // A newer local or realtime change won; the next refresh reconciles again.
        return;
    }
    if changes.is_empty() && seeded {
        return;
    }
    if let Some(values) = persist_mutes(&s, changes, complete).await {
        publish_preferences(&s, values, &gui_tx, &live, &ctx);
    }
}

fn outgoing_command(cmd: &WorkerCmd) -> Option<(String, ChatMessage)> {
    use crate::outgoing::{pending, Payload};
    match cmd {
        WorkerCmd::RetryOutgoing { thread_id, message } => {
            Some((thread_id.clone(), message.clone()))
        }
        WorkerCmd::SendMentioned {
            to_id,
            draft,
            quoted,
        } => {
            let mut quoted = quoted.clone();
            if let Some(q) = quoted.as_mut() {
                q.outgoing_job = None;
            }
            let mut msg = pending(
                draft.text.clone(),
                true,
                Payload::Mentioned {
                    mentions: draft.mentions.clone(),
                    quoted: quoted.clone().map(Box::new),
                },
            );
            msg.wire_ref.fields.insert(
                "mentions".into(),
                serde_json::to_value(&draft.mentions).unwrap_or_default(),
            );
            if let Some(q) = quoted {
                msg.reply_to_id = Some(q.id.clone());
                msg.reply_to_sender = Some(q.sender.clone());
                msg.reply_to_text = Some(q.text.clone());
            }
            Some((to_id.clone(), msg))
        }
        WorkerCmd::SendMessage {
            to_id,
            text,
            is_group,
        } => {
            let msg = pending(text.clone(), *is_group, Payload::Text);
            Some((to_id.clone(), msg))
        }
        WorkerCmd::SendQuote {
            to_id,
            text,
            quoted,
            is_group,
        } => {
            let mut quote = quoted.clone();
            quote.outgoing_job = None;
            let mut msg = pending(text.clone(), *is_group, Payload::Quote(Box::new(quote)));
            msg.reply_to_id = Some(quoted.id.clone());
            msg.reply_to_sender = Some(quoted.sender.clone());
            msg.reply_to_text = Some(quoted.text.clone());
            Some((to_id.clone(), msg))
        }
        WorkerCmd::SendSticker {
            to_id,
            sticker_id,
            cate_id,
            sticker_type,
            is_group,
        } => {
            let mut msg = pending(
                "[Nhãn dán]".into(),
                *is_group,
                Payload::Sticker {
                    id: *sticker_id,
                    category: *cate_id,
                    kind: *sticker_type,
                },
            );
            msg.sticker_id = Some(*sticker_id);
            msg.sticker_cate_id = Some(*cate_id);
            msg.sticker_type = Some(*sticker_type);
            Some((to_id.clone(), msg))
        }
        WorkerCmd::Forward {
            target_id,
            target_is_group,
            msg,
        } if crate::chat::can_forward_message(msg) => {
            let mut original = msg.clone();
            original.outgoing_job = None;
            let mut outgoing = pending(
                msg.text.clone(),
                *target_is_group,
                Payload::Forward(Box::new(original)),
            );
            outgoing.sticker_id = msg.sticker_id;
            outgoing.sticker_cate_id = msg.sticker_cate_id;
            outgoing.sticker_type = msg.sticker_type;
            Some((target_id.clone(), outgoing))
        }
        _ => None,
    }
}

async fn persist_outgoing(session: &Session, thread_id: &str, message: &ChatMessage) {
    let session = session.clone();
    let thread_id = thread_id.to_owned();
    let message = message.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        if !session.is_active() {
            return Ok(());
        }
        let mut store = crate::store::MessageStore::open(session.history_path())?;
        let saved = store
            .all_threads()?
            .into_iter()
            .find(|thread| thread.thread_id == thread_id);
        let is_group = message
            .outgoing_job
            .as_ref()
            .is_some_and(|job| job.is_group);
        let thread = Thread {
            id: thread_id.clone(),
            name: saved.as_ref().map(|t| t.name.clone()).unwrap_or(thread_id),
            is_group: saved.as_ref().map(|t| t.is_group).unwrap_or(is_group),
            messages: vec![message],
            ..Default::default()
        };
        crate::store::persist_local_threads(&mut store, &[thread])?;
        Ok(())
    })
    .await;
    if !matches!(result, Ok(Ok(()))) {
        crate::logger::log_error("outgoing persistence", "Cannot persist outgoing message");
    }
}

fn publish_live(
    session: &Session,
    updates: &crate::live_updates::LiveUpdates,
    threads: Vec<Thread>,
) -> bool {
    if !session.is_active() {
        return false;
    }
    let access = session.hidden.view(session.account_key());
    let policy = session
        .inbox_policy
        .read()
        .map(|policy| policy.clone())
        .unwrap_or_default();
    let cloud = session
        .login
        .as_ref()
        .map(|login| login.send2me_id.as_str())
        .unwrap_or("");
    let changes = threads
        .iter()
        .map(|thread| {
            let mut metadata = updates.inbox_metadata(&thread.id);
            metadata.merge(&thread.presentation.inbox, true);
            if metadata.official_account.is_none() {
                metadata.official_account = policy.profile_types.get(&thread.id).copied();
            }
            let placement = policy.placement_metadata(
                &thread.id,
                thread.is_group,
                &metadata,
                thread.id == cloud,
            );
            if placement == crate::inbox::Placement::Unknown
                && !thread.is_group
                && crate::server_pins::identity(&thread.id, false).is_ok()
            {
                updates.request_metadata();
            }
            (
                thread.id.clone(),
                !access.allows(&thread.id, thread.is_group)
                    || placement != crate::inbox::Placement::Normal,
            )
        })
        .collect::<Vec<_>>();
    updates.suppress(changes);
    let wake = updates.publish(threads);
    #[cfg(windows)]
    {
        crate::desktop_notice::count(session.account_key(), updates.total());
        if let Some(mut notice) = updates.take_notice() {
            notice.owner = session.account_key().to_owned();
            if !crate::tray::foreground() {
                if let Some(url) = notice.avatar.clone() {
                    static AVATAR_LOADING: std::sync::atomic::AtomicBool =
                        std::sync::atomic::AtomicBool::new(false);
                    if !AVATAR_LOADING.swap(true, std::sync::atomic::Ordering::AcqRel) {
                        let account = session.clone();
                        tokio::spawn(async move {
                            struct Guard;
                            impl Drop for Guard {
                                fn drop(&mut self) {
                                    AVATAR_LOADING
                                        .store(false, std::sync::atomic::Ordering::Release);
                                }
                            }
                            let _guard = Guard;
                            let key = format!("ava_{}", crate::crypto::md5_hex(&url));
                            if let Ok(image) =
                                fetch_gui_image(&account, crate::storage::root(), &key, &url).await
                            {
                                if account.is_active() {
                                    crate::desktop_notice::cache_avatar(
                                        account.account_key().into(),
                                        url,
                                        image,
                                    );
                                }
                            }
                        });
                    }
                }
                crate::desktop_notice::publish(notice, updates.total());
            }
        }
        let _ = updates.take_notification();
    }
    wake
}

fn tokio_cache_prune(cache: crate::media_resources::PrivateMediaCache) {
    static LAST: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

    static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());

    if last.is_some_and(|t| t.elapsed() < Duration::from_secs(10))
        || RUNNING.swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        return;
    }

    *last = Some(std::time::Instant::now());

    drop(last);

    std::thread::spawn(move || {
        if cache.prune().is_err() {
            crate::logger::log_error("image cache", "Cannot prune cache");
        }

        RUNNING.store(false, std::sync::atomic::Ordering::Release);
    });
}

async fn account_tools(
    session: Session,
    command: super::tools_panel::ToolsCommand,
) -> anyhow::Result<super::tools_panel::ToolsResult> {
    use super::tools_panel::{ToolsCommand as C, ToolsResult as R};
    if matches!(command, C::ExportHistory) {
        crate::conversation_policy::snapshot(&session, false).await?;
    }

    tokio::task::spawn_blocking(move || -> anyhow::Result<R> {

        anyhow::ensure!(session.is_active() && !session.account_key().is_empty(),"Phiên làm việc đã đóng");

        let mut store=crate::store::MessageStore::open(session.history_path())?;

        match command {
            C::OpenThread {thread,group}=>{
                anyhow::ensure!(session.hidden.view(session.account_key()).allows(&thread,group),"Mở khóa PIN trước khi mở hội thoại");
                let identity=crate::server_pins::identity(&thread,group)?;let id=if group {identity}else{identity[1..].to_owned()};
                let mut threads=vec![Thread {id:id.clone(),name:id.clone(),is_group:group,..Default::default()}];store.hydrate_selected_threads(&mut threads,200)?;
                let mut thread=threads.pop().ok_or_else(||anyhow::anyhow!("Không mở được hội thoại"))?;
                if let Some(saved)=store.all_threads()?.into_iter().find(|saved|saved.thread_id==id){thread.name=saved.name;}
                Ok(R::Hit {thread:Box::new(thread),id:String::new()})
            }
            C::SearchPage(request) => {
                let access=session.hidden.view(session.account_key());
                anyhow::ensure!(access.ready,"Chưa xác minh danh sách hội thoại ẩn");
                if let Some(thread)=&request.thread {anyhow::ensure!(access.allows(thread,thread.starts_with('g')),"Hội thoại đang khóa PIN");}
                let result=crate::search::query_with_cancel(&store,&request,&||!session.is_active()).map_err(|e|e.to_string());
                let senders=match request.thread.as_deref() {Some(thread)=>crate::search::senders(&store,thread)?,None=>Vec::new()};
                let result=result.map(|mut page|{page.hits.retain(|hit|access.allows(&hit.thread,hit.group));page});
                Ok(R::SearchPage{request,result,senders})
            }
            C::OpenSearchHit{generation,thread,id} => {anyhow::ensure!(session.hidden.view(session.account_key()).allows(&thread,thread.starts_with('g')),"Hội thoại đang khóa PIN");Ok(R::SearchOpened{generation,thread:Box::new(crate::tools::open_hit(&store,&thread,&id)?),id})},
            C::OpenHit{
thread,id}
=>{anyhow::ensure!(session.hidden.view(session.account_key()).allows(&thread,thread.starts_with('g')),"Hội thoại đang khóa PIN");Ok(R::Hit{thread:Box::new(crate::tools::open_hit(&store,&thread,&id)?),id})},
            C::Resume=>{
crate::tools::resume(&store)?;
Ok(R::Loaded{
config:crate::tools::preferences(&store)?,tasks:crate::tools::scheduled(&store,session.account_key())?,paused:false}
)}
,
            C::Update{
id,text,at}
=>{
crate::tools::update_pending(&store,session.account_key(),&id,&text,at)?;
Ok(R::Loaded{
config:crate::tools::preferences(&store)?,tasks:crate::tools::scheduled(&store,session.account_key())?,paused:crate::tools::paused(&store)?}
)}
,
            C::Search{
query,hidden}
=>Ok(R::Search{
hits:crate::tools::search(&store,&query,hidden)?.into_iter().filter(|hit|session.hidden.view(session.account_key()).allows(&hit.thread_id,hit.thread_id.starts_with('g'))).collect(),query}
),
            C::ExportHistory=>{
                anyhow::ensure!(session.hidden.view(session.account_key()).export_allowed(),"Mở khóa PIN trước khi xuất lịch sử");
                let Some(path)=tools_file(true,"Xuất lịch sử",&["db"])else{return Ok(R::Status("Đã hủy xuất lịch sử".into()))};
                let n=crate::tools::export_history(&store,&path,session.account_key(),&||session.is_active()&&session.hidden.view(session.account_key()).export_allowed())?;
                Ok(R::Status(format!("Đã xuất {n} tin nhắn vào SQLite")))
            },
            C::Import{
directory,confirmed}
=>{

                let path=if directory {
super::messages::pick_native_folder_dialog()}
else{
tools_file(false,"Nhập lịch sử", &["db","sqlite","sqlite3","json"])}
;

                let Some(path)=path else{
return Ok(R::Status("Đã hủy chọn nguồn".into()));
}
;

                let n=crate::tools::import_history(&mut store,&path,session.account_key(),confirmed,&||session.is_active())?;

                Ok(R::Status(format!("Đã nhập {n} tin mới. Tìm trong toàn bộ lịch sử để xem tin cũ.")))
            }

            C::Load=>Ok(R::Loaded{
config:crate::tools::preferences(&store)?,tasks:crate::tools::scheduled(&store,session.account_key())?,paused:crate::tools::paused(&store)?}
),
            C::Save(config)=>{
anyhow::ensure!(session.is_active(),"Đã đăng xuất");
let config=crate::tools::save_preferences(&store,config)?;
Ok(R::Loaded{
config,tasks:crate::tools::scheduled(&store,session.account_key())?,paused:crate::tools::paused(&store)?}
)}
,
            C::Schedule{
request,thread,group,text,at}
=>{
let result=crate::tools::schedule(&store,session.account_key(),&thread,group,&text,at).map_err(|e|e.to_string());
Ok(R::ScheduleSaved{request,thread,text,at,result})}
,
            C::Edit{
id,at}
=>{
crate::tools::edit_job(&store,session.account_key(),&id,at)?;
Ok(R::Loaded{
config:crate::tools::preferences(&store)?,tasks:crate::tools::scheduled(&store,session.account_key())?,paused:crate::tools::paused(&store)?}
)}
,
            C::ReadConfig{
stickers}
=>{

                let Some(path)=tools_file(false,"Chọn cấu hình JSON",&["json"])else{
return Ok(R::Status("Đã hủy".into()));
}
;

                let bytes=crate::limits::read_file(&path,1024*1024)?;
let mut config=crate::tools::preferences(&store)?;

                if stickers {
config.stickers=serde_json::from_slice(&bytes)?;
}
else{
config.theme=serde_json::from_slice(&bytes)?;
}

                anyhow::ensure!(session.is_active(),"Đã đăng xuất");
let config=crate::tools::save_preferences(&store,config)?;
Ok(R::Loaded{
config,tasks:crate::tools::scheduled(&store,session.account_key())?,paused:crate::tools::paused(&store)?}
)
            }

            C::ExportConfig{
stickers}
=>{

                let Some(path)=tools_file(true,"Xuất cấu hình JSON",&["json"])else{
return Ok(R::Status("Đã hủy".into()));
}
;

                let config=crate::tools::preferences(&store)?;

                let value=if stickers {
if config.stickers.is_empty(){
serde_json::json!([{
"cate_id":61900,"name":"Sticker của tôi","sticker_ids":[98930,98931],"sticker_type":7}
])}
else{
serde_json::to_value(&config.stickers)?}
}
else if config.theme.is_null(){
serde_json::json!({
"dark":{
"primary":"#528BFF","bg_sidebar":"#21252B"}
,"light":{
"primary":"#0068FF","bg_sidebar":"#FFFFFF"}
}
)}
else{
config.theme}
;

                let mut file=crate::media_resources::AtomicPrivateFile::create(&path,&crate::param_cipher::random_hex(12))?;
file.write_all(&serde_json::to_vec_pretty(&value)?)?;
file.commit()?;
Ok(R::Status("Đã xuất cấu hình JSON".into()))
            }

        }

    }
).await?
}

fn tools_file(save: bool, title: &str, extensions: &[&str]) -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let picker = rfd::FileDialog::new()
            .set_title(title)
            .add_filter("Dữ liệu", extensions);

        if save {
            picker.save_file()
        } else {
            picker.pick_file()
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (save, title, extensions);

        None
    }
}

type ImageFlights = std::sync::Arc<std::sync::Mutex<HashSet<(String, String)>>>;

struct ImageFlight {
    owner: String,
    key: String,
    flights: ImageFlights,
}

impl ImageFlight {
    fn start(flights: ImageFlights, owner: &str, key: &str) -> Option<Self> {
        let inserted = flights
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((owner.into(), key.into()));

        inserted.then(|| Self {
            owner: owner.into(),
            key: key.into(),
            flights,
        })
    }
}

impl Drop for ImageFlight {
    fn drop(&mut self) {
        self.flights
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&(self.owner.clone(), self.key.clone()));
    }
}

#[cfg(test)]
mod image_flight_tests {

    use super::*;

    #[test]
    fn duplicate_prefetch_and_visible_request_share_an_account_flight() {
        let flights = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));

        let first = ImageFlight::start(flights.clone(), "owner", "thumb_1").unwrap();

        assert!(ImageFlight::start(flights.clone(), "owner", "thumb_1").is_none());

        assert!(ImageFlight::start(flights.clone(), "other", "thumb_1").is_some());

        drop(first);
        assert!(ImageFlight::start(flights, "owner", "thumb_1").is_some());
    }
}

/// Identity for renewal and stable caching of one attachment.
#[derive(Clone)]
pub struct ChatImage {
    pub thread: String,
    pub group: bool,
    pub message: ChatMessage,
    pub preview: bool,
}
fn media_context_alive(session: &Session, context: Option<&ChatImage>) -> bool {
    session.is_active()
        && context.is_none_or(|source| {
            (!session.hidden.enforced()
                || session
                    .hidden
                    .view(session.account_key())
                    .allows(&source.thread, source.group))
                && !crate::message_lifecycle::expired(
                    &source.message,
                    crate::login::unix_now() * 1000,
                )
        })
}
async fn fetch_chat_image(
    session: &Session,
    key: &str,
    url: &str,
    context: Option<ChatImage>,
    events: &Sender<GuiEvent>,
) -> anyhow::Result<egui::ColorImage> {
    anyhow::ensure!(
        media_context_alive(session, context.as_ref()),
        "Tin nhắn đã hết hạn hoặc phiên đã đóng"
    );
    let result = fetch_chat_image_inner(session, key, url, context.clone(), events).await;
    anyhow::ensure!(
        media_context_alive(session, context.as_ref()),
        "Tin nhắn đã hết hạn hoặc phiên đã đóng"
    );
    result
}
async fn fetch_chat_image_inner(
    s: &Session,
    key: &str,
    url: &str,
    context: Option<ChatImage>,
    events: &Sender<GuiEvent>,
) -> anyhow::Result<egui::ColorImage> {
    if let Some(source) = &context {
        if let Some(path) = crate::desktop_backup::restored_path(s, &source.message, source.preview)
        {
            if tokio::fs::symlink_metadata(&path)
                .await
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= 64 * 1024 * 1024)
            {
                let key = key.to_owned();
                let restored = tokio::task::spawn_blocking(move || {
                    let bytes = std::fs::read(path)?;
                    let image = crate::desktop_backup::preview_image(&bytes, &key)?;
                    Ok(egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    ))
                })
                .await?;
                // An unsupported local encoding must not strand a valid remote attachment.
                if restored.is_ok() {
                    return restored;
                }
            }
        }
    }
    let first = fetch_gui_image(s, crate::storage::root(), key, url).await;
    let should = first
        .as_ref()
        .err()
        .and_then(|e| e.downcast_ref::<crate::media_recovery::Failure>())
        .is_some_and(|f| f.renew_candidate());
    let Some(mut context) = context.filter(|_| should) else {
        return first;
    };
    let media = crate::media_renew::renew(s, &context.thread, context.group, &context.message)
        .await
        .map_err(|_| crate::media_recovery::Failure::Recovery)?;
    let next = if context.preview {
        media.preview.clone()
    } else {
        media.thumbnail.clone()
    }
    .ok_or(crate::media_recovery::Failure::Missing)?;
    persist_media_projection(s, &mut context, &media, events).await?;
    fetch_gui_image(s, crate::storage::root(), key, &next).await
}

async fn save_media_target<F: FnMut(u64, Option<u64>)>(
    url: &str,
    path: &std::path::Path,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    progress: F,
    visual: bool,
    lease: Option<&crate::video_player::CachedVideo>,
    cached: bool,
) -> anyhow::Result<u64> {
    if let Some(lease) = lease {
        crate::media_download::save_video(url, path, cancel, progress, lease).await
    } else if cached {
        crate::media_download::save_cached(url, path, cancel, progress, visual).await
    } else {
        crate::media_download::save_classified(url, path, cancel, progress, visual).await
    }
}

async fn persist_media_projection(
    s: &Session,
    context: &mut ChatImage,
    media: &crate::message_view::Media,
    events: &Sender<GuiEvent>,
) -> anyhow::Result<()> {
    context.message.link_thumb = media.thumbnail.clone();
    context.message.link_url = media.original.clone();
    // Normalized renewal metadata survives reload without rewriting original archive evidence.
    context
        .message
        .wire_ref
        .fields
        .insert("renewed_media".into(), serde_json::to_value(media)?);
    let account = s.clone();
    let thread = context.thread.clone();
    let message = context.message.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        anyhow::ensure!(account.is_active(), "Đã đăng xuất");
        crate::store::MessageStore::open(account.history_path())?
            .update_media_metadata(&thread, &message)
    })
    .await??;
    if s.is_active() {
        let _ = events.send(GuiEvent::MediaRenewed {
            thread: context.thread.clone(),
            message: Box::new(context.message.clone()),
        });
    }
    Ok(())
}

#[cfg(test)]
mod mentioned_outgoing_tests {
    use super::*;
    #[test]
    fn durable_job_retains_uid_quote_and_client_identity_on_retry() {
        let mut draft = crate::mentions::Draft::from("Hi ");
        draft.insert(3, 3, "42", "An");
        let quoted = ChatMessage {
            id: "123".into(),
            sender: "An".into(),
            text: "quoted".into(),
            ..Default::default()
        };
        let (_, message) = outgoing_command(&WorkerCmd::SendMentioned {
            to_id: "group_1".into(),
            draft: draft.clone(),
            quoted: Some(quoted),
        })
        .unwrap();
        let persisted = serde_json::to_string(&message).unwrap();
        let loaded: ChatMessage = serde_json::from_str(&persisted).unwrap();
        let (_, retry) = outgoing_command(&WorkerCmd::RetryOutgoing {
            thread_id: "group_1".into(),
            message: loaded,
        })
        .unwrap();
        assert_eq!(retry.client_message_id, message.client_message_id);
        assert_eq!(retry.reply_to_id.as_deref(), Some("123"));
        let crate::outgoing::Payload::Mentioned { mentions, quoted } =
            &retry.outgoing_job.unwrap().payload
        else {
            panic!("lost mentions")
        };
        assert_eq!(*mentions, draft.mentions);
        assert_eq!(quoted.as_ref().unwrap().text, "quoted");
    }
}

/// Shared bounded upload path; the session is captured before any picker/preview await.
async fn send_attachment_batch(
    s: Session,
    batch: super::file_drop::Batch,
    events: Sender<GuiEvent>,
    ctx: egui::Context,
) {
    let owner = s.account_key().to_owned();
    static ATTACHMENT_QUEUE: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    let attachment_queue =
        ATTACHMENT_QUEUE.get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(16)));
    for (path, expected_stamp) in batch.files {
        let Ok(queued) = attachment_queue.clone().acquire_owned().await else {
            break;
        };
        if !s.is_active() {
            break;
        }
        let current_stamp = crate::outgoing::FileStamp::read(&path).await.ok();
        let invalid_source = current_stamp.is_none();
        let source_stamp = expected_stamp.clone().or_else(|| current_stamp.clone());
        let original_path = path.clone();
        let copy_dir = crate::photo_quality::outgoing_dir(s.account_key());
        let mut hd_copy = if invalid_source
            || batch.original
            || expected_stamp
                .as_ref()
                .is_some_and(|expected| Some(expected) != current_stamp.as_ref())
        {
            None
        } else {
            tokio::task::spawn_blocking(move || {
                crate::photo_quality::reduce(&original_path, &copy_dir)
            })
            .await
            .ok()
            .and_then(|r| {
                r.inspect_err(|e| crate::logger::log_error("photo_quality", &e.to_string()))
                    .ok()
            })
            .flatten()
        };
        if hd_copy.is_some() && source_stamp != crate::outgoing::FileStamp::read(&path).await.ok() {
            if let Some(copy) = hd_copy.take() {
                // Best-effort cleanup of a copy made while its source was being modified.
                let _ = tokio::fs::remove_file(copy).await;
            }
        }
        let hd_source = hd_copy.as_ref().map(|_| crate::outgoing::HdSource {
            stamp: expected_stamp.clone().or_else(|| current_stamp.clone()),
            path: path.clone(),
        });
        let path = hd_copy.clone().unwrap_or(path);
        let stamp = if hd_copy.is_none() {
            expected_stamp.or(current_stamp)
        } else {
            crate::outgoing::FileStamp::read(&path).await.ok()
        };
        let mut message = crate::outgoing::pending(
            format!(
                "[Tệp] {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            batch.group,
            crate::outgoing::Payload::Attachment {
                path,
                caption: batch.caption.clone(),
                stamp,
                hd_source,
            },
        );
        if invalid_source {
            message.delivery = crate::message_model::DeliveryState::Failed;
            if let Some(job) = message.outgoing_job.as_mut() {
                job.error = "Không đọc được tệp thường; thư mục không được hỗ trợ".into();
            }
        }
        let Some(active) = crate::outgoing::Active::start(&message.client_message_id) else {
            continue;
        };
        persist_outgoing(&s, &batch.thread, &message).await;
        let _ = events.send(GuiEvent::OutgoingState {
            owner: owner.clone(),
            thread_id: batch.thread.clone(),
            message: message.clone(),
        });
        ctx.request_repaint();
        if invalid_source {
            drop(active);
            continue;
        }
        let s = s.clone();
        let owner = owner.clone();
        let tx = events.clone();
        let ctx = ctx.clone();
        let thread_id = batch.thread.clone();
        tokio::spawn(async move {
            let _queued = queued;
            match crate::outgoing::send(&s, &thread_id, &message).await {
                Ok(sent) => message = sent,
                Err(error) => {
                    message.delivery = crate::message_model::DeliveryState::Failed;
                    if let Some(job) = message.outgoing_job.as_mut() {
                        job.error = crate::outgoing::failure_message(&error);
                    }
                }
            }
            persist_outgoing(&s, &thread_id, &message).await;
            drop(active);
            let _ = tx.send(GuiEvent::OutgoingState {
                owner,
                thread_id,
                message,
            });
            ctx.request_repaint();
        });
    }
}

async fn publish_pins(s: &Session, snapshot: crate::server_pins::Snapshot, tx: &Sender<GuiEvent>) {
    if !s.is_active() {
        return;
    }
    let account = s.clone();
    let saved = snapshot.clone();
    let result=tokio::task::spawn_blocking(move||->anyhow::Result<()>{
        if !account.is_active(){return Ok(());}
        let store=crate::store::MessageStore::open(account.history_path())?;
        store.conn.execute("INSERT INTO tool_config(key,value) VALUES('server_pins',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(&saved)?])?;Ok(())
    }).await;
    if !matches!(result, Ok(Ok(()))) {
        let _ = tx.send(GuiEvent::StatusUpdate(
            "Không lưu được danh sách ghim server".into(),
        ));
    }
    if s.is_active() {
        let _ = tx.send(GuiEvent::ServerPins {
            owner: s.account_key().into(),
            snapshot,
        });
    }
}

/// Invalidate first, refresh authoritative state later; socket reading never waits for HTTP/SQLite.
fn apply_metadata_impact(
    s: &Session,
    impact: crate::account_controls::Impact,
    notify: &std::sync::Arc<tokio::sync::Notify>,
    tx: &Sender<GuiEvent>,
    ctx: &egui::Context,
) {
    if impact.hidden {
        s.hidden.invalidate();
        let _ = tx.send(GuiEvent::Hidden {
            owner: s.account_key().into(),
            generation: None,
            view: s.hidden.view(s.account_key()),
            result: Err("Danh sách/PIN thay đổi từ thiết bị khác; đang xác minh lại".into()),
        });
    }
    if impact.profiles {
        if let Ok(mut policy) = s.inbox_policy.write() {
            policy.profile_types.clear();
        }
    }
    for (uid, status) in impact.activities {
        let _ = tx.send(GuiEvent::ActivityUpdated {
            owner: s.account_key().into(),
            uid,
            status,
        });
    }
    if impact.account {
        if let Ok(mut policy) = s.inbox_policy.write() {
            if impact.privacy {
                policy.direct = None;
                policy.show_online = None;
            }
            if impact.relations {
                policy.friends = None;
                policy.profile_types.clear();
            }
            if impact.profiles {
                policy.business = None;
            }
        }
        if impact.privacy || impact.relations || impact.profiles {
            let _ = tx.send(GuiEvent::AccountMetadataInvalidated {
                owner: s.account_key().into(),
                privacy: impact.privacy,
                relations: impact.relations,
                business: impact.profiles,
            });
        }
        notify.notify_one();
    }
    if impact.all_groups || !impact.groups.is_empty() {
        if impact.all_groups {
            crate::group_state::invalidate(s, None);
        } else {
            for id in &impact.groups {
                crate::group_state::invalidate(s, Some(id));
            }
        }
        let _ = tx.send(GuiEvent::GroupInvalidated {
            owner: s.account_key().into(),
            revision: s.group_revision.load(std::sync::atomic::Ordering::SeqCst),
            groups: impact.groups.clone(),
            all: impact.all_groups,
        });
        let account = s.clone();
        tokio::spawn(async move {
            let _ = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                if !account.is_active() {
                    return Ok(());
                }
                let store = crate::store::MessageStore::open(account.history_path())?;
                if impact.all_groups {
                    store.conn.execute("DELETE FROM group_metadata", [])?;
                } else {
                    for id in impact.groups {
                        let bare = &id[1..];
                        store.conn.execute(
                            "DELETE FROM group_metadata WHERE thread_id IN (?1,?2,?3)",
                            rusqlite::params![bare, id, format!("group_{bare}")],
                        )?;
                    }
                }
                Ok(())
            })
            .await;
        });
    }
    ctx.request_repaint();
}

#[cfg(test)]
mod hidden_media_gate_tests {
    use super::*;
    #[test]
    fn an_enforced_unknown_pin_state_blocks_a_stale_media_request() {
        let session = Session::new(false, false, false).unwrap();
        let source = ChatImage {
            thread: "12".into(),
            group: false,
            message: ChatMessage::default(),
            preview: true,
        };
        assert!(media_context_alive(&session, Some(&source)));
        session.hidden.enable();
        assert!(!media_context_alive(&session, Some(&source)));
    }
}
