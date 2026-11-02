use super::*;

pub enum WorkerCmd {
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
        value: crate::store::ConversationPreference,
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
        accept: bool,
    },
    SetCallMediaState {
        is_on_mic: bool,
        is_on_cam: bool,
    },
    EndCall,
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
pub(super) struct WorkerMailbox(
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<WorkerCmd>>>,
);
impl WorkerMailbox {
    pub(super) fn get(&self) -> Option<tokio::sync::mpsc::UnboundedSender<WorkerCmd>> {
        self.0.lock().ok()?.clone()
    }
    fn set(&self, sender: tokio::sync::mpsc::UnboundedSender<WorkerCmd>) {
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

pub(super) fn extract_msg_id(resp: &serde_json::Value) -> String {
    crate::chat::sent_server_id(resp)
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
pub(super) fn preload_disk_sticker_thumbnails(
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
        if let Ok(disk_bytes) = std::fs::read(&cache_path) {
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
    http: &reqwest::Client,
    missing: Vec<(String, String, std::path::PathBuf)>,
    gui_tx: &Sender<GuiEvent>,
    egui_ctx: &egui::Context,
) {
    if missing.is_empty() {
        return;
    }
    let futs = missing.into_iter().map(|(key, url, cache_path)| {
        let client = http.clone();
        let tx = gui_tx.clone();
        let ctx = egui_ctx.clone();
        async move {
            let Ok(_permit) = crate::limits::IMAGE_JOBS.acquire().await else {
                return;
            };
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(b) =
                        crate::limits::response_bytes(resp, crate::limits::IMAGE_BYTES).await
                    {
                        let vec = b.to_vec();
                        if let Ok(dyn_img) = crate::limits::decode_image(&vec) {
                            let _ = std::fs::write(&cache_path, &vec);
                            let rgba = dyn_img.to_rgba8();
                            let size = [rgba.width() as usize, rgba.height() as usize];
                            let pixels = rgba.as_flat_samples();
                            let color_img =
                                egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                            let _ = tx.send(GuiEvent::StickerImageReady {
                                key,
                                image: color_img,
                            });
                            ctx.request_repaint();
                        }
                    }
                }
            }
        }
    });
    futures_util::future::join_all(futs).await;
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

async fn persist_received(session: &Session, threads: &[Thread]) {
    let session = session.clone();
    let threads = threads.to_vec();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        if !session.is_active() {
            return Ok(());
        }
        let mut store = crate::store::MessageStore::open(session.history_path())?;
        crate::store::persist_threads(&mut store, &threads)?;
        Ok(())
    })
    .await;
    if !matches!(result, Ok(Ok(()))) {
        eprintln!("Cannot persist received message batch");
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
        let response = s
            .http
            .get(&url)
            .timeout(Duration::from_secs(20))
            .send()
            .await?;
        anyhow::ensure!(response.status().is_success(), "Image download failed");
        crate::limits::response_bytes(response, crate::limits::IMAGE_BYTES).await?
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
    let rgba = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let image = crate::limits::preview_image(&bytes, &key)?;
        if crate::storage::atomic_write(&path, &bytes).is_err() || cache.prune().is_err() {
            // Rendering may proceed when a full/read-only cache cannot be updated.
            crate::logger::log_error("image cache", "Cannot save/prune preview cache");
        }
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
    let image_jobs = crate::media_resources::MediaJobQueue::default();
    let commands = cmd_tx.downgrade();
    WORKER_TX.set(cmd_tx);

    let refresh_in_flight = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
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
                    let s = session.lock().await.clone();
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
                                            if ev.is_chat_ish() {
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
                                                ws_threads.retain(|t| !t.messages.is_empty());
                                                if !ws_threads.is_empty() {
                                                    crate::chat::resolve_thread_names(
                                                        &s_snap,
                                                        &mut ws_threads,
                                                    )
                                                    .await;
                                                    persist_received(&s_snap, &ws_threads).await;
                                                    if deliver_thread_updates() {
                                                        let _ = gui_tx_ws.send(GuiEvent::ThreadsUpdated {threads:ws_threads,source:ThreadUpdateSource::Live});
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

                // 2) Background HTTP long-poll fallback for VoIP control events (`lp_control`)
                {
                    let s_ctrl = session.clone();
                    let cps = ctrl_poll_state.clone();
                    let cb = call_bridge.clone();
                    tokio::spawn(async move {
                        loop {
                            let s_snap = s_ctrl.lock().await.clone();
                            if s_snap.login.is_some() {
                                let voip_res = {
                                    let mut st = cps.lock().await;
                                    crate::call::poll_voip_controls(&s_snap, &mut st).await
                                };
                                if let Ok(events) = voip_res {
                                    for ev in events {
                                        cb.handle_control_event(ev);
                                    }
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

                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        WorkerCmd::SetConversationPreference { owner, thread_id, value } => {
                            let s = session.lock().await.clone();
                            if owner.is_empty() || owner != s.account_key() || !s.is_active() { continue; }
                            let owner = owner.clone();
                            let result=tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                anyhow::ensure!(s.is_active(), "Phiên đã kết thúc");
                                let store=crate::store::MessageStore::open(s.history_path())?;
                                store.set_conversation_preference(&thread_id,value)?;
                                store.conversation_preferences()
                            }).await;
                            match result {
                                Ok(Ok(values)) => { let _ = gui_tx.send(GuiEvent::ConversationPreferences { owner, values }); }
                                _ => { let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể lưu tùy chọn hội thoại".into())); }
                            }
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
                        }
                        WorkerCmd::SyncPhoneHistory => {
                            if phone_sync_running.swap(true,std::sync::atomic::Ordering::SeqCst) {continue;}
                            phone_sync_cancel.store(false,std::sync::atomic::Ordering::Relaxed);
                            let cancel=phone_sync_cancel.clone();let running=phone_sync_running.clone();
                            let transport=phone_transport_rx.borrow().clone();
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let _=tx.send(GuiEvent::PhoneSyncStatus{running:true,message:"Đang chuẩn bị đồng bộ từ điện thoại…".into()});
                            ctx.request_repaint();
                            tokio::spawn(async move {
                                let _guard=PollGuard(running);
                                let result=if let Some(transport)=transport {
                                    crate::phone_sync::download(&account,transport,cancel,
                                        |message|{let _=tx.send(GuiEvent::PhoneSyncStatus{running:true,message});ctx.request_repaint();},
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
                                match tokio::task::spawn_blocking(move || crate::store::MessageStore::open(s_pref.history_path())?.conversation_preferences()).await {
                                    Ok(Ok(values)) => { let _ = gui_tx.send(GuiEvent::ConversationPreferences {owner,values}); }
                                    _ => { let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể đọc tùy chọn hội thoại".into())); }
                                }
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
                                    persist_received(&s, &list).await;
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
                                                if more == 0 && !has_reset {
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
                                        temp_threads.retain(|t| !t.messages.is_empty());
                                        if !temp_threads.is_empty() {
                                            crate::chat::resolve_thread_names(
                                                &s,
                                                &mut temp_threads,
                                            )
                                            .await;
                                            persist_received(&s, &temp_threads).await;
                                            if deliver_thread_updates() {
                                                let _ = gui_tx_clone.send(GuiEvent::ThreadsUpdated {threads:temp_threads,source:ThreadUpdateSource::Live});
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
                                        persist_received(&s, std::slice::from_ref(&t)).await;
                                        let _ = gui_tx_clone.send(GuiEvent::ThreadsUpdated {threads:vec![t],source:ThreadUpdateSource::History});
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Đã đọc {cnt} tin từ lịch sử cục bộ và tin gần đây; lịch sử server theo dõi trong Đồng bộ"
                                        )));
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                        }
                        WorkerCmd::SendMessage {
                            to_id,
                            text,
                            is_group,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let client_id = crate::chat::next_client_message_id();
                                if let Some(found_url) = crate::chat::extract_first_url(&text) {
                                    let meta = crate::chat::parse_link_metadata(&s, &found_url)
                                        .await
                                        .unwrap_or_else(|_| crate::chat::LinkMetadata {
                                            url: found_url.clone(),
                                            title: found_url.clone(),
                                            desc: String::new(),
                                            thumb: String::new(),
                                            src: "Liên kết".into(),
                                        });
                                    let _ =
                                        gui_tx_clone.send(GuiEvent::LinkPreviewReady(meta.clone()));
                                    match crate::chat::send_link_message_with_client_id(
                                        &s, &to_id, &text, &meta, is_group, client_id)
                                    .await
                                    {
                                        Ok(resp) => {
                                            let msg_id = extract_msg_id(&resp);
                                            let now = crate::ui::clock();
                                            let final_id = if msg_id.is_empty() {
                                                format!("out_{client_id}")
                                            } else {
                                                msg_id
                                            };
                                            let msg = ChatMessage {
                                                id: final_id,
                                                client_message_id: client_id.to_string(),
                                                sender: "Tôi".into(),
                                                text,
                                                time: now,
                                                is_outgoing: true,
                                                ts: (crate::login::unix_now() * 1000),
                                                link_url: Some(meta.url),
                                                link_title: if meta.title.is_empty() {
                                                    None
                                                } else {
                                                    Some(meta.title)
                                                },
                                                link_desc: if meta.desc.is_empty() {
                                                    None
                                                } else {
                                                    Some(meta.desc)
                                                },
                                                link_thumb: if meta.thumb.is_empty() {
                                                    None
                                                } else {
                                                    Some(meta.thumb)
                                                },
                                                link_src: if meta.src.is_empty() {
                                                    None
                                                } else {
                                                    Some(meta.src)
                                                },
                                                ..Default::default()
                                            };
                                            let _ = gui_tx_clone.send(GuiEvent::AttachmentSent {
                                                thread_id: to_id,
                                                msg,
                                            });
                                            ctx_clone.request_repaint();
                                            return;
                                        }
                                        Err(e) => {
                                            let err_s = e.to_string();
                                            crate::logger::log_error("gui::send_link", &err_s);
                                            let _ = gui_tx_clone.send(GuiEvent::SendFailed {
                                                thread_id: to_id,
                                                text,
                                                error: err_s,
                                            });
                                            ctx_clone.request_repaint();
                                            return;
                                        }
                                    }
                                }

                                let res = if is_group {
                                    crate::chat::send_group_message_with_client_id(&s, &to_id, &text, client_id).await
                                } else {
                                    crate::chat::send_message_with_client_id(&s, &to_id, &text, client_id).await
                                };
                                match res {
                                    Ok(resp) => {
                                        let msg_id = extract_msg_id(&resp);
                                        let _ = gui_tx_clone.send(GuiEvent::MessageSent {
                                            client_message_id: client_id.to_string(),
                                            thread_id: to_id,
                                            text,
                                            msg_id,
                                        });
                                        ctx_clone.request_repaint();
                                    }
                                    Err(e) => {
                                        let err_s = e.to_string();
                                        crate::logger::log_error("gui::send_message", &err_s);
                                        let _ = gui_tx_clone.send(GuiEvent::SendFailed {
                                            thread_id: to_id,
                                            text,
                                            error: err_s.clone(),
                                        });
                                        if is_session_expired_error(&err_s) {
                                            if let Err(e) = crate::storage::remove_session() { eprintln!("Cannot remove session: {e}"); }
                                            {
                                                let mut sl = session_clone.lock().await;
                                                sl.logout();
                                            }
                                            let _ = gui_tx_clone.send(GuiEvent::SessionExpired(
                                                "Không thể gửi vì phiên zpw_sek đã hết hạn".into(),
                                            ));
                                        }
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                        }
                        WorkerCmd::SendQuote {
                            to_id,
                            text,
                            quoted,
                            is_group,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let client_id = crate::chat::next_client_message_id();
                                match crate::chat::send_quote_message_with_client_id(
                                    &s, &to_id, &text, &quoted, is_group, client_id)
                                .await
                                {
                                    Ok(resp) => {
                                        let _ = gui_tx_clone.send(GuiEvent::QuoteSent {
                                            client_message_id: client_id.to_string(),
                                            thread_id: to_id,
                                            text,
                                            msg_id: extract_msg_id(&resp),
                                            quoted,
                                        });
                                    }
                                    Err(e) => {
                                        let err_s = e.to_string();
                                        crate::logger::log_error("gui::send_quote", &err_s);
                                        let _ = gui_tx_clone.send(GuiEvent::SendFailed {
                                            thread_id: to_id,
                                            text,
                                            error: err_s,
                                        });
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::Forward {
                            target_id,
                            target_is_group,
                            msg,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let client_id = crate::chat::next_client_message_id();
                                let res = crate::chat::forward_message_with_client_id(
                                    &s,
                                    &target_id,
                                    target_is_group,
                                    &msg, client_id)
                                .await;
                                match res {
                                    Ok(resp) if resp_error_code(&resp).is_none() => {
                                        let msg_id = extract_msg_id(&resp);
                                        if let Some(sid) = msg.sticker_id.filter(|v| *v > 0) {
                                            let _ = gui_tx_clone.send(GuiEvent::StickerSent {
                                                client_message_id: client_id.to_string(),
                                                thread_id: target_id,
                                                msg_id,
                                                sticker_id: sid,
                                                cate_id: msg.sticker_cate_id.unwrap_or(0),
                                                sticker_type: msg.sticker_type.unwrap_or(7),
                                            });
                                        } else {
                                            let _ = gui_tx_clone.send(GuiEvent::MessageSent {
                                                client_message_id: client_id.to_string(),
                                                thread_id: target_id,
                                                text: msg.text.clone(),
                                                msg_id,
                                            });
                                        }
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(
                                            "Đã chuyển tiếp tin nhắn".into(),
                                        ));
                                    }
                                    Ok(resp) => {
                                        let err_s = format!(
                                            "server error {}",
                                            resp_error_code(&resp).unwrap_or(0)
                                        );
                                        crate::logger::log_error("gui::forward", &err_s);
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Chuyển tiếp thất bại: {err_s}"
                                        )));
                                    }
                                    Err(e) => {
                                        crate::logger::log_error("gui::forward", &e.to_string());
                                        let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                            "Chuyển tiếp thất bại: {e}"
                                        )));
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
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
                                        let (tid, mid, ts) =
                                            (to_id.clone(), msg.id.clone(), msg.ts);
                                        let _ = tokio::task::spawn_blocking(move || {
                                            if let Ok(mut store) =
                                                crate::store::MessageStore::open(s.history_path())
                                            {
                                                let _ = store.delete_message(&tid, &mid, ts);
                                            }
                                        })
                                        .await;
                                        let _ = gui_tx_clone.send(GuiEvent::MessageDeleted {
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
                                    Ok(serde_json::json!({}))
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
                                            "Đã thu hồi cục bộ (Máy chủ: {err_s})"
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
                        WorkerCmd::SendSticker {
                            to_id,
                            sticker_id,
                            cate_id,
                            sticker_type,
                            is_group,
                        } => {
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let client_id = crate::chat::next_client_message_id();
                                let res = crate::chat::send_sticker_with_client_id(
                                    &s,
                                    &to_id,
                                    sticker_id,
                                    cate_id,
                                    sticker_type,
                                    is_group, client_id)
                                .await;
                                match res {
                                    Ok(resp) => {
                                        let msg_id = extract_msg_id(&resp);
                                        let _ = gui_tx_clone.send(GuiEvent::StickerSent {
                                            client_message_id: client_id.to_string(),
                                            thread_id: to_id,
                                            msg_id,
                                            sticker_id,
                                            cate_id,
                                            sticker_type,
                                        });
                                        ctx_clone.request_repaint();
                                    }
                                    Err(e) => {
                                        let err_s = e.to_string();
                                        crate::logger::log_error("gui::send_sticker", &err_s);
                                        let _ = gui_tx_clone.send(GuiEvent::SendFailed {
                                            thread_id: to_id,
                                            text: String::new(),
                                            error: err_s.clone(),
                                        });
                                        if is_session_expired_error(&err_s) {
                                            if let Err(e) = crate::storage::remove_session() { eprintln!("Cannot remove session: {e}"); }
                                            {
                                                let mut sl = session_clone.lock().await;
                                                sl.logout();
                                            }
                                            let _ = gui_tx_clone.send(GuiEvent::SessionExpired(
                                                "Không thể gửi nhãn dán vì phiên zpw_sek đã hết hạn"
                                                    .into(),
                                            ));
                                        }
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                        }
                        WorkerCmd::FetchStickerImage { key, url } => {
                            let Some(admission)=image_jobs.try_admit() else {
                                let _=gui_tx.send(GuiEvent::ImageUnavailable {key});egui_ctx.request_repaint();continue;
                            };
                            let s=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let Some(job)=admission.start().await else {return;};
                                let Ok(_permit)=crate::limits::IMAGE_JOBS.acquire().await else {return;};
                                match fetch_gui_image(&s,crate::storage::root(),&key,&url).await {
                                    Ok(image)=>{let _=tx.send(GuiEvent::PreviewImageReady {key,image,permit:job});},
                                    Err(_)=>{let _=tx.send(GuiEvent::ImageUnavailable {key});}
                                }
                                ctx.request_repaint();
                            });
                        }
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
                                        );
                                        let _ = gui_tx_clone.send(GuiEvent::StickerSearchResults {
                                            keyword,
                                            items,
                                        });
                                        ctx_clone.request_repaint();
                                        prefetch_missing_sticker_thumbnails(
                                            &s.http,
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
                            let session_clone = session.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = session_clone.lock().await.clone();
                                let items = crate::chat::search_stickers_by_keyword(&s, &keyword)
                                    .await
                                    .unwrap_or_default();
                                let missing =
                                    preload_disk_sticker_thumbnails(&items, &gui_tx_clone, 16);
                                let _ =
                                    gui_tx_clone.send(GuiEvent::ComposerStickerSuggestions {
                                        keyword,
                                        items,
                                    });
                                ctx_clone.request_repaint();
                                prefetch_missing_sticker_thumbnails(
                                    &s.http,
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
                                let picked_path = if let Some(p) = preset_path {
                                    Some(p)
                                } else {
                                    tokio::task::spawn_blocking(move || {
                                        pick_native_file_dialog(images_only)
                                    })
                                    .await
                                    .ok()
                                    .flatten()
                                };

                                let Some(path) = picked_path else {
                                    return;
                                };
                                let fname = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("tập tin")
                                    .to_string();
                                let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!(
                                    "⏳ Đang tải lên và gửi '{fname}'…"
                                )));
                                ctx_clone.request_repaint();

                                let s = session_clone.lock().await.clone();
                                match crate::chat::send_attachment(
                                    &s, &to_id, is_group, &path, &caption,
                                )
                                .await
                                {
                                    Ok((_resp, cm)) => {
                                        let _ = gui_tx_clone.send(GuiEvent::AttachmentSent {
                                            thread_id: to_id,
                                            msg: cm,
                                        });
                                    }
                                    Err(e) => {
                                        let err_s = e.to_string();
                                        crate::logger::log_error("gui::send_attachment", &err_s);
                                        let _ = gui_tx_clone.send(GuiEvent::SendFailed {
                                            thread_id: to_id,
                                            text: caption,
                                            error: format!("Lỗi gửi '{fname}': {err_s}"),
                                        });
                                    }
                                }
                                ctx_clone.request_repaint();
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
                        WorkerCmd::AnswerCall { accept } => {
                            call_bridge.answer_call(accept);
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::SetCallMediaState {
                            is_on_mic,
                            is_on_cam,
                        } => {
                            call_bridge.set_media_state(is_on_mic, is_on_cam);
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::EndCall => {
                            call_bridge.end_call();
                            let _ = gui_tx.send(GuiEvent::CallStateChanged("free".into()));
                            let _ = gui_tx.send(GuiEvent::StatusUpdate(
                                "Đã kết thúc cuộc gọi".into(),
                            ));
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
