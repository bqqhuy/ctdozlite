use super::*;

pub enum WorkerCmd {
    LoadComposerDraft {
        owner: String,
        thread: String,
        generation: u64,
    },
    SaveComposerDraft {
        owner: String,
        thread: String,
        generation: u64,
        text: String,
    },
    CheckUpdate,
    PrepareUpdate(crate::updater::ReleaseUpdate),
    LaunchUpdate(crate::updater::ReadyUpdate),
    PersistInfoDrawer {
        owner: String,
        open: bool,
    },
    PrepareNotificationSound,
    DownloadMedia {
        url: String,
        name: String,
        play: bool,
        context: Option<ChatImage>,
    },
    OpenDownloaded {
        path: std::path::PathBuf,
        folder: bool,
    },
    Tools {
        owner: String,
        command: super::tools_panel::ToolsCommand,
    },
    RunAutomation,
    RetryOutgoing {
        thread_id: String,
        message: ChatMessage,
    },
    ReadClipboardImage,
    RequestQr,
    PersistSession,
    LoginPassword {
        phone: String,
        pass: String,
    },
    RefreshThreads,
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
    },
    CancelHistorySync,
    /// Explicit phone-approved history transfer; never started by login/refresh.
    SyncPhoneHistory,
    RescanPhoneHistory,
    SyncPhoneRecent,
    CancelPhoneHistory,
    PollMessages {
        known_threads: Vec<(String, String, bool)>,
    },
    LoadGroupHistory {
        group_id: String,
        is_group: bool,
        limit: usize,
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
    EndCall { call_id: Option<i64> },
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
pub(super) async fn preload_disk_sticker_thumbnails(items:&[crate::chat::StickerItem],gui_tx:&Sender<GuiEvent>,limit:usize)->Vec<(String,String,std::path::PathBuf)> {
    let items=items.to_vec();let tx=gui_tx.clone();
    match tokio::task::spawn_blocking(move||preload_disk_sticker_inner(&items,&tx,limit)).await {
        Ok(missing)=>missing,Err(_)=>{crate::logger::log_error("sticker cache","Thumbnail cache worker failed");Vec::new()}
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
        if let Ok(disk_bytes) = crate::limits::read_file(&cache_path,crate::limits::IMAGE_BYTES) {
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

async fn persist_received(session: &Session, threads: &mut Vec<Thread>) {
    let session = session.clone();
    let mut batch = threads.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Thread>> {
        anyhow::ensure!(session.is_active(), "Account changed");
        let mut store = crate::store::MessageStore::open(session.history_path())?;
        store.filter_deleted(&mut batch)?;
        // Once deletion filtering succeeds, a cache-write or automation failure must
        // not suppress otherwise valid live delivery (history persistence is fail-open).
        match crate::store::persist_threads(&mut store, &batch) {
            Ok(_) => {
                if crate::tools::auto_reply(&store, session.account_key(), &batch).is_err() {
                    eprintln!("Cannot evaluate auto reply for received batch");
                }
            }
            Err(_) => {
                eprintln!("Cannot persist received message batch; delivering filtered messages")
            }
        }
        Ok(batch)
    })
    .await;
    match result {
        Ok(Ok(batch)) => *threads = batch,
        _ => {
            eprintln!("Cannot persist or filter received message batch");
            threads.clear();
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
    if let Some(bytes)=crate::bundled_stickers::thumbnail(key) {
        let key=key.to_owned();
        return tokio::task::spawn_blocking(move|| {
            let image=crate::limits::preview_image(bytes,&key)?;
            Ok(egui::ColorImage::from_rgba_unmultiplied([image.width() as usize,image.height() as usize],image.as_raw()))
        }).await?;
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
    let mute_sync = MuteSync::default();
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
                                match fetch_chat_image(&s,&key,&url,context,&tx).await {
                                    Ok(image) => { let _ = tx.send(GuiEvent::PreviewImageReady { key,image,permit:job }); },
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

                // 1) Real-time `zpw_ws` WebSocket loop for instant chat messages + VoIP events
                {
                    let live_updates = live_updates.clone();
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
                                    phone_transport_tx.send_replace(Some(transport));
                                }
                                if let Some(rx) = ws_rx.as_mut() {
                                    match tokio::time::timeout(
                                        Duration::from_millis(1500),
                                        rx.recv(),
                                    )
                                    .await
                                    {
                                        Ok(Some(ev)) => {
                                            let data_node = ev
                                                .payload
                                                .get("data")
                                                .unwrap_or(&ev.payload);
                                            for voip_ev in
                                                crate::call::extract_voip_controls(data_node)
                                            {
                                                cb.handle_control_event(voip_ev);
                                            }
                                            if ev.is_voip_ish() {
                                                let mutes = crate::mute_sync::parse_mute_controls(data_node, crate::login::unix_now());
                                                apply_server_mutes(&s_snap, mutes, &mute_sync, &gui_tx_ws, &live_updates, &ctx_ws).await;
                                            }
                                            if ev.is_chat_ish() || data_node.get("clearUnreads").is_some()
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
                                                if !ws_threads.is_empty() {
                                                    crate::chat::resolve_thread_names(
                                                        &s_snap,
                                                        &mut ws_threads,
                                                    )
                                                    .await;
                                                    persist_received(&s_snap, &mut ws_threads).await;
                                                    if publish_live(&s_snap, &live_updates, ws_threads) {
                                                        let _ = gui_tx_ws.send(GuiEvent::LiveThreadsReady(live_updates.clone()));
                                                    }
                                                    ctx_ws.request_repaint();
                                                }
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
                                if let Ok((events, mutes)) = voip_res {
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

                let mut current_qr_task: Option<tokio::task::JoinHandle<()>> = None;
                let is_polling = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let known_threads = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<(String, String, bool)>::new()));
                let auto_history_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                {
                    let commands = commands.clone();
                    let session = session.clone();
                    let known = known_threads.clone();
                    let polling = is_polling.clone();
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
                            let snapshot = known.lock().await.clone();
                            if let Some(tx) = commands.upgrade() {
                                if tx.send(WorkerCmd::PollMessages {known_threads: snapshot}).is_err() { break; }
                            } else { break; }
                        }
                    });
                }

                {
                    let commands=commands.clone();
                    tokio::spawn(async move {let mut tick=tokio::time::interval(Duration::from_secs(2));loop{tick.tick().await;let Some(tx)=commands.upgrade()else{break;};if tx.send(WorkerCmd::RunAutomation).is_err(){break;}}});
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
                            let result=tokio::task::spawn_blocking(move||->anyhow::Result<String>{
                                anyhow::ensure!(s.is_active() && !expected.is_empty() && s.account_key()==expected,"Tài khoản đã thay đổi");
                                crate::store::MessageStore::open(s.history_path())?.composer_draft(&id)
                            }).await.map_err(|_|"Không đọc được bản nháp".to_owned()).and_then(|r|r.map_err(|_|"Không đọc được bản nháp".into()));
                            let _=gui_tx.send(GuiEvent::ComposerDraftLoaded{owner,thread,generation,result});egui_ctx.request_repaint();
                        }
                        WorkerCmd::SaveComposerDraft{owner,thread,generation,text}=>{
                            let s=session.lock().await.clone();
                            let (id,expected,value)=(thread.clone(),owner.clone(),text.clone());
                            let result=tokio::task::spawn_blocking(move||->anyhow::Result<()>{
                                anyhow::ensure!(s.is_active() && !expected.is_empty() && s.account_key()==expected,"Tài khoản đã thay đổi");
                                crate::store::MessageStore::open(s.history_path())?.save_composer_draft(&id,&value)
                            }).await.map_err(|_|"Không lưu được bản nháp".to_owned()).and_then(|r|r.map_err(|_|"Không lưu được bản nháp".into()));
                            let _=gui_tx.send(GuiEvent::ComposerDraftSaved{owner,thread,generation,text,result});egui_ctx.request_repaint();
                        }
                        WorkerCmd::Tools {owner,command}=>{
                            let s=session.lock().await.clone();if owner!=s.account_key() || owner.is_empty() || !s.is_active(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let schedule=match &command {super::tools_panel::ToolsCommand::Schedule{request,thread,text,at,..}=>Some((*request,thread.clone(),text.clone(),*at)),_=>None};
                            tokio::spawn(async move{
                                let mut result=account_tools(s,command).await.map_err(|e|e.to_string());
                                if let (Err(error),Some((request,thread,text,at)))=(&result,schedule){result=Ok(super::tools_panel::ToolsResult::ScheduleSaved{request,thread,text,at,result:Err(error.clone())});}
                                let _=tx.send(GuiEvent::Tools{owner,result});ctx.request_repaint();
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
                        WorkerCmd::OpenDownloaded{path,folder}=>{
                            tokio::task::spawn_blocking(move||{if folder{if let Some(parent)=path.parent(){crate::platform::open_folder(parent);}}else{crate::platform::open_file(&path);}});
                        }
                        WorkerCmd::DownloadMedia{mut url,name,play,context}=>{
                            static REQUESTS:tokio::sync::Semaphore=tokio::sync::Semaphore::const_new(4);
                            let Ok(request_slot)=REQUESTS.try_acquire() else {let _=gui_tx.send(GuiEvent::StatusUpdate("Đang có nhiều yêu cầu tải; vui lòng chờ hoặc hủy bớt".into()));continue;};
                            let account=session.lock().await.clone();if !account.is_active() || account.account_key().is_empty(){continue;}
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move{
                                let _request_slot=request_slot;
                                let name=crate::media_download::safe_name(&name);
                                let id=crate::chat::next_client_message_id().to_string();
                                let owner=account.account_key().to_owned();
                                let video_lease=if play {
                                    let root=account.history_path().with_extension("video-cache");let name=name.clone();let id=id.clone();
                                    tokio::task::spawn_blocking(move||crate::video_player::CachedVideo::reserve(&root,&format!("{id}-{name}"))).await.ok().and_then(Result::ok)
                                }else{None};
                                let destination=if play {video_lease.as_ref().map(|v|v.path().to_owned())}else{
                                    let name=name.clone();tokio::task::spawn_blocking(move||{
                                        #[cfg(windows)]{rfd::FileDialog::new().set_title("Lưu tệp về máy").set_file_name(&name).save_file()}
                                        #[cfg(not(windows))]{let _=name;None}
                                    }).await.ok().flatten()
                                };
                                let Some(path)=destination else{return;};
                                let cancel=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                                let _=tx.send(GuiEvent::MediaStarted{owner:owner.clone(),id:id.clone(),name:name.clone(),cancel:cancel.clone()});ctx.request_repaint();
                                let visual=play||context.as_ref().is_some_and(|c|matches!(crate::message_view::media(&c.message).kind,crate::message_view::MediaKind::Photo|crate::message_view::MediaKind::Video));
                                let mut result=save_media_target(&url,&path,cancel.clone(),|bytes,total|{
                                    if !account.is_active(){cancel.store(true,std::sync::atomic::Ordering::Release);}
                                    let _=tx.send(GuiEvent::MediaProgress{owner:owner.clone(),id:id.clone(),bytes,total});ctx.request_repaint();
                                },visual,video_lease.as_ref()).await;
                                if result.as_ref().err().and_then(|e|e.downcast_ref::<crate::media_recovery::Failure>()).is_some_and(|e|e.renew_candidate()) {
                                    if let Some(context)=context.as_ref(){
                                        if let Ok(media)=crate::media_renew::renew(&account,&context.thread,context.group,&context.message).await {
                                            if let Some(next)=media.original.clone() {let mut renewed=context.clone();if persist_media_projection(&account,&mut renewed,&media,&tx).await.is_err(){crate::logger::log_error("media renewal","Cannot persist renewed file metadata");}url=next;result=save_media_target(&url,&path,cancel.clone(),|bytes,total|{let _=tx.send(GuiEvent::MediaProgress{owner:owner.clone(),id:id.clone(),bytes,total});ctx.request_repaint();},visual,video_lease.as_ref()).await;}
                                        }
                                    }
                                }
                                let result=result.map(|_|path.clone()).map_err(|e|e.to_string());
                                let success=result.is_ok();let _=tx.send(GuiEvent::MediaFinished{owner:owner.clone(),id:id.clone(),result});ctx.request_repaint();
                                if success && play && account.is_active(){
                                    let result=tokio::task::spawn_blocking(move||crate::video_player::play(&path,&name,||account.is_active())).await;
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
                        WorkerCmd::PrepareNotificationSound=>{
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();tokio::spawn(async move{
                                if crate::notification_sound::prepare().await.is_err(){let _=tx.send(GuiEvent::StatusUpdate("Chưa tải được âm báo Zalo; kiểm tra mạng rồi bật lại âm thanh".into()));ctx.request_repaint();}
                            });
                        }
                        WorkerCmd::ReadClipboardImage => {
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let image = tokio::task::spawn_blocking(super::messages::read_clipboard_image_to_png).await.ok().flatten();
                                let _ = tx.send(GuiEvent::ClipboardImageRead(image));
                                ctx.request_repaint();
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
                        WorkerCmd::Logout => {
                            if let Some(task) = current_qr_task.take() { task.abort(); }
                            call_bridge.end_call();
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
                        WorkerCmd::PersistSession => {
                            let s = session.lock().await;
                            if s.login.is_some() {
                                if let Err(e) = s.save_to_file("session.json") {
                                    let _ = gui_tx.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                }
                            }
                        }
                        WorkerCmd::RequestQr => {
                            if let Some(task) = current_qr_task.take() {
                                task.abort();
                            }
                            let session_clone = session.clone();
                            let poll_clone = poll_state.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            let commands = commands.clone();

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
                        command @ (WorkerCmd::SyncPhoneHistory | WorkerCmd::RescanPhoneHistory | WorkerCmd::SyncPhoneRecent) => {
                            let full_rescan=matches!(command,WorkerCmd::RescanPhoneHistory);
                            let recent_only=matches!(command,WorkerCmd::SyncPhoneRecent);
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
                                    crate::phone_sync::download(&account,transport,cancel,full_rescan,recent_only,
                                        move |message|{if !progress_cancel.load(std::sync::atomic::Ordering::Acquire){let _=progress_tx.send(GuiEvent::PhoneSyncStatus{running:true,message});progress_ctx.request_repaint();}},
                                        |thread|{if deliver_thread_updates() {let _=tx.send(GuiEvent::ThreadsUpdated{threads:vec![thread],source:ThreadUpdateSource::History});ctx.request_repaint();}}).await
                                }else{Err(anyhow::anyhow!("WebSocket chưa sẵn sàng; chờ kết nối rồi thử lại"))};
                                let message=result.unwrap_or_else(|e|format!("Đồng bộ điện thoại đã dừng: {e}"));
                                let _=tx.send(GuiEvent::PhoneSyncStatus{running:false,message});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::CancelHistorySync => {
                            history_sync_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        WorkerCmd::SyncHistory { threads } => {
                            if history_sync_running.swap(true, std::sync::atomic::Ordering::SeqCst) { continue; }
                            history_sync_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
                            let running = history_sync_running.clone();
                            let cancel = history_sync_cancel.clone();
                            let s = session.lock().await.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            let _ = tx.send(GuiEvent::HistorySyncStatus { running: true, message: "Đang tải lịch sử nhóm và Cloud từ máy chủ…".into() });
                            ctx.request_repaint();
                            tokio::spawn(async move {
                                let result = crate::history_sync::download(&s, threads, cancel,
                                    |message| { if deliver_thread_updates() { let _ = tx.send(GuiEvent::HistorySyncStatus { running: true, message }); ctx.request_repaint(); } },
                                    |thread| { if deliver_thread_updates() { let _ = tx.send(GuiEvent::ThreadsUpdated {threads:vec![thread],source:ThreadUpdateSource::History}); ctx.request_repaint(); } }).await;
                                running.store(false, std::sync::atomic::Ordering::SeqCst);
                                let message = result.unwrap_or_else(|e| format!("Lỗi tải lịch sử: {e}"));
                                let _ = tx.send(GuiEvent::HistorySyncStatus { running: false, message });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::RefreshThreads => {
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
                            let commands = commands.clone();
                            let known = known_threads.clone();
                            let auto_history_started = auto_history_started.clone();
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
                                    crate::store::load_history_into_threads(
                                        s.history_path(),
                                        &mut list,
                                        crate::store::MessageStore::PRELOAD_MSGS,
                                    );
                                    let recent_result={
                                        let mut ps = poll_clone.lock().await;
                                        crate::chat::poll_new_messages(&s, &mut ps, &mut list).await
                                    };
                                    persist_received(&s, &mut list).await;
                                    *known.lock().await = list.iter().map(|t| (t.id.clone(), t.name.clone(), t.is_group)).collect();
                                    let eligible = list.iter().any(|t| t.is_group || s.login.as_ref().is_some_and(|li| !li.send2me_id.is_empty() && li.send2me_id == t.id));
                                    if eligible && !auto_history_started.swap(true, std::sync::atomic::Ordering::SeqCst) {
                                        if let Some(tx) = commands.upgrade() {
                                            let _ = tx.send(WorkerCmd::SyncHistory { threads: list.clone() });
                                        }
                                    }
                                    let count = list.len();
                                    let _ = gui_tx_clone.send(GuiEvent::ThreadsUpdated {threads:list,source:ThreadUpdateSource::History});
                                    notice.message=match recent_result {
                                        Ok(_) => format!("Đã cập nhật {count} hội thoại và tin gần đây; không phải toàn bộ lịch sử điện thoại"),
                                        Err(error) => format!("Đã cập nhật {count} hội thoại; chưa lấy được tin gần đây: {error}"),
                                    };
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
                            tokio::spawn(async move {
                                let _poll_guard = poll_guard;
                                let s = session_clone.lock().await.clone();
                                if s.login.is_some() {
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

                                    if let Some(err_s) = poll_err {
                                        if is_session_expired_error(&err_s) {
                                            if let Err(e) = crate::storage::remove_session() { eprintln!("Cannot remove session: {e}"); }
                                            {
                                                let mut sl = session_clone.lock().await;
                                                sl.logout();
                                            }
                                            let _ = gui_tx_clone.send(GuiEvent::SessionExpired(
                                                "Phiên đăng nhập (zpw_sek) đã hết hạn".into(),
                                            ));
                                            ctx_clone.request_repaint();
                                        }
                                    } else {
                                        temp_threads.retain(|t| !t.messages.is_empty() || t.unread.fresh);
                                        if !temp_threads.is_empty() {
                                            crate::chat::resolve_thread_names(
                                                &s,
                                                &mut temp_threads,
                                            )
                                            .await;
                                            persist_received(&s, &mut temp_threads).await;
                                            if publish_live(&s, &live_updates, temp_threads) {
                                                let _ = gui_tx_clone.send(GuiEvent::LiveThreadsReady(live_updates.clone()));
                                            }
                                            ctx_clone.request_repaint();
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
                                    if is_group && !is_send2me {
                                        if let Ok(resp) =
                                            crate::chat::get_group_history(&s, &group_id, 50).await
                                        {
                                            crate::chat::merge_group_history(&mut t, &resp);
                                        }
                                    }
                                    if is_group || is_send2me {
                                        if let Ok(cm_resp) = crate::chat::get_cloud_messages(
                                            &s,
                                            &group_id,
                                            "0",
                                            false,
                                            50,
                                        )
                                        .await
                                        {
                                            crate::chat::merge_group_history(&mut t, &cm_resp);
                                        }
                                    }
                                    if !t.messages.is_empty() {
                                        let cnt = t.messages.len();
                                        let mut filtered=vec![t];
                                        persist_received(&s, &mut filtered).await;
                                        let _ = gui_tx_clone.send(GuiEvent::ThreadsUpdated {threads:filtered,source:ThreadUpdateSource::History});
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Đã đọc {cnt} tin từ lịch sử cục bộ và tin gần đây; lịch sử server theo dõi trong Đồng bộ"
                                        )));
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                        }
                        WorkerCmd::SendMessage { .. } | WorkerCmd::SendQuote { .. } | WorkerCmd::Forward { .. } | WorkerCmd::SendSticker { .. } => {
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
                                        let _ = gui_tx_clone.send(GuiEvent::MessageRecalled {
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
                        WorkerCmd::PickAndSendAttachment {
                            to_id,
                            is_group,
                            images_only,
                            caption,
                            preset_path,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let paths = if let Some(path) = preset_path { vec![path] } else {
                                    tokio::task::spawn_blocking(move || super::messages::pick_native_files_dialog(images_only)).await.ok().flatten().unwrap_or_default()
                                };
                                let s = session_clone.lock().await.clone();
                                let owner = s.account_key().to_owned();
                                static ATTACHMENT_QUEUE:std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>>=std::sync::OnceLock::new();
                                let attachment_queue=ATTACHMENT_QUEUE.get_or_init(||std::sync::Arc::new(tokio::sync::Semaphore::new(16)));
                                for path in paths {
                                    let Ok(queued)=attachment_queue.clone().acquire_owned().await else{break;};
                                    if !s.is_active(){break;}
                                    let stamp = crate::outgoing::FileStamp::read(&path).await.ok();
                                    let mut message = crate::outgoing::pending(format!("[Tệp] {}", path.file_name().unwrap_or_default().to_string_lossy()), is_group,
                                        crate::outgoing::Payload::Attachment { path, caption: caption.clone(), stamp });
                                    let Some(active) = crate::outgoing::Active::start(&message.client_message_id) else { continue; };
                                    persist_outgoing(&s, &to_id, &message).await;
                                    let _ = gui_tx_clone.send(GuiEvent::OutgoingState { owner: owner.clone(), thread_id: to_id.clone(), message: message.clone() });
                                    ctx_clone.request_repaint();
                                    let s = s.clone();
                                    let owner = owner.clone();
                                    let tx = gui_tx_clone.clone(); let ctx = ctx_clone.clone(); let thread_id = to_id.clone();
                                    tokio::spawn(async move {
                                        let _queued=queued;
                                        match crate::outgoing::send(&s, &thread_id, &message).await {
                                            Ok(sent) => message = sent,
                                            Err(error) => { message.delivery = crate::message_model::DeliveryState::Failed; if let Some(job) = message.outgoing_job.as_mut() { job.error = crate::outgoing::failure_message(&error); } }
                                        }
                                        persist_outgoing(&s, &thread_id, &message).await;
                                        drop(active);
                                        let _ = tx.send(GuiEvent::OutgoingState { owner, thread_id, message }); ctx.request_repaint();
                                    });
                                }
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
                        WorkerCmd::EndCall { call_id } => {
                            call_bridge.end_call_for(call_id);

                            egui_ctx.request_repaint();
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
                            if let Some(task) = current_qr_task.take() { task.abort(); }
                            session.lock().await.logout();
                            let session_clone = session.clone();
                            let commands = commands.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
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
        let mut session=Session::new(false,false,false).unwrap();
        session.login=Some(crate::session::LoginInfo{uin:"bundled-thumbnail-fixture".into(),..Default::default()});
        let root=std::env::temp_dir().join(format!("ctdoz-no-cache-{}",crate::param_cipher::random_hex(16)));
        let image=fetch_gui_image(&session,&root,"stk_99901_999001","https://untrusted.invalid/not-contacted").await.unwrap();
        assert!(image.size[0]>0 && image.size[1]>0);
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
            crate::logger::log_error("mute sync", "Mute snapshot unavailable; keeping local state");
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
                local_only.push((change.thread_id.clone(), store.thread_is_group(&change.thread_id)?));
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
        WorkerCmd::SendMessage {
            to_id,
            text,
            is_group,
        } => Some((
            to_id.clone(),
            pending(text.clone(), *is_group, Payload::Text),
        )),
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

fn publish_live(session:&Session, updates: &crate::live_updates::LiveUpdates, threads: Vec<Thread>) -> bool {
    if !session.is_active(){return false;}
    let wake=updates.publish(threads);
    #[cfg(windows)]{
        crate::desktop_notice::count(session.account_key(),updates.total());
        if let Some(mut notice)=updates.take_notice(){notice.owner=session.account_key().to_owned();if !crate::tray::previews_visible(){
                if let Some(url)=notice.avatar.clone(){
                    static AVATAR_LOADING:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
                    if !AVATAR_LOADING.swap(true,std::sync::atomic::Ordering::AcqRel){
                        let account=session.clone();tokio::spawn(async move{
                            struct Guard;impl Drop for Guard{fn drop(&mut self){AVATAR_LOADING.store(false,std::sync::atomic::Ordering::Release);}}let _guard=Guard;
                            let key=format!("ava_{}",crate::crypto::md5_hex(&url));
                            if let Ok(image)=fetch_gui_image(&account,crate::storage::root(),&key,&url).await{if account.is_active(){crate::desktop_notice::cache_avatar(account.account_key().into(),url,image);}}
                        });
                    }
                }
                crate::desktop_notice::publish(notice,updates.total());
            }}
        let _=updates.take_notification();
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

    tokio::task::spawn_blocking(move || -> anyhow::Result<R> {

        anyhow::ensure!(session.is_active() && !session.account_key().is_empty(),"Phiên làm việc đã đóng");

        let mut store=crate::store::MessageStore::open(session.history_path())?;

        match command {

            C::OpenHit{
thread,id}
=>Ok(R::Hit{
thread:Box::new(crate::tools::open_hit(&store,&thread,&id)?),id}
),
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
hits:crate::tools::search(&store,&query,hidden)?,query}
),
            C::ExportHistory=>{
                let Some(path)=tools_file(true,"Xuất lịch sử",&["db"])else{return Ok(R::Status("Đã hủy xuất lịch sử".into()))};
                let n=crate::tools::export_history(&store,&path,session.account_key(),&||session.is_active())?;
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
async fn fetch_chat_image(
    s: &Session,
    key: &str,
    url: &str,
    context: Option<ChatImage>,
    events: &Sender<GuiEvent>,
) -> anyhow::Result<egui::ColorImage> {
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
) -> anyhow::Result<u64> {
    if let Some(lease) = lease {
        crate::media_download::save_video(url, path, cancel, progress, lease).await
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
