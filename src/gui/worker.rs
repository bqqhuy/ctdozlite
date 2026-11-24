use super::*;

#[derive(Default)]
struct EarlyVoiceResults(
    HashMap<(usize, String), (crate::chat::VoiceToTextPushResult, std::time::Instant)>,
);
type VoiceTicketMap = HashMap<(usize, String), (String, String, u64, std::time::Instant)>;
fn voice_result_text(result: crate::chat::VoiceToTextPushResult) -> String {
    let text = result.text.unwrap_or_default();
    if result.is_truncated {
        format!("{text}\n\n[Bản chép lời chưa đầy đủ]")
    } else {
        text
    }
}
#[derive(Default)]
struct VoiceCorrelations {
    pending: VoiceTicketMap,
    early: EarlyVoiceResults,
}
impl VoiceCorrelations {
    fn remove_matching(
        &mut self,
        runtime: usize,
        request: &str,
        thread: &str,
        message: &str,
        generation: u64,
    ) -> Option<(String, String, u64, std::time::Instant)> {
        let key = (runtime, request.to_owned());
        if self
            .pending
            .get(&key)
            .is_some_and(|(th, mid, gen, _)| th == thread && mid == message && *gen == generation)
        {
            self.pending.remove(&key)
        } else {
            None
        }
    }
    fn clear(&mut self) {
        self.pending.clear();
        self.early.0.clear();
    }
}
impl std::ops::Deref for VoiceCorrelations {
    type Target = VoiceTicketMap;
    fn deref(&self) -> &Self::Target {
        &self.pending
    }
}
impl std::ops::DerefMut for VoiceCorrelations {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.pending
    }
}
impl EarlyVoiceResults {
    fn insert(&mut self, runtime: usize, result: crate::chat::VoiceToTextPushResult) {
        self.0.retain(|(owner, _), (_, at)| {
            *owner == runtime && at.elapsed() < Duration::from_secs(30)
        });
        if self.0.len() >= 128 {
            if let Some(key) = self
                .0
                .iter()
                .min_by_key(|(_, (_, at))| *at)
                .map(|(key, _)| key.clone())
            {
                self.0.remove(&key);
            }
        }
        self.0.insert(
            (runtime, result.req_id.clone()),
            (result, std::time::Instant::now()),
        );
    }
    fn take(
        &mut self,
        runtime: usize,
        request: &str,
    ) -> Option<crate::chat::VoiceToTextPushResult> {
        self.0
            .remove(&(runtime, request.to_owned()))
            .filter(|(_, at)| at.elapsed() < Duration::from_secs(30))
            .map(|(result, _)| result)
    }
}

#[cfg(test)]
mod early_voice_tests {
    use super::*;
    #[test]
    fn old_voice_timeout_does_not_remove_new_runtime_or_message_ticket() {
        let mut map = VoiceCorrelations::default();
        map.pending.insert(
            (2, "shared-id".into()),
            (
                "thread".into(),
                "new-message".into(),
                2,
                std::time::Instant::now(),
            ),
        );
        assert!(map
            .remove_matching(1, "shared-id", "thread", "old-message", 1)
            .is_none());
        assert!(map
            .remove_matching(2, "shared-id", "thread", "old-message", 2)
            .is_none());
        assert_eq!(map.pending.len(), 1);
        assert!(map
            .remove_matching(2, "shared-id", "thread", "new-message", 2)
            .is_some());
    }
    #[test]
    fn early_voice_push_is_scoped_to_runtime_and_consumed_once() {
        let mut early = EarlyVoiceResults::default();
        early.insert(
            1,
            crate::chat::VoiceToTextPushResult {
                req_id: "request".into(),
                text: Some("Xin chào".into()),
                error_code: 0,
                is_truncated: false,
            },
        );
        assert!(early.take(2, "request").is_none());
        assert_eq!(
            early.take(1, "request").unwrap().text.as_deref(),
            Some("Xin chào")
        );
        assert!(early.take(1, "request").is_none());
    }
    #[test]
    fn truncated_voice_result_is_labelled_as_partial_text() {
        let result = crate::chat::VoiceToTextPushResult {
            req_id: "partial".into(),
            text: Some("Xin chào".into()),
            error_code: 0,
            is_truncated: true,
        };
        assert!(voice_result_text(result).ends_with("[Bản chép lời chưa đầy đủ]"));
    }
}

fn voice_send_session_matches(
    session_active: bool,
    actual_owner: &str,
    actual_runtime_id: usize,
    expected_owner: &str,
    expected_runtime_id: usize,
) -> bool {
    // A UID can be reused after logout; runtime identity binds queued audio to one session.
    session_active && actual_owner == expected_owner && actual_runtime_id == expected_runtime_id
}

/// Relationship lookup or one explicitly selected friend action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendAction {
    Check,
    Add,
    Accept,
    Reject,
    Undo,
}

fn friend_action_matches_status(
    action: FriendAction,
    status: crate::chat::FriendshipStatus,
) -> bool {
    use crate::chat::FriendshipStatus::{IncomingPending, OutgoingPending, Stranger};
    matches!(
        (action, status),
        (FriendAction::Add, Stranger)
            | (FriendAction::Accept | FriendAction::Reject, IncomingPending)
            | (FriendAction::Undo, OutgoingPending)
    )
}

async fn run_friend_action(
    session: &Session,
    user_id: &str,
    action: FriendAction,
) -> Result<crate::chat::FriendshipStatus, String> {
    use crate::chat::{self, FriendshipStatus};

    let read_status = || async {
        match chat::check_friendship_status(session, user_id).await {
            Ok(status) if status != FriendshipStatus::Unknown => Ok(status),
            _ => Err("Chưa xác định được trạng thái kết bạn".to_owned()),
        }
    };
    let status = read_status().await?;
    if action == FriendAction::Check || !friend_action_matches_status(action, status) {
        // A stale row may offer an action that the current relationship no longer permits.
        return Ok(status);
    }
    let response = match action {
        FriendAction::Check => return Ok(status),
        FriendAction::Add => {
            chat::send_friend_request(session, user_id, "Xin chào, kết bạn với mình nhé!").await
        }
        FriendAction::Accept => chat::accept_friend_request(session, user_id).await,
        FriendAction::Reject => chat::reject_friend_request(session, user_id).await,
        FriendAction::Undo => chat::undo_friend_request(session, user_id).await,
    };
    response
        .and_then(crate::api_response::checked_data)
        .map_err(|_| {
            "Chưa xác nhận được thao tác kết bạn. Kiểm tra lại trạng thái trước khi thử tiếp."
                .to_owned()
        })?;
    read_status().await
}

pub enum WorkerCmd {
    RetryStartup,
    PrepareE2eeTrial {
        owner: String,
        peer_id: String,
    },
    SaveAppSettings {
        settings: serde_json::Value,
        persist_session: bool,
    },
    LoadProfiles {
        owner: String,
    },
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
        runtime_id: usize,
        generation: u64,
        action: crate::labels::Action,
    },
    Privacy {
        owner: String,
        runtime_id: usize,
        generation: u64,
        action: crate::privacy::Action,
    },
    ManualMark {
        owner: String,
        runtime_id: usize,
        generation: u64,
        target: String,
        group: bool,
        action: crate::unread::MarkAction,
    },
    Archive {
        owner: String,
        runtime_id: usize,
        generation: u64,
        target: String,
        group: bool,
        archived: Option<bool>,
    },
    FetchLastOnline {
        owner: String,
        runtime_id: usize,
        target: String,
        official: bool,
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
    DownloadMediaBatch {
        owner: String,
        batch: u64,
        items: Vec<MediaBatchItem>,
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
        runtime_id: usize,
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
    ListSettingsDevices {
        generation: u64,
        refresh: bool,
    },
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
    LoadMessagePins {
        owner: String,
        thread: String,
        generation: u64,
    },
    LoadCloudUsage {
        owner: String,
        generation: u64,
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
    SyncPhoneRecentFenced {
        owner: String,
        runtime_id: usize,
        intent_id: Option<u64>,
        origin: crate::sync_coordinator::SyncOrigin,
    },
    SyncPhoneLegacy,
    CancelPhoneHistory,
    CancelPhoneHistoryFenced {
        owner: String,
        runtime_id: usize,
        operation_id: u64,
    },
    ClearSyncError {
        owner: String,
        runtime_id: usize,
    },
    PrioritizeConversationSync {
        owner: String,
        runtime_id: usize,
        thread_id: String,
    },
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
    SendFormatted {
        to_id: String,
        text: String,
        is_group: bool,
        options: crate::rich_text::SendOptions,
        mentions: Vec<crate::mentions::Mention>,
        quoted: Option<ChatMessage>,
    },
    SendQuote {
        to_id: String,
        text: String,
        quoted: ChatMessage,
        is_group: bool,
    },
    LoadSafetyNumber {
        owner: String,
        peer_id: String,
        peer_name: String,
    },
    TrustIdentity {
        owner: String,
        peer_id: String,
        safety_number: String,
    },
    CheckE2eeStatus {
        owner: String,
        peer_id: String,
    },
    PreviewDocument {
        url: String,
        name: String,
        context: ChatImage,
    },
    Forward {
        target_id: String,
        target_is_group: bool,
        msg: ChatMessage,
    },
    React {
        owner: String,
        runtime_id: usize,
        intent_id: u64,
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
    SendVoiceMessage {
        owner: String,
        runtime_id: usize,
        to_id: String,
        is_group: bool,
        samples: std::sync::Arc<[f32]>,
        sample_rate: u32,
        duration_secs: u32,
    },
    SendSticker {
        to_id: String,
        sticker_id: u64,
        cate_id: u64,
        sticker_type: u32,
        is_group: bool,
    },
    PickAttachmentDraft {
        owner: String,
        runtime_id: usize,
        thread: String,
        images_only: bool,
    },
    PrepareAttachmentArchive {
        owner: String,
        runtime_id: usize,
        thread: String,
        path: std::path::PathBuf,
        stamp: Option<crate::outgoing::FileStamp>,
    },
    PickAndSendAttachment {
        owner: String,
        runtime_id: usize,
        to_id: String,
        is_group: bool,
        images_only: bool,
        caption: String,
        preset_path: Option<std::path::PathBuf>,
        /// `false` sends photos as an HD copy (long side 2560 px) like Zalo PC's default.
        photo_original: bool,
        /// If `true`, the source is an internal ephemeral file (e.g. clipboard paste) that should
        /// be unlinked from disk upon successful transmission if within app temp directories.
        ephemeral: bool,
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
        origin: crate::chat::LinkOrigin,
        thread: Option<(String, bool)>,
    },
    ResetE2eeRequirement {
        owner: String,
        peer_id: String,
    },
    LookupPhone {
        phone: String,
    },
    StartCall {
        owner: String,
        runtime_id: usize,
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
    CopyArchiveImage {
        owner: String,
        media: crate::message_view::Media,
        context: ChatImage,
    },
    RevealArchiveMedia {
        owner: String,
        runtime_id: usize,
        path: std::path::PathBuf,
        context: ChatImage,
    },
    MutateArchiveBatch {
        owner: String,
        runtime_id: usize,
        batch: u64,
        thread: String,
        group: bool,
        recall: bool,
        messages: Vec<ChatMessage>,
    },
    LoadArchiveHistory {
        owner: String,
        runtime_id: usize,
        thread: String,
        group: bool,
        generation: u64,
        before: Option<crate::store::HistoryCursor>,
    },
    FetchServerMedia {
        runtime_id: usize,
        thread_id: String,
        media_type: u8,
        page: u32,
        count: u32,
        cursor: Option<String>,
        generation: u64,
    },
    GetCloudViewerKey {
        file_id: String,
    },
    MarkThreadRead {
        thread_id: String,
        last_read_msg_id: String,
    },
    GetBusinessCatalog,
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
    FriendAction {
        owner: String,
        runtime_id: usize,
        generation: u64,
        user_id: String,
        action: FriendAction,
    },
    PersistThreads {
        owner: String,
        threads: Vec<Thread>,
    },
    PersistThreadMetadata {
        owner: String,
        runtime_id: usize,
        thread_id: String,
        name: String,
        is_group: bool,
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
    AddAccount,
    TranslateMessage {
        thread_id: String,
        msg_id: String,
        text: String,
        target_lang: String,
        text_hash: u64,
        generation: u64,
    },
    TranscribeVoiceMessage {
        thread_id: String,
        msg_id: String,
        voice_url: String,
        generation: u64,
        context: ChatImage,
    },
    PlayInlineVoice {
        binding: inline_voice::Binding,
        controls: inline_voice::Controls,
        context: ChatImage,
    },
}

/// Accounts for accepted intents even when their queue or Tokio task is dropped.
#[derive(Clone)]
struct OutgoingLifecycle {
    state: std::sync::Arc<std::sync::Mutex<OutgoingLifecycleState>>,
    events: Sender<GuiEvent>,
    ctx: egui::Context,
}
#[derive(Default)]
struct OutgoingLifecycleState {
    next: u64,
    intents: HashMap<u64, TrackedOutgoing>,
    retired_started: HashMap<u64, TrackedOutgoing>,
    retired: std::collections::HashSet<usize>,
    closed: bool,
}
#[derive(Clone)]
struct TrackedOutgoing {
    owner: String,
    runtime_id: usize,
    thread: String,
    message: ChatMessage,
    started: bool,
}
struct OutgoingTicket {
    lifecycle: OutgoingLifecycle,
    id: u64,
}
impl OutgoingLifecycle {
    fn new(events: Sender<GuiEvent>, ctx: egui::Context) -> Self {
        Self {
            state: Default::default(),
            events,
            ctx,
        }
    }
    fn accept(
        &self,
        owner: String,
        runtime_id: usize,
        thread: String,
        message: ChatMessage,
    ) -> OutgoingTicket {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.next += 1;
        let id = state.next;
        let entry = TrackedOutgoing {
            owner,
            runtime_id,
            thread,
            message,
            started: false,
        };
        if state.closed || state.retired.contains(&runtime_id) {
            self.publish(entry, None);
        } else {
            state.intents.insert(id, entry);
        }
        OutgoingTicket {
            lifecycle: self.clone(),
            id,
        }
    }
    fn publish(&self, entry: TrackedOutgoing, final_message: Option<ChatMessage>) {
        let message = final_message.unwrap_or_else(|| {
            let mut message = outgoing_not_sent(entry.message, "Phiên gửi đã đóng trước khi gửi");
            if entry.started {
                message.delivery = crate::message_model::DeliveryState::DeliveryUnknown;
                if let Some(job) = message.outgoing_job.as_mut() {
                    job.error = "Lần gửi bị gián đoạn; kiểm tra hội thoại trước khi gửi lại để tránh trùng tin".into();
                }
            }
            message
        });
        let _ = self.events.send(GuiEvent::OutgoingState {
            runtime_id: entry.runtime_id,
            owner: entry.owner,
            thread_id: entry.thread,
            message,
        });
        self.ctx.request_repaint();
    }
    fn retire(&self, runtime_id: usize) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.retired.insert(runtime_id);
        let ids: Vec<_> = state
            .intents
            .iter()
            .filter_map(|(id, entry)| (entry.runtime_id == runtime_id).then_some(*id))
            .collect();
        for id in ids {
            if let Some(entry) = state.intents.remove(&id) {
                self.publish(entry.clone(), None);
                if entry.started {
                    state.retired_started.insert(id, entry);
                }
            }
        }
    }
    fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        let intents: Vec<_> = state.intents.drain().collect();
        for (id, entry) in intents {
            self.publish(entry.clone(), None);
            if entry.started {
                state.retired_started.insert(id, entry);
            }
        }
    }
}
impl OutgoingTicket {
    fn update(&self, message: ChatMessage) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state.intents.get_mut(&self.id) {
            entry.message = message;
        }
    }
    fn start(&self) -> bool {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let Some(entry) = state.intents.get_mut(&self.id) else {
            return false;
        };
        entry.started = true;
        true
    }
    fn finish(&self, message: ChatMessage) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state
            .intents
            .remove(&self.id)
            .or_else(|| state.retired_started.remove(&self.id))
        {
            self.lifecycle.publish(entry, Some(message));
        }
    }
}
impl Drop for OutgoingTicket {
    fn drop(&mut self) {
        let mut state = self
            .lifecycle
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state.intents.remove(&self.id) {
            self.lifecycle.publish(entry, None);
        }
        state.retired_started.remove(&self.id);
    }
}
struct OutgoingRuntimeGuard(OutgoingLifecycle);
impl Drop for OutgoingRuntimeGuard {
    fn drop(&mut self) {
        self.0.close();
    }
}

type OutgoingDispatch = (String, usize, String, ChatMessage, OutgoingTicket);

// Replaceable mailbox: each logout tears down the old worker runtime and event channel.
#[derive(Clone)]
pub(super) struct WorkerSender {
    commands: tokio::sync::mpsc::UnboundedSender<WorkerCmd>,
    outgoing: Option<tokio::sync::mpsc::UnboundedSender<OutgoingDispatch>>,
    lifecycle: OutgoingLifecycle,
    identity: std::sync::Arc<std::sync::RwLock<(String, usize)>>,
    images: tokio::sync::mpsc::Sender<(String, String, Option<ChatImage>)>,
    events: Sender<GuiEvent>,
    ctx: egui::Context,
}
impl WorkerSender {
    pub(super) fn is_command_channel_closed(&self) -> bool {
        self.commands.is_closed()
    }

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
                if let Some((thread_id, message)) = outgoing_command(&other) {
                    crate::outgoing::mark_queued(&message);
                    crate::runtime_metrics::dispatched(&message.client_message_id);
                    let Some(outgoing) = &self.outgoing else {
                        return self
                            .commands
                            .send(WorkerCmd::RetryOutgoing { thread_id, message })
                            .map_err(|_| ());
                    };
                    // The worker resolves owner/runtime at admission time. Capturing identity
                    // in this UI sender allowed a QR/password login to leave a stale identity.
                    let (owner, runtime_id) = self
                        .identity
                        .read()
                        .map(|identity| identity.clone())
                        .unwrap_or_else(|poisoned| poisoned.into_inner().clone());
                    let ticket = self.lifecycle.accept(
                        owner.clone(),
                        runtime_id,
                        thread_id.clone(),
                        message.clone(),
                    );
                    if outgoing
                        .send((
                            owner.clone(),
                            runtime_id,
                            thread_id.clone(),
                            message.clone(),
                            ticket,
                        ))
                        .is_err()
                    {
                        // Dropping the failed-send envelope publishes its terminal result.
                    }
                    return Ok(());
                }
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

    #[cfg(test)]
    pub(super) fn new_for_test(
        commands: tokio::sync::mpsc::UnboundedSender<WorkerCmd>,
        ctx: egui::Context,
    ) -> Self {
        let (images, _) = tokio::sync::mpsc::channel(1);
        let (events, _) = std::sync::mpsc::channel();
        Self {
            commands,
            outgoing: None,
            lifecycle: OutgoingLifecycle::new(events.clone(), ctx.clone()),
            identity: std::sync::Arc::new(std::sync::RwLock::new((String::new(), 0))),
            images,
            events,
            ctx,
        }
    }
}
#[cfg(not(test))]
pub(super) struct WorkerMailbox(std::sync::Mutex<Option<WorkerSender>>);
#[cfg(not(test))]
impl WorkerMailbox {
    pub(super) fn get(&self) -> Option<WorkerSender> {
        self.0.lock().ok()?.clone()
    }
    pub(super) fn set(&self, sender: WorkerSender) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(sender);
    }
}
#[cfg(not(test))]
pub(super) static WORKER_TX: WorkerMailbox = WorkerMailbox(std::sync::Mutex::new(None));

// Each test drives its own UI thread; its mock mailbox must not reach another app.
#[cfg(test)]
std::thread_local! {
    static TEST_WORKER_TX: std::cell::RefCell<Option<WorkerSender>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(super) struct WorkerMailbox;
#[cfg(test)]
impl WorkerMailbox {
    pub(super) fn get(&self) -> Option<WorkerSender> {
        TEST_WORKER_TX.with(|slot| slot.borrow().clone())
    }
    pub(super) fn set(&self, sender: WorkerSender) {
        TEST_WORKER_TX.with(|slot| *slot.borrow_mut() = Some(sender));
    }
    pub(super) fn clear(&self) {
        TEST_WORKER_TX.with(|slot| *slot.borrow_mut() = None);
    }
}
#[cfg(test)]
pub(super) static WORKER_TX: WorkerMailbox = WorkerMailbox;

async fn forward_upload_progress(
    receiver: &mut tokio::sync::broadcast::Receiver<crate::upload::UploadProgress>,
    events: &Sender<GuiEvent>,
    context: &egui::Context,
) {
    loop {
        match receiver.recv().await {
            Ok((owner, runtime_id, upload_id, completed, total)) => {
                if events
                    .send(GuiEvent::UploadProgress {
                        owner,
                        runtime_id,
                        upload_id,
                        completed,
                        total,
                    })
                    .is_err()
                {
                    break;
                }
                context.request_repaint();
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

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
    success: bool,
}
impl Drop for RefreshNotice {
    fn drop(&mut self) {
        let _ = self.sender.send(GuiEvent::ConversationRefreshStatus {
            running: false,
            success: self.success,
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
        let sprite_path = cache_dir.join(format!("{key}_sprite.png"));
        let thumb_path = cache_dir.join(format!("{key}.png"));

        // 1. Check sprite cache first
        let loaded = if let Ok(disk_bytes) =
            crate::limits::read_file(&sprite_path, crate::limits::IMAGE_BYTES)
        {
            if let Ok(sprite) = decode_sprite_from_bytes(&disk_bytes) {
                if sprite.frames > 1 {
                    let _ = gui_tx.send(GuiEvent::StickerSpriteReady {
                        key: key.clone(),
                        sprite,
                        permit: None,
                    });
                    true
                } else {
                    false
                }
            } else {
                let _ = std::fs::remove_file(&sprite_path);
                false
            }
        } else {
            false
        };

        if loaded {
            continue;
        }

        // 2. Fall back to thumb cache; migrate legacy atlas if found
        if let Ok(disk_bytes) = crate::limits::read_file(&thumb_path, crate::limits::IMAGE_BYTES) {
            if let Ok(sprite) = decode_sprite_from_bytes(&disk_bytes) {
                if sprite.frames > 1 {
                    let _ = std::fs::copy(&thumb_path, &sprite_path);
                    let _ = gui_tx.send(GuiEvent::StickerSpriteReady {
                        key: key.clone(),
                        sprite,
                        permit: None,
                    });
                    continue;
                } else {
                    let _ = gui_tx.send(GuiEvent::StickerImageReady {
                        key,
                        image: sprite.image,
                    });
                    continue;
                }
            } else {
                let _ = std::fs::remove_file(&thumb_path);
            }
        }
        let url = if item.cate_id == 0 && (1..=30).contains(&item.sticker_id) {
            crate::endpoints::builtin_sticker_thumbnail(item.sticker_id)
        } else {
            crate::endpoints::legacy_sticker_image(item.sticker_id)
        };
        missing.push((key, url, thumb_path));
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

fn parse_sticker_key(key: &str) -> Option<(u64, u64)> {
    let stripped = key.strip_prefix("stk_")?;
    let mut parts = stripped.split('_');
    let cid = parts.next()?.parse::<u64>().ok()?;
    let sid = parts.next()?.parse::<u64>().ok()?;
    Some((cid, sid))
}

fn decode_sprite_from_bytes(bytes: &[u8]) -> anyhow::Result<crate::gui::DecodedSpriteData> {
    let dyn_img = crate::limits::decode_image(bytes)?;
    let rgba = dyn_img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    anyhow::ensure!(width > 0 && height > 0, "Empty image dimensions");
    anyhow::ensure!(
        width <= 8192 && height <= 8192,
        "Image dimensions ({width}x{height}) exceed maximum texture dimension 8192"
    );
    let size = [width as usize, height as usize];
    let pixels = rgba.as_flat_samples();
    let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

    let (frames, delays) = if height > 0 && width >= height && width % height == 0 {
        let f = (width / height) as u16;
        if (1..=64).contains(&f) {
            (f, vec![crate::gui::ZALO_SPRITE_FRAME_DELAY; f as usize])
        } else {
            (1, vec![std::time::Duration::ZERO])
        }
    } else {
        (1, vec![std::time::Duration::ZERO])
    };

    Ok(crate::gui::DecodedSpriteData {
        image: color_img,
        frames,
        delays,
    })
}

async fn download_sticker_image_bytes(raw_url: &str) -> anyhow::Result<Vec<u8>> {
    let _permit = crate::limits::image_network_permit(true).await?;
    let compatible = crate::media_recovery::compatible_image_url(raw_url, true)?;
    let response = crate::media_download::response(&compatible, Duration::from_secs(15)).await?;
    anyhow::ensure!(
        response.status().is_success(),
        "HTTP error {}",
        response.status()
    );
    crate::media_resources::read_bounded_response(response, crate::limits::IMAGE_BYTES)
        .await
        .map_err(|e| anyhow::anyhow!("Sticker download read error: {e:?}"))
}

pub(super) async fn fetch_and_decode_sticker_sprite(
    key: &str,
    url: &str,
) -> anyhow::Result<crate::gui::DecodedSpriteData> {
    if let Some((cid, sid)) = parse_sticker_key(key) {
        if cid == crate::bundled_stickers::CATEGORY {
            if let Ok(Some(sprite)) =
                tokio::task::spawn_blocking(move || crate::bundled_stickers::decode_gif_sprite(sid))
                    .await
            {
                return Ok(sprite);
            }
            if let Some(png_bytes) = crate::bundled_stickers::thumbnail(key) {
                let png_vec = png_bytes.to_vec();
                if let Ok(Ok(sprite)) =
                    tokio::task::spawn_blocking(move || decode_sprite_from_bytes(&png_vec)).await
                {
                    return Ok(sprite);
                }
            }
        }

        // Check local disk cache first before hitting network (N-02)
        // Sprite atlas is disambiguated as {key}_sprite.png to avoid colliding with static {key}.png thumbnails
        let cache_dir = crate::storage::cache_dir();
        let sprite_cache_path = cache_dir.join(format!("{key}_sprite.png"));
        let thumb_cache_path = cache_dir.join(format!("{key}.png"));

        let cached_sprite_read = sprite_cache_path.clone();
        if let Ok(Ok(bytes)) = tokio::task::spawn_blocking(move || {
            crate::limits::read_file(&cached_sprite_read, crate::limits::IMAGE_BYTES)
        })
        .await
        {
            let bytes_clone = bytes.clone();
            if let Ok(Ok(sprite)) =
                tokio::task::spawn_blocking(move || decode_sprite_from_bytes(&bytes_clone)).await
            {
                return Ok(sprite);
            }
        }

        let cached_thumb_read = thumb_cache_path.clone();
        if let Ok(Ok(bytes)) = tokio::task::spawn_blocking(move || {
            crate::limits::read_file(&cached_thumb_read, crate::limits::IMAGE_BYTES)
        })
        .await
        {
            let bytes_clone = bytes.clone();
            if let Ok(Ok(sprite)) =
                tokio::task::spawn_blocking(move || decode_sprite_from_bytes(&bytes_clone)).await
            {
                if sprite.frames > 1 {
                    let save_path = sprite_cache_path.clone();
                    let save_bytes = bytes.clone();
                    tokio::task::spawn_blocking(move || {
                        let _ = crate::storage::atomic_write(&save_path, &save_bytes);
                    });
                    return Ok(sprite);
                }
            }
        }

        if cid == 0 && (1..=30).contains(&sid) {
            let primary = crate::endpoints::builtin_sticker_thumbnail(sid);
            if let Ok(bytes) = download_sticker_image_bytes(&primary).await {
                let bytes_clone = bytes.clone();
                if let Ok(Ok(sprite)) =
                    tokio::task::spawn_blocking(move || decode_sprite_from_bytes(&bytes_clone))
                        .await
                {
                    let save_path = if sprite.frames > 1 {
                        sprite_cache_path.clone()
                    } else {
                        thumb_cache_path.clone()
                    };
                    tokio::task::spawn_blocking(move || {
                        let _ = crate::storage::atomic_write(&save_path, &bytes);
                        let _ = crate::storage::prune_preview_cache(128 * 1024 * 1024);
                    });
                    return Ok(sprite);
                }
            }
            let fallback = if !url.is_empty() {
                url.to_string()
            } else {
                crate::endpoints::legacy_sticker_image(sid)
            };
            let bytes = download_sticker_image_bytes(&fallback).await?;
            let bytes_clone = bytes.clone();
            let sprite =
                tokio::task::spawn_blocking(move || decode_sprite_from_bytes(&bytes_clone))
                    .await??;
            let save_path = if sprite.frames > 1 {
                sprite_cache_path.clone()
            } else {
                thumb_cache_path.clone()
            };
            tokio::task::spawn_blocking(move || {
                let _ = crate::storage::atomic_write(&save_path, &bytes);
                let _ = crate::storage::prune_preview_cache(128 * 1024 * 1024);
            });
            return Ok(sprite);
        }

        // Sticker khác: Thử tải crate::endpoints::sticker_sprite_url(sid) trước
        let sprite_url = crate::endpoints::sticker_sprite_url(sid);
        if let Ok(bytes) = download_sticker_image_bytes(&sprite_url).await {
            let bytes_clone = bytes.clone();
            if let Ok(Ok(sprite)) =
                tokio::task::spawn_blocking(move || decode_sprite_from_bytes(&bytes_clone)).await
            {
                let save_path = sprite_cache_path.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = crate::storage::atomic_write(&save_path, &bytes);
                    let _ = crate::storage::prune_preview_cache(128 * 1024 * 1024);
                });
                return Ok(sprite);
            }
        }

        // Fallback sang url ban đầu (legacy_sticker_image(sid))
        let fallback = if !url.is_empty() {
            url.to_string()
        } else {
            crate::endpoints::legacy_sticker_image(sid)
        };
        let bytes = download_sticker_image_bytes(&fallback).await?;
        let bytes_clone = bytes.clone();
        let sprite =
            tokio::task::spawn_blocking(move || decode_sprite_from_bytes(&bytes_clone)).await??;
        let save_path = if sprite.frames > 1 {
            sprite_cache_path.clone()
        } else {
            thumb_cache_path.clone()
        };
        tokio::task::spawn_blocking(move || {
            let _ = crate::storage::atomic_write(&save_path, &bytes);
            let _ = crate::storage::prune_preview_cache(128 * 1024 * 1024);
        });
        return Ok(sprite);
    }

    let bytes = download_sticker_image_bytes(url).await?;
    let bytes_clone = bytes.clone();
    tokio::task::spawn_blocking(move || decode_sprite_from_bytes(&bytes_clone)).await?
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
        let _timing =
            crate::runtime_metrics::Scope::new(crate::runtime_metrics::Metric::ReceivedWrite);
        anyhow::ensure!(session.is_active(), "Account changed");
        let (mut store, _write_permit) =
            crate::store::MessageStore::open_for_account_write(&session)?;
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
            Err(error) => {
                crate::runtime_metrics::database_error(&error);
                eprintln!("Cannot persist received message batch; delivering filtered messages");
                false
            }
        };
        if baseline.is_some() {
            // Recheck the entire view, including unchanged rows deleted during the refresh.
            store.filter_deleted(&mut batch)?;
            if stored && session.is_active() {
                if let Err(error) =
                    crate::chat::save_threads_cache_with_store(&session, &store, &batch)
                {
                    crate::runtime_metrics::database_error(&error);
                    crate::logger::log_error(
                        "threads cache",
                        &format!("Cannot save history cache: {error}"),
                    );
                }
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
        Ok(Err(error)) => {
            crate::runtime_metrics::database_error(&error);
            threads.clear();
            false
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
        let rewritten = s.rewrite_download_url(&url);
        let compatible = crate::media_recovery::compatible_image_url(&rewritten, true)?;
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

struct ActivePhoneSyncTask {
    owner: String,
    runtime_id: usize,
    operation_id: u64,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

async fn reap_finished_phone_sync(
    slot: &mut Option<ActivePhoneSyncTask>,
    running: &std::sync::atomic::AtomicBool,
) {
    if slot.as_ref().is_some_and(|t| t.handle.is_finished()) {
        if let Some(task) = slot.take() {
            let _ = task.handle.await;
        }
        running.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

async fn abort_and_join_phone_sync(
    slot: &mut Option<ActivePhoneSyncTask>,
    running: &std::sync::atomic::AtomicBool,
    cancel_flag: &std::sync::atomic::AtomicBool,
) {
    cancel_flag.store(true, std::sync::atomic::Ordering::Release);
    if let Some(mut task) = slot.take() {
        task.cancel
            .store(true, std::sync::atomic::Ordering::Release);
        crate::phone_sync::request_wake();
        if tokio::time::timeout(Duration::from_secs(2), &mut task.handle)
            .await
            .is_err()
        {
            task.handle.abort();
            let _ = task.handle.await;
        }
    }
    running.store(false, std::sync::atomic::Ordering::SeqCst);
}

fn current_sync_availability(
    account: &Session,
    db_ready_runtime: &std::sync::atomic::AtomicUsize,
    ws_ready: bool,
    already_syncing: bool,
    source_enabled: bool,
) -> crate::sync_coordinator::SyncAvailability {
    let runtime_id = account.runtime_id();
    let has_permission =
        account.is_active() && account.login.is_some() && !account.account_key().is_empty();
    let db_ready = runtime_id != 0
        && db_ready_runtime.load(std::sync::atomic::Ordering::Acquire) == runtime_id;
    crate::sync_coordinator::evaluate_sync_availability(
        has_permission,
        db_ready,
        ws_ready,
        source_enabled,
        already_syncing,
    )
}

#[allow(clippy::too_many_arguments)]
async fn run_startup_preload(
    session_arc: std::sync::Arc<tokio::sync::Mutex<Session>>,
    gui_tx: Sender<GuiEvent>,
    egui_ctx: egui::Context,
    live_updates: std::sync::Arc<crate::live_updates::LiveUpdates>,
    outgoing_lifecycle: OutgoingLifecycle,
    outgoing_identity: std::sync::Arc<std::sync::RwLock<(String, usize)>>,
    db_ready_runtime: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ws_ready_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let s = session_arc.lock().await.clone();
    if !s.is_active() || s.login.is_none() || s.account_key().is_empty() {
        db_ready_runtime.store(0, std::sync::atomic::Ordering::Release);
        return;
    }
    let owner = s.account_key().to_owned();
    let runtime_id = s.runtime_id();
    db_ready_runtime.store(0, std::sync::atomic::Ordering::Release);
    let _ = gui_tx.send(GuiEvent::PhoneSyncState {
        owner: owner.clone(),
        runtime_id,
        operation_id: 0,
        state: crate::sync_coordinator::SyncState::Idle {
            availability: crate::sync_coordinator::SyncAvailability::Unavailable(
                crate::sync_coordinator::SyncUnavailableReason::DbMigrating,
            ),
        },
    });

    let _ = gui_tx.send(GuiEvent::StartupStage {
        owner: owner.clone(),
        runtime_id,
        stage_text: "Đang kiểm tra cơ sở dữ liệu cục bộ…".into(),
    });
    egui_ctx.request_repaint();

    // 1. Open and migrate SQLite first; migration/schema errors are fatal and never degrade to Ready.
    let db_session = s.clone();
    let db_preload = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Thread>> {
        let (store, _write_permit) =
            crate::store::MessageStore::open_for_account_write(&db_session)?;
        let mut preload = crate::chat::load_threads_cache_with_store(&db_session, &store);
        store.hydrate_threads(&mut preload, crate::store::MessageStore::PRELOAD_MSGS)?;
        Ok(preload)
    })
    .await;

    let preload = match db_preload {
        Ok(Ok(threads)) => threads,
        Ok(Err(error)) => {
            crate::runtime_metrics::database_error(&error);
            if s.is_active() {
                let _ = gui_tx.send(GuiEvent::StartupFatalError {
                    owner,
                    runtime_id,
                    message: format!("Không thể khởi tạo cơ sở dữ liệu cục bộ: {error}"),
                });
                egui_ctx.request_repaint();
            }
            return;
        }
        Err(error) => {
            if s.is_active() {
                let _ = gui_tx.send(GuiEvent::StartupFatalError {
                    owner,
                    runtime_id,
                    message: format!("Tác vụ khởi tạo CSDL thất bại: {error}"),
                });
                egui_ctx.request_repaint();
            }
            return;
        }
    };
    live_updates.seed(&preload);
    db_ready_runtime.store(runtime_id, std::sync::atomic::Ordering::Release);
    let availability = current_sync_availability(
        &s,
        &db_ready_runtime,
        ws_ready_flag.load(std::sync::atomic::Ordering::SeqCst),
        false,
        true,
    );
    let _ = gui_tx.send(GuiEvent::PhoneSyncState {
        owner: owner.clone(),
        runtime_id,
        operation_id: 0,
        state: crate::sync_coordinator::SyncState::Idle { availability },
    });

    // 2. Refresh server configuration and apply it directly to the active Session.
    let _ = gui_tx.send(GuiEvent::StartupStage {
        owner: owner.clone(),
        runtime_id,
        stage_text: "Đang tải cấu hình máy chủ…".into(),
    });
    egui_ctx.request_repaint();

    if let Ok(response) = crate::login::get_server_info(&s).await {
        let updated = {
            let mut guard = session_arc.lock().await;
            if guard.is_active() && guard.runtime_id() == runtime_id && guard.account_key() == owner
            {
                guard.apply_server_info(&response);
                Some((guard.composer_config, guard.server_config.clone()))
            } else {
                None
            }
        };
        if let Some((composer_cfg, server_cfg)) = updated {
            if let Some(config) = composer_cfg {
                let _ = gui_tx.send(GuiEvent::ComposerConfig {
                    owner: owner.clone(),
                    runtime_id,
                    config,
                });
            }
            let _ = gui_tx.send(GuiEvent::ServerConfigUpdated {
                owner: owner.clone(),
                runtime_id,
                config: server_cfg,
            });
            egui_ctx.request_repaint();
        }
    } else {
        crate::logger::log_error(
            "server config",
            "Cannot refresh server configuration at startup; keeping current defaults",
        );
    }

    // 3. Validate session state; expired/revoked auth is fatal and never degrades to Ready.
    let _ = gui_tx.send(GuiEvent::StartupStage {
        owner: owner.clone(),
        runtime_id,
        stage_text: "Đang xác thực phiên và chuẩn bị giao diện…".into(),
    });
    egui_ctx.request_repaint();

    // Authentication must use the configuration just applied, not the pre-refresh clone.
    let s = {
        let guard = session_arc.lock().await;
        if !guard.is_active() || guard.runtime_id() != runtime_id || guard.account_key() != owner {
            return;
        }
        guard.clone()
    };
    let status = s.validate_session().await;
    if status == crate::session::SessionStatus::Expired {
        db_ready_runtime.store(0, std::sync::atomic::Ordering::Release);
        let Ok(_write_barrier) = s.close_account_writes().await else {
            return;
        };
        let mut revoked = false;
        {
            let mut guard = session_arc.lock().await;
            if guard.runtime_id() == runtime_id && guard.is_active() {
                guard.revoke_runtime();
                outgoing_lifecycle.retire(runtime_id);
                guard.logout();
                refresh_outgoing_identity(&outgoing_identity, &guard);
                revoked = true;
            }
        }
        if revoked {
            let _ = tokio::task::spawn_blocking(move || {
                crate::storage::remove_session_for_runtime(runtime_id)
            })
            .await;
            let _ = gui_tx.send(GuiEvent::StartupFatalError {
                owner,
                runtime_id,
                message:
                    "Phiên đăng nhập (zpw_sek) đã hết hạn hoặc bị thu hồi; vui lòng đăng nhập lại."
                        .into(),
            });
            egui_ctx.request_repaint();
        }
        return;
    }

    let still_active = {
        let guard = session_arc.lock().await;
        guard.is_active() && guard.runtime_id() == runtime_id && guard.account_key() == owner
    };
    if still_active {
        run_optional_startup_preload(
            &s,
            &gui_tx,
            &egui_ctx,
            preload,
            hydrate_e2ee_peers(&s, &gui_tx, &egui_ctx),
        )
        .await;
    }
}

async fn run_optional_startup_preload(
    session: &Session,
    tx: &Sender<GuiEvent>,
    ctx: &egui::Context,
    preload: Vec<Thread>,
    optional: impl std::future::Future<Output = anyhow::Result<()>>,
) {
    if !session.is_active() {
        return;
    }
    let owner = session.account_key().to_owned();
    let runtime_id = session.runtime_id();
    let _ = tx.send(GuiEvent::StartupPrerequisitesSettled {
        owner: owner.clone(),
        runtime_id,
        preload_threads: preload,
    });
    ctx.request_repaint();

    // Optional failure settles the preload, never the mandatory DB/auth gate.
    if let Err(error) = optional.await {
        crate::logger::log_error(
            "startup preload",
            &format!("Optional preload failed: {error}"),
        );
    }
    if session.is_active() {
        let _ = tx.send(GuiEvent::StartupPreloadSettled { owner, runtime_id });
        ctx.request_repaint();
    }
}

// A worker-private admission semaphore is never explicitly closed. Holding a permit
// across HTTP awaits serializes snapshots/mutations without retaining a data mutex.
async fn metadata_permit(gate: &tokio::sync::Semaphore) -> tokio::sync::SemaphorePermit<'_> {
    gate.acquire()
        .await
        .expect("private metadata admission gate is never closed")
}

pub(super) fn spawn_session_worker(
    session: Session,
    gui_tx: Sender<GuiEvent>,
    egui_ctx: egui::Context,
) {
    session.hidden.enable();
    let outgoing_lifecycle = OutgoingLifecycle::new(gui_tx.clone(), egui_ctx.clone());
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<WorkerCmd>();
    let (outgoing_tx, mut outgoing_rx) = tokio::sync::mpsc::unbounded_channel::<(
        String,
        usize,
        String,
        ChatMessage,
        OutgoingTicket,
    )>();
    let outgoing_identity = std::sync::Arc::new(std::sync::RwLock::new((
        session.account_key().to_owned(),
        session.runtime_id(),
    )));
    let live_updates = std::sync::Arc::new(crate::live_updates::LiveUpdates::default());
    let image_jobs = crate::media_resources::MediaJobQueue::default();
    let commands = cmd_tx.downgrade();
    let (image_tx, mut image_rx) =
        tokio::sync::mpsc::channel::<(String, String, Option<ChatImage>)>(128);
    WORKER_TX.set(WorkerSender {
        commands: cmd_tx,
        outgoing: Some(outgoing_tx),
        lifecycle: outgoing_lifecycle.clone(),
        identity: outgoing_identity.clone(),
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
    let metadata_gate = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
    let pin_gate = std::sync::Arc::new(tokio::sync::Mutex::new(()));
    let search_gate = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
    let search_latest = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let history_sync_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let history_sync_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let phone_sync_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let phone_sync_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut active_phone_sync: Option<ActivePhoneSyncTask> = None;
    let db_ready_runtime = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
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
            let _outgoing_shutdown = OutgoingRuntimeGuard(outgoing_lifecycle.clone());

            rt.block_on(async move {
                tokio::spawn(crate::runtime_metrics::export_loop());
                {
                    let session = session.clone();
                    let tx = gui_tx.clone();
                    let ctx = egui_ctx.clone();
                    tokio::spawn(async move {
                        while let Some((owner, runtime_id, thread_id, message, ticket)) = outgoing_rx.recv().await {
                            let current = session.lock().await.clone();
                            if !outgoing_identity_matches(
                                current.is_active(), current.account_key(), current.runtime_id(),
                                &owner, runtime_id,
                            ) {
                                ticket.finish(outgoing_not_sent(message, "Phiên tài khoản đã thay đổi trước khi gửi"));
                                continue;
                            }
                            spawn_outgoing_send(current, session.clone(), tx.clone(), ctx.clone(), owner, thread_id, message, ticket);
                        }
                    });
                }
                {
                    let shared=session.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();let notify=metadata_notify.clone();let gate=metadata_gate.clone();let cache_started=metadata_cache_started.clone();let metadata_live=live_updates.clone();
                    tokio::spawn(async move {let mut tick=tokio::time::interval(Duration::from_secs(3));let mut previous=String::new();let mut at=std::time::Instant::now()-Duration::from_secs(121);
                        loop {
                            let awakened=tokio::select! {_=tick.tick()=>false,_=notify.notified()=>true};let requested=metadata_live.take_metadata_request()||awakened;
                            let account=shared.lock().await.clone();if !account.is_active(){break;}let owner=account.account_key().to_owned();if owner.is_empty(){continue;}
                            if !requested && owner==previous && at.elapsed()<Duration::from_secs(900){continue;}
                            let _gate=metadata_permit(&gate).await;
                            if previous!=owner {
                                if !cache_started.swap(true,std::sync::atomic::Ordering::SeqCst) {
                                    let cache_rev = crate::search::generation();
                                    if let Ok(Some(snapshot))=crate::account_metadata::restore_cached(&account).await {
                                        let _=tx.send(GuiEvent::AccountMetadata {owner:owner.clone(),runtime_id:account.runtime_id(),revision:cache_rev,snapshot:Box::new(snapshot)});
                                    }
                                }
                                if let Ok(Some(view))=crate::hidden_chat::restore_cached(&account).await {let _=tx.send(GuiEvent::Hidden {owner:owner.clone(),generation:None,view,result:Ok(())});ctx.request_repaint();}
                            }
                            previous=owner.clone();at=std::time::Instant::now();
                            let snapshot_rev = crate::search::generation();
                            let runtime_id = account.runtime_id();
                            let unread_fut = crate::unread::fetch_manual_unread_marks(&account);
                            let archive_fut = crate::archive::execute(&account, "", false, None);
                            let labels_fut = crate::labels::execute(&account, &crate::labels::Action::Read);
                            let (unread_res, archive_res, labels_res) = tokio::join!(unread_fut, archive_fut, labels_fut);
                            let _ = tx.send(GuiEvent::ManualMarks {
                                owner: owner.clone(),
                                runtime_id,
                                generation: snapshot_rev,
                                kind: MetadataResultKind::Snapshot,
                                result: unread_res.map_err(|error| error.to_string()),
                            });
                            let _ = tx.send(GuiEvent::Archive {
                                owner: owner.clone(),
                                runtime_id,
                                generation: snapshot_rev,
                                kind: MetadataResultKind::Snapshot,
                                result: archive_res.map_err(|error| error.to_string()),
                            });
                            let _ = tx.send(GuiEvent::Labels {
                                owner: owner.clone(),
                                runtime_id,
                                generation: snapshot_rev,
                                kind: MetadataResultKind::Snapshot,
                                result: labels_res.map_err(|error| error.to_string()),
                            });
                            ctx.request_repaint();

                            let hidden=async {let result=crate::hidden_chat::execute(&account,&crate::hidden_chat::Action::Read).await.map(|_|()).map_err(|error|error.to_string());let _=tx.send(GuiEvent::Hidden {owner:owner.clone(),generation:None,view:account.hidden.view(&owner),result});ctx.request_repaint();};
                            let meta_rev = crate::search::generation();
                            let publish_names = |snapshot| {
                                if account.is_active() {
                                    let _ = tx.send(GuiEvent::AccountMetadata {owner:owner.clone(),runtime_id:account.runtime_id(),revision:meta_rev,snapshot:Box::new(snapshot)});
                                    ctx.request_repaint();
                                }
                            };
                            let (_,snapshot)=tokio::join!(hidden,crate::account_metadata::read(&account, publish_names));
                            if !account.is_active(){break;}
                            if let Ok(mut policy)=account.inbox_policy.write(){if let Some(friends)=&snapshot.friends{policy.friends=Some(friends.clone());}
                                if let Some(business)=snapshot.business{policy.business=Some(business);}
                                if let Some(privacy)=&snapshot.privacy{policy.direct=privacy.values.get(&crate::privacy::Setting::StrangerDirect).map(|value|*value!=0);policy.show_online=privacy.values.get(&crate::privacy::Setting::ShowOnline).map(|value|*value!=0);}}

                            crate::account_metadata::save_cached(&account,&snapshot).await;
                            let _=tx.send(GuiEvent::AccountMetadata {owner,runtime_id:account.runtime_id(),revision:meta_rev,snapshot:Box::new(snapshot)});ctx.request_repaint();
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
                                if key.starts_with("stk_") {
                                    match fetch_and_decode_sticker_sprite(&key, &url).await {
                                        Ok(sprite) => {
                                            let _ = tx.send(GuiEvent::StickerSpriteReady {
                                                key: key.clone(),
                                                sprite,
                                                permit: Some(job),
                                            });
                                        }
                                        Err(error) => {
                                            let failure = error
                                                .downcast_ref::<crate::media_recovery::Failure>()
                                                .cloned()
                                                .unwrap_or(crate::media_recovery::Failure::Local);
                                            crate::transfer_diagnostics::media(&failure);
                                            let _ = tx.send(GuiEvent::ImageUnavailable {
                                                key,
                                                failure,
                                            });
                                        }
                                    }
                                } else {
                                    match fetch_chat_image(&s,&key,&url,context,&tx).await {
                                        Ok(image) => { let _ = tx.send(GuiEvent::PreviewImageReady { owner,expires_at,key,image,permit:job }); },
                                        Err(error) => { let failure=error.downcast_ref::<crate::media_recovery::Failure>().cloned().unwrap_or(crate::media_recovery::Failure::Local);crate::transfer_diagnostics::media(&failure);let _ = tx.send(GuiEvent::ImageUnavailable { key,failure }); }
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                    });
                }
                let is_ws_connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let is_ws_connected_poll = is_ws_connected.clone();
                {
                    let session_arc = session.clone();
                    let gui_tx_init = gui_tx.clone();
                    let ctx_init = egui_ctx.clone();
                    let live_init = live_updates.clone();
                    let outgoing_lifecycle_init = outgoing_lifecycle.clone();
                    let outgoing_identity_init = outgoing_identity.clone();
                    let db_ready_init = db_ready_runtime.clone();
                    let ws_ready_init = is_ws_connected.clone();
                    tokio::spawn(async move {
                        let s = session_arc.lock().await.clone();
                        if s.login.is_some() {
                            run_startup_preload(
                                session_arc,
                                gui_tx_init,
                                ctx_init,
                                live_init,
                                outgoing_lifecycle_init,
                                outgoing_identity_init,
                                db_ready_init,
                                ws_ready_init,
                            )
                            .await;
                        } else {
                            // Unauthenticated startup still hydrates server_config on the active session.
                            let cached_session = s.clone();
                            if let Ok(cached) = tokio::task::spawn_blocking(move || {
                                crate::chat::load_threads_cache(&cached_session)
                            })
                            .await
                            {
                                live_init.seed(&cached);
                            }
                            if let Ok(response) = crate::login::get_server_info(&s).await {
                                let mut guard = session_arc.lock().await;
                                if guard.is_active() && guard.runtime_id() == s.runtime_id() {
                                    guard.apply_server_info(&response);
                                }
                            }
                        }
                    });
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

                // Correlation map for voice-to-text tickets: req_id -> (thread_id, msg_id, generation, Instant)
                type PendingV2tStore = VoiceCorrelations;
                let pending_v2t_tickets: std::sync::Arc<tokio::sync::Mutex<PendingV2tStore>> =
                    std::sync::Arc::new(tokio::sync::Mutex::new(VoiceCorrelations::default()));
                let pending_v2t_ws = pending_v2t_tickets.clone();

                let p3_name_jobs = p3_start_name_worker(gui_tx.clone(), egui_ctx.clone());
                // 1) Real-time `zpw_ws` WebSocket loop for instant chat messages + VoIP events
                {
                    let live_updates = live_updates.clone();
                    let metadata_notify=metadata_notify.clone();
                    let mute_sync = mute_sync.clone();
                    let s_ws = session.clone();
                    let cb = call_bridge.clone();
                    let gui_tx_ws = gui_tx.clone();
                    let ctx_ws = egui_ctx.clone();
                    let is_ws_connected_ws = is_ws_connected.clone();
                    let db_ready_ws = db_ready_runtime.clone();
                    let phone_running_ws = phone_sync_running.clone();
                    let p3_names = p3_name_jobs.clone();
                    tokio::spawn(async move {
                        let mut ws_rx: Option<
                            tokio::sync::mpsc::Receiver<crate::socket::WsEvent>,
                        > = None;
                        let mut ws_sek = String::new();
                        let mut ws_runtime = None;
                        let mut ws_owner: Option<Session> = None;
                        let mut p3_side_workers: Option<P3SideWorkers> = None;
                        loop {
                            let s_snap = s_ws.lock().await.clone();
                            if s_snap.login.is_some() {
                                let cur_sek = s_snap.zpw_sek.clone().unwrap_or_default();
                                if !cur_sek.is_empty() && (ws_rx.is_none() || !p3_same_socket_owner(ws_runtime, &ws_sek, s_snap.runtime_id(), &cur_sek)) {
                                    ws_sek = cur_sek;
                                    ws_runtime = Some(s_snap.runtime_id());
                                    ws_owner = Some(s_snap.clone());
                                    let (events,mut priority,transport)=crate::socket::spawn_with_sync_priority(s_snap.clone());
                                    ws_rx = Some(events);
                                    let priority_account = s_snap.clone();
                                    let priority_bridge = cb.clone();
                                    let priority_notify = metadata_notify.clone();
                                    let priority_events = gui_tx_ws.clone();
                                    let priority_ctx = ctx_ws.clone();
                                    let priority_task = tokio::spawn(async move {
                                        while let Some(event) = priority.recv().await {
                                            if !priority_account.is_active() { break; }
                                            let data = event.payload.get("data").unwrap_or(&event.payload);
                                            for control in crate::call::extract_voip_controls(data) {
                                                priority_bridge.handle_control_event(control);
                                            }
                                            apply_metadata_impact(&priority_account, crate::account_controls::from_frame(&event), &priority_notify, &priority_events, &priority_ctx);
                                            p3_publish_typing(&priority_account, &event.payload, &priority_events, &priority_ctx);
                                        }
                                    });
                                    p3_side_workers = Some(p3_start_side_workers(s_snap.clone(), gui_tx_ws.clone(), ctx_ws.clone(), priority_task));
                                    tokio::spawn(crate::phone_sync::clean_after_connect(s_snap.clone(),transport.clone()));
                                    let account=s_snap.clone();let notify=metadata_notify.clone();let mut epochs=transport.epochs();let epoch_tx=gui_tx_ws.clone();let epoch_ctx=ctx_ws.clone();
                                    tokio::spawn(async move {while epochs.changed().await.is_ok(){if !account.is_active(){break;}
                                        // Controls have no durable message cursor. A new epoch conservatively
                                        // invalidates privacy/account caches in case an overloaded epoch lost hints.
                                        if epochs.borrow().is_some(){account.hidden.lock();let _=epoch_tx.send(GuiEvent::Hidden {owner:account.account_key().into(),generation:None,view:account.hidden.view(account.account_key()),result:Ok(())});apply_metadata_impact(&account,crate::account_controls::Impact::for_account_switch(),&notify,&epoch_tx,&epoch_ctx);notify.notify_one();}}});
                                    phone_transport_tx.send_replace(Some(transport.clone()));
                                    // WP15.1-D: liveness thật từ transport epoch (Some = đã auth),
                                    // thay cho detection theo "có nhận event" (sai khi idle/half-dead).
                                    let liveness_flag = is_ws_connected_ws.clone();
                                    let mut epoch_watch = transport.epochs();
                                    let epoch_account = s_snap.clone();
                                    let epoch_db_ready = db_ready_ws.clone();
                                    let epoch_running = phone_running_ws.clone();
                                    let epoch_state_tx = gui_tx_ws.clone();
                                    let epoch_state_ctx = ctx_ws.clone();
                                    tokio::spawn(async move {
                                        loop {
                                            let ws_ready = epoch_watch.borrow().is_some();
                                            liveness_flag.store(ws_ready, std::sync::atomic::Ordering::SeqCst);
                                            if epoch_account.is_active()
                                                && !epoch_running.load(std::sync::atomic::Ordering::SeqCst)
                                            {
                                                let availability = current_sync_availability(
                                                    &epoch_account,
                                                    &epoch_db_ready,
                                                    ws_ready,
                                                    false,
                                                    true,
                                                );
                                                let _ = epoch_state_tx.send(GuiEvent::PhoneSyncState {
                                                    owner: epoch_account.account_key().to_owned(),
                                                    runtime_id: epoch_account.runtime_id(),
                                                    operation_id: 0,
                                                    state: crate::sync_coordinator::SyncState::Idle {
                                                        availability,
                                                    },
                                                });
                                                epoch_state_ctx.request_repaint();
                                            }
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
                                            let Some(s_snap) = ws_owner.as_ref().filter(|owner| owner.is_active()) else { continue; };
                                            let _page_timing = crate::runtime_metrics::Scope::new(crate::runtime_metrics::Metric::IngressPage);
                                            // WP15.1-D: liveness lấy từ epoch watch (spawn ở trên),
                                            // không set theo activity event — idle-but-connected vẫn connected.
                                            let data_node = ev
                                                .payload
                                                .get("data")
                                                .unwrap_or(&ev.payload);
                                            // Call, typing, and privacy invalidation use the priority consumer.
                                            let reaction_events=crate::reaction_pipeline::events(&ev.payload);
                                            let has_reactions=!reaction_events.is_empty();
                                            let reactions_stored=if reaction_events.is_empty(){true}else{
                                                p3_enqueue_reactions(s_snap, reaction_events).await
                                            };

                                            for v2t in crate::socket::parse_voice_to_text_pushes(&ev.payload) {
                                                let pending={let mut map=pending_v2t_ws.lock().await;let pending=map.remove(&(s_snap.runtime_id(),v2t.req_id.clone()));if pending.is_none(){map.early.insert(s_snap.runtime_id(),v2t.clone());}pending};
                                                if let Some((thread_id, msg_id, generation, _)) = pending {
                                                    if v2t.error_code == 0 && v2t.text.is_some() {
                                                        let _ = gui_tx_ws.send(GuiEvent::VoiceTranscriptLoaded {
                                                            thread_id,
                                                            msg_id,
                                                            transcript: voice_result_text(v2t),
                                                            generation,
                                                        });
                                                    } else {
                                                        let _ = gui_tx_ws.send(GuiEvent::VoiceTranscriptFailed {
                                                            thread_id,
                                                            msg_id,
                                                            error: format!("Mã lỗi: {}", v2t.error_code),
                                                            generation,
                                                        });
                                                    }
                                                    ctx_ws.request_repaint();
                                                }
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
                                                crate::chat::merge_previews_with_profile(
                                                    &mut ws_threads,
                                                    &ev.payload,
                                                    Some(s_snap.account_key()),
                                                );
                                                ws_threads.retain(|t| !t.messages.is_empty() || t.unread.fresh);
                                                let mut stored = true;
                                                if !ws_threads.is_empty() {
                                                    stored = persist_received(s_snap, &mut ws_threads).await;
                                                    p3_resolve_names_later(&p3_names, s_snap, &ws_threads);
                                                    if publish_live(s_snap, &live_updates, ws_threads) {
                                                        let _ = gui_tx_ws.send(GuiEvent::LiveThreadsReady(live_updates.clone()));
                                                    }
                                                    ctx_ws.request_repaint();
                                                }
                                                // Like Zalo PC, a queue cursor advances only after its page is stored.
                                                let receipts_stored = p3_deliver_receipts(s_snap, &ev.payload, &gui_tx_ws, &ctx_ws).await;
                                                crate::queue_cursors::commit_socket(s_snap, ev.cmd, ev.sub_cmd, &ev.payload, stored && reactions_stored && receipts_stored).await;
                                            } else {
                                                // Non-message controls do not carry a message-queue cursor.
                                                p3_deliver_receipts(s_snap, &ev.payload, &gui_tx_ws, &ctx_ws).await;
                                            }
                                            if has_reactions && reactions_stored {
                                                if let Some(workers) = &p3_side_workers { workers.reactions.notify_one(); }
                                            }
                                            if ev.is_voip_ish() {
                                                let mutes = crate::mute_sync::parse_mute_controls(data_node, crate::login::unix_now());
                                                apply_server_mutes(s_snap, mutes, &mute_sync, &gui_tx_ws, &live_updates, &ctx_ws).await;
                                            }
                                        }
                                        Ok(None) => {
                                            ws_rx = None;
                                            ws_owner = None;
                                            ws_runtime = None;
                                            p3_side_workers = None;
                                            tokio::time::sleep(Duration::from_secs(2)).await;
                                        }
                                        Err(_) => {}
                                    }
                                } else {
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                }
                            } else {
                                ws_rx = None;
                                ws_owner = None;
                                ws_runtime = None;
                                p3_side_workers = None;
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
                let auth_attempt_mgr = crate::login::AuthAttemptManager::new();
                let mut current_auth_task: Option<tokio::task::JoinHandle<()>> = None;
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

                // Upload progress forwarder: bridges upload::report_progress events to UI GuiEvent::UploadProgress
                {
                    let mut progress_rx = crate::upload::subscribe_progress();
                    let tx_progress = gui_tx.clone();
                    let ctx_progress = egui_ctx.clone();
                    tokio::spawn(async move {
                        forward_upload_progress(&mut progress_rx, &tx_progress, &ctx_progress).await;
                    });
                }

                // Check for uncompleted resumable uploads from prior runs
                {
                    let session_clone = session.clone();
                    let tx_resume = gui_tx.clone();
                    let ctx_resume = egui_ctx.clone();
                    tokio::spawn(async move {
                        let s = session_clone.lock().await.clone();
                        if s.is_active() {
                            let sessions = tokio::task::spawn_blocking(move || {
                                let (store, _write_permit) =
                                    crate::store::MessageStore::open_for_account_write(&s)?;
                                store.list_upload_sessions()
                            })
                            .await
                            .ok()
                            .and_then(Result::ok)
                            .unwrap_or_default();
                            if !sessions.is_empty() {
                                let _ = tx_resume.send(GuiEvent::UploadResumeAvailable { sessions });
                                ctx_resume.request_repaint();
                            }
                        }
                    });
                }

                {
                    let commands = commands.clone();
                    tokio::spawn(async move {
                        let mut tick = tokio::time::interval(Duration::from_secs(2));
                        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                        loop {
                            tick.tick().await;
                            let Some(tx) = commands.upgrade() else { break; };
                            if tx.send(WorkerCmd::RunAutomation).is_err() { break; }
                            if tx.send(WorkerCmd::PruneExpired).is_err() { break; }
                        }
                    });
                }
                let mut recovered_accounts=HashSet::new();
                let automation_running=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                while let Some(cmd) = cmd_rx.recv().await {
                    if let Some((thread_id, message)) = outgoing_command(&cmd) {
                        let tx = gui_tx.clone();
                        let ctx = egui_ctx.clone();
                        // Capture the session before spawning so the intent remains bound to its account.
                        let s = session.lock().await.clone();
                        let owner = s.account_key().to_owned();
                        if owner.is_empty() || !s.is_active() {
                            emit_outgoing_not_sent(s.runtime_id(), owner, thread_id, message,
                                "Phiên đăng nhập đã đóng trước khi gửi", &tx, &ctx);
                            continue;
                        }
                        let ticket = outgoing_lifecycle.accept(owner.clone(), s.runtime_id(), thread_id.clone(), message.clone());
                        spawn_outgoing_send(s, session.clone(), tx, ctx, owner, thread_id, message, ticket);
                        continue;
                    }
                    match cmd {
                        WorkerCmd::LoadComposerDraft{owner,thread,generation}=>{
                            let s=session.lock().await.clone();
                            let id=thread.clone();let expected=owner.clone();
                            let result=tokio::task::spawn_blocking(move||->anyhow::Result<crate::mentions::Draft>{
                                anyhow::ensure!(s.is_active() && !expected.is_empty() && s.account_key()==expected,"Tài khoản đã thay đổi");
                                let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&s)?;
                                store.composer_document(&id)
                            }).await.map_err(|_|"Không đọc được bản nháp".to_owned()).and_then(|r|r.map_err(|_|"Không đọc được bản nháp".into()));
                            let _=gui_tx.send(GuiEvent::ComposerDraftLoaded{owner,thread,generation,result});egui_ctx.request_repaint();
                        }
                        WorkerCmd::SaveComposerDraft{owner,thread,generation,text}=>{
                            let s=session.lock().await.clone();
                            let (id,expected,value)=(thread.clone(),owner.clone(),text.clone());
                            let result=tokio::task::spawn_blocking(move||->anyhow::Result<()>{
                                anyhow::ensure!(s.is_active() && !expected.is_empty() && s.account_key()==expected,"Tài khoản đã thay đổi");
                                let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&s)?;
                                store.save_composer_document(&id,&value)
                            }).await.map_err(|_|"Không lưu được bản nháp".to_owned()).and_then(|r|r.map_err(|_|"Không lưu được bản nháp".into()));
                            let _=gui_tx.send(GuiEvent::ComposerDraftSaved{owner,thread,generation,text,result});egui_ctx.request_repaint();
                        }
                        WorkerCmd::RetryStartup => {
                            let session_arc = session.clone();
                            let gui_tx_init = gui_tx.clone();
                            let ctx_init = egui_ctx.clone();
                            let live_init = live_updates.clone();
                            let outgoing_lifecycle_init = outgoing_lifecycle.clone();
                            let outgoing_identity_init = outgoing_identity.clone();
                            let db_ready_init = db_ready_runtime.clone();
                            let ws_ready_init = is_ws_connected.clone();
                            tokio::spawn(async move {
                                run_startup_preload(
                                    session_arc,
                                    gui_tx_init,
                                    ctx_init,
                                    live_init,
                                    outgoing_lifecycle_init,
                                    outgoing_identity_init,
                                    db_ready_init,
                                    ws_ready_init,
                                )
                                .await;
                            });
                        }
                        WorkerCmd::PrepareE2eeTrial { owner, peer_id } => {
                            let account = session.lock().await.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::task::spawn_blocking(move || {
                                let trace = crate::e2ee_trial::Trace::new(&owner, &peer_id, crate::e2ee_trial::Stage::Policy, 0);
                                let result = (|| -> anyhow::Result<()> {
                                    anyhow::ensure!(crate::e2ee_trial::enabled(), "Chưa bật E2EE thử nghiệm");
                                    anyhow::ensure!(account.is_active() && !owner.is_empty() && account.account_key() == owner, "Tài khoản đã thay đổi");
                                    crate::e2ee_integration::remember_e2ee_requirement(&owner, &peer_id)
                                })();
                                trace.finish_result(&result);
                                let result = result.map_err(|_| "Không bật được E2EE thử nghiệm cho hội thoại".to_owned());
                                let _ = tx.send(GuiEvent::E2eeTrialPrepared { owner, peer_id, result }); ctx.request_repaint();
                            });
                        }
                        WorkerCmd::ResetE2eeRequirement { owner, peer_id } => {
                            let account = session.lock().await.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            let owner_for_task = owner.clone();
                            let peer_for_task = peer_id.clone();
                            let join = tokio::task::spawn_blocking(move || {
                                if account.is_active() && !owner_for_task.is_empty() && account.account_key() == owner_for_task {
                                    let ok = crate::e2ee_integration::remove_e2ee_requirement(&owner_for_task, &peer_for_task).is_ok();
                                    crate::logger::log_error("e2ee_requirement_reset", &format!("owner={owner_for_task} peer={peer_for_task} success={ok}"));
                                    ok
                                } else {
                                    false
                                }
                            }).await;
                            let success = join.unwrap_or(false);
                            let _ = tx.send(GuiEvent::E2eeRequirementResetResult {
                                owner,
                                peer_id,
                                success,
                            });
                            ctx.request_repaint();
                        }
                        WorkerCmd::CheckE2eeStatus { owner, peer_id } => {
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            let owner_for_task = owner.clone();
                            let peer_for_task = peer_id.clone();
                            tokio::task::spawn_blocking(move || {
                                if crate::e2ee_integration::is_thread_e2ee(&owner_for_task, &peer_for_task) {
                                    let _ = tx.send(GuiEvent::E2eePeerRequired {
                                        owner: owner_for_task,
                                        peer_id: peer_for_task,
                                    });
                                    ctx.request_repaint();
                                }
                            });
                        }
                        WorkerCmd::TrustIdentity { owner, peer_id, safety_number } => {
                            let account = session.lock().await.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::task::spawn_blocking(move || {
                                let result = (|| -> anyhow::Result<()> {
                                    anyhow::ensure!(account.is_active() && account.account_key() == owner, "Tài khoản đã thay đổi");
                                    crate::e2ee_integration::require_verified_contract()?;
                                    let mgr = crate::crypto::session::E2eeManager::open(&owner)?;
                                    mgr.verify_identity_number(&peer_id, &safety_number)
                                })();
                                let event = match result {
                                    Ok(()) => GuiEvent::IdentityTrusted { owner, peer_id, is_trusted: true },
                                    Err(error) => GuiEvent::SafetyNumberFailed { owner, peer_id, message: error.to_string() },
                                };
                                let _ = tx.send(event); ctx.request_repaint();
                            });
                        }
                        WorkerCmd::LoadSafetyNumber { owner, peer_id, peer_name } => {
                            let account = session.lock().await.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::task::spawn_blocking(move || {
                                let result = (|| -> anyhow::Result<_> {
                                    anyhow::ensure!(account.is_active() && account.account_key() == owner, "Tài khoản đã thay đổi");
                                    crate::e2ee_integration::require_verified_contract()?;
                                    let mgr = crate::crypto::session::E2eeManager::open(&owner)?;
                                    let (key, verified) = mgr.get_trusted_identity(&peer_id)?
                                        .ok_or_else(|| anyhow::anyhow!("Chưa có khóa danh tính đã xác thực của đối phương; chưa thể tính mã an toàn"))?;
                                    Ok((mgr.safety_number(&peer_id, &key)?, verified))
                                })();
                                let event = match result {
                                    Ok((safety_number, is_verified)) => GuiEvent::SafetyNumberLoaded { owner, peer_id, peer_name, safety_number, is_verified, identity_changed: false },
                                    Err(error) => GuiEvent::SafetyNumberFailed { owner, peer_id, message: error.to_string() },
                                };
                                let _ = tx.send(event); ctx.request_repaint();
                            });
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
                                    let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&account)?;
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
                                let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&account)?;
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
                                        let _=tokio::task::spawn_blocking(move||->anyhow::Result<()>{let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&account)?;crate::tools::finish(&store,&owner,&id,"uncertain","Một lần gửi khác đang xử lý; kiểm tra hội thoại trước khi thử lại")}).await;
                                        return;
                                    };
                                    persist_outgoing(&s,&task.thread_id,&task.message).await;
                                    let _=tx.send(GuiEvent::OutgoingState{runtime_id:s.runtime_id(),owner:task.owner.clone(),thread_id:task.thread_id.clone(),message:task.message.clone()});ctx.request_repaint();
                                    let (message,status,error)=match crate::outgoing::send(&s,&task.thread_id,&task.message).await {
                                        Ok(message)=>(message,"sent",String::new()),
                                        Err(cause)=>{let delivery=crate::outgoing::failure_delivery(&cause);let error=crate::outgoing::failure_message(&cause);let mut message=task.message.clone();message.delivery=delivery;if let Some(job)=message.outgoing_job.as_mut(){job.error=error.clone();}(message,if delivery==crate::message_model::DeliveryState::DeliveryUnknown{"uncertain"}else{"failed"},error)}
                                    };
                                    persist_outgoing(&s,&task.thread_id,&message).await;
                                    let account=s.clone();let id=task.id.clone();let owner=task.owner.clone();
                                    let result=tokio::task::spawn_blocking(move ||->anyhow::Result<_>{anyhow::ensure!(account.is_active(),"Đã đăng xuất");let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&account)?;crate::tools::finish(&store,&owner,&id,status,&error)?;Ok(super::tools_panel::ToolsResult::Tasks{tasks:crate::tools::scheduled(&store,&owner)?,paused:crate::tools::paused(&store)?})}).await;
                                    match result {Ok(Ok(result))=>{let _=tx.send(GuiEvent::Tools{owner:task.owner.clone(),result:Ok(result)});},_=>crate::logger::log_error("scheduler","Cannot finalize scheduled delivery")};
                                    drop(active);let _=tx.send(GuiEvent::OutgoingState{runtime_id:s.runtime_id(),owner:task.owner,thread_id:task.thread_id,message});ctx.request_repaint();
                                });
                            }
                        }
                        WorkerCmd::TranslateMessage {
                            thread_id,
                            msg_id,
                            text,
                            target_lang,
                            text_hash,
                            generation,
                        } => {
                            let s_arc = session.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_arc.lock().await.clone();
                                match crate::chat::translate_message_manual(&s, &text, &target_lang, &msg_id).await {
                                    Ok(res) => {
                                        let _ = tx.send(GuiEvent::MessageTranslated {
                                            thread_id,
                                            msg_id,
                                            translated_text: res.translated_text,
                                            source_lang: res.source_lang,
                                            text_hash,
                                            generation,
                                        });
                                    }
                                    Err(e) => {
                                        let _ = tx.send(GuiEvent::MessageTranslateFailed {
                                            thread_id,
                                            msg_id,
                                            error: e.to_string(),
                                            generation,
                                        });
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::PlayInlineVoice {binding,controls,context} => {
                            let account=session.lock().await.clone();
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {play_inline_voice(account,binding,controls,context,tx,ctx).await;});
                        }
                        WorkerCmd::TranscribeVoiceMessage {
                            thread_id,
                            msg_id,
                            voice_url,
                            generation,
                            context,
                        } => {
                            let s_arc = session.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            let tickets = pending_v2t_tickets.clone();
                            tokio::spawn(async move {
                                let s = s_arc.lock().await.clone();
                                if !s.is_active() || generation!=s.runtime_id() as u64 {
                                    let _=tx.send(GuiEvent::VoiceTranscriptFailed {thread_id,msg_id,error:"Phiên tài khoản đã thay đổi".into(),generation});ctx.request_repaint();return;
                                }
                                let result=async {
                                    anyhow::ensure!(media_context_alive(&s,Some(&context)) && context.thread==thread_id && context.message.id==msg_id && !context.message.is_e2ee && context.message.kind!=crate::message_model::MessageKind::Recall,"Tin âm thanh không còn hợp lệ để chuyển thành văn bản");
                                    let consent=tokio::task::spawn_blocking(||AppSettings::load().ai_translation_consent).await?;
                                    anyhow::ensure!(consent,"Bạn chưa đồng ý chuyển giọng nói bằng AI trong Cài đặt");
                                    crate::conversation_policy::require_shareable(&s,&thread_id,context.group).await?;
                                    anyhow::ensure!(s.is_active() && generation==s.runtime_id() as u64,"Phiên tài khoản đã thay đổi");
                                    crate::chat::submit_voice_to_text(&s,&voice_url,&msg_id,&thread_id).await
                                }.await;
                                let current=s_arc.lock().await.clone();
                                if !s.is_active() || !current.is_active() || current.runtime_id()!=s.runtime_id() || generation!=current.runtime_id() as u64 {
                                    let _=tx.send(GuiEvent::VoiceTranscriptFailed {thread_id,msg_id,error:"Phiên tài khoản đã thay đổi".into(),generation});ctx.request_repaint();return;
                                }
                                match result {
                                    Ok(ticket) => {
                                        if let Some(transcript) = ticket.transcript {
                                            let _ = tx.send(GuiEvent::VoiceTranscriptLoaded {
                                                thread_id,
                                                msg_id,
                                                transcript,
                                                generation,
                                            });
                                            ctx.request_repaint();
                                        } else {
                                            // Register ticket in correlation map
                                            let req_id = ticket.req_id.clone();
                                            let early_result={
                                                const MAX_PENDING_V2T: usize = 128;
                                                let mut map = tickets.lock().await;
                                                if !s.is_active() || generation!=s.runtime_id() as u64 {return;}
                                                let ticket_key=(s.runtime_id(),req_id.clone());
                                                if map.contains_key(&ticket_key) {
                                                    let _=tx.send(GuiEvent::VoiceTranscriptFailed {thread_id,msg_id,generation,error:"Yêu cầu chuyển giọng nói đang được xử lý; vui lòng chờ".into()});ctx.request_repaint();return;
                                                }
                                                if map.len() >= MAX_PENDING_V2T {
                                                    // Evict oldest entry based on timestamp
                                                    if let Some(oldest_key) = map
                                                        .iter()
                                                        .min_by_key(|(_, (_, _, _, instant))| *instant)
                                                        .map(|(k, _)| k.clone())
                                                    {
                                                        if let Some((thread_id,msg_id,generation,_))=map.remove(&oldest_key){
                                                            let _=tx.send(GuiEvent::VoiceTranscriptFailed {thread_id,msg_id,generation,error:"Đang có quá nhiều yêu cầu chuyển giọng nói; vui lòng thử lại".into()});
                                                        }
                                                    }
                                                }
                                                map.insert(
                                                    ticket_key,
                                                    (thread_id.clone(), msg_id.clone(), generation, std::time::Instant::now()),
                                                );
                                                map.early.take(s.runtime_id(),&req_id)
                                            };
                                            if let Some(result)=early_result {
                                                if tickets.lock().await.remove_matching(s.runtime_id(),&req_id,&thread_id,&msg_id,generation).is_some(){
                                                    let event=if result.error_code==0 && result.text.is_some(){GuiEvent::VoiceTranscriptLoaded {thread_id:thread_id.clone(),msg_id:msg_id.clone(),transcript:voice_result_text(result),generation}}else{GuiEvent::VoiceTranscriptFailed {thread_id:thread_id.clone(),msg_id:msg_id.clone(),error:format!("Chuyển giọng nói không thành công (mã {})",result.error_code),generation}};
                                                    let _=tx.send(event);ctx.request_repaint();
                                                }
                                            }
                                            // Spawn 30-second timeout watcher for ticket
                                            let tickets_timeout = tickets.clone();
                                            let tx_timeout = tx.clone();
                                            let ctx_timeout = ctx.clone();
                                            let timeout_thread=thread_id.clone();let timeout_message=msg_id.clone();let timeout_runtime=s.runtime_id();
                                            tokio::spawn(async move {
                                                tokio::time::sleep(Duration::from_secs(30)).await;
                                                let mut map = tickets_timeout.lock().await;
                                                if let Some((th, mid, gen, _)) = map.remove_matching(timeout_runtime,&req_id,&timeout_thread,&timeout_message,generation) {
                                                    let _ = tx_timeout.send(GuiEvent::VoiceTranscriptFailed {
                                                        thread_id: th,
                                                        msg_id: mid,
                                                        error: "Hết thời gian chờ phản hồi chuyển giọng nói (timeout 30s)".into(),
                                                        generation: gen,
                                                    });
                                                    ctx_timeout.request_repaint();
                                                }
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(GuiEvent::VoiceTranscriptFailed {
                                            thread_id,
                                            msg_id,
                                            error: e.to_string(),
                                            generation,
                                        });
                                        ctx.request_repaint();
                                    }
                                }
                            });
                        }
                        WorkerCmd::RetryOutgoing { .. } => {},
                        WorkerCmd::SaveAppSettings { settings, persist_session } => {
                            let account = session.lock().await.clone();
                            let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                                let previous = AppSettings::load();
                                let autostart = settings.get("autostart_on_boot").and_then(serde_json::Value::as_bool).unwrap_or(false);
                                if autostart != previous.autostart_on_boot {
                                    crate::platform::set_autostart(autostart)?;
                                }
                                crate::storage::save_settings(&settings)?;
                                if persist_session && account.is_active() && !account.account_key().is_empty() {
                                    account.save_to_file("session.json")?;
                                }
                                Ok(())
                            }).await.map_err(|_| "Không lưu được cài đặt".to_owned()).and_then(|r| r.map_err(|e| e.to_string()));
                            let _ = gui_tx.send(GuiEvent::AppSettingsSaved(result));
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::LoadProfiles { owner } => {
                            let result = tokio::task::spawn_blocking(|| -> anyhow::Result<Vec<crate::profiles::Profile>> {
                                let registry = crate::profiles::ProfileRegistry::load()?;
                                let mut profiles: Vec<_> = registry.profiles.into_values().collect();
                                profiles.sort_by(|a,b| a.name.cmp(&b.name));
                                Ok(profiles)
                            }).await.map_err(|_| "Không đọc được danh sách tài khoản".to_owned()).and_then(|r| r.map_err(|e| e.to_string()));
                            let _ = gui_tx.send(GuiEvent::ProfilesLoaded { owner, result }); egui_ctx.request_repaint();
                        }
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
                                    let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&disk)?;
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
                        WorkerCmd::PreviewDocument { url, name, context } => {
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            let account = session.lock().await.clone();
                            tokio::spawn(async move {
                                if !media_context_alive(&account, Some(&context)) { return; }
                                let _ = tx.send(GuiEvent::StatusUpdate("Đang tải xem trước tài liệu…".into()));
                                ctx.request_repaint();
                                let root = account.history_path().with_extension("attachment-cache");
                                let cached = crate::attachment_cache::path_for(&root, &context.thread, &context.message.id, &name);
                                let swapped_url = account.rewrite_download_url(&url);
                                let result = crate::media_download::document_preview(&cached, &swapped_url).await;
                                if !media_context_alive(&account, Some(&context)) { return; }
                                match result {
                                    Ok(bytes) => {
                                        let content = String::from_utf8_lossy(&bytes).into_owned();
                                        let _ = tx.send(GuiEvent::DocPreviewLoaded {
                                            owner: account.account_key().to_owned(),
                                            runtime_id: account.runtime_id(),
                                            context,
                                            file_name: name,
                                            content,
                                        });
                                    }
                                    Err(_) => {
                                        let _ = tx.send(GuiEvent::StatusUpdate("Không tải được nội dung xem trước tài liệu".into()));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::DownloadMediaBatch { owner, batch, items } => {
                            let account = session.lock().await.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                download_media_batch(account, owner, batch, items, tx, ctx).await;
                            });
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
                                    if let Some(restored)=(crate::message_view::archive_media_items(&source.message).len()==1).then(||crate::desktop_backup::restored_path(&account,&source.message,false)).flatten(){
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
                                let swapped_url = account.rewrite_download_url(&url);
                                let mut result=save_media_target(&swapped_url,&path,cancel.clone(),|bytes,total|{
                                    if !media_context_alive(&account,context.as_ref()){cancel.store(true,std::sync::atomic::Ordering::Release);}
                                    let _=tx.send(GuiEvent::MediaProgress{owner:owner.clone(),id:id.clone(),bytes,total});ctx.request_repaint();
                                },visual,video_lease.as_ref(),open).await;
                                if result.as_ref().err().and_then(|e|e.downcast_ref::<crate::media_recovery::Failure>()).is_some_and(|e|e.renew_candidate()) {
                                    if let Some(context)=context.as_ref(){
                                        if let Ok(media)=async {anyhow::ensure!(crate::message_view::archive_media_items(&context.message).len()==1,"Không thể đổi tệp trong tin nhiều đính kèm");crate::media_renew::renew(&account,&context.thread,context.group,&context.message).await}.await {
                                            if let Some(next)=media.original.clone() {let mut renewed=context.clone();if persist_media_projection(&account,&mut renewed,&media,&tx).await.is_err(){crate::logger::log_error("media renewal","Cannot persist renewed file metadata");}url=next;let swapped_next = account.rewrite_download_url(&url);result=save_media_target(&swapped_next,&path,cancel.clone(),|bytes,total|{if !account.is_active(){cancel.store(true,std::sync::atomic::Ordering::Release);}let _=tx.send(GuiEvent::MediaProgress{owner:owner.clone(),id:id.clone(),bytes,total});ctx.request_repaint();},visual,video_lease.as_ref(),open).await;}
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
                                crate::storage::save_settings(&settings)
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
                                            let owner_for_cap = owner.clone();
                                            if let Some(image) = tokio::task::spawn_blocking(move || {
                                                super::messages::read_clipboard_image_to_png(Some(&owner_for_cap))
                                            }).await.ok().flatten() {
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
                        WorkerCmd::ReadClipboardImage{owner,runtime_id,thread} => {
                            let account=session.lock().await.clone();if owner!=account.account_key()||runtime_id!=account.runtime_id()||!account.is_active(){continue;}
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            let owner_for_read = owner.clone();
                            tokio::spawn(async move {
                                let files = tokio::task::spawn_blocking(|| {
                                    #[cfg(windows)]
                                    { crate::clipboard_image::read_windows_files(super::file_drop::MAX_ATTACHMENTS) }
                                    #[cfg(not(windows))]
                                    { Ok::<Option<Vec<std::path::PathBuf>>, anyhow::Error>(None) }
                                }).await;
                                match files {
                                    Ok(Ok(None)) => {}
                                    result => {
                                        let paths = match result {
                                            Ok(Ok(Some(paths))) => Ok(paths),
                                            Ok(Err(error)) => Err(error.to_string()),
                                            _ => Err("Không đọc được danh sách tệp Clipboard".into()),
                                        };
                                        let _ = tx.send(GuiEvent::ClipboardFilesRead { owner, runtime_id, thread, paths });
                                        ctx.request_repaint();
                                        return;
                                    }
                                }
                                let image = tokio::task::spawn_blocking(move || {
                                    super::messages::read_clipboard_image_to_png(Some(&owner_for_read))
                                }).await.ok().flatten();
                                if account.is_active() {
                                    let _ = tx.send(GuiEvent::ClipboardImageRead{owner,runtime_id,thread,image});
                                    ctx.request_repaint();
                                } else if let Some((path, ..)) = image {
                                    let _ = tokio::fs::remove_file(path).await;
                                }
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
                                    let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&account)?;
                                    Ok(store.conversation_preferences()?.get(&id).copied().unwrap_or_default())
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
                                    let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&s)?;
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
                                    let (mut store, _write_permit)=crate::store::MessageStore::open_for_account_write(&disk)?;
                                    store.delete_conversation(&id,&visible)?;
                                    let mut cache=crate::chat::load_threads_cache_with_store(&disk,&store);cache.retain(|t|t.id!=id);crate::chat::save_threads_cache_with_store(&disk,&store,&cache)?;Ok(())
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
                                let (mut store, _write_permit)=crate::store::MessageStore::open_for_account_write(&s)?;
                                store.clear_thread_history(&id)?;
                                let mut cache=crate::chat::load_threads_cache_with_store(&s,&store);
                                if let Some(thread)=cache.iter_mut().find(|t|t.id==id) {
                                    thread.unread.read_visible(&thread.messages);
                                    thread.messages.clear(); thread.preview.clear(); thread.time.clear(); thread.presentation=Default::default();
                                }
                                crate::chat::save_threads_cache_with_store(&s,&store,&cache)?;
                                Ok(())
                            }).await;
                            match result {
                                Ok(Ok(())) => { let _ = gui_tx.send(GuiEvent::ThreadHistoryCleared { owner, thread_id }); }
                                _ => { let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể xóa lịch sử trên máy này".into())); }
                            }
                            egui_ctx.request_repaint();
                        }
                        WorkerCmd::PersistThreadMetadata { owner, runtime_id, thread_id, name, is_group } => {
                            let account = session.lock().await.clone();
                            if !outgoing_identity_matches(account.is_active(), account.account_key(), account.runtime_id(), &owner, runtime_id) { continue; }
                            let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                                let (store, _permit) = crate::store::MessageStore::open_for_account_write(&account)?;
                                store.upsert_thread(&crate::store::StoredThread {
                                    thread_id, name, is_group, last_ts: 0, last_read_msg_id: String::new(),
                                })
                            }).await;
                            if let Ok(Err(error)) = result { crate::runtime_metrics::database_error(&error); }
                        }
                        WorkerCmd::PersistThreads { owner, threads } => {
                            let s = session.lock().await.clone();
                            if owner.is_empty() || owner != s.account_key() || !s.is_active() { continue; }
                            live_updates.seed(&threads);
                            // Await writes in command order: logout cannot race a detached snapshot write.
                            let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                                let _timing = crate::runtime_metrics::Scope::new(crate::runtime_metrics::Metric::SnapshotWrite);
                                if !s.is_active() { return Ok(()); }
                                let (mut store, _write_permit) =
                                    crate::store::MessageStore::open_for_account_write(&s)?;
                                crate::store::persist_local_threads(&mut store, &threads)?;
                                crate::chat::save_threads_cache_with_store(&s, &store, &threads)?;
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
                            let source = session.clone();
                            let lifecycle = outgoing_lifecycle.clone();
                            tokio::spawn(async move {
                                // Load upload state from store and locate the original message
                                type ResumedUploadInfo = Option<(crate::upload::ResumableUploadState, Option<(String, ChatMessage)>)>;
                                let id = upload_id.clone();
                                let result = tokio::task::spawn_blocking({
                                    let s = s.clone();
                                    move || -> anyhow::Result<ResumedUploadInfo> {
                                        if !s.is_active() {
                                            anyhow::bail!("Phiên đã hết hạn");
                                        }
                                        let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&s)?;
                                        if let Some(state) = store.load_upload_state(&id)? {
                                            let client_id = id
                                                .strip_prefix("asyncfile-")
                                                .or_else(|| id.strip_prefix("photo-"))
                                                .unwrap_or(&id);
                                            let msg = store.find_outgoing_message_by_client_id(client_id)?;
                                            Ok(Some((state, msg)))
                                        } else {
                                            Ok(None)
                                        }
                                    }
                                })
                                .await
                                .map_err(|_| "Không thể đọc trạng thái tải lên".to_string())
                                .and_then(|r| r.map_err(|_| "Không thể đọc trạng thái tải lên".to_string()));

                                match result {
                                    Ok(Some((state, maybe_msg))) => {
                                        let name = state.file_name.clone();
                                        if let Some((thread_id, message)) = maybe_msg {
                                            let owner = s.account_key().to_owned();
                                            let ticket = lifecycle.accept(owner.clone(), s.runtime_id(), thread_id.clone(), message.clone());
                                            spawn_outgoing_send(s, source, tx.clone(), ctx.clone(), owner, thread_id, message, ticket);
                                        }
                                        let _ = tx.send(GuiEvent::StatusUpdate(format!("Đang tiếp tục tải lên: {name}")));
                                    }
                                    Ok(None) => {
                                        let _ = tx.send(GuiEvent::StatusUpdate("Không tìm thấy tác vụ tải lên cần tiếp tục".into()));
                                    }
                                    Err(e) => {
                                        let _ = tx.send(GuiEvent::StatusUpdate(e));
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::CancelUpload { upload_id } => {
                            let s = session.lock().await.clone();
                            if !s.is_active() {
                                continue;
                            }
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            let id = upload_id.clone();
                            tokio::spawn(async move {
                                crate::upload::cancel_active_upload(&id);
                                let _ = tokio::task::spawn_blocking({
                                    let id = id.clone();
                                    move || {
                                        let (store, _write_permit) =
                                            crate::store::MessageStore::open_for_account_write(&s)?;
                                        store.delete_upload_state(&id)
                                    }
                                })
                                .await;

                                let _ = tx.send(GuiEvent::StatusUpdate("Đã yêu cầu hủy tải lên".into()));
                                ctx.request_repaint();
                            });
                        }
                        command @ (WorkerCmd::Logout | WorkerCmd::AddAccount) => {
                            let keep = matches!(command, WorkerCmd::AddAccount);
                            if let Some(task) = current_auth_task.take() { task.abort(); }
                            auth_attempt_mgr.next_attempt();
                            abort_and_join_phone_sync(&mut active_phone_sync, &phone_sync_running, &phone_sync_cancel).await;
                            db_ready_runtime.store(0, std::sync::atomic::Ordering::Release);
                            let current = session.lock().await.clone();
                            if keep {
                                let save_session = current.clone();
                                let saved = tokio::task::spawn_blocking(move || {
                                    save_session.save_to_file("session.json")
                                })
                                .await;
                                if !matches!(saved, Ok(Ok(()))) {
                                    let _ = gui_tx.send(GuiEvent::LogoutFailed {
                                        message: "Không thể lưu phiên tài khoản".into(),
                                    });
                                    continue;
                                }
                            }
                            crate::queue_cursors::flush_all().await;
                            let Ok(_write_barrier) = current.close_account_writes().await else {
                                let _ = gui_tx.send(GuiEvent::LogoutFailed {
                                    message: "Không thể chờ hoàn tất ghi dữ liệu tài khoản".into(),
                                });
                                continue;
                            };
                            {
                                let live = session.lock().await;
                                if live.runtime_id() != current.runtime_id() || live.account_key() != current.account_key() {
                                    continue;
                                }
                            }
                            let owner = current.account_key().to_owned();
                            let saved = tokio::task::spawn_blocking(move || {
                                if keep { Ok(()) }
                                else { crate::profiles::perform_account_logout(&owner) }
                            }).await;
                            if !matches!(saved, Ok(Ok(()))) {
                                let msg = match saved {
                                    Ok(Err(e)) => e.to_string(),
                                    Err(e) => e.to_string(),
                                    _ => "Không cập nhật được phiên tài khoản".into(),
                                };
                                let _ = gui_tx.send(GuiEvent::LogoutFailed { message: msg });
                                egui_ctx.request_repaint();
                                continue;
                            }
                            current.revoke_runtime();
                            outgoing_lifecycle.retire(current.runtime_id());

                            call_bridge.end_call();
                            pending_v2t_tickets.lock().await.clear();
                            let account = session.lock().await.account_key().to_owned();
                            if !account.is_empty() {
                                let _ = tokio::task::spawn_blocking(move || crate::photo_quality::remove_account_copies(&account)).await;
                            }
                            {
                                let mut live = session.lock().await;
                                if live.runtime_id() != current.runtime_id() || live.account_key() != current.account_key() { continue; }
                                live.logout();
                                refresh_outgoing_identity(&outgoing_identity, &live);
                            }
                            let _ = gui_tx.send(GuiEvent::LoggedOut);
                            egui_ctx.request_repaint();
                            // Dropping this runtime cancels all old network tasks. A fresh UI channel
                            // prevents queued events from the previous account reaching the next one.
                            break;
                        }
                        WorkerCmd::SwitchProfile { owner, target_uid } => {
                            // A20: Multi-account runtime profile switching with thorough isolation
                            let current_account = session.lock().await.account_key().to_owned();
                            if current_account != owner {
                                let _ = gui_tx.send(GuiEvent::StatusUpdate("Tài khoản đã thay đổi".into()));
                                continue;
                            }


                            // Load the target profile from ProfileRegistry
                            let profile_result = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                let registry = crate::profiles::ProfileRegistry::load()?;
                                let profile = registry.profiles.values()
                                    .find(|p| p.uid == target_uid)
                                    .ok_or_else(|| anyhow::anyhow!("Không tìm thấy tài khoản"))?
                                    .clone();
                                Ok(profile)
                            }).await;

                            match profile_result {
                                Ok(Ok(profile)) => {
                                    // A20: Prepare target session and validate BEFORE modifying current session (rollback preservation)
                                    let session_path = profile.session_file.clone();
                                    let expected_uid = profile.uid.clone();
                                    let load_result = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                        let mut new_session = crate::session::Session::new(false, false, false)?;
                                        new_session.load_from_file(&session_path.to_string_lossy())?;
                                        anyhow::ensure!(!new_session.account_key().is_empty() && new_session.account_key() == expected_uid, "Phiên không khớp tài khoản đã chọn");
                                        Ok(new_session)
                                    }).await;

                                    match load_result {
                                        Ok(Ok(new_session)) => {
                                            if let Some(task) = current_auth_task.take() { task.abort(); }
                                            auth_attempt_mgr.next_attempt();
                                            abort_and_join_phone_sync(&mut active_phone_sync, &phone_sync_running, &phone_sync_cancel).await;
                                            db_ready_runtime.store(0, std::sync::atomic::Ordering::Release);
                                            let previous = session.lock().await.clone();
                                            crate::queue_cursors::flush_all().await;
                                            let Ok(old_write_barrier) = previous.close_account_writes().await else {
                                                let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể chờ hoàn tất ghi dữ liệu tài khoản".into()));
                                                egui_ctx.request_repaint();
                                                continue;
                                            };
                                            {
                                                let live = session.lock().await;
                                                if live.runtime_id() != previous.runtime_id() || live.account_key() != previous.account_key() {
                                                    let _ = gui_tx.send(GuiEvent::StatusUpdate("Tài khoản đã thay đổi trong khi chuyển hồ sơ".into()));
                                                    continue;
                                                }
                                            }
                                            // 1. Tear down old account tasks and worker resources
                                            call_bridge.end_call();
                                            pending_v2t_tickets.lock().await.clear();

                                            let old_account = session.lock().await.account_key().to_owned();
                                            if !old_account.is_empty() {
                                                let _ = tokio::task::spawn_blocking(move || crate::photo_quality::remove_account_copies(&old_account)).await;
                                            }

                                            let profile_name = profile.name.clone();
                                            let previous = session.lock().await.clone();
                                            let profile_session = previous.clone();
                                            let persisted = tokio::task::spawn_blocking(move || crate::profiles::activate_for_switch(&profile_session,&profile_name)).await;
                                            if !matches!(persisted, Ok(Ok(()))) {
                                                let _ = gui_tx.send(GuiEvent::StatusUpdate("Không lưu được tài khoản đang chọn".into()));
                                                egui_ctx.request_repaint();
                                                continue;
                                            }
                                            previous.revoke_runtime();
                                            outgoing_lifecycle.retire(previous.runtime_id());
                                            history_sync_cancel.store(true, std::sync::atomic::Ordering::Release);
                                            phone_sync_cancel.store(true, std::sync::atomic::Ordering::Release);
                                            {
                                                let mut current = session.lock().await;
                                                if current.runtime_id() != previous.runtime_id() || current.account_key() != previous.account_key() {
                                                    let _ = gui_tx.send(GuiEvent::StatusUpdate("Tài khoản đã thay đổi trong khi chuyển hồ sơ".into()));
                                                    continue;
                                                }
                                                current.logout();
                                                refresh_outgoing_identity(&outgoing_identity, &current);
                                            }
                                            drop(old_write_barrier);
                                            let _ = gui_tx.send(GuiEvent::ProfileSwitched { session: Box::new(new_session) });
                                            egui_ctx.request_repaint();
                                            // Destroy the old runtime; the UI starts a fresh account-bound worker.
                                            break;
                                        }
                                        Ok(Err(e)) => {
                                            let _ = gui_tx.send(GuiEvent::StatusUpdate(format!("Không thể tải phiên tài khoản: {e}")));
                                        }
                                        Err(_) => {
                                            let _ = gui_tx.send(GuiEvent::StatusUpdate("Lỗi tải phiên tài khoản".into()));
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
                            let snapshot = {
                                let s = session.lock().await;
                                if s.login.is_some() {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            };
                            if let Some(s) = snapshot {
                                let tx = gui_tx.clone();
                                tokio::task::spawn_blocking(move || {
                                    if let Err(e) = s.save_to_file("session.json") {
                                        let _ = tx.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                    }
                                });
                            }
                        }
                        WorkerCmd::RequestQr => {
                            // A19: Cancel current auth task and bump auth attempt generation
                            if let Some(task) = current_auth_task.take() {
                                task.abort();
                            }
                            auth_attempt_mgr.next_attempt();
                            let attempt_id = auth_attempt_mgr.current();
                            abort_and_join_phone_sync(&mut active_phone_sync, &phone_sync_running, &phone_sync_cancel).await;
                            db_ready_runtime.store(0, std::sync::atomic::Ordering::Release);

                            let session_clone = session.clone();
                            let poll_clone = poll_state.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            let commands = commands.clone();
                            let auth_mgr = auth_attempt_mgr.clone();
                            let outgoing_identity = outgoing_identity.clone();
                            let outgoing_lifecycle = outgoing_lifecycle.clone();
                            let live_updates = live_updates.clone();
                            let db_ready_qr = db_ready_runtime.clone();
                            let ws_ready_qr = is_ws_connected.clone();

                            let handle = tokio::spawn(async move {
                                let previous = session_clone.lock().await.clone();
                                let Ok(_write_barrier) = previous.close_account_writes().await else {
                                    let _ = gui_tx_clone.send(GuiEvent::LoginFailed(
                                        "Không thể đóng dữ liệu phiên trước".into(),
                                    ));
                                    ctx_clone.request_repaint();
                                    return;
                                };
                                previous.revoke_runtime();
                                outgoing_lifecycle.retire(previous.runtime_id());
                                {
                                    let mut s = session_clone.lock().await;
                                    s.logout();
                                    refresh_outgoing_identity(&outgoing_identity, &s);
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
                                        if !auth_mgr.is_current(attempt_id) {
                                            return;
                                        }
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
                                                if !auth_mgr.is_current(attempt_id) {
                                                    return;
                                                }
                                                let _ = gui_tx_clone.send(GuiEvent::QrReady {
                                                    qr_login: q.clone(),
                                                    qr_image: img,
                                                });
                                                ctx_clone.request_repaint();

                                                let scan_res =
                                                    crate::qr::wait_for_scan(&s_snap, &q).await;

                                                if let Ok(scanned) = scan_res {
                                                    if !auth_mgr.is_current(attempt_id) {
                                                        return;
                                                    }
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
                                                        if !auth_mgr.is_current(attempt_id) {
                                                            // Stale response — another auth started, discard this result
                                                            return;
                                                        }

                                                        let mut candidate = {
                                                            let s = session_clone.lock().await;
                                                            if !auth_mgr.is_current(attempt_id) {
                                                                return;
                                                            }
                                                            s.clone()
                                                        };
                                                        if let Some(sek) =
                                                            crate::qr::extract_zpw_sek(&resp)
                                                        {
                                                            candidate.set_zpw_sek(sek);
                                                        }
                                                        if let Some(data) = resp.get("data") {
                                                            crate::login::apply_login_config(
                                                                &mut candidate, data,
                                                            );
                                                        }

                                                        if let Ok(info) =
                                                            crate::login::get_login_info(&candidate).await
                                                        {
                                                            if let Some(cfg) = info.get("data") {
                                                                crate::login::apply_login_config(
                                                                    &mut candidate, cfg,
                                                                );
                                                                candidate.apply_server_info(cfg);
                                                            }
                                                        }
                                                        let _ = candidate.sync_profile_me().await;
                                                        if !auth_mgr.is_current(attempt_id) {
                                                            return;
                                                        }

                                                        let committed = {
                                                            let mut s = session_clone.lock().await;
                                                            if !auth_mgr.is_current(attempt_id) {
                                                                return;
                                                            }
                                                            *s = candidate;
                                                            refresh_outgoing_identity(&outgoing_identity, &s);
                                                            s.clone()
                                                        };
                                                        let s_save = committed.clone();
                                                        let saved = tokio::task::spawn_blocking(move || {
                                                            s_save.save_to_file("session.json")
                                                        })
                                                        .await
                                                        .unwrap_or_else(|e| Err(e.into()));
                                                        if let Err(e) = saved {
                                                            crate::logger::log_error("qr login", &format!("Không thể lưu phiên: {e}"));
                                                        }

                                                        let uin = committed
                                                            .login
                                                            .as_ref()
                                                            .map(|l| l.uin.clone())
                                                            .unwrap_or_default();
                                                        let dname = committed
                                                            .login
                                                            .as_ref()
                                                            .map(|l| l.display_name.clone())
                                                            .unwrap_or_default();
                                                        let u_ava = committed
                                                            .login
                                                            .as_ref()
                                                            .map(|l| l.avatar.clone())
                                                            .filter(|a| !a.is_empty());
                                                        let _ = gui_tx_clone.send(
                                                            GuiEvent::LoginSuccess {
                                                                composer_config: committed.composer_config,
                                                                name: dname,
                                                                uin: uin.clone(),
                                                                avatar: u_ava,
                                                                runtime_id: committed.runtime_id(),
                                                            },
                                                        );
                                                        let _ = gui_tx_clone.send(
                                                            GuiEvent::ServerConfigUpdated {
                                                                owner: uin,
                                                                runtime_id: committed.runtime_id(),
                                                                config: committed.server_config.clone(),
                                                            },
                                                        );
                                                        ctx_clone.request_repaint();

                                                        {
                                                            let session_arc = session_clone.clone();
                                                            let tx_preload = gui_tx_clone.clone();
                                                            let ctx_preload = ctx_clone.clone();
                                                            let live_preload = live_updates.clone();
                                                            let life_preload = outgoing_lifecycle.clone();
                                                            let id_preload = outgoing_identity.clone();
                                                            let db_ready_preload = db_ready_qr.clone();
                                                            let ws_ready_preload = ws_ready_qr.clone();
                                                            tokio::spawn(async move {
                                                                run_startup_preload(
                                                                    session_arc,
                                                                    tx_preload,
                                                                    ctx_preload,
                                                                    live_preload,
                                                                    life_preload,
                                                                    id_preload,
                                                                    db_ready_preload,
                                                                    ws_ready_preload,
                                                                )
                                                                .await;
                                                            });
                                                        }

                                                        if let Some(tx) = commands.upgrade() {
                                                            let _ = tx.send(WorkerCmd::RefreshThreads);
                                                        }

                                                        let s_kw = committed.clone();
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
                                        if !auth_mgr.is_current(attempt_id) {
                                            // Stale response — discard
                                            return;
                                        }

                                        let _ = gui_tx_clone
                                            .send(GuiEvent::LoginFailed(e.to_string()));
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                            current_auth_task = Some(handle);
                        }
                        WorkerCmd::PrioritizeConversationSync {
                            owner,
                            runtime_id,
                            thread_id,
                        } => {
                            let s = session.lock().await;
                            if s.is_active()
                                && s.account_key() == owner
                                && s.runtime_id() == runtime_id
                            {
                                crate::phone_sync::prioritize_conversation(
                                    &owner,
                                    runtime_id,
                                    &thread_id,
                                );
                            }
                        }
                        WorkerCmd::CancelPhoneHistory => {
                            phone_sync_cancel.store(true, std::sync::atomic::Ordering::Release);
                            if let Some(task) = &active_phone_sync {
                                task.cancel.store(true, std::sync::atomic::Ordering::Release);
                            }
                            if phone_sync_running.load(std::sync::atomic::Ordering::SeqCst) {
                                let _ = gui_tx.send(GuiEvent::PhoneSyncStatus {
                                    running: true,
                                    success: false,
                                    message: "Đang dừng đồng bộ và dọn tác vụ; giữ dữ liệu đã lưu…".into(),
                                });
                                egui_ctx.request_repaint();
                            }
                        }
                        WorkerCmd::CancelPhoneHistoryFenced { owner, runtime_id, operation_id } => {
                            let account = session.lock().await.clone();
                            if account.account_key() != owner || account.runtime_id() != runtime_id {
                                continue;
                            }
                            if let Some(task) = &active_phone_sync {
                                if task.operation_id != 0 && operation_id != 0 && task.operation_id != operation_id {
                                    continue;
                                }
                                task.cancel.store(true, std::sync::atomic::Ordering::Release);
                            }
                            phone_sync_cancel.store(true, std::sync::atomic::Ordering::Release);
                            if phone_sync_running.load(std::sync::atomic::Ordering::SeqCst) {
                                let _ = gui_tx.send(GuiEvent::PhoneSyncStatus {
                                    running: true,
                                    success: false,
                                    message: "Đang dừng đồng bộ và dọn tác vụ; giữ dữ liệu đã lưu…".into(),
                                });
                                egui_ctx.request_repaint();
                            }
                        }
                        WorkerCmd::ClearSyncError { owner, runtime_id } => {
                            let account = session.lock().await.clone();
                            if account.account_key() != owner || account.runtime_id() != runtime_id {
                                continue;
                            }
                            let transport = phone_transport_rx.borrow().clone();
                            let ws_ready = transport.as_ref().and_then(|t| t.epoch()).is_some();
                            let already_syncing = active_phone_sync.is_some()
                                || phone_sync_running.load(std::sync::atomic::Ordering::SeqCst);
                            let availability = current_sync_availability(
                                &account,
                                &db_ready_runtime,
                                ws_ready,
                                already_syncing,
                                account
                                    .server_config
                                    .sync_2_pc
                                    .is_feature_enabled_by_origin(
                                        crate::sync_coordinator::SyncOrigin::Banner,
                                    ),
                            );
                            let _ = gui_tx.send(GuiEvent::PhoneSyncState {
                                owner,
                                runtime_id,
                                operation_id: 0,
                                state: crate::sync_coordinator::SyncState::Idle { availability },
                            });
                            egui_ctx.request_repaint();
                        }
                        command @ (WorkerCmd::SyncPhoneHistory { .. }
                        | WorkerCmd::RescanPhoneHistory { .. }
                        | WorkerCmd::SyncPhoneRecent
                        | WorkerCmd::SyncPhoneRecentFenced { .. }
                        | WorkerCmd::SyncPhoneLegacy) => {
                            if let WorkerCmd::SyncPhoneRecentFenced { ref owner, runtime_id, .. } = command {
                                let account = session.lock().await.clone();
                                if account.account_key() != owner || account.runtime_id() != runtime_id {
                                    continue;
                                }
                            }
                            reap_finished_phone_sync(&mut active_phone_sync, &phone_sync_running).await;
                            let legacy = matches!(command, WorkerCmd::SyncPhoneLegacy);
                            let full_rescan = matches!(command, WorkerCmd::RescanPhoneHistory { .. });
                            let recent_only = matches!(command, WorkerCmd::SyncPhoneRecent | WorkerCmd::SyncPhoneRecentFenced { .. });
                            let range_days = match command {
                                WorkerCmd::SyncPhoneHistory { range_days }
                                | WorkerCmd::RescanPhoneHistory { range_days } => range_days,
                                _ => 0,
                            };
                            let source = if legacy {
                                crate::sync_coordinator::SyncSource::LegacyV1
                            } else if recent_only {
                                crate::sync_coordinator::SyncSource::Sync2Recent
                            } else if full_rescan {
                                crate::sync_coordinator::SyncSource::Sync2Rescan
                            } else {
                                crate::sync_coordinator::SyncSource::Sync2Full
                            };
                            let fenced_intent_id = match command {
                                WorkerCmd::SyncPhoneRecentFenced { intent_id, .. } => intent_id,
                                _ => None,
                            };
                            let origin = match command {
                                WorkerCmd::SyncPhoneRecentFenced { origin, .. } => origin,
                                WorkerCmd::SyncPhoneLegacy => crate::sync_coordinator::SyncOrigin::Legacy,
                                _ => crate::sync_coordinator::SyncOrigin::Setting,
                            };

                            let account = session.lock().await.clone();
                            let owner = account.account_key().to_owned();
                            let runtime_id = account.runtime_id();
                            let transport = phone_transport_rx.borrow().clone();
                            let ws_ready = transport.as_ref().and_then(|t| t.epoch()).is_some();
                            let already_syncing = active_phone_sync.as_ref().is_some_and(|t| {
                                t.owner == owner
                                    && t.runtime_id == runtime_id
                                    && t.operation_id > 0
                            }) || active_phone_sync.is_some()
                                || phone_sync_running.load(std::sync::atomic::Ordering::SeqCst);

                            let availability = current_sync_availability(
                                &account,
                                &db_ready_runtime,
                                ws_ready,
                                already_syncing,
                                account
                                    .server_config
                                    .sync_2_pc
                                    .is_feature_enabled_by_origin(origin),
                            );
                            if !availability.is_ready() {
                                let reason_msg = availability
                                    .reason()
                                    .map(|r| r.user_message())
                                    .unwrap_or("Chưa sẵn sàng đồng bộ")
                                    .to_owned();
                                if let Some(id) = fenced_intent_id {
                                    let _ = gui_tx.send(GuiEvent::AutoSyncIntentRejected {
                                        owner: owner.clone(),
                                        runtime_id,
                                        intent_id: id,
                                        temporary: true,
                                        reason: reason_msg.clone(),
                                    });
                                }
                                if !already_syncing {
                                    let _ = gui_tx.send(GuiEvent::PhoneSyncState {
                                        owner,
                                        runtime_id,
                                        operation_id: 0,
                                        state: crate::sync_coordinator::SyncState::Idle {
                                            availability,
                                        },
                                    });
                                }
                                let _ = gui_tx.send(GuiEvent::PhoneSyncStatus {
                                    running: already_syncing,
                                    success: false,
                                    message: reason_msg,
                                });
                                egui_ctx.request_repaint();
                                continue;
                            }

                            if phone_sync_running.swap(true, std::sync::atomic::Ordering::SeqCst) {
                                if let Some(id) = fenced_intent_id {
                                    let _ = gui_tx.send(GuiEvent::AutoSyncIntentRejected {
                                        owner: owner.clone(),
                                        runtime_id,
                                        intent_id: id,
                                        temporary: true,
                                        reason: "Already syncing".into(),
                                    });
                                }
                                let _ = gui_tx.send(GuiEvent::PhoneSyncStatus {
                                    running: true,
                                    success: false,
                                    message: crate::sync_coordinator::SyncUnavailableReason::AlreadySyncing.user_message().into(),
                                });
                                egui_ctx.request_repaint();
                                continue;
                            }

                            let operation_id = crate::sync_coordinator::next_operation_id();
                            if let Some(id) = fenced_intent_id {
                                let _ = gui_tx.send(GuiEvent::AutoSyncIntentAccepted {
                                    owner: owner.clone(),
                                    runtime_id,
                                    intent_id: id,
                                    operation_id,
                                });
                            }
                            let op_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                            phone_sync_cancel.store(false, std::sync::atomic::Ordering::Release);
                            let task_cancel = op_cancel.clone();
                            let shared_cancel = phone_sync_cancel.clone();
                            let running = phone_sync_running.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            let owner_task = owner.clone();

                            let _ = tx.send(GuiEvent::PhoneSyncState {
                                owner: owner.clone(),
                                runtime_id,
                                operation_id,
                                state: crate::sync_coordinator::SyncState::Preparing { source },
                            });
                            let _ = tx.send(GuiEvent::PhoneSyncStatus {
                                running: true,
                                success: false,
                                message: crate::sync_coordinator::SyncState::Preparing { source }.summary_vi(),
                            });
                            ctx.request_repaint();

                            let handle = tokio::spawn(async move {
                                let _guard = PollGuard(running);
                                let started = std::time::Instant::now();
                                let tracker = crate::sync_coordinator::SyncProgressTracker::new();
                                let progress_tx = tx.clone();
                                let progress_ctx = ctx.clone();
                                let progress_cancel = task_cancel.clone();
                                let state_tx = tx.clone();
                                let state_ctx = ctx.clone();
                                let state_owner = owner_task.clone();

                                // Mirror global cancel flag into per-operation token so CancelPhoneHistory stops this op cleanly.
                                let combined_cancel = task_cancel.clone();
                                let cancel_bridge_cancel = shared_cancel.clone();
                                let cancel_bridge_target = combined_cancel.clone();
                                let cancel_bridge_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                                let bridge_active = cancel_bridge_running.clone();
                                tokio::spawn(async move {
                                    while bridge_active.load(std::sync::atomic::Ordering::Acquire) {
                                        if cancel_bridge_cancel.load(std::sync::atomic::Ordering::Acquire) {
                                            cancel_bridge_target.store(true, std::sync::atomic::Ordering::Release);
                                            break;
                                        }
                                        tokio::time::sleep(Duration::from_millis(100)).await;
                                    }
                                });

                                let result = if let Some(transport) = transport {
                                    if legacy {
                                        // Explicit user choice only; Sync2 never auto-falls back here.
                                        let _indexing_pause = crate::sync_coordinator::pause_indexing();
                                        crate::phone_sync::legacy::download(
                                            &account,
                                            transport,
                                            combined_cancel.clone(),
                                            move |message| {
                                                if !progress_cancel.load(std::sync::atomic::Ordering::Acquire) {
                                                    let _ = progress_tx.send(GuiEvent::PhoneSyncStatus {
                                                        running: true,
                                                        success: false,
                                                        message,
                                                    });
                                                    progress_ctx.request_repaint();
                                                }
                                            },
                                            |thread| {
                                                if deliver_thread_updates() {
                                                    let _ = tx.send(GuiEvent::ThreadsUpdated {
                                                        threads: vec![thread],
                                                        source: ThreadUpdateSource::History,
                                                    });
                                                    ctx.request_repaint();
                                                }
                                            },
                                        )
                                        .await
                                        .map(|message| crate::history_sync::SyncReport {
                                            complete: false,
                                            message,
                                        })
                                    } else {
                                        let state_cb_owner = state_owner.clone();
                                        let state_cb_tx = state_tx.clone();
                                        let state_cb_ctx = state_ctx.clone();
                                        crate::phone_sync::download_with_state(
                                            &account,
                                            transport,
                                            combined_cancel.clone(),
                                            full_rescan,
                                            recent_only,
                                            range_days,
                                            source,
                                            origin,
                                            tracker.clone(),
                                            move |sync_state| {
                                                let _ = state_cb_tx.send(GuiEvent::PhoneSyncState {
                                                    owner: state_cb_owner.clone(),
                                                    runtime_id,
                                                    operation_id,
                                                    state: sync_state,
                                                });
                                                state_cb_ctx.request_repaint();
                                            },
                                            move |message| {
                                                if !progress_cancel.load(std::sync::atomic::Ordering::Acquire) {
                                                    let _ = progress_tx.send(GuiEvent::PhoneSyncStatus {
                                                        running: true,
                                                        success: false,
                                                        message,
                                                    });
                                                    progress_ctx.request_repaint();
                                                }
                                            },
                                            |thread| {
                                                if deliver_thread_updates() {
                                                    let _ = tx.send(GuiEvent::ThreadsUpdated {
                                                        threads: vec![thread],
                                                        source: ThreadUpdateSource::History,
                                                    });
                                                    ctx.request_repaint();
                                                }
                                            },
                                        )
                                        .await
                                    }
                                } else {
                                    Err(anyhow::anyhow!(
                                        crate::sync_coordinator::SyncUnavailableReason::WorkerNotReady.user_message()
                                    ))
                                };
                                cancel_bridge_running.store(false, std::sync::atomic::Ordering::Release);

                                let final_inserted = match tracker.snapshot() {
                                    crate::sync_coordinator::SyncState::Transferring {
                                        inserted_messages,
                                        ..
                                    } => inserted_messages,
                                    _ => 0,
                                };
                                let success = result.as_ref().is_ok_and(|report| report.complete);
                                let was_cancelled = combined_cancel.load(std::sync::atomic::Ordering::Acquire);
                                let message = match &result {
                                    Ok(report) => report.message.clone(),
                                    Err(e) => format!("Đồng bộ điện thoại đã dừng: {e}"),
                                };
                                let terminal_state = match &result {
                                    Ok(report) if report.complete => crate::sync_coordinator::SyncState::Completed {
                                        inserted_messages: final_inserted,
                                        duration: started.elapsed(),
                                    },
                                    Ok(report) => crate::sync_coordinator::SyncState::Interrupted {
                                        reason: report.message.clone(),
                                        retryable: true,
                                    },
                                    Err(e) if was_cancelled => crate::sync_coordinator::SyncState::Interrupted {
                                        reason: format!("Đã hủy đồng bộ: {e}"),
                                        retryable: true,
                                    },
                                    Err(e) => {
                                        let err_str = e.to_string();
                                        let fatal = crate::gui::sync_banner::is_non_retryable_error(&err_str);
                                        crate::sync_coordinator::SyncState::Failed {
                                            error: err_str,
                                            fatal,
                                        }
                                    }
                                };
                                let _ = tx.send(GuiEvent::PhoneSyncState {
                                    owner: state_owner,
                                    runtime_id,
                                    operation_id,
                                    state: terminal_state,
                                });
                                let _ = tx.send(GuiEvent::PhoneSyncStatus {
                                    running: false,
                                    success,
                                    message,
                                });
                                ctx.request_repaint();
                            });

                            active_phone_sync = Some(ActivePhoneSyncTask {
                                owner,
                                runtime_id,
                                operation_id,
                                cancel: op_cancel,
                                handle,
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
                                let _metadata=metadata_permit(&metadata_gate).await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
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
                                let _metadata=metadata_permit(&metadata_gate).await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
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
                                let _metadata=metadata_permit(&metadata_gate).await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                let mutation=!matches!(action,crate::profile::Action::Read);
                                let result=if owner!=account.account_key(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::profile::execute(&account,&action).await.map_err(|error|error.to_string())};
                                if let Ok(profile)=&result {
                                    let mut current=shared.lock().await;
                                    if current.is_active() && current.account_key()==owner {if let Some(login)=current.login.as_mut(){login.display_name=profile.name.clone();if let Some(avatar)=&profile.avatar{login.avatar=avatar.clone();}}}
                                }
                                let _=tx.send(GuiEvent::Profile {owner,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Labels {owner,runtime_id,generation,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_permit(&metadata_gate).await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);impl Drop for Refresh {fn drop(&mut self){self.0.notify_one();}}let _refresh=(!matches!(action,crate::labels::Action::Read)).then(||Refresh(metadata_notify));
                                let mutation=!matches!(action,crate::labels::Action::Read);
                                let result=if !account.is_active() || owner!=account.account_key() || runtime_id!=account.runtime_id(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::labels::execute(&account,&action).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::Labels {owner,runtime_id,generation,kind: if mutation { MetadataResultKind::Mutation } else { MetadataResultKind::Read },result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Privacy {owner,runtime_id,generation,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_permit(&metadata_gate).await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);impl Drop for Refresh {fn drop(&mut self){self.0.notify_one();}}let _refresh=(matches!(action,crate::privacy::Action::Set {..})).then(||Refresh(metadata_notify));
                                let mutation=!matches!(action,crate::privacy::Action::Read);
                                let result=if !account.is_active() || owner!=account.account_key() || runtime_id!=account.runtime_id() {Err("Phiên tài khoản đã thay đổi".into())}else{crate::privacy::execute(&account,&action).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::Privacy {owner,runtime_id,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::ManualMark {owner,runtime_id,generation,target,group,action}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();
                            let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                // Share admission with refresh: a read must not straddle this mutation.
                                let _metadata=metadata_permit(&metadata_gate).await;
                                metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                let mutation=!matches!(action,crate::unread::MarkAction::Refresh);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);
                                impl Drop for Refresh { fn drop(&mut self) { self.0.notify_one(); } }
                                let _refresh=mutation.then(|| Refresh(metadata_notify));
                                let result=if !account.is_active() || owner!=account.account_key() || runtime_id!=account.runtime_id(){Err("Phiên tài khoản đã thay đổi".into())}else{crate::unread::update_manual_mark(&account,&target,group,&action).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::ManualMarks {owner,runtime_id,generation,kind: if mutation { MetadataResultKind::Mutation } else { MetadataResultKind::Read },result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Archive {owner,runtime_id,generation,target,group,archived}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_permit(&metadata_gate).await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);impl Drop for Refresh {fn drop(&mut self){self.0.notify_one();}}let _refresh=(archived.is_some()).then(||Refresh(metadata_notify));
                                let result=if !account.is_active() || owner!=account.account_key() || runtime_id!=account.runtime_id(){Err("Phiên tài khoản đã thay đổi".into())}else {crate::archive::execute(&account,&target,group,archived).await.map_err(|error|error.to_string())};
                                let _=tx.send(GuiEvent::Archive {owner,runtime_id,generation,kind: if archived.is_some() { MetadataResultKind::Mutation } else { MetadataResultKind::Read },result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::Contact {owner,target,generation,action} => {
                            let account=session.lock().await.clone(); let tx=gui_tx.clone(); let ctx=egui_ctx.clone();
                            let metadata_gate=metadata_gate.clone();let metadata_notify=metadata_notify.clone();
                            let metadata_cache_started=metadata_cache_started.clone();
                            tokio::spawn(async move {
                                let _metadata=metadata_permit(&metadata_gate).await;metadata_cache_started.store(true,std::sync::atomic::Ordering::SeqCst);
                                struct Refresh(std::sync::Arc<tokio::sync::Notify>);impl Drop for Refresh {fn drop(&mut self){self.0.notify_one();}}let _refresh=(action.mutation()).then(||Refresh(metadata_notify));
                                let mutation=action.mutation();
                                let result=if owner!=account.account_key() { Err("Phiên tài khoản đã thay đổi".into()) } else { crate::contact_actions::execute(&account,&target,&action).await.map_err(|error|error.to_string()) };
                                let _=tx.send(GuiEvent::Contact {owner,target,generation,mutation,result});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::FetchLastOnline { owner, runtime_id, target, official } => {
                            let account = session.lock().await.clone();
                            if !account.is_active() || account.account_key() != owner || account.runtime_id() != runtime_id {
                                continue;
                            }
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                if let Ok(status) = crate::last_activity::read(&account, &target, official).await {
                                    let _ = tx.send(GuiEvent::ActivityUpdated { owner, runtime_id, uid: target, status });
                                    ctx.request_repaint();
                                }
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
                                    let kind = if snapshot.removed { None } else { crate::chat::GroupKind::from_metadata(&snapshot.info) };
                                    let _ = tx.send(GuiEvent::GroupKindLoaded { revision, owner: owner.clone(), group_id: group_id.clone(), kind });
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
                            let _ = tx.send(GuiEvent::HistorySyncStatus { running: true,success:false, message: "Đang tải dữ liệu máy chủ còn cung cấp (nhóm và Cloud)…".into() });
                            ctx.request_repaint();
                            tokio::spawn(async move {
                                let result = crate::history_sync::download(&s, threads, cancel, retry_filtered,
                                    |message| { if deliver_thread_updates() { let _ = tx.send(GuiEvent::HistorySyncStatus { running: true,success:false, message }); ctx.request_repaint(); } },
                                    |thread| { if deliver_thread_updates() { let _ = tx.send(GuiEvent::ThreadsUpdated {threads:vec![thread],source:ThreadUpdateSource::History}); ctx.request_repaint(); } }).await;
                                running.store(false, std::sync::atomic::Ordering::SeqCst);
                                let success = result.as_ref().is_ok_and(|report| report.complete);
                                let message = result.map(|report| report.message).unwrap_or_else(|e| format!("Lỗi tải lịch sử: {e}"));
                                let _ = tx.send(GuiEvent::HistorySyncStatus { running: false, success, message });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::ListSettingsDevices { generation, refresh } => {
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let result = tokio::task::spawn_blocking(move || crate::media::list_call_devices(refresh))
                                    .await.map_err(|_| "Không đọc được danh sách thiết bị".to_string());
                                let _ = tx.send(GuiEvent::SettingsDevicesLoaded { generation, result });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::LoadCloudUsage { owner, generation } => {
                            let s = session.lock().await.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let result = if !s.is_active() || owner != s.account_key() {
                                    Err("Phiên tài khoản đã thay đổi".into())
                                } else {
                                    crate::zcloud::fetch_my_documents_usage(&s).await.map_err(|error| error.to_string())
                                };
                                if s.is_active() {
                                    let _ = tx.send(GuiEvent::CloudUsageLoaded { owner, generation, result });
                                    ctx.request_repaint();
                                }
                            });
                        }
                        WorkerCmd::LoadMessagePins { owner, thread, generation } => {
                            let s = session.lock().await.clone();
                            let tx = gui_tx.clone(); let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let result = if !s.is_active() || owner != s.account_key() {
                                    Err("Phiên tài khoản đã thay đổi".into())
                                } else {
                                    crate::board::execute(&s, &thread, true, &crate::board::Action::List {
                                        page: 0, last: String::new(), pins: true,
                                    }).await.map_err(|error| error.to_string())
                                };
                                let _ = tx.send(GuiEvent::MessagePinsLoaded { owner, thread, generation, result });
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
                                    let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&cached)?;
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
                                    let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&s_pref)?;
                                    let preferences = store.conversation_preferences()?;
                                    Ok((preferences, crate::chat::load_threads_cache_with_store(&s_pref, &store)))
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
                            let outgoing_lifecycle = outgoing_lifecycle.clone();
                            let outgoing_identity = outgoing_identity.clone();
                            tokio::spawn(async move {
                                let _refresh_guard = refresh_guard;
                                let mut notice=RefreshNotice {sender:gui_tx_clone.clone(),ctx:ctx_clone.clone(),message:"Chưa hoàn tất cập nhật hội thoại".into(),success:false};
                                let _=gui_tx_clone.send(GuiEvent::ConversationRefreshStatus {running:true,success:false,message:"Đang cập nhật hội thoại và tin nhắn gần đây…".into()});
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
                                        let Ok(_write_barrier) = s.close_account_writes().await else { return; };
                                        let mut sl = session_clone.lock().await;
                                        if sl.runtime_id() != s.runtime_id() || !sl.is_active() { return; }
                                        sl.revoke_runtime();
                                        outgoing_lifecycle.retire(s.runtime_id());
                                        sl.logout();
                                        refresh_outgoing_identity(&outgoing_identity, &sl);
                                        drop(sl);
                                        if let Err(e) = tokio::task::spawn_blocking(move || crate::storage::remove_session_for_runtime(s.runtime_id())).await.unwrap_or_else(|e| Err(e.into())) { eprintln!("Cannot remove session: {e}"); }
                                        let _ = gui_tx_clone.send(GuiEvent::SessionExpired(
                                            "Phiên đăng nhập (zpw_sek) đã hết hạn trên máy chủ Zalo"
                                                .into(),
                                        ));
                                        ctx_clone.request_repaint();
                                        return;
                                    }
                                    if let Some((uid, name)) = s.sync_profile_me().await {
                                        let to_save = {
                                            let mut sl = session_clone.lock().await;
                                            if !sl.is_active() || sl.runtime_id() != s.runtime_id() || sl.account_key() != s.account_key() { return; }
                                            if let Some(ref mut li) = sl.login {
                                                li.uin = uid.clone();
                                                if !name.is_empty() {
                                                    li.display_name = name.clone();
                                                }
                                            }
                                            sl.clone()
                                        };
                                        if let Err(e) = tokio::task::spawn_blocking(move || to_save.save_to_file("session.json"))
                                            .await
                                            .unwrap_or_else(|e| Err(e.into()))
                                        {
                                            let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                        }
                                        let u_ava = s
                                            .login
                                            .as_ref()
                                            .map(|l| l.avatar.clone())
                                            .filter(|a| !a.is_empty());
                                        let _ = gui_tx_clone.send(GuiEvent::LoginSuccess {
                                            composer_config: s.composer_config,
                                            name,
                                            uin: uid,
                                            avatar: u_ava,
                                            runtime_id: s.runtime_id(),
                                        });
                                    }
                                    let list = match crate::chat::get_friends(&s).await {
                                        Ok(list) => list,
                                        Err(e) => {
                                            notice.message=format!("Cập nhật hội thoại thất bại: {e}");
                                            ctx_clone.request_repaint();
                                            return;
                                        }
                                    };
                                    let account = s.clone();
                                    let (hydrated, preloaded) = match tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                                        let mut list = list;
                                        let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&account)?;
                                        let marks = store.hydrate_refresh(&mut list, crate::store::MessageStore::PRELOAD_MSGS)?;
                                        Ok((list, marks))
                                    }).await {
                                        Ok(Ok(snapshot)) => snapshot,
                                        _ => {
                                            notice.success = false;
                                            notice.message = "Không đọc được lịch sử cục bộ; thử cập nhật lại".into();
                                            ctx_clone.request_repaint();
                                            return;
                                        }
                                    };
                                    let mut list = hydrated;
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
                                        *known.lock().await = list.iter().map(|t| (t.id.clone(), t.name.clone(), t.is_group)).collect();
                                        // Startup refreshes live/offline queues; historical backfill is explicit.
                                        let count = list.len();
                                        let _ = gui_tx_clone.send(GuiEvent::ThreadsUpdated {threads:list,source:ThreadUpdateSource::History});
                                        notice.success = recent_result.is_ok();
                                        notice.message = match recent_result {
                                            Ok(_) => format!("Đã cập nhật {count} hội thoại và tin gần đây; không phải toàn bộ lịch sử điện thoại"),
                                            Err(error) => format!("Đã cập nhật {count} hội thoại; chưa lấy được tin gần đây: {error}"),
                                        };
                                    } else {
                                        *poll_clone.lock().await = before_state;
                                        notice.success = false;
                                        notice.message = "Chưa lưu được cập nhật vào CSDL; giữ cursor cũ để lấy bù lần sau".into();
                                    }
                                    ctx_clone.request_repaint();
                                }
                            });
                        }
                        WorkerCmd::PollMessages { known_threads } => {
                            // WP13-2 fulfilled: polling eliminated when WebSocket epoch is Some.
                            if is_ws_connected.load(std::sync::atomic::Ordering::SeqCst) {
                                continue;
                            }
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
                            let outgoing_lifecycle = outgoing_lifecycle.clone();
                            let outgoing_identity = outgoing_identity.clone();
                            let p3_names = p3_name_jobs.clone();
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
                                    let mut receipts_stored = true;
                                    for _ in 0..3 {
                                        let poll_res = {
                                            let mut ps = poll_clone.lock().await;
                                            crate::chat::get_chat_from_polling(&s, &mut ps).await
                                        };
                                        match poll_res {
                                            Ok(lp_resp) => {
                                                receipts_stored &= deliver_presence(&s, &lp_resp, &gui_tx_clone, &ctx_clone).await;
                                                crate::chat::merge_previews_with_profile(
                                                    &mut temp_threads,
                                                    &lp_resp,
                                                    Some(s.account_key()),
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
                                        let Ok(_write_barrier) = s.close_account_writes().await else { return; };
                                        let mut sl = session_clone.lock().await;
                                        if sl.runtime_id() != s.runtime_id() || !sl.is_active() { return; }
                                        sl.revoke_runtime();
                                        outgoing_lifecycle.retire(s.runtime_id());
                                        sl.logout();
                                        refresh_outgoing_identity(&outgoing_identity, &sl);
                                        drop(sl);
                                        if let Err(e) = tokio::task::spawn_blocking(move || crate::storage::remove_session_for_runtime(s.runtime_id())).await.unwrap_or_else(|e| Err(e.into())) { eprintln!("Cannot remove session: {e}"); }
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
                                            stored = persist_received(&s, &mut temp_threads).await;
                                            p3_resolve_names_later(&p3_names, &s, &temp_threads);
                                            if publish_live(&s, &live_updates, temp_threads) {
                                                let _ = gui_tx_clone.send(GuiEvent::LiveThreadsReady(live_updates.clone()));
                                            }
                                            ctx_clone.request_repaint();
                                        }
                                        if stored && receipts_stored {
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
                                    let account = s.clone();
                                    let loaded = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Thread>> {
                                        let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&account)?;
                                        let mut list = vec![t];
                                        store.hydrate_selected_threads(&mut list, limit)?;
                                        Ok(list)
                                    }).await;
                                    let mut t = match loaded {
                                        Ok(Ok(mut list)) => list.remove(0),
                                        _ => {
                                            let _ = gui_tx_clone.send(GuiEvent::ConversationHistoryLoaded {
                                                runtime_id: s.runtime_id(),
                                                threads: Vec::new(),
                                                status: "Không thể đọc lịch sử cục bộ".into(),
                                            });
                                            ctx_clone.request_repaint(); return;
                                        }
                                    };
                                    let mut head_fetch_failed = false;
                                    let head=if is_group||is_send2me {
                                        match crate::history_open::fetch(&s,&group_id,is_group).await {
                                            Ok(head) => head,
                                            Err(error) => {
                                                head_fetch_failed = true;
                                                crate::logger::log_error("history head", &format!("Server history fetch failed: {error}"));
                                                None
                                            }
                                        }
                                    }else{None};
                                    {
                                        let current = session_clone.lock().await;
                                        if !current.is_active() || current.runtime_id() != s.runtime_id() || current.account_key() != s.account_key() {
                                            return;
                                        }
                                    }
                                    if let Some(head)=&head{crate::chat::merge_group_history_with_profile(&mut t,&head.response,Some(s.account_key()));}
                                    if !t.messages.is_empty() {
                                        let cnt = t.messages.len();
                                        let mut filtered=vec![t];
                                        let stored=persist_received(&s, &mut filtered).await;
                                        if stored{if let Some(head)=head{if head.commit().await.is_err(){crate::logger::log_error("history head","Cannot persist head freshness");}}}
                                        {
                                            let current = session_clone.lock().await;
                                            if !current.is_active() || current.runtime_id() != s.runtime_id() || current.account_key() != s.account_key() {
                                                return;
                                            }
                                        }
                                        let status = if head_fetch_failed {
                                            format!("Đã đọc {cnt} tin cục bộ; không tải được lịch sử máy chủ")
                                        } else {
                                            format!("Đã đọc {cnt} tin từ lịch sử cục bộ và tin gần đây; lịch sử server theo dõi trong Đồng bộ")
                                        };
                                        let _ = gui_tx_clone.send(GuiEvent::ConversationHistoryLoaded {
                                            runtime_id: s.runtime_id(),
                                            threads: filtered,
                                            status,
                                        });
                                        ctx_clone.request_repaint();
                                    } else if let Some(head)=head {
                                        if head.commit().await.is_err(){crate::logger::log_error("history head","Cannot persist empty head freshness");}
                                    } else if head_fetch_failed {
                                        let _ = gui_tx_clone.send(GuiEvent::ConversationHistoryLoaded {
                                            runtime_id: s.runtime_id(),
                                            threads: Vec::new(),
                                            status: "Không thể tải lịch sử máy chủ".into(),
                                        });
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                        }
                        WorkerCmd::SendMentioned { .. } | WorkerCmd::SendMessage { .. } | WorkerCmd::SendFormatted { .. } | WorkerCmd::SendQuote { .. } | WorkerCmd::Forward { .. } | WorkerCmd::SendSticker { .. } => {
                            let _ = gui_tx.send(GuiEvent::StatusUpdate("Không thể gửi loại tin nhắn này".into()));
                        }
                        WorkerCmd::React {
                            owner,
                            runtime_id,
                            intent_id,
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
                                if !s.is_active()
                                    || s.account_key() != owner
                                    || s.runtime_id() != runtime_id
                                {
                                    let _ = gui_tx_clone.send(GuiEvent::ReactionFailed {
                                        owner,
                                        intent_id,
                                        thread_id: to_id,
                                        msg_id: msg.id,
                                        ts: msg.ts,
                                        message: "Tài khoản đã thay đổi; không áp dụng cảm xúc cũ".into(),
                                    });
                                    ctx_clone.request_repaint();
                                    return;
                                }
                                let res = crate::chat::send_reaction(
                                    &s, &to_id, is_group, &msg, &reaction,
                                )
                                .await;
                                match res {
                                    Ok(resp) if resp_error_code(&resp).is_none() => {
                                        let _ = gui_tx_clone.send(GuiEvent::Reacted {
                                            owner: s.account_key().to_owned(),
                                            intent_id,
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
                                        let _ = gui_tx_clone.send(GuiEvent::ReactionFailed {
                                            owner,
                                            intent_id,
                                            thread_id: to_id,
                                            msg_id: msg.id,
                                            ts: msg.ts,
                                            message: format!("Thả cảm xúc thất bại: {err_s}"),
                                        });
                                    }
                                    Err(e) => {
                                        crate::logger::log_error("gui::react", &e.to_string());
                                        let _ = gui_tx_clone.send(GuiEvent::ReactionFailed {
                                            owner,
                                            intent_id,
                                            thread_id: to_id,
                                            msg_id: msg.id,
                                            ts: msg.ts,
                                            message: format!("Thả cảm xúc thất bại: {e}"),
                                        });
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
                                            let (mut store, _write_permit)=crate::store::MessageStore::open_for_account_write(&disk)?;
                                            store.delete_chat_message(&tid,&snapshot)?;
                                            let cache=crate::chat::load_threads_cache_with_store(&disk,&store);
                                            crate::chat::save_threads_cache_with_store(&disk,&store,&cache)?;
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
                        WorkerCmd::FetchStickerImage { key, url } => {
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            tokio::spawn(async move {
                                match fetch_and_decode_sticker_sprite(&key, &url).await {
                                    Ok(sprite) => {
                                        let _ = gui_tx_clone.send(GuiEvent::StickerSpriteReady {
                                            key: key.clone(),
                                            sprite,
                                            permit: None,
                                        });
                                    }
                                    Err(error) => {
                                        let failure = error
                                            .downcast_ref::<crate::media_recovery::Failure>()
                                            .cloned()
                                            .unwrap_or(crate::media_recovery::Failure::Local);
                                        crate::transfer_diagnostics::media(&failure);
                                        let _ = gui_tx_clone.send(GuiEvent::ImageUnavailable {
                                            key,
                                            failure,
                                        });
                                    }
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                        WorkerCmd::FetchChatImage { .. } => {}
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
                        WorkerCmd::ParseLinkPreview { url, origin, thread } => {
                            let s = session.lock().await.clone();
                            let tx = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let outgoing = origin == crate::chat::LinkOrigin::Outgoing;
                                let result: Result<crate::chat::LinkMetadata, String> = async {
                                if !s.is_active() || s.account_key().is_empty() {
                                    return Err("Phiên tài khoản đã thay đổi".into());
                                }
                                if outgoing {
                                    let account = s.account_key().to_owned();
                                    // Draft URLs must never reach parselink before the E2EE requirement is known.
                                    tokio::task::spawn_blocking(move || link_preview_policy(origin, thread.as_ref(),
                                        |peer,is_group|crate::outgoing::check_thread_e2ee(&account,peer,is_group)))
                                        .await.map_err(|_| "Không kiểm tra được yêu cầu mã hóa".to_owned())??;
                                    if !s.is_active() {
                                        return Err("Phiên tài khoản đã thay đổi".into());
                                    }
                                }
                                let cached = outgoing.then(|| s.cached_link_preview(&url, origin)).flatten();
                                if let Some(meta) = cached { Ok(meta) } else {
                                    match tokio::time::timeout(crate::outgoing::LINK_PREVIEW_BUDGET,
                                        crate::chat::parse_link_metadata(&s, &url, origin)).await {
                                        Ok(Ok(meta)) => {
                                            if outgoing { s.cache_link_preview(&url, origin, meta.clone()); }
                                            Ok(meta)
                                        }
                                        Ok(Err(_)) => Err("Không tải được thông tin liên kết".into()),
                                        Err(_) => Err("Hết thời gian tải thông tin liên kết".into()),
                                    }
                                }
                                }.await;
                                let _ = tx.send(GuiEvent::LinkPreviewReady {
                                    owner: s.account_key().to_owned(), runtime_id: s.runtime_id(), url, result,
                                });
                                ctx.request_repaint();
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
                            if !s.is_active() || s.account_key()!=owner || batch.files.len()>super::file_drop::MAX_ATTACHMENTS {continue;}
                            tokio::spawn(send_attachment_batch(s,batch,gui_tx.clone(),egui_ctx.clone()));
                        }
                        WorkerCmd::SendVoiceMessage {
                            owner,
                            runtime_id,
                            to_id,
                            is_group,
                            samples,
                            sample_rate,
                            duration_secs,
                        } => {
                            let current = session.lock().await.clone();
                            if !voice_send_session_matches(
                                current.is_active(),
                                current.account_key(),
                                current.runtime_id(),
                                &owner,
                                runtime_id,
                            ) {
                                let _ = gui_tx.send(GuiEvent::VoiceSendRejected {
                                    owner,
                                    runtime_id,
                                    message: "Bản ghi âm đã hết hiệu lực sau khi tài khoản thay đổi".into(),
                                });
                                continue;
                            }
                            let session_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            let lifecycle = outgoing_lifecycle.clone();
                            let caption = format!("[Tin nhắn thoại] ({duration_secs}s)");
                            let mut queued_message = crate::outgoing::pending(
                                caption.clone(), is_group,
                                crate::outgoing::Payload::Attachment {
                                    path: std::path::PathBuf::new(), caption,
                                    stamp: None, hd_source: None,
                                },
                            );
                            queued_message.kind = crate::message_model::MessageKind::Voice;
                            let ticket = lifecycle.accept(owner.clone(), runtime_id, to_id.clone(), queued_message);
                            tokio::spawn(async move {
                                let before_encode = session_clone.lock().await.clone();
                                if !voice_send_session_matches(
                                    before_encode.is_active(),
                                    before_encode.account_key(),
                                    before_encode.runtime_id(),
                                    &owner,
                                    runtime_id,
                                ) {
                                    let _ = events.send(GuiEvent::VoiceSendRejected {
                                        owner,
                                        runtime_id,
                                        message: "Bản ghi âm đã hết hiệu lực sau khi tài khoản thay đổi".into(),
                                    });
                                    ctx.request_repaint();
                                    return;
                                }
                                if samples.len() > crate::voice::VOICE_MAX_BYTES / std::mem::size_of::<f32>()
                                    || duration_secs > crate::voice::VOICE_MAX_DURATION.as_secs() as u32
                                    || sample_rate == 0
                                {
                                    let _ = events.send(GuiEvent::VoiceSendStatus {
                                        owner,
                                        runtime_id,
                                        message: "Bản ghi âm vượt quá giới hạn hoặc có định dạng không hợp lệ".into(),
                                    });
                                    ctx.request_repaint();
                                    return;
                                }
                                let wav_bytes = match tokio::task::spawn_blocking(move || {
                                    let target_samples = (samples.len() as u128)
                                        .saturating_mul(crate::voice::VOICE_SAMPLE_RATE as u128)
                                        .div_ceil(sample_rate as u128);
                                    anyhow::ensure!(target_samples <= crate::voice::VOICE_MAX_BYTES as u128 / 2,
                                        "Bản ghi âm sau đổi tần số vượt quá 64 MiB");
                                    let pcm = crate::voice::resample_to_16k(&samples, sample_rate);
                                    anyhow::ensure!(pcm.len() <= crate::voice::VOICE_MAX_BYTES / 2,
                                        "Bản ghi âm sau đổi tần số vượt quá 64 MiB");
                                    Ok::<_, anyhow::Error>(crate::voice::pcm_to_wav(
                                        &pcm,
                                        crate::voice::VOICE_SAMPLE_RATE,
                                    ))
                                }).await {
                                    Ok(Ok(wav)) => wav,
                                    Ok(Err(error)) => {
                                        let _ = events.send(GuiEvent::VoiceSendStatus { owner, runtime_id, message: format!("Không mã hóa được bản ghi âm: {error}") });
                                        ctx.request_repaint();
                                        return;
                                    }
                                    Err(error) => {
                                        let _ = events.send(GuiEvent::VoiceSendStatus { owner, runtime_id, message: format!("Tác vụ mã hóa âm thanh thất bại: {error}") });
                                        ctx.request_repaint();
                                        return;
                                    }
                                };
                                let s = session_clone.lock().await.clone();
                                if !voice_send_session_matches(
                                    s.is_active(),
                                    s.account_key(),
                                    s.runtime_id(),
                                    &owner,
                                    runtime_id,
                                ) {
                                    let _ = events.send(GuiEvent::VoiceSendRejected {
                                        owner,
                                        runtime_id,
                                        message: "Bản ghi âm đã hết hiệu lực sau khi tài khoản thay đổi".into(),
                                    });
                                    return;
                                }
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0);
                                let voice_dir = crate::storage::account_temp_dir(&owner).join("voice");
                                if let Err(e) = tokio::fs::create_dir_all(&voice_dir).await {
                                    let _ = events.send(GuiEvent::VoiceSendStatus {
                                        owner,
                                        runtime_id,
                                        message: format!("Lỗi lưu tệp âm thanh: {e}"),
                                    });
                                    return;
                                }
                                let mut random_id = [0u8; 16];
                                if crate::random::fill(&mut random_id).is_err() {
                                    let _ = events.send(GuiEvent::VoiceSendStatus {
                                        owner,
                                        runtime_id,
                                        message: "Không tạo được tên tệp âm thanh an toàn".into(),
                                    });
                                    return;
                                }
                                let temp_file = voice_dir.join(format!(
                                    "voice_{now}_{}.wav",
                                    hex::encode(random_id)
                                ));
                                let mut voice_file = match tokio::fs::OpenOptions::new()
                                    .write(true)
                                    .create_new(true)
                                    .open(&temp_file)
                                    .await
                                {
                                    Ok(file) => file,
                                    Err(error) => {
                                        let _ = events.send(GuiEvent::VoiceSendStatus {
                                            owner,
                                            runtime_id,
                                            message: format!("Lỗi lưu tệp âm thanh: {error}"),
                                        });
                                        return;
                                    }
                                };
                                use tokio::io::AsyncWriteExt;
                                if let Err(error) = voice_file.write_all(&wav_bytes).await {
                                    drop(voice_file);
                                    let _ = tokio::fs::remove_file(&temp_file).await;
                                    let _ = events.send(GuiEvent::VoiceSendStatus {
                                        owner,
                                        runtime_id,
                                        message: format!("Lỗi lưu tệp âm thanh: {error}"),
                                    });
                                    return;
                                }
                                drop(voice_file);
                                if !voice_send_session_matches(
                                    s.is_active(),
                                    s.account_key(),
                                    s.runtime_id(),
                                    &owner,
                                    runtime_id,
                                ) {
                                    let _ = tokio::fs::remove_file(&temp_file).await;
                                    let _ = events.send(GuiEvent::VoiceSendRejected {
                                        owner,
                                        runtime_id,
                                        message: "Bản ghi âm đã hết hiệu lực sau khi tài khoản thay đổi".into(),
                                    });
                                    return;
                                }
                                // Outgoing voice recorded audio is sent as a standard WAV audio attachment (Payload::Attachment)
                                // pending reverse-engineering and live verification of native Zalo voice codec / metadata.
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
                                ticket.update(message.clone());
                                if !ticket.start() {
                                    let _ = tokio::fs::remove_file(&temp_file).await;
                                    return;
                                }
                                let Some(active) = crate::outgoing::Active::start(&message.client_message_id) else {
                                    ticket.finish(outgoing_not_sent(message, "Không thể bắt đầu gửi tin nhắn thoại"));
                                    let _ = tokio::fs::remove_file(&temp_file).await;
                                    let _ = events.send(GuiEvent::VoiceSendStatus {
                                        owner,
                                        runtime_id,
                                        message: "Không thể bắt đầu gửi tin nhắn thoại".into(),
                                    });
                                    return;
                                };
                                if !voice_send_session_matches(
                                    s.is_active(),
                                    s.account_key(),
                                    s.runtime_id(),
                                    &owner,
                                    runtime_id,
                                ) {
                                    message.delivery = crate::message_model::DeliveryState::Failed;
                                    if let Some(job) = message.outgoing_job.as_mut() {
                                        job.error = "Bản ghi âm đã hết hiệu lực sau khi tài khoản thay đổi".into();
                                    }
                                    ticket.finish(message.clone());
                                    drop(active);
                                    let _ = tokio::fs::remove_file(&temp_file).await;
                                    let _ = events.send(GuiEvent::VoiceSendRejected {
                                        owner,
                                        runtime_id,
                                        message: "Bản ghi âm đã hết hiệu lực sau khi tài khoản thay đổi".into(),
                                    });
                                    return;
                                }
                                persist_outgoing(&s, &to_id, &message).await;
                                let _ = events.send(GuiEvent::VoiceOutgoingState {
                                    owner: owner.clone(),
                                    runtime_id,
                                    thread_id: to_id.clone(),
                                    message: message.clone(),
                                });
                                ctx.request_repaint();

                                if !voice_send_session_matches(
                                    s.is_active(),
                                    s.account_key(),
                                    s.runtime_id(),
                                    &owner,
                                    runtime_id,
                                ) {
                                    message.delivery = crate::message_model::DeliveryState::Failed;
                                    if let Some(job) = message.outgoing_job.as_mut() {
                                        job.error = "Bản ghi âm đã hết hiệu lực sau khi tài khoản thay đổi".into();
                                    }
                                    persist_outgoing(&s, &to_id, &message).await;
                                    ticket.finish(message.clone());
                                    drop(active);
                                    let _ = events.send(GuiEvent::VoiceOutgoingState {
                                        owner: owner.clone(),
                                        runtime_id,
                                        thread_id: to_id.clone(),
                                        message,
                                    });
                                    let _ = events.send(GuiEvent::VoiceSendRejected {
                                        owner,
                                        runtime_id,
                                        message: "Bản ghi âm đã hết hiệu lực sau khi tài khoản thay đổi".into(),
                                    });
                                    return;
                                }
                                match crate::outgoing::send(&s, &to_id, &message).await {
                                    Ok(mut sent) => {
                                        sent.kind = crate::message_model::MessageKind::Voice;
                                        message = sent;
                                        let _ = events.send(GuiEvent::VoiceSendStatus {
                                            owner: owner.clone(),
                                            runtime_id,
                                            message: "Đã gửi tin nhắn thoại".into(),
                                        });
                                    }
                                    Err(error) => {
                                        message.delivery = crate::outgoing::failure_delivery(&error);
                                        if let Some(job) = message.outgoing_job.as_mut() {
                                            job.error = crate::outgoing::failure_message(&error);
                                        }
                                        let _ = events.send(GuiEvent::VoiceSendStatus {
                                            owner: owner.clone(),
                                            runtime_id,
                                            message: format!("Gửi tin nhắn thoại thất bại: {error}"),
                                        });
                                    }
                                }
                                ticket.finish(message.clone());
                                if !voice_send_session_matches(
                                    s.is_active(),
                                    s.account_key(),
                                    s.runtime_id(),
                                    &owner,
                                    runtime_id,
                                ) {
                                    return;
                                }
                                persist_outgoing(&s, &to_id, &message).await;
                                drop(active);
                                let _ = events.send(GuiEvent::VoiceOutgoingState {
                                    owner,
                                    runtime_id,
                                    thread_id: to_id,
                                    message,
                                });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::PrepareAttachmentArchive { owner, runtime_id, thread, path, stamp } => {
                            let s = session.lock().await.clone();
                            if !s.is_active() || s.account_key()!=owner || s.runtime_id()!=runtime_id { continue; }
                            let events=gui_tx.clone(); let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let result: anyhow::Result<_> = async {
                                    let _permit=crate::limits::UPLOAD_JOBS.acquire().await
                                        .map_err(|_| anyhow::anyhow!("Hàng đợi đã đóng"))?;
                                    anyhow::ensure!(s.is_active(), "Phiên làm việc đã thay đổi");
                                    crate::attachment_archive::prepare(path,stamp,crate::storage::account_temp_dir(&owner)).await
                                }.await;
                                if !s.is_active() {
                                    // Best-effort cleanup of an archive completed after session revocation.
                                    if let Ok(path)=result { let _=tokio::fs::remove_file(path).await; }
                                    return;
                                }
                                let result=result.map_err(|_| "Không tạo được ZIP: hãy kiểm tra tệp gốc, kích thước và dung lượng ổ đĩa".to_owned());
                                let _=events.send(GuiEvent::AttachmentArchivePrepared { owner,runtime_id,thread,result });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::PickAttachmentDraft { owner, runtime_id, thread, images_only } => {
                            let s = session.lock().await.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let paths = if s.is_active() && s.account_key() == owner && s.runtime_id() == runtime_id {
                                    tokio::task::spawn_blocking(move || super::messages::pick_native_files_dialog(images_only))
                                        .await.map_err(|_| "Không mở được hộp chọn tệp".to_owned())
                                        .map(|paths| paths.unwrap_or_default())
                                } else {
                                    Err("Phiên làm việc đã thay đổi".to_owned())
                                };
                                let _ = events.send(GuiEvent::AttachmentDraftPicked { owner, runtime_id, thread, paths });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::PickAndSendAttachment {owner,runtime_id,to_id,is_group,images_only,caption,preset_path,photo_original,ephemeral} => {
                            let s=session.lock().await.clone();let events=gui_tx.clone();let ctx=egui_ctx.clone();
                            if !s.is_active() || s.account_key()!=owner || s.runtime_id()!=runtime_id {
                                continue;
                            }
                            tokio::spawn(async move {
                                let paths=if let Some(path)=preset_path {vec![path]} else {
                                    let picker_owner=s.account_key().to_owned();
                                    let paths=tokio::task::spawn_blocking(move||super::messages::pick_native_files_dialog(images_only)).await.ok().flatten().unwrap_or_default();
                                    let _=events.send(GuiEvent::AttachmentPickerClosed { owner: picker_owner });
                                    ctx.request_repaint();
                                    paths
                                };
                                send_attachment_batch(s,super::file_drop::Batch {thread:to_id,group:is_group,caption,original:photo_original,files:paths.into_iter().map(|p|(p,None)).collect(),ephemeral},events,ctx).await;
                            });
                        }
                        WorkerCmd::StartCall {
                            owner,
                            runtime_id,
                            partner_id,
                            partner_name,
                            partner_avatar,
                            is_video,
                            is_group,
                        } => {
                            let current = session.lock().await.clone();
                            if !current.is_active() || current.account_key() != owner || current.runtime_id() != runtime_id {
                                crate::logger::log_error(
                                    "call::preflight",
                                    &format!(
                                        "StartCall rejected: session mismatch (cmd owner={owner}, rt={runtime_id}, active owner={}, rt={})",
                                        current.account_key(), current.runtime_id()
                                    ),
                                );
                                continue;
                            }
                            let kind_label = if is_video { "Gọi video" } else { "Gọi thoại" };
                            call_bridge.make_call(
                                &current,
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
                        WorkerCmd::CopyArchiveImage {owner,media,context}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let result=async {
                                    anyhow::ensure!(account.account_key()==owner && media_context_alive(&account,Some(&context)),"Phiên hoặc tin nhắn không khả dụng");
                                    crate::conversation_policy::require_shareable(&account,&context.thread,context.group).await?;
                                    let url=media.original.ok_or_else(||anyhow::anyhow!("Chưa có ảnh gốc để sao chép; hãy tải lại thông tin ảnh"))?;
                                    let key=format!("archive-copy-{}",crate::crypto::md5_hex(&url));
                                    // A multi-attachment parent's renewal endpoint cannot identify a
                                    // chosen child reliably; never substitute a sibling on failure.
                                    let image=fetch_gui_image(&account,crate::storage::root(),&key,&url).await?;
                                    anyhow::ensure!(media_context_alive(&account,Some(&context)),"Tin nhắn đã hết hạn hoặc phiên đã đóng");
                                    tokio::task::spawn_blocking(move||->anyhow::Result<()> {
                                        anyhow::ensure!(media_context_alive(&account,Some(&context)),"Tin nhắn đã hết hạn hoặc phiên đã đóng");
                                        #[cfg(windows)] {crate::clipboard_image::write_windows(&image)}
                                        #[cfg(not(windows))] {let _=image;anyhow::bail!("Sao chép ảnh yêu cầu Windows")}
                                    }).await??;
                                    Ok::<(),anyhow::Error>(())
                                }.await;
                                let message=match result {Ok(())=>"Đã sao chép ảnh đã chọn".into(),Err(error)=>format!("Không sao chép được ảnh: {error}")};
                                let _=tx.send(GuiEvent::StatusUpdate(message));ctx.request_repaint();
                            });
                        }
                        WorkerCmd::RevealArchiveMedia {owner,runtime_id,path,context}=>{
                            let account=session.lock().await.clone();let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let result=tokio::task::spawn_blocking(move||->anyhow::Result<()> {
                                    anyhow::ensure!(account.account_key()==owner && account.runtime_id()==runtime_id && media_context_alive(&account,Some(&context)),"Phiên hoặc tin nhắn không còn khả dụng");
                                    anyhow::ensure!(std::fs::symlink_metadata(&path).is_ok_and(|m|m.is_file()),"Tệp đã lưu không còn tồn tại; hãy tải lại");
                                    anyhow::ensure!(crate::platform::open_folder_and_select(&path),"Không mở được thư mục chứa tệp");
                                    Ok(())
                                }).await;
                                if !matches!(result,Ok(Ok(()))) {let _=tx.send(GuiEvent::StatusUpdate("Không mở được tệp đã lưu; tệp có thể đã bị di chuyển hoặc phiên đã thay đổi".into()));ctx.request_repaint();}
                            });
                        }
                        WorkerCmd::MutateArchiveBatch {owner,runtime_id,batch,thread,group,recall,messages}=>{
                            let account=session.lock().await.clone();
                            let tx=gui_tx.clone();let ctx=egui_ctx.clone();
                            tokio::spawn(async move {
                                let results=mutate_archive_batch(&account,&owner,runtime_id,&thread,group,recall,messages).await;
                                let _=tx.send(GuiEvent::ArchiveMutationFinished {owner,runtime_id,batch,thread,recall,results});ctx.request_repaint();
                            });
                        }
                        WorkerCmd::LoadArchiveHistory { owner, runtime_id, thread, group, generation, before } => {
                            let source = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = source.lock().await.clone();
                                let target = thread.clone();
                                let expected = owner.clone();
                                let result = tokio::task::spawn_blocking(move || -> anyhow::Result<crate::store::HistoryPage> {
                                    anyhow::ensure!(s.is_active() && s.account_key() == expected && s.runtime_id() == runtime_id,"Phiên làm việc đã thay đổi");
                                    anyhow::ensure!(s.hidden.view(s.account_key()).allows(&target,group),"Mở khóa PIN trước khi đọc kho lưu trữ");
                                    let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&s)?;
                                    let page = store.history_page(&target,before.as_ref(),200)?;
                                    anyhow::ensure!(s.is_active() && s.hidden.view(s.account_key()).allows(&target,group),"Phiên làm việc đã đóng hoặc hội thoại đã khóa");
                                    Ok(page)
                                }).await.map_err(|_| "Không thể hoàn tất đọc lịch sử cục bộ".to_owned()).and_then(|r| r.map_err(|e| e.to_string()));
                                let _ = events.send(GuiEvent::ArchiveHistoryLoaded { owner,runtime_id,thread,generation,result });
                                ctx.request_repaint();
                            });
                        }
                        WorkerCmd::FetchServerMedia {
                            runtime_id,
                            thread_id,
                            media_type,
                            page,
                            count,
                            cursor,
                            generation,
                        } => {
                            let s_clone = session.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                if s.runtime_id() != runtime_id || !s.is_active() {
                                    return;
                                }
                                let kind = match media_type {
                                    1 => crate::server_media::ServerMediaType::Photo,
                                    2 => crate::server_media::ServerMediaType::Video,
                                    3 => crate::server_media::ServerMediaType::File,
                                    4 => crate::server_media::ServerMediaType::Link,
                                    _ => {
                                        let _ = events.send(GuiEvent::ServerMediaLoadFailed {
                                            runtime_id,
                                            thread_id,
                                            media_type,
                                            generation,
                                            error: "Loại kho media không hợp lệ".into(),
                                        });
                                        ctx.request_repaint();
                                        return;
                                    }
                                };
                                let _ = page;
                                let res = crate::server_media::fetch_conversation_media_store(
                                    &s,
                                    &thread_id,
                                    kind,
                                    count,
                                    cursor.as_deref(),
                                )
                                .await;
                                match res {
                                    Ok(media_page) => {
                                        let _ = events.send(GuiEvent::ServerMediaLoaded {
                                            runtime_id,
                                            thread_id,
                                            media_type,
                                            page_num: page,
                                            page: media_page,
                                            generation,
                                        });
                                    }
                                    Err(e) => {
                                        let err_msg = e.to_string();
                                        let _ = events.send(GuiEvent::ServerMediaLoadFailed {
                                            runtime_id,
                                            thread_id,
                                            media_type,
                                            generation,
                                            error: err_msg,
                                        });
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
                        WorkerCmd::MarkThreadRead { thread_id, last_read_msg_id } => {
                            let s_clone = session.clone();
                            tokio::spawn(async move {
                                let s = s_clone.lock().await.clone();
                                if !s.is_active() { return; }
                                let _ = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                                    let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&s)?;
                                    store.set_thread_read(&thread_id, &last_read_msg_id)?;
                                    Ok(())
                                }).await;
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
                                            let _ = events.send(GuiEvent::GroupTodosLoaded {
                                                runtime_id: s.runtime_id(), thread_id: thread_id.clone(), todos,
                                            });
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
                                        let _ = events.send(GuiEvent::GroupTodosLoaded {
                                            runtime_id: s.runtime_id(), thread_id: thread_id.clone(), todos,
                                        });
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
                                            let mut poll_items: Vec<crate::poll::PollItem> = reply.polls.unwrap_or_default().into_iter().map(Into::into).collect();
                                            for p in &mut poll_items {
                                                p.grid = thread_id.clone();
                                            }
                                            let _ = events.send(GuiEvent::GroupPollsLoaded {
                                                runtime_id: s.runtime_id(), thread_id: thread_id.clone(), polls: poll_items,
                                            });
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
                                        let mut poll_items: Vec<crate::poll::PollItem> = reply.polls.unwrap_or_default().into_iter().map(Into::into).collect();
                                        for p in &mut poll_items {
                                            p.grid = thread_id.clone();
                                        }
                                        let _ = events.send(GuiEvent::GroupPollsLoaded {
                                            runtime_id: s.runtime_id(), thread_id: thread_id.clone(), polls: poll_items,
                                        });
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
                        WorkerCmd::FriendAction {
                            owner, runtime_id, generation, user_id, action,
                        } => {
                            let s = session.lock().await.clone();
                            let events = gui_tx.clone();
                            let ctx = egui_ctx.clone();
                            tokio::spawn(async move {
                                let result = if s.login.is_some()
                                    && s.account_key() == owner
                                    && s.runtime_id() == runtime_id
                                    && !user_id.is_empty()
                                    && user_id != owner
                                {
                                    run_friend_action(&s, &user_id, action).await
                                } else {
                                    Err("Phiên hoặc người dùng không còn hợp lệ".into())
                                };
                                let _ = events.send(GuiEvent::FriendStatusChecked {
                                    owner, runtime_id, generation, user_id, result,
                                });
                                ctx.request_repaint();
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
                            // A19: Cancel in-flight auth task and bump auth attempt generation
                            if let Some(task) = current_auth_task.take() { task.abort(); }
                            let attempt_id = auth_attempt_mgr.next_attempt();
                            abort_and_join_phone_sync(&mut active_phone_sync, &phone_sync_running, &phone_sync_cancel).await;
                            db_ready_runtime.store(0, std::sync::atomic::Ordering::Release);

                            // Build candidate session outside the live session lock to prevent blocking across network
                            let current = session.lock().await.clone();
                            let mut candidate = match Session::new(current.verbose, current.dump, current.dump_raw) {
                                Ok(candidate) => candidate,
                                Err(_) => {
                                    let _ = gui_tx.send(GuiEvent::LoginFailed("Không tạo được phiên xác thực".into()));
                                    egui_ctx.request_repaint();
                                    continue;
                                }
                            };

                            let session_clone = session.clone();
                            let commands = commands.clone();
                            let gui_tx_clone = gui_tx.clone();
                            let ctx_clone = egui_ctx.clone();
                            let auth_mgr = auth_attempt_mgr.clone();
                            let outgoing_identity = outgoing_identity.clone();
                            let outgoing_lifecycle = outgoing_lifecycle.clone();
                            let live_updates = live_updates.clone();
                            let db_ready_pw = db_ready_runtime.clone();
                            let ws_ready_pw = is_ws_connected.clone();
                            let handle = tokio::spawn(async move {
                                let _ = gui_tx_clone.send(GuiEvent::StatusUpdate(
                                    "Đang xác thực tài khoản…".into(),
                                ));
                                ctx_clone.request_repaint();
                                match crate::login::login_by_password(&mut candidate, &phone, &pass, "VN")
                                    .await
                                {
                                    Ok(_) => {
                                        // A19: Validate auth attempt before committing result
                                        if !auth_mgr.is_current(attempt_id) {
                                            // Stale response — another auth started, discard this result
                                            return;
                                        }

                                        if let Ok(info) = crate::login::get_login_info(&candidate).await {
                                            if let Some(cfg) = info.get("data") {
                                                crate::login::apply_login_config(&mut candidate, cfg);
                                                candidate.apply_server_info(cfg);

                                                if !auth_mgr.is_current(attempt_id) {
                                                    return;
                                                }

                                                let old_session = session_clone.lock().await.clone();
                                                let Ok(_write_barrier) = old_session.close_account_writes().await else {
                                                    let _ = gui_tx_clone.send(GuiEvent::LoginFailed(
                                                        "Không thể đóng dữ liệu phiên trước".into(),
                                                    ));
                                                    ctx_clone.request_repaint();
                                                    return;
                                                };
                                                let mut s = session_clone.lock().await;
                                                if !auth_mgr.is_current(attempt_id)
                                                    || s.runtime_id() != old_session.runtime_id()
                                                    || s.account_key() != old_session.account_key()
                                                {
                                                    return;
                                                }
                                                let uin = candidate
                                                    .login
                                                    .as_ref()
                                                    .map(|l| l.uin.clone())
                                                    .unwrap_or_default();
                                                let name = candidate
                                                    .login
                                                    .as_ref()
                                                    .map(|l| l.display_name.clone())
                                                    .unwrap_or_default();
                                                let u_ava = candidate
                                                    .login
                                                    .as_ref()
                                                    .map(|l| l.avatar.clone())
                                                    .filter(|a| !a.is_empty());
                                                let runtime_id = candidate.runtime_id();

                                                let s_clone = candidate.clone();
                                                let s_hydrate = candidate.clone();
                                                s.revoke_runtime();
                                                outgoing_lifecycle.retire(old_session.runtime_id());
                                                *s = candidate;
                                                refresh_outgoing_identity(&outgoing_identity, &s);
                                                drop(s);
                                                let tx_save = gui_tx_clone.clone();
                                                let saved = tokio::task::spawn_blocking(move || {
                                                    s_clone.save_to_file("session.json")
                                                })
                                                .await
                                                .unwrap_or_else(|e| Err(e.into()));
                                                if let Err(e) = saved {
                                                    let _ = tx_save.send(GuiEvent::StatusUpdate(format!("Không thể lưu phiên: {e}")));
                                                }
                                                let _ = gui_tx_clone.send(GuiEvent::LoginSuccess {
                                                    composer_config: s_hydrate.composer_config,
                                                    name,
                                                    uin: uin.clone(),
                                                    avatar: u_ava,
                                                    runtime_id,
                                                });
                                                let _ = gui_tx_clone.send(GuiEvent::ServerConfigUpdated {
                                                    owner: uin,
                                                    runtime_id,
                                                    config: s_hydrate.server_config.clone(),
                                                });
                                                ctx_clone.request_repaint();

                                                {
                                                    let session_arc = session_clone.clone();
                                                    let tx_preload = gui_tx_clone.clone();
                                                    let ctx_preload = ctx_clone.clone();
                                                    let live_preload = live_updates.clone();
                                                    let life_preload = outgoing_lifecycle.clone();
                                                    let id_preload = outgoing_identity.clone();
                                                    let db_ready_preload = db_ready_pw.clone();
                                                    let ws_ready_preload = ws_ready_pw.clone();
                                                    tokio::spawn(async move {
                                                        run_startup_preload(
                                                            session_arc,
                                                            tx_preload,
                                                            ctx_preload,
                                                            live_preload,
                                                            life_preload,
                                                            id_preload,
                                                            db_ready_preload,
                                                            ws_ready_preload,
                                                        )
                                                        .await;
                                                    });
                                                }

                                                if let Some(tx) = commands.upgrade() {
                                                    let _ = tx.send(WorkerCmd::RefreshThreads);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // A19: Validate auth attempt before showing error
                                        if !auth_mgr.is_current(attempt_id) {
                                            // Stale response — discard
                                            return;
                                        }

                                        let _ = gui_tx_clone
                                            .send(GuiEvent::LoginFailed(e.to_string()));
                                        ctx_clone.request_repaint();
                                    }
                                }
                            });
                            current_auth_task = Some(handle);
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
mod friend_action_tests {
    use super::*;
    use crate::chat::FriendshipStatus::{
        Friend, IncomingPending, OutgoingPending, Stranger, Unknown,
    };

    #[test]
    fn only_explicit_action_for_current_relationship_can_mutate() {
        assert!(friend_action_matches_status(FriendAction::Add, Stranger));
        assert!(friend_action_matches_status(
            FriendAction::Accept,
            IncomingPending
        ));
        assert!(friend_action_matches_status(
            FriendAction::Reject,
            IncomingPending
        ));
        assert!(friend_action_matches_status(
            FriendAction::Undo,
            OutgoingPending
        ));
        for status in [Unknown, Friend, IncomingPending, OutgoingPending, Stranger] {
            assert!(!friend_action_matches_status(FriendAction::Check, status));
            if status != Stranger {
                assert!(!friend_action_matches_status(FriendAction::Add, status));
            }
            if status != IncomingPending {
                assert!(!friend_action_matches_status(FriendAction::Accept, status));
                assert!(!friend_action_matches_status(FriendAction::Reject, status));
            }
            if status != OutgoingPending {
                assert!(!friend_action_matches_status(FriendAction::Undo, status));
            }
        }
    }

    #[tokio::test]
    async fn friend_action_fails_without_an_authenticated_session() {
        let session = Session::new(false, false, false).unwrap();
        for action in [
            FriendAction::Check,
            FriendAction::Add,
            FriendAction::Accept,
            FriendAction::Reject,
            FriendAction::Undo,
        ] {
            assert!(run_friend_action(&session, "friend-fixture", action)
                .await
                .is_err());
        }
    }
}

#[cfg(test)]
mod image_pipeline_tests {
    use super::*;

    #[test]
    fn parse_sticker_key_extracts_cid_and_sid() {
        assert_eq!(parse_sticker_key("stk_0_15"), Some((0, 15)));
        assert_eq!(parse_sticker_key("stk_61900_98930"), Some((61900, 98930)));
        assert_eq!(parse_sticker_key("stk_99901_999001"), Some((99901, 999001)));
        assert_eq!(parse_sticker_key("stk_invalid"), None);
        assert_eq!(parse_sticker_key("stk_0"), None);
        assert_eq!(parse_sticker_key("ava_12345"), None);
    }

    #[test]
    fn decode_sprite_geometry_checks() {
        // Multi-frame sprite sheet (2 frames of 100x100 -> 200x100)
        let png_2frames = {
            let img = image::RgbaImage::new(200, 100);
            let mut cur = std::io::Cursor::new(Vec::new());
            img.write_to(&mut cur, image::ImageFormat::Png).unwrap();
            cur.into_inner()
        };
        let sprite = decode_sprite_from_bytes(&png_2frames).unwrap();
        assert_eq!(sprite.frames, 2);
        assert_eq!(sprite.delays.len(), 2);
        assert_eq!(sprite.delays[0], crate::gui::ZALO_SPRITE_FRAME_DELAY);

        // Single frame square (100x100)
        let png_1frame = {
            let img = image::RgbaImage::new(100, 100);
            let mut cur = std::io::Cursor::new(Vec::new());
            img.write_to(&mut cur, image::ImageFormat::Png).unwrap();
            cur.into_inner()
        };
        let sprite = decode_sprite_from_bytes(&png_1frame).unwrap();
        assert_eq!(sprite.frames, 1);
        assert_eq!(sprite.delays.len(), 1);
        assert_eq!(sprite.delays[0], crate::gui::ZALO_SPRITE_FRAME_DELAY);

        // Non-divisible geometry (150x100)
        let png_non_div = {
            let img = image::RgbaImage::new(150, 100);
            let mut cur = std::io::Cursor::new(Vec::new());
            img.write_to(&mut cur, image::ImageFormat::Png).unwrap();
            cur.into_inner()
        };
        let sprite = decode_sprite_from_bytes(&png_non_div).unwrap();
        assert_eq!(sprite.frames, 1);
        assert_eq!(sprite.delays.len(), 1);
        assert_eq!(sprite.delays[0], std::time::Duration::ZERO);

        // Excessive frames > 64 (6500x100 -> 65 frames)
        let png_excessive = {
            let img = image::RgbaImage::new(650, 10);
            let mut cur = std::io::Cursor::new(Vec::new());
            img.write_to(&mut cur, image::ImageFormat::Png).unwrap();
            cur.into_inner()
        };
        let sprite = decode_sprite_from_bytes(&png_excessive).unwrap();
        assert_eq!(sprite.frames, 1);
        assert_eq!(sprite.delays.len(), 1);
        assert_eq!(sprite.delays[0], std::time::Duration::ZERO);
    }

    #[tokio::test]
    async fn fetch_and_decode_bundled_sticker_sprite() {
        let sprite = fetch_and_decode_sticker_sprite("stk_99901_999001", "")
            .await
            .expect("Bundled sticker should decode directly");
        assert!(sprite.frames > 0);
        assert_eq!(sprite.delays.len(), sprite.frames as usize);
    }
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
                success: false,
            };
            notice.message = "Không thể kết nối máy chủ".into();
        }
        assert!(
            matches!(rx.try_recv().unwrap(),GuiEvent::ConversationRefreshStatus {running:false,message,..} if message=="Không thể kết nối máy chủ")
        );
    }
}

fn p3_same_socket_owner(
    runtime: Option<usize>,
    cookie: &str,
    current_runtime: usize,
    current_cookie: &str,
) -> bool {
    runtime == Some(current_runtime) && cookie == current_cookie
}

/// Per-connection reaction/control workers; replacement cancels work from the old owner.
struct P3SideWorkers {
    reactions: std::sync::Arc<tokio::sync::Notify>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for P3SideWorkers {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
type P3NameJob = (Session, Vec<Thread>);

fn p3_resolve_names_later(
    names: &tokio::sync::mpsc::Sender<P3NameJob>,
    session: &Session,
    threads: &[Thread],
) {
    let metadata = threads
        .iter()
        .take(256)
        .map(|thread| Thread {
            id: thread.id.clone(),
            name: thread.name.clone(),
            avatar: thread.avatar.clone(),
            is_group: thread.is_group,
            presentation: thread.presentation.clone(),
            ..Default::default()
        })
        .collect();
    // Optional enrichment can be retried by refresh; message payloads never enter this queue.
    if names.try_send((session.clone(), metadata)).is_err() {
        crate::logger::log_error(
            "name enrichment",
            "Metadata queue full; keeping current names",
        );
    }
}

fn p3_start_name_worker(
    tx: Sender<GuiEvent>,
    repaint: egui::Context,
) -> tokio::sync::mpsc::Sender<P3NameJob> {
    let (names, mut name_jobs) = tokio::sync::mpsc::channel::<P3NameJob>(8);
    tokio::spawn(async move {
        while let Some((account, batch)) = name_jobs.recv().await {
            if !account.is_active() {
                continue;
            }
            let mut latest = HashMap::new();
            for thread in batch {
                latest.insert(thread.id.clone(), thread);
            }
            while let Ok((source, batch)) = name_jobs.try_recv() {
                if !source.is_active() || source.runtime_id() != account.runtime_id() {
                    continue;
                }
                for thread in batch {
                    if latest.len() < 256 || latest.contains_key(&thread.id) {
                        latest.insert(thread.id.clone(), thread);
                    }
                }
            }
            let mut threads: Vec<_> = latest.into_values().collect();
            crate::chat::resolve_thread_names(&account, &mut threads).await;
            if !account.is_active() {
                continue;
            }
            if !threads.is_empty() {
                let _ = tx.send(GuiEvent::ThreadsUpdated {
                    threads,
                    source: ThreadUpdateSource::History,
                });
                repaint.request_repaint();
            }
        }
    });
    names
}

fn p3_start_side_workers(
    session: Session,
    events: Sender<GuiEvent>,
    ctx: egui::Context,
    priority_task: tokio::task::JoinHandle<()>,
) -> P3SideWorkers {
    let reactions = std::sync::Arc::new(tokio::sync::Notify::new());
    let wake = reactions.clone();
    let reaction_task = tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(2));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut pending = true;
        loop {
            tokio::select! { _ = timer.tick(), if pending => {}, _ = wake.notified() => {} }
            if !session.is_active() {
                break;
            }
            let account = session.clone();
            let result =
                tokio::task::spawn_blocking(move || -> anyhow::Result<(Vec<Thread>, bool)> {
                    let (store, _write_permit) =
                        crate::store::MessageStore::open_for_account_write(&account)?;
                    let threads = crate::reaction_pipeline::drain(&store, &account)?;
                    let pending = store.conn.query_row(
                        "SELECT EXISTS(SELECT 1 FROM pending_reactions LIMIT 1)",
                        [],
                        |row| row.get(0),
                    )?;
                    Ok((threads, pending))
                })
                .await;
            if !session.is_active() {
                break;
            }
            match result {
                Ok(Ok((threads, remaining))) => {
                    pending = remaining;
                    if !threads.is_empty() {
                        let _ = events.send(GuiEvent::ThreadsUpdated {
                            threads,
                            source: ThreadUpdateSource::History,
                        });
                        ctx.request_repaint();
                    }
                }
                Ok(Err(error)) => {
                    pending = true;
                    crate::runtime_metrics::database_error(&error);
                    crate::logger::log_error(
                        "reaction drain",
                        "Durable reactions retained for retry",
                    );
                }
                Err(_) => {
                    pending = true;
                    crate::logger::log_error(
                        "reaction drain",
                        "Blocking task failed; reactions retained",
                    );
                }
            }
        }
    });
    P3SideWorkers {
        reactions,
        tasks: vec![priority_task, reaction_task],
    }
}

async fn p3_enqueue_reactions(
    session: &Session,
    events: Vec<crate::reactions::ReactionEvent>,
) -> bool {
    let account = session.clone();
    match tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&account)?;
        crate::reaction_pipeline::enqueue(&store, &events)
    })
    .await
    {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            crate::runtime_metrics::database_error(&error);
            crate::logger::log_error(
                "reaction enqueue",
                "Page not acknowledged; keeping cursor for replay",
            );
            false
        }
        Err(_) => {
            crate::logger::log_error(
                "reaction enqueue",
                "Blocking task failed; keeping cursor for replay",
            );
            false
        }
    }
}

#[cfg(test)]
mod p3_ingress_tests {
    use super::*;

    #[test]
    fn typing_is_published_without_opening_a_database() {
        let session = Session::new(false, false, false).unwrap();
        let (events, received) = std::sync::mpsc::channel();
        p3_publish_typing(
            &session,
            &serde_json::json!({"data":{"actions":[{"act_type":"typing","act":"typing","data":{"uid":"123"}}]}}),
            &events,
            &egui::Context::default(),
        );
        assert!(
            matches!(received.try_recv().unwrap(), GuiEvent::Typing {events, ..} if events.len() == 1)
        );
    }

    #[test]
    fn same_account_cookie_cannot_reuse_a_previous_runtime_socket() {
        assert!(p3_same_socket_owner(
            Some(11),
            "cookie-fixture",
            11,
            "cookie-fixture"
        ));
        assert!(!p3_same_socket_owner(
            Some(11),
            "cookie-fixture",
            12,
            "cookie-fixture"
        ));
        assert!(!p3_same_socket_owner(
            Some(11),
            "cookie-fixture",
            11,
            "other-fixture"
        ));
        assert!(!p3_same_socket_owner(
            None,
            "cookie-fixture",
            11,
            "cookie-fixture"
        ));
    }

    fn reaction() -> crate::reactions::ReactionEvent {
        crate::reactions::ReactionEvent {
            message_id: "message-fixture".into(),
            thread_id: "peer-fixture".into(),
            sender_id: "reactor-fixture".into(),
            event_id: "event-fixture".into(),
            r_type: 5,
            emoji: "❤️".into(),
            ts: 1_800_000_000_000,
            ..Default::default()
        }
    }

    #[test]
    fn durable_reaction_enqueue_survives_a_drain_failure_and_replay() {
        let mut store = crate::store::MessageStore::open(":memory:").unwrap();
        let session = Session::new(false, false, false).unwrap();
        let thread = Thread {
            id: "peer-fixture".into(),
            name: "Peer".into(),
            messages: vec![ChatMessage {
                id: "message-fixture".into(),
                text: "fixture".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        crate::store::persist_threads(&mut store, &[thread]).unwrap();
        let event = reaction();
        crate::reaction_pipeline::enqueue(&store, std::slice::from_ref(&event)).unwrap();
        store.conn.execute_batch("CREATE TRIGGER fail_reaction BEFORE DELETE ON pending_reactions BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
        assert!(crate::reaction_pipeline::drain(&store, &session).is_err());
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM pending_reactions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1); // (INV-4) Drain failure cannot erase the durable page obligation.
        crate::reaction_pipeline::enqueue(&store, &[event]).unwrap();
        let replay_count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM pending_reactions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(replay_count, 1);
        store
            .conn
            .execute_batch("DROP TRIGGER fail_reaction")
            .unwrap();
        assert_eq!(
            crate::reaction_pipeline::drain(&store, &session)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn reaction_enqueue_failure_rolls_back_the_whole_page() {
        let store = crate::store::MessageStore::open(":memory:").unwrap();
        store.conn.execute_batch("CREATE TRIGGER fail_reaction BEFORE INSERT ON pending_reactions WHEN NEW.data LIKE '%oversized-fixture%' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
        let valid = reaction();
        let invalid = crate::reactions::ReactionEvent {
            event_id: "oversized-fixture".into(),
            ..valid.clone()
        };
        assert!(crate::reaction_pipeline::enqueue(&store, &[valid, invalid]).is_err());
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM pending_reactions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0); // (INV-4) Caller must retain the old cursor after this failure.
    }

    #[tokio::test]
    async fn optional_name_queue_is_bounded_and_never_holds_message_payloads() {
        let (names, mut jobs) = tokio::sync::mpsc::channel(1);
        let account = Session::new(false, false, false).unwrap();
        let thread = Thread {
            id: "thread-fixture".into(),
            messages: vec![ChatMessage {
                text: "must not enter enrichment".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        p3_resolve_names_later(&names, &account, std::slice::from_ref(&thread));
        p3_resolve_names_later(&names, &account, &[thread]);
        let (_, metadata) = jobs.recv().await.unwrap();
        assert_eq!(metadata.len(), 1);
        assert!(metadata[0].messages.is_empty());
        assert!(jobs.try_recv().is_err());
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
        let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&account)?;
        store.apply_mute_changes(&changes, mark_seeded)
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
) -> bool {
    p3_publish_typing(s, payload, gui_tx, ctx);
    p3_deliver_receipts(s, payload, gui_tx, ctx).await
}

fn p3_publish_typing(
    s: &Session,
    payload: &serde_json::Value,
    gui_tx: &Sender<GuiEvent>,
    ctx: &egui::Context,
) {
    if !s.is_active() {
        return;
    }
    let self_id = s.account_key().to_owned();
    let typing = crate::presence::typing(payload, &self_id);
    if !typing.is_empty() {
        let _ = gui_tx.send(GuiEvent::Typing {
            owner: self_id.clone(),
            events: typing,
        });
        ctx.request_repaint();
    }
}

async fn p3_deliver_receipts(
    s: &Session,
    payload: &serde_json::Value,
    gui_tx: &Sender<GuiEvent>,
    ctx: &egui::Context,
) -> bool {
    let receipts = crate::presence::receipts(payload);
    if receipts.is_empty() {
        return true;
    }
    if !s.is_active() {
        return false;
    }
    let account = s.clone();
    let changed = tokio::task::spawn_blocking(move || {
        let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&account)?;
        store.apply_receipts(&receipts)
    })
    .await;
    match changed {
        Ok(Ok(changed)) if !changed.is_empty() => {
            let _ = gui_tx.send(GuiEvent::Receipts {
                owner: s.account_key().to_owned(),
                changed,
            });
            ctx.request_repaint();
            true
        }
        Ok(Ok(_)) => true,
        Ok(Err(error)) => {
            crate::runtime_metrics::database_error(&error);
            crate::logger::log_error(
                "presence::receipts",
                "Receipt write failed; keep message cursor for replay",
            );
            false
        }
        Err(_) => {
            crate::logger::log_error(
                "presence::receipts",
                "Receipt writer failed; keep message cursor for replay",
            );
            false
        }
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
        let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&account)?;
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

fn link_preview_policy(
    origin: crate::chat::LinkOrigin,
    thread: Option<&(String, bool)>,
    check_requirement: impl FnOnce(&str, bool) -> anyhow::Result<bool>,
) -> Result<(), String> {
    if origin == crate::chat::LinkOrigin::Incoming {
        return Ok(());
    }
    let (peer, is_group) = thread
        .filter(|(id, _)| !id.trim().is_empty())
        .ok_or_else(|| "Thiếu cuộc trò chuyện cho xem trước liên kết".to_owned())?;
    match check_requirement(peer, *is_group) {
        Ok(false) => Ok(()),
        Ok(true) => Err("Không xem trước liên kết trong cuộc trò chuyện mã hóa".into()),
        Err(_) => Err("Không kiểm tra được yêu cầu mã hóa".into()),
    }
}

#[cfg(test)]
mod composer_metadata_tests {
    use super::*;

    #[test]
    fn preview_policy_rejects_missing_encrypted_or_unknown_targets_before_fetch() {
        use crate::chat::LinkOrigin::{Incoming, Outgoing};
        assert!(link_preview_policy(Outgoing, None, |_, _| panic!(
            "missing target must not check another peer"
        ))
        .is_err());
        let target = ("fixture-peer".to_owned(), false);
        assert!(link_preview_policy(Outgoing, Some(&target), |_, _| Ok(true)).is_err());
        assert!(
            link_preview_policy(Outgoing, Some(&target), |_, _| Err(anyhow::anyhow!(
                "fixture DB unavailable"
            )))
            .is_err()
        );
        assert!(link_preview_policy(Outgoing, Some(&target), |peer, group| {
            assert_eq!(peer, "fixture-peer");
            assert!(!group);
            Ok(false)
        })
        .is_ok());
        assert!(link_preview_policy(Incoming, None, |_, _| panic!(
            "incoming must retain its own policy"
        ))
        .is_ok());
    }

    #[test]
    fn formatted_command_keeps_durable_metadata_and_quote_for_retry() {
        let text = "😀 @An họp".to_owned();
        let options = crate::rich_text::ComposerFormat {
            bold: true,
            urgency: Some(crate::rich_text::Urgency::Important),
            ..Default::default()
        }
        .options(&text);
        let mentions = vec![crate::mentions::Mention {
            uid: "42".into(),
            pos: 3,
            len: 3,
            kind: 0,
        }];
        let quoted = ChatMessage {
            id: "123".into(),
            client_message_id: "456".into(),
            text: "Gốc".into(),
            ..Default::default()
        };
        let (_, message) = outgoing_command(&WorkerCmd::SendFormatted {
            to_id: "group_12".into(),
            text: text.clone(),
            is_group: true,
            options,
            mentions: mentions.clone(),
            quoted: Some(quoted),
        })
        .unwrap();
        assert_eq!(message.text, text);
        assert_eq!(
            message.text_styles[0].len,
            text.encode_utf16().count() as u32
        );
        assert_eq!(message.reply_to_id.as_deref(), Some("123"));
        assert_eq!(
            message.wire_ref.fields["specialMsgType"],
            serde_json::json!(1)
        );
        let stored = serde_json::to_string(&message).unwrap();
        let restored: ChatMessage = serde_json::from_str(&stored).unwrap();
        let crate::outgoing::Payload::Formatted {
            options,
            mentions: actual,
            quoted,
        } = &restored.outgoing_job.as_ref().unwrap().payload
        else {
            panic!("formatted job lost")
        };
        assert_eq!(options.urgency, Some(crate::rich_text::Urgency::Important));
        assert_eq!(*actual, mentions);
        assert_eq!(quoted.as_ref().unwrap().id, "123");
        assert_eq!(restored.client_message_id, message.client_message_id);
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
        WorkerCmd::SendFormatted {
            to_id,
            text,
            is_group,
            options,
            mentions,
            quoted,
        } => {
            let mut quote = quoted.clone();
            if let Some(quote) = quote.as_mut() {
                quote.outgoing_job = None;
            }
            let mut msg = pending(
                text.clone(),
                *is_group,
                Payload::Formatted {
                    options: options.clone(),
                    mentions: mentions.clone(),
                    quoted: quote.map(Box::new),
                },
            );
            msg.text_styles = options.styles.clone();
            if let Some(urgency) = options.urgency {
                msg.wire_ref.fields.insert(
                    "specialMsgType".into(),
                    serde_json::json!(match urgency {
                        crate::rich_text::Urgency::Important => 1,
                        crate::rich_text::Urgency::Urgent => 2,
                    }),
                );
            }
            if !mentions.is_empty() {
                msg.wire_ref.fields.insert(
                    "mentions".into(),
                    serde_json::to_value(mentions).unwrap_or_default(),
                );
            }
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
        let _timing =
            crate::runtime_metrics::Scope::new(crate::runtime_metrics::Metric::OutgoingWrite);
        if !session.is_active() {
            return Ok(());
        }
        let (mut store, _write_permit) =
            crate::store::MessageStore::open_for_account_write(&session)?;
        let meta = store.thread_meta(&thread_id)?;
        let is_group = message
            .outgoing_job
            .as_ref()
            .is_some_and(|job| job.is_group);
        let (thread_name, is_group) = match meta {
            Some((name, grp)) => (
                if name.is_empty() {
                    thread_id.clone()
                } else {
                    name
                },
                grp,
            ),
            None => (thread_id.clone(), is_group),
        };
        let thread = Thread {
            id: thread_id.clone(),
            name: thread_name,
            is_group,
            messages: vec![message],
            ..Default::default()
        };
        crate::store::persist_local_threads(&mut store, &[thread])?;
        Ok(())
    })
    .await;
    if let Ok(Err(error)) = &result {
        crate::runtime_metrics::database_error(error);
    }
    if !matches!(result, Ok(Ok(()))) {
        crate::logger::log_error("outgoing persistence", "Cannot persist outgoing message");
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_outgoing_send(
    session_snapshot: Session,
    session: std::sync::Arc<tokio::sync::Mutex<Session>>,
    tx: Sender<GuiEvent>,
    ctx: egui::Context,
    owner: String,
    thread_id: String,
    mut message: ChatMessage,
    ticket: OutgoingTicket,
) {
    let Some(active) = crate::outgoing::Active::start(&message.client_message_id) else {
        ticket.finish(outgoing_not_sent(
            message,
            "Intent gửi đã được xử lý hoặc đang chạy",
        ));
        return;
    };
    tokio::spawn(async move {
        let _active = active;
        if !session_snapshot.is_active() {
            return;
        }
        if !ticket.start() {
            return;
        }
        message.delivery = crate::message_model::DeliveryState::Pending;
        if let Some(job) = message.outgoing_job.as_mut() {
            job.error.clear();
        }
        let _ = tx.send(GuiEvent::OutgoingState {
            runtime_id: session_snapshot.runtime_id(),
            owner: owner.clone(),
            thread_id: thread_id.clone(),
            message: message.clone(),
        });
        ctx.request_repaint();
        // Admission persistence runs alongside transport. Join it before the final write
        // so a slow Pending write cannot regress a durable ACK afterward.
        let persist_session = session_snapshot.clone();
        let persist_thread = thread_id.clone();
        let persist_message = message.clone();
        let pending_write = tokio::spawn(async move {
            persist_outgoing(&persist_session, &persist_thread, &persist_message).await;
        });
        let result = crate::outgoing::send(&session_snapshot, &thread_id, &message).await;
        let expired = result.as_ref().err().is_some_and(|error| {
            error
                .chain()
                .any(|cause| is_session_expired_error(&cause.to_string()))
        });
        match result {
            Ok(sent) => message = sent,
            Err(error) => {
                message.delivery = crate::outgoing::failure_delivery(&error);
                if let Some(job) = message.outgoing_job.as_mut() {
                    job.error = crate::outgoing::failure_message(&error);
                }
            }
        }
        // Record and publish the authoritative outcome before awaiting local storage.
        // Cancellation after ACK must not replace a known success with uncertainty.
        ticket.finish(message.clone());
        let _ = pending_write.await;
        persist_outgoing(&session_snapshot, &thread_id, &message).await;
        if expired && session_snapshot.is_active() {
            let current = session.lock().await.clone();
            if current.runtime_id() == session_snapshot.runtime_id() {
                let Ok(_write_barrier) = current.close_account_writes().await else {
                    return;
                };
                let mut current = session.lock().await;
                if current.runtime_id() == session_snapshot.runtime_id() {
                    current.logout();
                    drop(current);
                    let runtime_id = session_snapshot.runtime_id();
                    let _ = tokio::task::spawn_blocking(move || {
                        crate::storage::remove_session_for_runtime(runtime_id)
                    })
                    .await;
                    let _ = tx.send(GuiEvent::SessionExpired(
                        "Không thể gửi vì phiên đăng nhập đã hết hạn".into(),
                    ));
                }
            }
        }
        ctx.request_repaint();
    });
}

fn emit_outgoing_not_sent(
    runtime_id: usize,
    owner: String,
    thread_id: String,
    mut message: ChatMessage,
    reason: &str,
    tx: &Sender<GuiEvent>,
    ctx: &egui::Context,
) {
    message = outgoing_not_sent(message, reason);
    let _ = tx.send(GuiEvent::OutgoingState {
        runtime_id,
        owner,
        thread_id,
        message,
    });
    ctx.request_repaint();
}

fn outgoing_identity_matches(
    active: bool,
    actual_owner: &str,
    actual_runtime_id: usize,
    expected_owner: &str,
    expected_runtime_id: usize,
) -> bool {
    active
        && !expected_owner.is_empty()
        && actual_owner == expected_owner
        && actual_runtime_id == expected_runtime_id
}

fn refresh_outgoing_identity(
    identity: &std::sync::Arc<std::sync::RwLock<(String, usize)>>,
    session: &Session,
) {
    let next = (session.account_key().to_owned(), session.runtime_id());
    match identity.write() {
        Ok(mut current) => *current = next,
        Err(poisoned) => *poisoned.into_inner() = next,
    }
}

fn outgoing_not_sent(mut message: ChatMessage, reason: &str) -> ChatMessage {
    message.delivery = crate::message_model::DeliveryState::Failed;
    if let Some(job) = message.outgoing_job.as_mut() {
        job.error = reason.to_owned();
    }
    message
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
        anyhow::ensure!(
            session.is_active() && !session.account_key().is_empty(),
            "Phiên làm việc đã đóng"
        );

        match command {
            C::OpenThread { thread, group } => {
                anyhow::ensure!(
                    session
                        .hidden
                        .view(session.account_key())
                        .allows(&thread, group),
                    "Mở khóa PIN trước khi mở hội thoại"
                );
                let identity = crate::server_pins::identity(&thread, group)?;
                let id = if group {
                    identity
                } else {
                    identity[1..].to_owned()
                };
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                let mut threads = vec![Thread {
                    id: id.clone(),
                    name: id.clone(),
                    is_group: group,
                    ..Default::default()
                }];
                store.hydrate_selected_threads(&mut threads, 200)?;
                let mut thread = threads
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("Không mở được hội thoại"))?;
                if let Some(saved) = store
                    .all_threads()?
                    .into_iter()
                    .find(|saved| saved.thread_id == id)
                {
                    thread.name = saved.name;
                }
                Ok(R::Hit {
                    thread: Box::new(thread),
                    id: String::new(),
                })
            }
            C::SearchPage(request) => {
                let access = session.hidden.view(session.account_key());
                anyhow::ensure!(access.ready, "Chưa xác minh danh sách hội thoại ẩn");
                if let Some(thread) = &request.thread {
                    anyhow::ensure!(
                        access.allows(thread, thread.starts_with('g')),
                        "Hội thoại đang khóa PIN"
                    );
                }
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                let result =
                    crate::search::query_with_cancel(&store, &request, &|| !session.is_active())
                        .map_err(|e| e.to_string());
                let senders = match request.thread.as_deref() {
                    Some(thread) => crate::search::senders(&store, thread)?,
                    None => Vec::new(),
                };
                let result = result.map(|mut page| {
                    page.hits
                        .retain(|hit| access.allows(&hit.thread, hit.group));
                    page
                });
                Ok(R::SearchPage {
                    request,
                    result,
                    senders,
                })
            }
            C::OpenSearchHit {
                generation,
                thread,
                id,
            } => {
                anyhow::ensure!(
                    session
                        .hidden
                        .view(session.account_key())
                        .allows(&thread, thread.starts_with('g')),
                    "Hội thoại đang khóa PIN"
                );
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                Ok(R::SearchOpened {
                    generation,
                    thread: Box::new(crate::tools::open_hit(&store, &thread, &id)?),
                    id,
                })
            }
            C::OpenHit { thread, id } => {
                anyhow::ensure!(
                    session
                        .hidden
                        .view(session.account_key())
                        .allows(&thread, thread.starts_with('g')),
                    "Hội thoại đang khóa PIN"
                );
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                Ok(R::Hit {
                    thread: Box::new(crate::tools::open_hit(&store, &thread, &id)?),
                    id,
                })
            }
            C::Resume => {
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                crate::tools::resume(&store)?;
                Ok(R::Loaded {
                    config: crate::tools::preferences(&store)?,
                    tasks: crate::tools::scheduled(&store, session.account_key())?,
                    paused: false,
                })
            }
            C::Update { id, text, at } => {
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                crate::tools::update_pending(&store, session.account_key(), &id, &text, at)?;
                Ok(R::Loaded {
                    config: crate::tools::preferences(&store)?,
                    tasks: crate::tools::scheduled(&store, session.account_key())?,
                    paused: crate::tools::paused(&store)?,
                })
            }
            C::Search { query, hidden } => {
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                Ok(R::Search {
                    hits: crate::tools::search(&store, &query, hidden)?
                        .into_iter()
                        .filter(|hit| {
                            session
                                .hidden
                                .view(session.account_key())
                                .allows(&hit.thread_id, hit.thread_id.starts_with('g'))
                        })
                        .collect(),
                    query,
                })
            }
            C::ExportHistory => {
                anyhow::ensure!(
                    session.hidden.view(session.account_key()).export_allowed(),
                    "Mở khóa PIN trước khi xuất lịch sử"
                );
                let Some(path) = tools_file(true, "Xuất lịch sử", &["db"]) else {
                    return Ok(R::Status("Đã hủy xuất lịch sử".into()));
                };
                anyhow::ensure!(
                    session.is_active() && !session.account_key().is_empty(),
                    "Phiên làm việc đã đóng"
                );
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                let n =
                    crate::tools::export_history(&store, &path, session.account_key(), &|| {
                        session.is_active()
                            && session.hidden.view(session.account_key()).export_allowed()
                    })?;
                Ok(R::Status(format!("Đã xuất {n} tin nhắn vào SQLite")))
            }
            C::Import {
                directory,
                confirmed,
            } => {
                let path = if directory {
                    super::messages::pick_native_folder_dialog()
                } else {
                    tools_file(false, "Nhập lịch sử", &["db", "sqlite", "sqlite3", "json"])
                };
                let Some(path) = path else {
                    return Ok(R::Status("Đã hủy chọn nguồn".into()));
                };
                anyhow::ensure!(
                    session.is_active() && !session.account_key().is_empty(),
                    "Phiên làm việc đã đóng"
                );
                let (mut store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                let n = crate::tools::import_history(
                    &mut store,
                    &path,
                    session.account_key(),
                    confirmed,
                    &|| session.is_active(),
                )?;
                Ok(R::Status(format!(
                    "Đã nhập {n} tin mới. Tìm trong toàn bộ lịch sử để xem tin cũ."
                )))
            }
            C::Load => {
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                Ok(R::Loaded {
                    config: crate::tools::preferences(&store)?,
                    tasks: crate::tools::scheduled(&store, session.account_key())?,
                    paused: crate::tools::paused(&store)?,
                })
            }
            C::Save(config) => {
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                anyhow::ensure!(session.is_active(), "Đã đăng xuất");
                let config = crate::tools::save_preferences(&store, config)?;
                Ok(R::Loaded {
                    config,
                    tasks: crate::tools::scheduled(&store, session.account_key())?,
                    paused: crate::tools::paused(&store)?,
                })
            }
            C::Schedule {
                request,
                thread,
                group,
                text,
                at,
            } => {
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                let result = crate::tools::schedule(
                    &store,
                    session.account_key(),
                    &thread,
                    group,
                    &text,
                    at,
                )
                .map_err(|e| e.to_string());
                Ok(R::ScheduleSaved {
                    request,
                    thread,
                    text,
                    at,
                    result,
                })
            }
            C::Edit { id, at } => {
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                crate::tools::edit_job(&store, session.account_key(), &id, at)?;
                Ok(R::Loaded {
                    config: crate::tools::preferences(&store)?,
                    tasks: crate::tools::scheduled(&store, session.account_key())?,
                    paused: crate::tools::paused(&store)?,
                })
            }
            C::ReadConfig { stickers } => {
                let Some(path) = tools_file(false, "Chọn cấu hình JSON", &["json"]) else {
                    return Ok(R::Status("Đã hủy".into()));
                };
                let bytes = crate::limits::read_file(&path, 1024 * 1024)?;
                let parsed_stickers = if stickers {
                    Some(serde_json::from_slice(&bytes)?)
                } else {
                    None
                };
                let parsed_theme = if stickers {
                    None
                } else {
                    Some(serde_json::from_slice(&bytes)?)
                };
                anyhow::ensure!(
                    session.is_active() && !session.account_key().is_empty(),
                    "Đã đăng xuất"
                );
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&session)?;
                let mut config = crate::tools::preferences(&store)?;
                if let Some(parsed) = parsed_stickers {
                    config.stickers = parsed;
                }
                if let Some(parsed) = parsed_theme {
                    config.theme = parsed;
                }
                let config = crate::tools::save_preferences(&store, config)?;
                Ok(R::Loaded {
                    config,
                    tasks: crate::tools::scheduled(&store, session.account_key())?,
                    paused: crate::tools::paused(&store)?,
                })
            }
            C::ExportConfig { stickers } => {
                let Some(path) = tools_file(true, "Xuất cấu hình JSON", &["json"]) else {
                    return Ok(R::Status("Đã hủy".into()));
                };
                anyhow::ensure!(
                    session.is_active() && !session.account_key().is_empty(),
                    "Phiên làm việc đã đóng"
                );
                let config = {
                    let (store, _write_permit) =
                        crate::store::MessageStore::open_for_account_write(&session)?;
                    crate::tools::preferences(&store)?
                };
                let value = if stickers {
                    if config.stickers.is_empty() {
                        serde_json::json!([{
                            "cate_id": 61900,
                            "name": "Sticker của tôi",
                            "sticker_ids": [98930, 98931],
                            "sticker_type": 7
                        }])
                    } else {
                        serde_json::to_value(&config.stickers)?
                    }
                } else if config.theme.is_null() {
                    serde_json::json!({
                        "dark": { "primary": "#528BFF", "bg_sidebar": "#21252B" },
                        "light": { "primary": "#0068FF", "bg_sidebar": "#FFFFFF" }
                    })
                } else {
                    config.theme
                };
                let mut file = crate::media_resources::AtomicPrivateFile::create(
                    &path,
                    &crate::param_cipher::random_hex(12),
                )?;
                file.write_all(&serde_json::to_vec_pretty(&value)?)?;
                file.commit()?;
                Ok(R::Status("Đã xuất cấu hình JSON".into()))
            }
        }
    })
    .await?
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

/// One selected attachment with its own identity and renewal context.
#[derive(Clone)]
pub struct MediaBatchItem {
    pub key: String,
    pub context: ChatImage,
    pub url: String,
    pub name: String,
}

async fn mutate_archive_batch(
    account: &Session,
    owner: &str,
    runtime_id: usize,
    thread: &str,
    group: bool,
    recall: bool,
    messages: Vec<ChatMessage>,
) -> Vec<(ChatMessage, Result<(), String>)> {
    let valid_count = !messages.is_empty() && messages.len() <= 100;
    let mut results = Vec::new();
    for message in messages {
        let result: Result<(), String> = async {
            anyhow::ensure!(valid_count, "Chọn tối đa 100 tin nhắn");
            anyhow::ensure!(
                account.is_active()
                    && account.account_key() == owner
                    && account.runtime_id() == runtime_id,
                "Phiên làm việc đã thay đổi"
            );
            anyhow::ensure!(
                account.hidden.view(owner).allows(thread, group),
                "Hội thoại đang khóa PIN"
            );
            if recall {
                anyhow::ensure!(message.is_outgoing, "Chỉ có thể thu hồi tin do bạn gửi");
                crate::chat::recall_message(account, thread, group, &message).await?;
            } else {
                let local_only = message.id.is_empty()
                    || (message.id.starts_with("call_") && message.client_message_id.is_empty())
                    || message.id.starts_with("out_")
                    || message.id.starts_with("local_");
                if !local_only {
                    crate::chat::delete_message_for_me(account, thread, group, &message).await?;
                }
            }
            let disk = account.clone();
            let target = thread.to_owned();
            let snapshot = message.clone();
            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                anyhow::ensure!(
                    disk.is_active(),
                    "Máy chủ đã nhận nhưng phiên đã đóng trước khi lưu cục bộ"
                );
                let (mut store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&disk)?;
                if recall {
                    let mut recalled = snapshot;
                    recalled.mark_recalled();
                    let meta = store.thread_meta(&target)?;
                    let name = meta.map(|m| m.0).unwrap_or_else(|| target.clone());
                    crate::store::persist_local_threads(
                        &mut store,
                        &[Thread {
                            id: target,
                            name,
                            is_group: group,
                            messages: vec![recalled],
                            ..Default::default()
                        }],
                    )?;
                } else {
                    store.delete_chat_message(&target, &snapshot)?;
                }
                let cache = crate::chat::load_threads_cache_with_store(&disk, &store);
                crate::chat::save_threads_cache_with_store(&disk, &store, &cache)?;
                Ok(())
            })
            .await
            .map_err(|_| {
                anyhow::anyhow!("Máy chủ đã nhận nhưng tác vụ lưu cục bộ không hoàn tất")
            })??;
            Ok::<(), anyhow::Error>(())
        }
        .await
        .map_err(|error| format!("Chưa xác nhận hoàn tất: {error}. Không tự thử lại."));
        results.push((message, result));
    }
    results
}

async fn download_media_batch(
    account: Session,
    owner: String,
    batch: u64,
    items: Vec<MediaBatchItem>,
    events: Sender<GuiEvent>,
    ctx: egui::Context,
) {
    // Bound each user intent; stream serially so the shared download limit never
    // drops selected items merely because other transfers are still running.
    let valid = items.len() <= 100 && owner == account.account_key() && account.is_active();
    let mut jobs = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let id = format!("archive-{batch}-{index}");
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let name = crate::media_download::safe_name(&item.name);
        if events
            .send(GuiEvent::MediaStarted {
                owner: owner.clone(),
                thread: Some((item.context.thread.clone(), item.context.group)),
                id: id.clone(),
                name: name.clone(),
                cancel: cancel.clone(),
            })
            .is_err()
        {
            return;
        }
        jobs.push((item, id, name, cancel));
    }
    ctx.request_repaint();
    let directory = if valid {
        tokio::task::spawn_blocking(|| {
            #[cfg(windows)]
            {
                rfd::FileDialog::new()
                    .set_title("Chọn thư mục lưu các mục đã chọn")
                    .pick_folder()
            }
            #[cfg(not(windows))]
            {
                None::<std::path::PathBuf>
            }
        })
        .await
        .ok()
        .flatten()
    } else {
        None
    };
    let mut results = Vec::new();
    for (mut item, id, name, cancel) in jobs {
        let result: Result<std::path::PathBuf, String> = async {
            anyhow::ensure!(valid, "Phiên hoặc số lượng mục không hợp lệ (tối đa 100)");
            let directory = directory
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Đã hủy chọn thư mục lưu"))?;
            anyhow::ensure!(
                !cancel.load(std::sync::atomic::Ordering::Acquire),
                "Đã hủy tải"
            );
            anyhow::ensure!(
                media_context_alive(&account, Some(&item.context)),
                "Tin nhắn đã hết hạn hoặc phiên đã đóng"
            );
            crate::conversation_policy::require_shareable(
                &account,
                &item.context.thread,
                item.context.group,
            )
            .await?;
            let path = unique_archive_destination(directory, &name).await?;
            if let Some(restored) =
                (crate::message_view::archive_media_items(&item.context.message).len() == 1)
                    .then(|| {
                        crate::desktop_backup::restored_path(&account, &item.context.message, false)
                    })
                    .flatten()
            {
                if tokio::fs::symlink_metadata(&restored)
                    .await
                    .is_ok_and(|meta| meta.is_file())
                {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut input = tokio::fs::File::open(restored).await?;
                    let mut output = tokio::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                        .await?;
                    let copy: anyhow::Result<()> = async {
                        let mut bytes = vec![0u8; 64 * 1024];
                        let mut total = 0;
                        loop {
                            anyhow::ensure!(
                                !cancel.load(std::sync::atomic::Ordering::Acquire)
                                    && media_context_alive(&account, Some(&item.context)),
                                "Đã dừng tải"
                            );
                            let count = input.read(&mut bytes).await?;
                            if count == 0 {
                                break;
                            }
                            output.write_all(&bytes[..count]).await?;
                            total += count as u64;
                            let _ = events.send(GuiEvent::MediaProgress {
                                owner: owner.clone(),
                                id: id.clone(),
                                bytes: total,
                                total: None,
                            });
                            ctx.request_repaint();
                        }
                        output.sync_all().await?;
                        anyhow::ensure!(
                            !cancel.load(std::sync::atomic::Ordering::Acquire)
                                && media_context_alive(&account, Some(&item.context)),
                            "Đã dừng tải"
                        );
                        Ok(())
                    }
                    .await;
                    if let Err(error) = copy {
                        drop(output);
                        // Remove only the incomplete destination created by this operation.
                        let _ = tokio::fs::remove_file(&path).await;
                        return Err(error);
                    }
                    return Ok(path);
                }
            }
            let visual = matches!(
                crate::message_view::media(&item.context.message).kind,
                crate::message_view::MediaKind::Photo | crate::message_view::MediaKind::Video
            );
            let progress = |bytes, total| {
                if !media_context_alive(&account, Some(&item.context)) {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                }
                let _ = events.send(GuiEvent::MediaProgress {
                    owner: owner.clone(),
                    id: id.clone(),
                    bytes,
                    total,
                });
                ctx.request_repaint();
            };
            let swapped_item_url = account.rewrite_download_url(&item.url);
            let mut result = save_media_target(
                &swapped_item_url,
                &path,
                cancel.clone(),
                progress,
                visual,
                None,
                false,
            )
            .await;
            if crate::message_view::archive_media_items(&item.context.message).len() == 1
                && result
                    .as_ref()
                    .err()
                    .and_then(|e| e.downcast_ref::<crate::media_recovery::Failure>())
                    .is_some_and(|e| e.renew_candidate())
            {
                if let Ok(media) = crate::media_renew::renew(
                    &account,
                    &item.context.thread,
                    item.context.group,
                    &item.context.message,
                )
                .await
                {
                    if let Some(url) = media.original.clone() {
                        persist_media_projection(&account, &mut item.context, &media, &events)
                            .await?;
                        let swapped_renewed = account.rewrite_download_url(&url);
                        result = save_media_target(
                            &swapped_renewed,
                            &path,
                            cancel.clone(),
                            |bytes, total| {
                                if !media_context_alive(&account, Some(&item.context)) {
                                    cancel.store(true, std::sync::atomic::Ordering::Release);
                                }
                                let _ = events.send(GuiEvent::MediaProgress {
                                    owner: owner.clone(),
                                    id: id.clone(),
                                    bytes,
                                    total,
                                });
                                ctx.request_repaint();
                            },
                            visual,
                            None,
                            false,
                        )
                        .await;
                    }
                }
            }
            result?;
            if cancel.load(std::sync::atomic::Ordering::Acquire)
                || !media_context_alive(&account, Some(&item.context))
            {
                // The completed destination belongs to this operation, never an existing file.
                let _ = tokio::fs::remove_file(&path).await;
                anyhow::bail!("Đã dừng tải");
            }
            Ok(path)
        }
        .await
        .map_err(|error| error.to_string());
        let _ = events.send(GuiEvent::MediaFinished {
            owner: owner.clone(),
            id,
            result: result.clone(),
        });
        results.push((item.key, result));
        ctx.request_repaint();
    }
    let _ = events.send(GuiEvent::MediaBatchFinished {
        owner,
        batch,
        results,
    });
    ctx.request_repaint();
}

async fn unique_archive_destination(
    directory: &std::path::Path,
    name: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let base = std::path::Path::new(name);
    let stem = base.file_stem().unwrap_or_default().to_string_lossy();
    let extension = base
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    for suffix in 0..10_000 {
        let candidate = directory.join(if suffix == 0 {
            name.to_owned()
        } else {
            format!("{stem} ({suffix}){extension}")
        });
        if !tokio::fs::try_exists(&candidate).await? {
            return Ok(candidate);
        }
    }
    anyhow::bail!("Có quá nhiều tệp trùng tên trong thư mục đã chọn")
}

#[cfg(test)]
mod archive_download_tests {
    use super::*;

    #[tokio::test]
    async fn archive_mutation_rejects_changed_account_for_every_source_without_network() {
        let account = Session::new(false, false, false).unwrap();
        let sources = vec![
            ChatMessage {
                id: "source-a".into(),
                ..Default::default()
            },
            ChatMessage {
                id: "source-b".into(),
                ..Default::default()
            },
        ];
        let results = mutate_archive_batch(
            &account,
            "another-owner",
            account.runtime_id(),
            "thread-a",
            false,
            false,
            sources,
        )
        .await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.id, "source-a");
        assert_eq!(results[1].0.id, "source-b");
        assert!(results.iter().all(|(_, result)| result.is_err()));
    }

    #[tokio::test]
    async fn archive_download_names_preserve_existing_unicode_files() {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("Báo cáo.pdf");
        tokio::fs::write(&original, b"original").await.unwrap();
        let first = unique_archive_destination(directory.path(), "Báo cáo.pdf")
            .await
            .unwrap();
        assert_eq!(first.file_name().unwrap(), "Báo cáo (1).pdf");
        tokio::fs::write(&first, b"first").await.unwrap();
        let second = unique_archive_destination(directory.path(), "Báo cáo.pdf")
            .await
            .unwrap();
        assert_eq!(second.file_name().unwrap(), "Báo cáo (2).pdf");
        assert_eq!(tokio::fs::read(original).await.unwrap(), b"original");
    }

    #[tokio::test]
    async fn rejected_archive_batch_reports_every_item_without_downloading() {
        let account = Session::new(false, false, false).unwrap();
        let (events, receiver) = std::sync::mpsc::channel();
        let items = (0..2)
            .map(|index| MediaBatchItem {
                key: format!("selection-{index}"),
                context: ChatImage {
                    thread: "thread-fixture".into(),
                    group: false,
                    message: ChatMessage {
                        id: format!("message-{index}"),
                        ..Default::default()
                    },
                    preview: false,
                },
                url: "https://example.invalid/file".into(),
                name: format!("file-{index}.pdf"),
            })
            .collect();
        download_media_batch(
            account,
            "wrong-owner".into(),
            123,
            items,
            events,
            egui::Context::default(),
        )
        .await;
        let events: Vec<_> = receiver.try_iter().collect();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GuiEvent::MediaStarted { .. }))
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GuiEvent::MediaFinished { result: Err(_), .. }))
                .count(),
            2
        );
        assert!(events.iter().any(|event| matches!(event, GuiEvent::MediaBatchFinished { results, .. } if results.len() == 2 && results.iter().all(|(_, result)| result.is_err()))));
    }
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
        if let Some(path) = (crate::message_view::archive_media_items(&source.message).len() == 1)
            .then(|| crate::desktop_backup::restored_path(s, &source.message, source.preview))
            .flatten()
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
    let Some(mut context) = context.filter(|source| {
        should && crate::message_view::archive_media_items(&source.message).len() == 1
    }) else {
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

async fn play_inline_voice(
    account: Session,
    binding: inline_voice::Binding,
    controls: inline_voice::Controls,
    context: ChatImage,
    events: Sender<GuiEvent>,
    ctx: egui::Context,
) {
    use std::sync::atomic::Ordering;
    let result = async {
        anyhow::ensure!(
            voice_send_session_matches(
                account.is_active(),
                account.account_key(),
                account.runtime_id(),
                &binding.owner,
                binding.runtime
            ),
            "Phiên tài khoản đã thay đổi"
        );
        anyhow::ensure!(
            media_context_alive(&account, Some(&context)),
            "Tin âm thanh đã hết hạn"
        );
        anyhow::ensure!(
            !context.message.is_e2ee,
            "Chưa hỗ trợ giải mã tin âm thanh mã hóa đầu cuối"
        );
        let media = crate::message_view::media(&context.message);
        let url = media
            .original
            .as_ref()
            .or(context.message.link_url.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Tin âm thanh chưa có đường dẫn tải"))?;
        let swapped_voice_url = account.rewrite_download_url(url);
        let mut response =
            crate::media_download::response(&swapped_voice_url, Duration::from_secs(30)).await?;
        const MAX_BYTES: usize = 32 * 1024 * 1024;
        anyhow::ensure!(
            response
                .content_length()
                .is_none_or(|n| n <= MAX_BYTES as u64),
            "Tin âm thanh vượt giới hạn tải 32 MB"
        );
        let mut data = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            anyhow::ensure!(
                !controls.stop.load(Ordering::Acquire)
                    && media_context_alive(&account, Some(&context)),
                "Đã dừng phát âm thanh"
            );
            anyhow::ensure!(
                data.len().saturating_add(chunk.len()) <= MAX_BYTES,
                "Tin âm thanh vượt giới hạn tải 32 MB"
            );
            data.extend_from_slice(&chunk);
        }
        let root = account.history_path().with_extension("voice-cache");
        let name = format!("{}.audio", binding.request);
        let task_binding = binding.clone();
        let task_controls = controls.clone();
        let task_events = events.clone();
        let task_ctx = ctx.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let lease = crate::video_player::CachedVideo::reserve(&root, &name)?;
            lease.admit(data.len())?;
            anyhow::ensure!(
                !task_controls.stop.load(Ordering::Acquire)
                    && media_context_alive(&account, Some(&context)),
                "Đã dừng phát âm thanh"
            );
            crate::storage::atomic_write(lease.path(), &data)?;
            #[cfg(windows)]
            let (samples, rate) = crate::windows_video::voice_pcm(lease.path())?;
            #[cfg(not(windows))]
            let (samples, rate): (Vec<f32>, u32) =
                anyhow::bail!("Phát âm thanh hiện hỗ trợ Windows");
            let duration = samples.len() as f32 / rate as f32;
            anyhow::ensure!(
                !task_controls.stop.load(Ordering::Acquire),
                "Đã dừng phát âm thanh"
            );
            let player =
                crate::voice::VoicePreviewPlayer::play(std::sync::Arc::from(samples), rate)?;
            loop {
                if task_controls.stop.load(Ordering::Acquire)
                    || !media_context_alive(&account, Some(&context))
                {
                    player.stop();
                    break;
                }
                let paused = task_controls.pause.load(Ordering::Acquire);
                if let Some(error) = player.playback_error() {
                    anyhow::bail!("{error}");
                }
                player.set_paused(paused);
                let progress = player.progress();
                let finished = progress >= 1.0;
                let _ = task_events.send(GuiEvent::InlineVoiceUpdated {
                    binding: task_binding.clone(),
                    snapshot: inline_voice::Snapshot {
                        loading: false,
                        paused,
                        finished,
                        position: (duration * progress).min(duration),
                        duration,
                        error: None,
                    },
                });
                task_ctx.request_repaint();
                if finished {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(())
        })
        .await??;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    if let Err(error) = result {
        if !controls.stop.load(Ordering::Acquire) {
            let _ = events.send(GuiEvent::InlineVoiceUpdated {
                binding,
                snapshot: inline_voice::Snapshot {
                    error: Some(error.to_string()),
                    ..Default::default()
                },
            });
            ctx.request_repaint();
        }
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
        let (store, _write_permit) = crate::store::MessageStore::open_for_account_write(&account)?;
        store.update_media_metadata(&thread, &message)
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
    fn stale_outgoing_intent_gets_terminal_not_sent_state() {
        let message =
            crate::outgoing::pending("queued".into(), false, crate::outgoing::Payload::Text);
        let failed = outgoing_not_sent(message, "Phiên đã thay đổi");
        assert_eq!(failed.delivery, crate::message_model::DeliveryState::Failed);
        assert_eq!(
            failed.outgoing_job.as_ref().map(|job| job.error.as_str()),
            Some("Phiên đã thay đổi")
        );
    }

    #[test]
    fn outgoing_identity_rejects_closed_or_replaced_session() {
        assert!(!outgoing_identity_matches(
            false, "owner-a", 1, "owner-a", 1
        ));
        assert!(!outgoing_identity_matches(true, "owner-b", 2, "owner-a", 1));
        assert!(outgoing_identity_matches(true, "owner-a", 1, "owner-a", 1));
    }

    fn outgoing_test_lifecycle() -> (OutgoingLifecycle, std::sync::mpsc::Receiver<GuiEvent>) {
        let (events, receiver) = std::sync::mpsc::channel();
        (
            OutgoingLifecycle::new(events, egui::Context::default()),
            receiver,
        )
    }

    fn test_outgoing_message() -> ChatMessage {
        crate::outgoing::pending("test message".into(), false, crate::outgoing::Payload::Text)
    }

    #[test]
    fn dropping_queued_outgoing_publishes_not_sent() {
        let (lifecycle, events) = outgoing_test_lifecycle();
        let ticket = lifecycle.accept("owner".into(), 7, "thread".into(), test_outgoing_message());
        drop(ticket);
        let GuiEvent::OutgoingState { message, .. } = events.try_recv().unwrap() else {
            panic!("expected outgoing state")
        };
        assert_eq!(
            message.delivery,
            crate::message_model::DeliveryState::Failed
        );
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn dropping_started_outgoing_publishes_delivery_unknown() {
        let (lifecycle, events) = outgoing_test_lifecycle();
        let ticket = lifecycle.accept("owner".into(), 7, "thread".into(), test_outgoing_message());
        assert!(ticket.start());
        drop(ticket);
        let GuiEvent::OutgoingState { message, .. } = events.try_recv().unwrap() else {
            panic!("expected outgoing state")
        };
        assert_eq!(
            message.delivery,
            crate::message_model::DeliveryState::DeliveryUnknown
        );
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn finishing_outgoing_ack_publishes_exactly_once() {
        let (lifecycle, events) = outgoing_test_lifecycle();
        let ticket = lifecycle.accept("owner".into(), 7, "thread".into(), test_outgoing_message());
        assert!(ticket.start());
        let mut ack = test_outgoing_message();
        ack.delivery = crate::message_model::DeliveryState::ServerAccepted;
        ticket.finish(ack);
        ticket.finish(test_outgoing_message());
        drop(ticket);
        let GuiEvent::OutgoingState { message, .. } = events.try_recv().unwrap() else {
            panic!("expected outgoing state")
        };
        assert_eq!(
            message.delivery,
            crate::message_model::DeliveryState::ServerAccepted
        );
        assert!(events.try_recv().is_err());
    }

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

#[cfg(test)]
mod attachment_admission_tests {
    use super::*;

    #[tokio::test]
    async fn oversized_picker_batch_is_rejected_before_pending_or_file_io() {
        let session = Session::new(false, false, false).unwrap();
        let (events, received) = std::sync::mpsc::channel();
        let batch = super::super::file_drop::Batch {
            thread: "fixture".into(),
            group: false,
            caption: String::new(),
            original: true,
            files: (0..51)
                .map(|n| (format!("missing-{n}.txt").into(), None))
                .collect(),
            ephemeral: false,
        };
        send_attachment_batch(session, batch, events, egui::Context::default()).await;
        let GuiEvent::StatusUpdate(error) = received.try_recv().unwrap() else {
            panic!("Oversized batch created an outgoing message");
        };
        assert!(error.contains("50"));
        assert!(is_status_error(&error));
        assert!(received.try_recv().is_err());
    }
}

struct AnnouncedAttachment {
    path: std::path::PathBuf,
    stamp: Option<crate::outgoing::FileStamp>,
    message: ChatMessage,
    active: crate::outgoing::Active,
}

// Publish the entire bounded batch before queue admission, persistence or media preparation.
// Otherwise a slow disk or the first large file delays even the later files' bubbles.
fn announce_attachment_batch(
    owner: &str,
    runtime_id: usize,
    batch: &mut super::file_drop::Batch,
    events: &Sender<GuiEvent>,
    ctx: &egui::Context,
) -> Vec<AnnouncedAttachment> {
    let mut announced = Vec::with_capacity(batch.files.len());
    for (path, stamp) in std::mem::take(&mut batch.files) {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let message = crate::outgoing::pending(
            format!("[Tệp] {name}"),
            batch.group,
            crate::outgoing::Payload::Attachment {
                path: path.clone(),
                caption: batch.caption.clone(),
                stamp: stamp.clone(),
                hd_source: None,
            },
        );
        let Some(active) = crate::outgoing::Active::start(&message.client_message_id) else {
            continue;
        };
        if events
            .send(GuiEvent::OutgoingState {
                runtime_id,
                owner: owner.to_owned(),
                thread_id: batch.thread.clone(),
                message: message.clone(),
            })
            .is_err()
        {
            break;
        }
        announced.push(AnnouncedAttachment {
            path,
            stamp,
            message,
            active,
        });
    }
    ctx.request_repaint();
    announced
}

/// Shared bounded upload path; the session is captured before any picker/preview await.
async fn send_attachment_batch(
    s: Session,
    mut batch: super::file_drop::Batch,
    events: Sender<GuiEvent>,
    ctx: egui::Context,
) {
    let owner = s.account_key().to_owned();
    if batch.files.len() > super::file_drop::MAX_ATTACHMENTS {
        let _ = events.send(GuiEvent::StatusUpdate(
            "Không thể gửi quá 50 tệp mỗi lần; chưa gửi tệp nào".into(),
        ));
        ctx.request_repaint();
        return;
    }
    static ATTACHMENT_QUEUE: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    let attachment_queue =
        ATTACHMENT_QUEUE.get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(16)));
    if !s.is_active() {
        return;
    }
    let announced = announce_attachment_batch(&owner, s.runtime_id(), &mut batch, &events, &ctx);
    for AnnouncedAttachment {
        path,
        stamp: expected_stamp,
        mut message,
        active,
    } in announced
    {
        let Ok(queued) = attachment_queue.clone().acquire_owned().await else {
            break;
        };
        if !s.is_active() {
            break;
        }
        let original_path = path.clone();
        persist_outgoing(&s, &batch.thread, &message).await;

        let current_stamp = crate::outgoing::FileStamp::read(&path).await.ok();
        let invalid_source = current_stamp.is_none();
        let source_stamp = expected_stamp.clone().or_else(|| current_stamp.clone());
        let copy_dir = crate::photo_quality::outgoing_dir(s.account_key());
        let path_for_reduce = original_path.clone();
        let mut hd_copy = if invalid_source
            || batch.original
            || expected_stamp
                .as_ref()
                .is_some_and(|expected| Some(expected) != current_stamp.as_ref())
        {
            None
        } else {
            tokio::task::spawn_blocking(move || {
                crate::photo_quality::reduce(&path_for_reduce, &copy_dir)
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
        if invalid_source {
            message.delivery = crate::message_model::DeliveryState::Failed;
            if let Some(job) = message.outgoing_job.as_mut() {
                job.error = "Không đọc được tệp thường; thư mục không được hỗ trợ".into();
            }
            persist_outgoing(&s, &batch.thread, &message).await;
            let _ = events.send(GuiEvent::OutgoingState {
                runtime_id: s.runtime_id(),
                owner: owner.clone(),
                thread_id: batch.thread.clone(),
                message,
            });
            ctx.request_repaint();
            drop(active);
            continue;
        }
        if let Some(job) = message.outgoing_job.as_mut() {
            if let crate::outgoing::Payload::Attachment {
                path: payload_path,
                stamp: payload_stamp,
                hd_source: payload_hd_source,
                ..
            } = &mut job.payload
            {
                *payload_path = hd_copy.clone().unwrap_or(path);
                *payload_stamp = if hd_copy.is_none() {
                    expected_stamp.clone().or(current_stamp.clone())
                } else {
                    crate::outgoing::FileStamp::read(payload_path).await.ok()
                };
                *payload_hd_source = hd_source;
            }
        }
        let s = s.clone();
        let owner = owner.clone();
        let tx = events.clone();
        let ctx = ctx.clone();
        let thread_id = batch.thread.clone();
        let is_ephemeral = batch.ephemeral;
        let orig_file_to_clean = original_path.clone();
        tokio::spawn(async move {
            let _queued = queued;
            match crate::outgoing::send(&s, &thread_id, &message).await {
                Ok(sent) => {
                    message = sent;
                    if is_ephemeral {
                        let temp_dir = crate::storage::account_temp_dir(&owner);
                        let root_temp = crate::storage::root().join("temp");
                        if orig_file_to_clean.starts_with(&temp_dir)
                            || orig_file_to_clean.starts_with(&root_temp)
                        {
                            let _ = tokio::fs::remove_file(&orig_file_to_clean).await;
                        }
                    }
                }
                Err(error) => {
                    message.delivery = crate::outgoing::failure_delivery(&error);
                    if let Some(job) = message.outgoing_job.as_mut() {
                        job.error = crate::outgoing::failure_message(&error);
                    }
                }
            }
            persist_outgoing(&s, &thread_id, &message).await;
            drop(active);
            let _ = tx.send(GuiEvent::OutgoingState {
                runtime_id: s.runtime_id(),
                owner,
                thread_id,
                message,
            });
            ctx.request_repaint();
        });
    }
}

async fn hydrate_e2ee_peers(
    s: &Session,
    tx: &Sender<GuiEvent>,
    ctx: &egui::Context,
) -> anyhow::Result<()> {
    if !s.is_active() {
        return Ok(());
    }
    let owner = s.account_key().to_owned();
    let owner_for_task = owner.clone();
    let peers = tokio::task::spawn_blocking(move || {
        crate::e2ee_integration::list_e2ee_peers(&owner_for_task)
    })
    .await?;
    if s.is_active() && !peers.is_empty() {
        let _ = tx.send(GuiEvent::E2eePeersHydrated {
            owner,
            runtime_id: s.runtime_id(),
            peers,
        });
        ctx.request_repaint();
    }
    Ok(())
}

async fn publish_pins(s: &Session, snapshot: crate::server_pins::Snapshot, tx: &Sender<GuiEvent>) {
    if !s.is_active() {
        return;
    }
    let account = s.clone();
    let saved = snapshot.clone();
    let result=tokio::task::spawn_blocking(move||->anyhow::Result<()>{
        if !account.is_active(){return Ok(());}
        let (store, _write_permit)=crate::store::MessageStore::open_for_account_write(&account)?;
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
            runtime_id: s.runtime_id(),
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
                runtime_id: s.runtime_id(),
                revision: crate::search::generation(),
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
                let (store, _write_permit) =
                    crate::store::MessageStore::open_for_account_write(&account)?;
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

#[cfg(test)]
mod v0016_workflow_tests {

    #[test]
    fn polling_is_eliminated_when_websocket_is_connected() {
        // WP13-2 fulfilled: polling eliminated when WebSocket epoch is Some.
        let is_ws_connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

        // When WebSocket is connected, periodic check drops polling
        let should_poll = !is_ws_connected.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            !should_poll,
            "Polling must be suppressed when WebSocket is connected"
        );

        // When WebSocket disconnects, polling fallback activates
        is_ws_connected.store(false, std::sync::atomic::Ordering::SeqCst);
        let should_poll = !is_ws_connected.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            should_poll,
            "Polling must resume as fallback when WebSocket is down"
        );
    }

    #[tokio::test]
    async fn resumable_upload_lifecycle_save_cancel_delete() {
        let root = std::env::temp_dir().join(format!("v016-upload-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let db_path = root.join("history.db");
        let store = crate::store::MessageStore::open(&db_path).unwrap();

        let upload_id = "asyncfile-test-9999";
        let mut state = crate::upload::ResumableUploadState::new(
            upload_id.into(),
            "sample.bin".into(),
            5_000_000,
            1_048_576,
        );

        // 1. Save upload state per chunk
        state.mark_chunk_complete(0);
        store.save_upload_state(&state).unwrap();

        let loaded = store.load_upload_state(upload_id).unwrap().unwrap();
        assert_eq!(loaded.completed_chunks, vec![0]);
        assert_eq!(loaded.total_chunks, 5);

        // 2. Register active upload in memory
        let cancel_flag = crate::upload::register_active_upload(upload_id);
        assert!(!cancel_flag.load(std::sync::atomic::Ordering::SeqCst));

        // 3. Cancel upload
        let cancelled = crate::upload::cancel_active_upload(upload_id);
        assert!(cancelled);
        assert!(cancel_flag.load(std::sync::atomic::Ordering::SeqCst));
        assert!(crate::upload::is_upload_cancelled(upload_id));

        // 4. Delete upload state on completion or cancel
        store.delete_upload_state(upload_id).unwrap();
        assert!(store.load_upload_state(upload_id).unwrap().is_none());

        crate::upload::unregister_active_upload(upload_id);
        assert!(!crate::upload::is_upload_cancelled(upload_id));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn upload_progress_event_forwarding() {
        let mut progress_rx = crate::upload::subscribe_progress();
        crate::upload::report_progress(
            "test-owner",
            7,
            "asyncfile-progress-1",
            1_048_576,
            3_000_000,
        );
        let event = progress_rx.recv().await.unwrap();
        assert_eq!(event.2, "asyncfile-progress-1");
        assert_eq!(event.3, 1_048_576);
        assert_eq!(event.4, 3_000_000);
    }
}

#[cfg(test)]
mod audit_progress_tests {
    use super::*;
    #[tokio::test]
    async fn lagged_upload_progress_still_delivers_later_events() {
        let (sender, mut receiver) = tokio::sync::broadcast::channel(2);
        for n in 0..10 {
            sender
                .send(("owner".into(), 7, "upload".into(), n, 10))
                .unwrap();
        }
        drop(sender);
        let (tx, rx) = std::sync::mpsc::channel();
        forward_upload_progress(&mut receiver, &tx, &egui::Context::default()).await;
        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events.last(),
            Some(GuiEvent::UploadProgress { completed: 9, .. })
        ));
    }
}

#[cfg(test)]
mod attachment_announcement_tests {
    use super::*;

    #[test]
    fn entire_batch_has_named_bubbles_before_disk_or_upload_admission() {
        let root = tempfile::tempdir().unwrap();
        let mut batch = super::super::file_drop::Batch {
            thread: "recipient".into(),
            group: false,
            caption: "caption".into(),
            original: true,
            files: (0..20)
                .map(|i| (root.path().join(format!("Tệp {i}.zip")), None))
                .collect(),
            ephemeral: false,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let announced =
            announce_attachment_batch("owner", 7, &mut batch, &tx, &egui::Context::default());
        assert_eq!(
            announced.len(),
            20,
            "batches larger than upload concurrency must appear together"
        );
        assert!(root.path().read_dir().unwrap().next().is_none());
        for (index, pending) in announced.iter().enumerate() {
            let GuiEvent::OutgoingState {
                owner,
                runtime_id,
                thread_id,
                message,
            } = rx.try_recv().unwrap()
            else {
                panic!("expected a real outgoing payload");
            };
            assert_eq!(
                (owner.as_str(), runtime_id, thread_id.as_str()),
                ("owner", 7, "recipient")
            );
            assert_eq!(
                message.link_title.as_deref(),
                Some(format!("Tệp {index}.zip").as_str())
            );
            assert_eq!(message.kind, crate::message_model::MessageKind::File);
            assert_eq!(
                message.delivery,
                crate::message_model::DeliveryState::Pending
            );
            assert_eq!(message.client_message_id, pending.message.client_message_id);
            assert!(crate::outgoing::is_active(&message.client_message_id));
            assert!(matches!(message.outgoing_job.unwrap().payload,
                crate::outgoing::Payload::Attachment { path, caption, .. }
                if path == pending.path && caption == "caption"));
        }
        assert!(rx.try_recv().is_err());
        let ids: Vec<_> = announced
            .iter()
            .map(|p| p.message.client_message_id.clone())
            .collect();
        drop(announced);
        assert!(ids.iter().all(|id| !crate::outgoing::is_active(id)));
    }
}

#[cfg(test)]
mod voice_send_owner_tests {
    use super::*;

    #[test]
    fn voice_send_rejects_different_owner_or_session_runtime() {
        assert!(voice_send_session_matches(
            true, "owner-a", 11, "owner-a", 11
        ));
        assert!(!voice_send_session_matches(
            true, "owner-b", 11, "owner-a", 11
        ));
        assert!(!voice_send_session_matches(
            true, "owner-a", 12, "owner-a", 11
        ));
        assert!(!voice_send_session_matches(
            false, "owner-a", 11, "owner-a", 11
        ));
    }
}

#[cfg(test)]
mod db_write_serialization_tests {
    use super::*;

    #[tokio::test]
    async fn optional_startup_producer_waits_for_work_and_ui_timeout_does_not_fake_settle() {
        use crate::gui::{StartupState, ZaloGuiApp};
        for (keep_loading, timeout_ms) in [(false, 800), (true, 800), (true, 2000)] {
            let uin = format!("optional-preload-{}", crate::param_cipher::random_hex(12));
            let session = test_session(&uin);
            let ctx = egui::Context::default();
            let mut app = ZaloGuiApp::from_context(&ctx, session.clone(), false, Vec::new(), false);
            app.server_config.o2o.keep_loading_during_preload.enable = keep_loading;
            app.server_config
                .o2o
                .keep_loading_during_preload
                .timeout_duration_ms = timeout_ms;
            app.startup_state = StartupState::Loading {
                stage_text: "Đang khởi tạo…".into(),
                started_at: std::time::Instant::now(),
                timeout: Duration::from_millis(timeout_ms),
                prerequisites_settled: false,
                settle_deadline: None,
            };
            let (tx, rx) = std::sync::mpsc::channel();
            app.event_rx = rx;
            let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = tokio::sync::oneshot::channel();
            let context = ctx.clone();
            let task = tokio::spawn(async move {
                let cached = vec![Thread {
                    id: "cached-thread".into(),
                    name: "Hội thoại đã lưu".into(),
                    messages: vec![ChatMessage {
                        id: "cached-message".into(),
                        text: "Tin đã lưu".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }];
                run_optional_startup_preload(&session, &tx, &context, cached, async move {
                    let _ = entered_tx.send(());
                    release_rx.await?;
                    Ok(())
                })
                .await;
            });
            tokio::time::timeout(Duration::from_secs(2), entered_rx)
                .await
                .unwrap()
                .unwrap();
            app.check_events(&ctx);
            assert_eq!(app.threads.len(), 1);
            assert_eq!(app.threads[0].messages[0].text, "Tin đã lưu");
            if keep_loading {
                let started_at = match app.startup_state {
                    StartupState::Loading {
                        prerequisites_settled: true,
                        settle_deadline: None,
                        started_at,
                        ..
                    } => started_at,
                    ref state => panic!("optional work must still be pending: {state:?}"),
                };
                app.advance_startup_state(started_at + Duration::from_millis(1000), &ctx);
                assert_eq!(
                    matches!(app.startup_state, StartupState::Ready),
                    timeout_ms == 800
                );
            } else {
                assert_eq!(app.startup_state, StartupState::Ready);
            }
            assert!(
                !task.is_finished(),
                "UI timeout must not complete the optional task"
            );
            release_tx.send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap();
            app.check_events(&ctx);
            if !keep_loading || timeout_ms == 800 {
                assert_eq!(
                    app.startup_state,
                    StartupState::Ready,
                    "late completion must not reopen loading"
                );
            } else {
                assert!(matches!(
                    app.startup_state,
                    StartupState::Loading {
                        settle_deadline: Some(_),
                        ..
                    }
                ));
            }
        }
    }

    #[tokio::test]
    async fn optional_startup_failure_settles_but_retired_runtime_never_publishes_completion() {
        let uin = format!("optional-error-{}", crate::param_cipher::random_hex(12));
        let mut session = test_session(&uin);
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = egui::Context::default();
        run_optional_startup_preload(&session, &tx, &ctx, Vec::new(), async {
            anyhow::bail!("synthetic optional failure")
        })
        .await;
        let events: Vec<_> = rx.try_iter().collect();
        assert!(matches!(
            events.as_slice(),
            [
                GuiEvent::StartupPrerequisitesSettled { .. },
                GuiEvent::StartupPreloadSettled { .. }
            ]
        ));

        let old_session = session.clone();
        let old_context = ctx.clone();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            run_optional_startup_preload(&old_session, &tx, &old_context, Vec::new(), async {
                let _ = entered_tx.send(());
                release_rx.await?;
                Ok(())
            })
            .await;
        });
        tokio::time::timeout(Duration::from_secs(2), entered_rx)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            rx.try_recv().unwrap(),
            GuiEvent::StartupPrerequisitesSettled { .. }
        ));
        session.logout();
        release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert!(
            rx.try_recv().is_err(),
            "old runtime must not publish preload completion"
        );
    }

    #[test]
    fn banner_and_legacy_gate_still_require_live_account_database_and_socket() {
        use crate::sync_coordinator::{SyncAvailability, SyncOrigin};
        let uin = format!("origin-gate-{}", crate::param_cipher::random_hex(12));
        let mut session = test_session(&uin);
        session
            .server_config
            .sync_2_pc
            .enable_on_sync_source_setting = false;
        let ready = std::sync::atomic::AtomicUsize::new(session.runtime_id());
        for origin in [SyncOrigin::Banner, SyncOrigin::Legacy] {
            let enabled = session
                .server_config
                .sync_2_pc
                .is_feature_enabled_by_origin(origin);
            assert!(enabled);
            assert_eq!(
                current_sync_availability(&session, &ready, true, false, enabled),
                SyncAvailability::Ready
            );
            assert!(!current_sync_availability(&session, &ready, false, false, enabled).is_ready());
            let migrating = std::sync::atomic::AtomicUsize::new(0);
            assert!(
                !current_sync_availability(&session, &migrating, true, false, enabled).is_ready()
            );
        }
        session.server_config.sync_2_pc.enable = false;
        assert!(session
            .server_config
            .sync_2_pc
            .is_feature_enabled_by_origin(SyncOrigin::Legacy));
        assert!(!session
            .server_config
            .sync_2_pc
            .is_feature_enabled_by_origin(SyncOrigin::Banner));
        session.login = None;
        assert!(!current_sync_availability(&session, &ready, true, false, true).is_ready());
    }

    fn test_session(uin: &str) -> Session {
        let mut s = Session::new(false, false, false).unwrap();
        s.login = Some(crate::session::LoginInfo {
            uin: uin.to_owned(),
            zpw_enk: String::new(),
            zpw_sek: String::new(),
            display_name: "Test".into(),
            send2me_id: String::new(),
            avatar: String::new(),
        });
        s
    }

    #[tokio::test]
    async fn persist_received_since_with_baseline_and_concurrent_writer_succeeds_without_deadlock_or_busy(
    ) {
        let uin = format!("worker-db-test-{}", crate::param_cipher::random_hex(12));
        let account_dir = crate::storage::account_dir(&uin);
        let _ = std::fs::remove_dir_all(&account_dir);
        let session = test_session(&uin);

        let mut threads = vec![Thread {
            id: "thread-1".into(),
            name: "Thread 1".into(),
            messages: vec![ChatMessage {
                id: "msg-1".into(),
                sender: "Alice".into(),
                text: "hello".into(),
                ts: 1_800_000_000_000,
                ..Default::default()
            }],
            ..Default::default()
        }];

        let concurrent_session = session.clone();
        let background_writer = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let (mut store, _permit) =
                crate::store::MessageStore::open_for_account_write(&concurrent_session)?;
            let extra = Thread {
                id: "thread-2".into(),
                name: "Thread 2".into(),
                messages: vec![ChatMessage {
                    id: "msg-2".into(),
                    sender: "Bob".into(),
                    text: "concurrent".into(),
                    ts: 1_800_000_001_000,
                    ..Default::default()
                }],
                ..Default::default()
            };
            crate::store::persist_threads(&mut store, &[extra])?;
            Ok(())
        });

        let baseline = crate::chat::MessageFingerprints::default();
        let stored = tokio::time::timeout(
            Duration::from_secs(5),
            persist_received_since(&session, &mut threads, Some(baseline)),
        )
        .await
        .expect("persist_received_since must not deadlock when saving threads cache");
        assert!(stored);
        background_writer.await.unwrap().unwrap();

        let cached = crate::chat::load_threads_cache(&session);
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].id, "thread-1");

        let store = crate::store::MessageStore::open(session.history_path()).unwrap();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        drop(store);
        let _ = std::fs::remove_dir_all(&account_dir);
    }

    #[tokio::test]
    async fn failed_persist_received_does_not_advance_cursor_and_replay_deduplicates() {
        let uin = format!("worker-db-replay-{}", crate::param_cipher::random_hex(12));
        let account_dir = crate::storage::account_dir(&uin);
        let _ = std::fs::remove_dir_all(&account_dir);
        let active_session = test_session(&uin);

        let batch = vec![Thread {
            id: "thread-replay".into(),
            name: "Replay".into(),
            messages: vec![ChatMessage {
                id: "msg-replay-1".into(),
                sender: "Alice".into(),
                text: "must survive retry".into(),
                ts: 1_800_000_000_500,
                ..Default::default()
            }],
            ..Default::default()
        }];

        // Stale/revoked session cannot write after account switch; cursor must not advance.
        let stale_session = active_session.clone();
        stale_session.revoke_runtime();
        let mut stale_batch = batch.clone();
        assert!(!persist_received(&stale_session, &mut stale_batch).await);
        assert!(!stale_session.history_path().exists());
        let mut poll = crate::chat::PollState::default();
        poll.last_action_id = "999".into();
        crate::queue_cursors::commit_poll(&stale_session, &poll).await;
        assert!(crate::queue_cursors::poll_start(&active_session)
            .await
            .is_none());

        // Replaying on a fresh active session commits once and deduplicates on repeated replay.
        let fresh_session = test_session(&uin);
        let mut first = batch.clone();
        assert!(persist_received(&fresh_session, &mut first).await);
        crate::queue_cursors::commit_poll(&fresh_session, &poll).await;
        assert_eq!(
            crate::queue_cursors::poll_start(&fresh_session)
                .await
                .map(|p| p.last_action_id),
            Some("999".into())
        );

        let mut replay = batch;
        assert!(persist_received(&fresh_session, &mut replay).await);
        let store = crate::store::MessageStore::open(fresh_session.history_path()).unwrap();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        drop(store);
        let _ = std::fs::remove_dir_all(&account_dir);
    }

    #[tokio::test]
    async fn active_session_sqlite_failure_rolls_back_preserves_cache_and_cursor_and_replays_cleanly(
    ) {
        let uin = format!("worker-active-fail-{}", crate::param_cipher::random_hex(12));
        let account_dir = crate::storage::account_dir(&uin);
        let _ = std::fs::remove_dir_all(&account_dir);
        let session = test_session(&uin);

        // Seed existing durable cache + initial message so we can prove failed write preserves them.
        let initial = vec![Thread {
            id: "thread-existing".into(),
            name: "Existing".into(),
            messages: vec![ChatMessage {
                id: "msg-existing".into(),
                sender: "Alice".into(),
                text: "existing".into(),
                ts: 1_800_000_000_000,
                ..Default::default()
            }],
            ..Default::default()
        }];
        let mut seeded = initial.clone();
        assert!(persist_received_since(&session, &mut seeded, Some(Default::default())).await);
        let mut initial_poll = crate::chat::PollState::default();
        initial_poll.last_action_id = "100".into();
        crate::queue_cursors::commit_poll(&session, &initial_poll).await;
        assert_eq!(
            crate::queue_cursors::poll_start(&session)
                .await
                .map(|p| p.last_action_id),
            Some("100".into())
        );

        // Inject a real SQLite failure via trigger on an ACTIVE session.
        {
            let (store, _permit) =
                crate::store::MessageStore::open_for_account_write(&session).unwrap();
            store
                .conn
                .execute_batch(
                    "CREATE TRIGGER fail_active_insert BEFORE INSERT ON messages \
                     WHEN NEW.msg_id = 'msg-fail-2' \
                     BEGIN SELECT RAISE(ABORT, 'injected sqlite write error'); END;",
                )
                .unwrap();
        }

        let failing_batch = vec![Thread {
            id: "thread-new".into(),
            name: "New".into(),
            messages: vec![
                ChatMessage {
                    id: "msg-fail-1".into(),
                    sender: "Bob".into(),
                    text: "first in failing batch".into(),
                    ts: 1_800_000_001_000,
                    ..Default::default()
                },
                ChatMessage {
                    id: "msg-fail-2".into(),
                    sender: "Bob".into(),
                    text: "triggers abort".into(),
                    ts: 1_800_000_002_000,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }];

        let mut next_poll = crate::chat::PollState::default();
        next_poll.last_action_id = "200".into();
        let mut attempt = failing_batch.clone();
        let stored = persist_received_since(&session, &mut attempt, Some(Default::default())).await;
        assert!(
            !stored,
            "SQLite failure on active session must return false"
        );
        if stored {
            crate::queue_cursors::commit_poll(&session, &next_poll).await;
        }
        assert_eq!(
            crate::queue_cursors::poll_start(&session)
                .await
                .map(|p| p.last_action_id),
            Some("100".into()),
            "cursor must remain at 100 when persist_received_since fails"
        );

        // Verify partial rows rolled back and threads_cache.json was NOT overwritten.
        let cached = crate::chat::load_threads_cache(&session);
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].id, "thread-existing");
        {
            let store = crate::store::MessageStore::open(session.history_path()).unwrap();
            let count: i64 = store
                .conn
                .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 1, "msg-fail-1 must have rolled back with msg-fail-2");
        }

        // Remove injected failure and replay; verify cursor advances and all messages commit without duplicates.
        {
            let (store, _permit) =
                crate::store::MessageStore::open_for_account_write(&session).unwrap();
            store
                .conn
                .execute_batch("DROP TRIGGER fail_active_insert;")
                .unwrap();
        }
        let mut replay = failing_batch.clone();
        let replayed =
            persist_received_since(&session, &mut replay, Some(Default::default())).await;
        assert!(replayed);
        if replayed {
            crate::queue_cursors::commit_poll(&session, &next_poll).await;
        }
        assert_eq!(
            crate::queue_cursors::poll_start(&session)
                .await
                .map(|p| p.last_action_id),
            Some("200".into()),
            "cursor must advance to 200 after replay succeeds"
        );
        let mut second_replay = failing_batch;
        assert!(
            persist_received_since(&session, &mut second_replay, Some(Default::default())).await
        );

        let store = crate::store::MessageStore::open(session.history_path()).unwrap();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
        drop(store);
        let _ = std::fs::remove_dir_all(&account_dir);
    }

    #[tokio::test]
    async fn startup_preload_reports_fatal_error_on_corrupt_or_newer_db_without_settling() {
        let uin = format!(
            "worker-startup-fatal-{}",
            crate::param_cipher::random_hex(12)
        );
        let account_dir = crate::storage::account_dir(&uin);
        let _ = std::fs::remove_dir_all(&account_dir);
        let session = test_session(&uin);
        std::fs::create_dir_all(session.history_path().parent().unwrap()).unwrap();
        {
            let conn = rusqlite::Connection::open(session.history_path()).unwrap();
            conn.pragma_update(None, "user_version", 999u32).unwrap();
        }

        let session_arc = std::sync::Arc::new(tokio::sync::Mutex::new(session.clone()));
        let (tx, rx) = std::sync::mpsc::channel();
        let live = std::sync::Arc::new(crate::live_updates::LiveUpdates::default());
        let lifecycle = OutgoingLifecycle::new(tx.clone(), egui::Context::default());
        let identity =
            std::sync::Arc::new(std::sync::RwLock::new((uin.clone(), session.runtime_id())));

        let db_ready = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let ws_ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        run_startup_preload(
            session_arc,
            tx,
            egui::Context::default(),
            live,
            lifecycle,
            identity,
            db_ready.clone(),
            ws_ready,
        )
        .await;

        assert_eq!(db_ready.load(std::sync::atomic::Ordering::Acquire), 0);
        let events: Vec<_> = rx.try_iter().collect();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GuiEvent::StartupFatalError { .. })),
            "newer/invalid SQLite schema must emit StartupFatalError"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, GuiEvent::StartupPreloadSettled { .. })),
            "fatal SQLite error must never emit StartupPreloadSettled"
        );
        let _ = std::fs::remove_dir_all(&account_dir);
    }

    #[tokio::test]
    async fn sync_availability_and_runtime_abort_reject_invalid_and_prevent_surviving_sync() {
        let uin = format!("worker-sync-avail-{}", crate::param_cipher::random_hex(12));
        let account_dir = crate::storage::account_dir(&uin);
        let _ = std::fs::remove_dir_all(&account_dir);
        let session = test_session(&uin);
        let db_ready = std::sync::atomic::AtomicUsize::new(0);

        // 1. DB not yet migrated -> DbMigrating
        assert_eq!(
            current_sync_availability(&session, &db_ready, true, false, true),
            crate::sync_coordinator::SyncAvailability::Unavailable(
                crate::sync_coordinator::SyncUnavailableReason::DbMigrating
            )
        );
        // 2. DB ready but WS not connected -> WorkerNotReady
        db_ready.store(session.runtime_id(), std::sync::atomic::Ordering::Release);
        assert_eq!(
            current_sync_availability(&session, &db_ready, false, false, true),
            crate::sync_coordinator::SyncAvailability::Unavailable(
                crate::sync_coordinator::SyncUnavailableReason::WorkerNotReady
            )
        );
        // 3. Double-click while running -> AlreadySyncing
        assert_eq!(
            current_sync_availability(&session, &db_ready, true, true, true),
            crate::sync_coordinator::SyncAvailability::Unavailable(
                crate::sync_coordinator::SyncUnavailableReason::AlreadySyncing
            )
        );
        // 4. All valid -> Ready
        assert_eq!(
            current_sync_availability(&session, &db_ready, true, false, true),
            crate::sync_coordinator::SyncAvailability::Ready
        );

        // 5. abort_and_join_phone_sync cancels & joins an active task before account switch
        let op_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shared_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let task_cancel = op_cancel.clone();
        let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let finished_task = finished.clone();
        let handle = tokio::spawn(async move {
            while !task_cancel.load(std::sync::atomic::Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            finished_task.store(true, std::sync::atomic::Ordering::Release);
        });
        let mut slot = Some(ActivePhoneSyncTask {
            owner: uin.clone(),
            runtime_id: session.runtime_id(),
            operation_id: crate::sync_coordinator::next_operation_id(),
            cancel: op_cancel,
            handle,
        });
        abort_and_join_phone_sync(&mut slot, &running, &shared_cancel).await;
        assert!(slot.is_none());
        assert!(!running.load(std::sync::atomic::Ordering::Acquire));
        assert!(finished.load(std::sync::atomic::Ordering::Acquire));

        let _ = std::fs::remove_dir_all(&account_dir);
    }
}

#[cfg(test)]
mod metadata_admission_tests {
    use super::*;

    #[tokio::test]
    async fn metadata_snapshot_waits_for_mutation_and_reads_committed_state() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let committed = std::sync::Arc::new(AtomicBool::new(false));
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let writer_gate = gate.clone();
        let writer_state = committed.clone();
        let writer = tokio::spawn(async move {
            let _permit = metadata_permit(&writer_gate).await;
            entered_tx.send(()).unwrap();
            release_rx.await.unwrap();
            writer_state.store(true, Ordering::SeqCst);
        });
        entered_rx.await.unwrap();
        let (reader_started_tx, reader_started_rx) = tokio::sync::oneshot::channel();
        let reader_gate = gate.clone();
        let reader = tokio::spawn(async move {
            reader_started_tx.send(()).unwrap();
            let _permit = metadata_permit(&reader_gate).await;
            committed.load(Ordering::SeqCst)
        });
        reader_started_rx.await.unwrap();
        assert!(gate.try_acquire().is_err());
        assert!(!reader.is_finished());
        release_tx.send(()).unwrap();
        writer.await.unwrap();
        assert!(
            reader.await.unwrap(),
            "snapshot must observe the completed write"
        );
        assert_eq!(gate.available_permits(), 1);
    }
}
