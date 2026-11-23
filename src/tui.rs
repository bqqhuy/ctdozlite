//! Responsive TUI interface powered by Ratatui & Crossterm.
//!
//! Design goals:
//! - Non-blocking: every network call runs in a `tokio::spawn` worker and reports
//!   back over an mpsc channel, so the 30 ms render loop never stalls.
//! - ASCII-safe chrome (single-width borders and tags) for Windows consoles while
//!   keeping original UTF-8 message content.
//! - GUI parity: search/filter/unread, scrollback, quote, reactions, delete,
//!   attachments, sticker store, group history and the dev log inspector.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::chat::{self, ChatMessage, StickerItem, StickerPackInfo, Thread};
use crate::logger::{self, FindingLevel};
use crate::session::Session;

const POLL_SECS: u64 = 4;
const WRITER_SHUTDOWN_BUDGET: Duration = Duration::from_secs(5);

/// Quick-reaction palette for keys `1`..`6`, same order as the GUI picker.
const QUICK_REACTIONS: [&str; 6] = [
    "\u{1F44D}",
    "\u{2764}\u{FE0F}",
    "\u{1F606}",
    "\u{1F62E}",
    "\u{1F622}",
    "\u{1F620}",
];

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Focus {
    Threads,
    Messages,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterTab {
    All,
    Direct,
    Group,
}

impl FilterTab {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Direct,
            Self::Direct => Self::Group,
            Self::Group => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "Tat ca",
            Self::Direct => "Ca nhan",
            Self::Group => "Nhom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Help,
    Log,
    Sticker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    Attachment,
    HiddenPin,
}

struct PromptState {
    kind: PromptKind,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmKind {
    Logout,
    DeleteMessage,
    RetryMessage,
}

struct ConfirmAction {
    question: String,
    detail: String,
    kind: ConfirmKind,
    thread_id: String,
    msg_id: String,
}

#[derive(Default)]
struct StickerState {
    packs: Vec<StickerPackInfo>,
    items: Vec<StickerItem>,
    pack_idx: usize,
    item_idx: usize,
    stage: u8,
    loading: bool,
    loaded_name: String,
}

// These infrequent terminal events retain the existing TUI ownership boundary.
#[allow(clippy::large_enum_variant)]
enum BgEvent {
    Hydrated(Vec<Thread>),
    PollDone {
        threads: Vec<Thread>,
        state: chat::PollState,
    },
    PollFailed(String),
    ThreadsRefreshed(Vec<Thread>),
    HistoryLoaded {
        thread: Thread,
    },
    SendOutcome {
        thread_id: String,
        client_message_id: String,
        outcome: SendDisposition,
    },
    ReactionDone {
        thread_id: String,
        msg_id: String,
        icon: String,
    },
    DeleteDone {
        thread_id: String,
        deleted: DeletedIdentity,
        result: std::result::Result<(), String>,
    },
    DeleteFailed(String),
    HiddenStateChanged(std::result::Result<(), String>),
    StickerPacks(Vec<StickerPackInfo>),
    StickerItems(Vec<StickerItem>),
    ActionFailed(String),
    StoreFailed(String),
    StoreSaved,
    LogoutDone {
        owner: String,
        result: std::result::Result<(), String>,
    },
}

enum SendDisposition {
    Accepted(Box<ChatMessage>),
    Rejected(String),
    Unknown(String),
    NotSent(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeStamp {
    owner: String,
    runtime_id: usize,
}

impl RuntimeStamp {
    fn for_session(session: &Session) -> Self {
        Self {
            owner: session.account_key().to_owned(),
            runtime_id: session.runtime_id(),
        }
    }

    fn matches(&self, session: &Session) -> bool {
        session.is_active()
            && self.owner == session.account_key()
            && self.runtime_id == session.runtime_id()
    }
}

struct BgEnvelope {
    stamp: RuntimeStamp,
    event: BgEvent,
}

#[derive(Clone)]
struct BgSender {
    tx: Sender<BgEnvelope>,
    stamp: RuntimeStamp,
}

impl BgSender {
    /// `Err` means the UI receiver is gone; the undelivered event is dropped.
    fn send(&self, event: BgEvent) -> std::result::Result<(), ()> {
        self.tx
            .send(BgEnvelope {
                stamp: self.stamp.clone(),
                event,
            })
            .map_err(|_| ())
    }
}

enum StoreCommand {
    Hydrate {
        session: Session,
        stamp: RuntimeStamp,
        threads: Vec<Thread>,
        completion: BgSender,
    },
    Snapshot {
        session: Session,
        stamp: RuntimeStamp,
        threads: Vec<Thread>,
        completion: BgSender,
    },
    Delete {
        session: Session,
        stamp: RuntimeStamp,
        thread_id: String,
        message: Box<ChatMessage>,
        threads: Vec<Thread>,
        reply: tokio::sync::oneshot::Sender<std::result::Result<(), String>>,
    },
    Outgoing {
        session: Session,
        stamp: RuntimeStamp,
        thread: Box<Thread>,
        reply: tokio::sync::oneshot::Sender<std::result::Result<(), String>>,
    },
    Shutdown {
        reply: tokio::sync::oneshot::Sender<()>,
    },
    #[cfg(test)]
    PanicForTest,
}

#[derive(Clone)]
struct StoreWriter {
    inner: Arc<StoreWriterInner>,
}

struct StoreWriterInner {
    tx: SyncSender<StoreEnvelope>,
    slots: Arc<tokio::sync::Semaphore>,
    accepting: AtomicBool,
    available: AtomicBool,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    done: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

struct StoreEnvelope {
    command: StoreCommand,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreAdmissionError {
    Full,
    Disconnected,
}

struct CriticalTask(Arc<AtomicUsize>);

impl CriticalTask {
    fn start(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self(counter.clone())
    }
}

impl Drop for CriticalTask {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl StoreWriter {
    const CAPACITY: usize = 16;

    fn new() -> Self {
        let (writer, rx) = Self::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        *writer
            .inner
            .done
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(done_rx);
        let spawned = std::thread::Builder::new()
            .name("tui-history-writer".into())
            .spawn(move || store_writer_loop(rx, done_tx));
        match spawned {
            Ok(worker) => {
                *writer
                    .inner
                    .worker
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(worker)
            }
            Err(_) => writer.inner.available.store(false, Ordering::Release),
        }
        writer
    }

    fn channel() -> (Self, Receiver<StoreEnvelope>) {
        let (tx, rx) = sync_channel(Self::CAPACITY);
        (
            Self {
                inner: Arc::new(StoreWriterInner {
                    tx,
                    slots: Arc::new(tokio::sync::Semaphore::new(Self::CAPACITY)),
                    accepting: AtomicBool::new(true),
                    available: AtomicBool::new(true),
                    worker: Mutex::new(None),
                    done: Mutex::new(None),
                }),
            },
            rx,
        )
    }

    /// Holds one admission slot while the remote delete is in flight.
    fn try_reserve_delete(
        &self,
    ) -> std::result::Result<tokio::sync::OwnedSemaphorePermit, StoreAdmissionError> {
        if !self.inner.available.load(Ordering::Acquire)
            || !self.inner.accepting.load(Ordering::Acquire)
        {
            return Err(StoreAdmissionError::Disconnected);
        }
        self.inner
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| StoreAdmissionError::Full)
    }

    fn try_send(&self, command: StoreCommand) -> std::result::Result<(), StoreAdmissionError> {
        let permit = self.try_reserve_delete()?;
        self.enqueue_reserved(command, permit)
    }

    fn enqueue_reserved(
        &self,
        command: StoreCommand,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> std::result::Result<(), StoreAdmissionError> {
        if !self.inner.available.load(Ordering::Acquire)
            || (!self.inner.accepting.load(Ordering::Acquire)
                && !matches!(&command, StoreCommand::Shutdown { .. }))
        {
            return Err(StoreAdmissionError::Disconnected);
        }
        match self.inner.tx.try_send(StoreEnvelope {
            command,
            _permit: permit,
        }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(StoreAdmissionError::Full),
            Err(TrySendError::Disconnected(_)) => Err(StoreAdmissionError::Disconnected),
        }
    }

    async fn shutdown_until(&self, deadline: tokio::time::Instant) -> bool {
        self.inner.accepting.store(false, Ordering::Release);
        if !self.inner.available.load(Ordering::Acquire) {
            return false;
        }
        let response;
        loop {
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            match self.inner.slots.clone().try_acquire_owned() {
                Ok(permit) => {
                    let (reply, wait) = tokio::sync::oneshot::channel();
                    match self.inner.tx.try_send(StoreEnvelope {
                        command: StoreCommand::Shutdown { reply },
                        _permit: permit,
                    }) {
                        Ok(()) => {
                            response = wait;
                            break;
                        }
                        Err(TrySendError::Full(envelope)) => drop(envelope),
                        Err(TrySendError::Disconnected(_)) => {
                            self.join_if_finished();
                            return false;
                        }
                    }
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
        if !matches!(
            tokio::time::timeout_at(deadline, response).await,
            Ok(Ok(()))
        ) {
            self.join_if_finished();
            return false;
        }
        let done = self
            .inner
            .done
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        let Some(done) = done else {
            self.join_if_finished();
            return false;
        };
        if !matches!(tokio::time::timeout_at(deadline, done).await, Ok(Ok(()))) {
            self.join_if_finished();
            return false;
        }
        while !self.worker_finished() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        self.join_if_finished()
    }

    fn worker_finished(&self) -> bool {
        self.inner
            .worker
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .is_none_or(std::thread::JoinHandle::is_finished)
    }

    fn join_if_finished(&self) -> bool {
        let mut worker = self
            .inner
            .worker
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(handle) = worker.as_ref() else {
            return true;
        };
        if !handle.is_finished() {
            return false;
        }
        worker
            .take()
            .expect("worker handle checked above")
            .join()
            .is_ok()
    }
}

struct WriterDone(Option<tokio::sync::oneshot::Sender<()>>);

impl Drop for WriterDone {
    fn drop(&mut self) {
        if let Some(done) = self.0.take() {
            let _ = done.send(());
        }
    }
}

fn store_writer_loop(rx: Receiver<StoreEnvelope>, done: tokio::sync::oneshot::Sender<()>) {
    let _done = WriterDone(Some(done));
    let mut path: Option<PathBuf> = None;
    let mut store: Option<crate::store::MessageStore> = None;
    while let Ok(envelope) = rx.recv() {
        let StoreEnvelope { command, _permit } = envelope;
        let command = match command {
            StoreCommand::Shutdown { reply } => {
                let _ = reply.send(());
                break;
            }
            command => command,
        };
        #[cfg(test)]
        if matches!(&command, StoreCommand::PanicForTest) {
            panic!("injected writer panic");
        }
        let (session, stamp) = match &command {
            StoreCommand::Hydrate { session, stamp, .. }
            | StoreCommand::Snapshot { session, stamp, .. }
            | StoreCommand::Delete { session, stamp, .. }
            | StoreCommand::Outgoing { session, stamp, .. } => (session, stamp),
            StoreCommand::Shutdown { .. } => unreachable!("shutdown is handled above"),
            #[cfg(test)]
            StoreCommand::PanicForTest => unreachable!(),
        };
        if !stamp.matches(session) {
            reply_store_error(command, "Phiên đăng nhập đã hết hiệu lực".into());
            continue;
        }
        if !session.is_active() || session.account_key().is_empty() {
            reply_store_error(command, "Phiên đăng nhập đã hết hiệu lực".into());
            continue;
        }
        let Ok(_write_permit) = session.begin_account_write_blocking() else {
            reply_store_error(command, "Phiên đăng nhập đã hết hiệu lực".into());
            continue;
        };
        let current_path = session.history_path();
        if path.as_ref() != Some(&current_path) || store.is_none() {
            store = crate::store::MessageStore::open(&current_path).ok();
            path = Some(current_path);
        }
        if let Some(store) = store.as_mut() {
            if let Err(error) = execute_store_command(store, command) {
                logger::log_error("tui::history_writer", &error);
            }
        } else {
            let error = "Không thể mở lịch sử cục bộ".to_owned();
            reply_store_error(command, error.clone());
            logger::log_error("tui::history_writer", &error);
        }
    }
}

fn reply_store_error(command: StoreCommand, error: String) {
    match command {
        StoreCommand::Hydrate { completion, .. } => {
            let _ = completion.send(BgEvent::ActionFailed(error));
        }
        StoreCommand::Delete { reply, .. } => {
            let _ = reply.send(Err(error));
        }
        StoreCommand::Outgoing { reply, .. } => {
            let _ = reply.send(Err(error));
        }
        StoreCommand::Snapshot { completion, .. } => {
            let _ = completion.send(BgEvent::StoreFailed(error));
        }
        StoreCommand::Shutdown { .. } => {}
        #[cfg(test)]
        StoreCommand::PanicForTest => {}
    }
}

fn execute_store_command(
    store: &mut crate::store::MessageStore,
    command: StoreCommand,
) -> std::result::Result<(), String> {
    match command {
        StoreCommand::Hydrate {
            mut threads,
            completion,
            ..
        } => {
            let result = store
                .hydrate_threads(&mut threads, 200)
                .and_then(|_| store.filter_deleted(&mut threads));
            match result {
                Ok(_) => {
                    let _ = completion.send(BgEvent::Hydrated(threads));
                }
                Err(error) => {
                    let _ = completion.send(BgEvent::ActionFailed(error.to_string()));
                }
            }
        }
        StoreCommand::Snapshot {
            session,
            mut threads,
            completion,
            ..
        } => {
            let result = store
                .filter_deleted(&mut threads)
                .and_then(|_| crate::store::persist_local_threads(store, &threads).map(|_| ()))
                .map_err(|error| error.to_string())
                .and_then(|_| write_threads_cache_unlocked(&session, &threads));
            match result {
                Ok(()) => {
                    let _ = completion.send(BgEvent::StoreSaved);
                }
                Err(error) => {
                    let _ = completion.send(BgEvent::StoreFailed(error));
                }
            }
        }
        StoreCommand::Delete {
            session,
            thread_id,
            message,
            mut threads,
            reply,
            ..
        } => {
            let result = (|| -> Result<()> {
                persist_acknowledged_delete(store, &thread_id, &message)?;
                store.filter_deleted(&mut threads)?;
                crate::store::persist_local_threads(store, &threads)?;
                write_threads_cache_unlocked(&session, &threads).map_err(anyhow::Error::msg)?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    let _ = reply.send(Ok(()));
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = reply.send(Err(message.clone()));
                    return Err(message);
                }
            }
        }
        StoreCommand::Outgoing {
            mut thread, reply, ..
        } => {
            let result = store
                .filter_deleted(std::slice::from_mut(&mut *thread))
                .and_then(|_| {
                    crate::store::persist_threads(store, std::slice::from_ref(&*thread)).map(|_| ())
                })
                .map_err(|error| error.to_string());
            match result {
                Ok(()) => {
                    let _ = reply.send(Ok(()));
                }
                Err(error) => {
                    let _ = reply.send(Err(error.clone()));
                    return Err(error);
                }
            }
        }
        StoreCommand::Shutdown { .. } => unreachable!("shutdown is handled by the actor loop"),
        #[cfg(test)]
        StoreCommand::PanicForTest => unreachable!("test panic is handled by the actor loop"),
    }
    Ok(())
}

/// Merge DB-hydrated threads into the live list without dropping live-only messages.
fn merge_hydrated(live: &mut Vec<Thread>, hydrated: Vec<Thread>) {
    for thread in hydrated {
        match live.iter().position(|current| current.id == thread.id) {
            Some(pos) => chat::merge_thread_into(&mut live[pos], thread, false),
            None => live.push(thread),
        }
    }
}

fn write_threads_cache_unlocked(
    session: &Session,
    threads: &[Thread],
) -> std::result::Result<(), String> {
    if !session.is_active() {
        return Err("Account runtime is closed for writes".to_owned());
    }
    let mut slim = threads.to_vec();
    for thread in &mut slim {
        let skip = thread
            .messages
            .len()
            .saturating_sub(crate::store::MessageStore::PRELOAD_MSGS);
        thread.messages.drain(..skip);
    }
    let bytes = serde_json::to_vec(&slim).map_err(|e| e.to_string())?;
    crate::storage::atomic_write(&session.cache_path(), &bytes).map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
async fn deliver_outgoing(
    session: Session,
    thread_id: String,
    thread: Thread,
    message: ChatMessage,
    writer: StoreWriter,
    tx: BgSender,
    _active: crate::outgoing::Active,
    _critical: CriticalTask,
) {
    let client_message_id = message.client_message_id.clone();
    let (reply_tx, persisted) = tokio::sync::oneshot::channel();
    let command = StoreCommand::Outgoing {
        session: session.clone(),
        stamp: RuntimeStamp::for_session(&session),
        thread: Box::new(thread),
        reply: reply_tx,
    };
    let admitted = writer.try_send(command).map_err(|error| match error {
        StoreAdmissionError::Full => "Hàng đợi lưu lịch sử đang đầy; tin chưa được gửi".to_owned(),
        StoreAdmissionError::Disconnected => "Bộ lưu lịch sử đã dừng; tin chưa được gửi".to_owned(),
    });
    let persisted = match admitted {
        Ok(()) => persisted
            .await
            .unwrap_or_else(|_| Err("Bộ lưu lịch sử đã dừng".into())),
        Err(error) => Err(error),
    };
    let outcome = if let Err(error) = persisted {
        SendDisposition::NotSent(error)
    } else if !session.is_active() {
        SendDisposition::NotSent("Phiên đăng nhập đã hết hiệu lực".into())
    } else {
        match crate::outgoing::send(&session, &thread_id, &message).await {
            Ok(sent) if sent.delivery == crate::message_model::DeliveryState::ServerAccepted => {
                SendDisposition::Accepted(Box::new(sent))
            }
            Ok(_) => SendDisposition::Unknown(
                "Máy chủ chưa trả mã xác nhận; hãy kiểm tra hội thoại trước khi gửi lại".into(),
            ),
            Err(error) => {
                let text = crate::outgoing::failure_message(&error);
                match crate::outgoing::send_failure_kind(&error) {
                    crate::outgoing::SendFailureKind::NotSent => SendDisposition::NotSent(text),
                    crate::outgoing::SendFailureKind::ExplicitlyRejected => {
                        SendDisposition::Rejected(text)
                    }
                    crate::outgoing::SendFailureKind::Unknown => SendDisposition::Unknown(text),
                }
            }
        }
    };
    let _ = tx.send(BgEvent::SendOutcome {
        thread_id,
        client_message_id,
        outcome,
    });
}

#[derive(Clone)]
struct DeletedIdentity {
    msg_id: String,
    client_message_id: String,
    is_local: bool,
    is_outgoing: bool,
    sender_id: String,
}

impl DeletedIdentity {
    fn from_message(message: &ChatMessage) -> Self {
        let msg_id = if message.id.is_empty() {
            format!("msg_{}", message.ts)
        } else {
            message.id.clone()
        };
        Self {
            is_local: msg_id.starts_with("out_") || msg_id.starts_with("local_"),
            msg_id,
            client_message_id: message.client_message_id.clone(),
            is_outgoing: message.is_outgoing,
            sender_id: message
                .wire_ref
                .sender
                .as_ref()
                .map(serde_json::Value::to_string)
                .unwrap_or_default(),
        }
    }

    fn matches(&self, message: &ChatMessage) -> bool {
        if self.msg_id == message.id {
            return true;
        }
        let provisional = message.id.starts_with("phone-client:")
            || message.id.starts_with("out_")
            || message.id.starts_with("local_");
        !self.client_message_id.is_empty()
            && self.client_message_id != "0"
            && self.client_message_id == message.client_message_id
            && self.is_outgoing == message.is_outgoing
            && (self.is_local || provisional)
            && (self.is_outgoing
                || (!self.sender_id.is_empty()
                    && self.sender_id
                        == message
                            .wire_ref
                            .sender
                            .as_ref()
                            .map(serde_json::Value::to_string)
                            .unwrap_or_default()))
    }
}

pub struct TuiApp<'a> {
    pub session: &'a mut Session,
    pub threads: Vec<Thread>,
    pub list_state: ListState,
    pub selected_thread: usize,
    pub input_text: String,
    pub focus: Focus,
    pub status: String,
    pub should_quit: bool,
    pub poll_state: chat::PollState,
    filtered: Vec<usize>,
    filter: FilterTab,
    search: String,
    search_mode: bool,
    unread: HashMap<String, usize>,
    deleted_messages: HashMap<String, Vec<DeletedIdentity>>,
    selected_msg: Option<usize>,
    msg_scroll: usize,
    follow_tail: bool,
    msg_ranges: Vec<(usize, usize)>,
    chat_view_h: usize,
    threads_width: u16,
    reply_target: Option<ChatMessage>,
    overlay: Overlay,
    log_scroll: usize,
    sticker: StickerState,
    prompt: Option<PromptState>,
    confirm: Option<ConfirmAction>,
    poll_inflight: bool,
    delete_inflight: bool,
    logout_inflight: bool,
    critical_inflight: Arc<AtomicUsize>,
    bg_rx: Receiver<BgEnvelope>,
    bg_tx: Sender<BgEnvelope>,
    writer: StoreWriter,
    /// Latest complete snapshot retained for retry when the bounded writer queue is full.
    pending_snapshot: Option<(Session, RuntimeStamp, Vec<Thread>)>,
}

impl<'a> TuiApp<'a> {
    pub fn new(session: &'a mut Session, threads: Vec<Thread>) -> Self {
        session.hidden.enable();
        let (tx, rx) = channel();
        let writer = StoreWriter::new();
        let hydrate_session = (*session).clone();
        let hydrate_queued = hydrate_session.account_key().is_empty()
            || writer
                .try_send(StoreCommand::Hydrate {
                    session: hydrate_session.clone(),
                    stamp: RuntimeStamp::for_session(&hydrate_session),
                    threads: threads.clone(),
                    completion: BgSender {
                        tx: tx.clone(),
                        stamp: RuntimeStamp::for_session(&hydrate_session),
                    },
                })
                .is_ok();

        let mut app = Self {
            session,
            threads,
            list_state: ListState::default(),
            selected_thread: 0,
            input_text: String::new(),
            focus: Focus::Threads,
            status: if hydrate_queued {
                "San sang".into()
            } else {
                "Không thể khởi tạo lưu trữ lịch sử".into()
            },
            should_quit: false,
            poll_state: chat::PollState::default(),
            filtered: Vec::new(),
            filter: FilterTab::All,
            search: String::new(),
            search_mode: false,
            unread: HashMap::new(),
            deleted_messages: HashMap::new(),
            selected_msg: None,
            msg_scroll: 0,
            follow_tail: true,
            msg_ranges: Vec::new(),
            chat_view_h: 10,
            threads_width: 0,
            reply_target: None,
            overlay: Overlay::None,
            log_scroll: 0,
            sticker: StickerState::default(),
            prompt: None,
            confirm: None,
            poll_inflight: false,
            delete_inflight: false,
            logout_inflight: false,
            critical_inflight: Arc::new(AtomicUsize::new(0)),
            bg_rx: rx,
            bg_tx: tx,
            writer,
            pending_snapshot: None,
        };
        app.rebuild_filter();
        app.ensure_msg_selection();
        app
    }

    fn session_clone(&self) -> Session {
        (*self.session).clone()
    }

    fn bg_sender(&self) -> BgSender {
        BgSender {
            tx: self.bg_tx.clone(),
            stamp: RuntimeStamp::for_session(self.session),
        }
    }

    fn critical_task(&self) -> CriticalTask {
        CriticalTask::start(&self.critical_inflight)
    }

    async fn shutdown_writer(&mut self) -> bool {
        let deadline = tokio::time::Instant::now() + WRITER_SHUTDOWN_BUDGET;
        while self.critical_inflight.load(Ordering::Acquire) > 0
            && tokio::time::Instant::now() < deadline
        {
            self.check_background_events();
            self.flush_pending_snapshot();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        self.check_background_events();
        while self.pending_snapshot.is_some() && tokio::time::Instant::now() < deadline {
            self.flush_pending_snapshot();
            if self.pending_snapshot.is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        let snapshots_admitted = self.pending_snapshot.is_none();
        let writer_stopped = self.writer.shutdown_until(deadline).await;
        let critical_finished = self.critical_inflight.load(Ordering::Acquire) == 0;
        self.session.revoke_runtime();
        snapshots_admitted && writer_stopped && critical_finished
    }

    fn current_thread(&self) -> Option<&Thread> {
        self.threads.get(self.selected_thread)
    }

    fn rebuild_filter(&mut self) {
        let preferred = self.current_thread().map(|t| t.id.clone());
        self.rebuild_filter_prefer(preferred);
    }

    fn rebuild_filter_prefer(&mut self, preferred: Option<String>) {
        let query = self.search.to_lowercase();
        let hidden = self.hidden_view();
        let previous_hidden = preferred.as_deref().is_some_and(|id| {
            self.threads
                .iter()
                .find(|thread| thread.id == id)
                .is_some_and(|thread| {
                    let identity = crate::server_pins::identity(&thread.id, thread.is_group).ok();
                    !Self::thread_allowed(&hidden, self.session.hidden.enforced(), thread)
                        || identity.is_some_and(|identity| hidden.ids.contains(&identity))
                })
        });
        let mut filtered = Vec::new();
        for (i, t) in self.threads.iter().enumerate() {
            if !Self::thread_matches_filter(
                t,
                &hidden,
                self.session.hidden.enforced(),
                self.filter,
                &query,
            ) {
                continue;
            }
            filtered.push(i);
        }
        self.filtered = filtered;

        if self.filtered.is_empty() {
            self.list_state.select(None);
            self.selected_thread = self.threads.len();
            self.selected_msg = None;
            self.msg_ranges.clear();
            if previous_hidden {
                self.clear_private_draft();
            }
            return;
        }

        let pos = preferred
            .as_ref()
            .and_then(|id| {
                self.filtered
                    .iter()
                    .position(|&i| self.threads[i].id == *id)
            })
            .unwrap_or(0)
            .min(self.filtered.len() - 1);
        let idx = self.filtered[pos];
        self.list_state.select(Some(pos));

        let resolved = self.threads.get(idx).map(|t| t.id.clone());
        if resolved != preferred {
            self.selected_msg = self.threads[idx].messages.len().checked_sub(1);
            self.follow_tail = true;
            self.msg_scroll = 0;
            if previous_hidden {
                self.clear_private_draft();
            }
        }
        self.selected_thread = idx;
    }

    fn clear_private_draft(&mut self) {
        self.reply_target = None;
        self.input_text.clear();
        if let Some(mut prompt) = self.prompt.take() {
            use zeroize::Zeroize;
            prompt.text.zeroize();
        }
    }

    fn hidden_view(&self) -> crate::hidden_chat::Snapshot {
        self.session.hidden.view(self.session.account_key())
    }

    fn thread_allowed(
        hidden: &crate::hidden_chat::Snapshot,
        enforced: bool,
        thread: &Thread,
    ) -> bool {
        !enforced || hidden.allows(&thread.id, thread.is_group)
    }

    fn filter_deleted_replays(thread: &mut Thread, deleted: &[DeletedIdentity]) {
        let last_was_deleted = deleted
            .iter()
            .any(|identity| identity.msg_id == thread.presentation.last_message_id);
        let previous_len = thread.messages.len();
        thread
            .messages
            .retain(|message| !deleted.iter().any(|identity| identity.matches(message)));
        if last_was_deleted || thread.messages.len() != previous_len {
            thread.refresh_after_removal();
        }
    }

    fn thread_matches_filter(
        thread: &Thread,
        hidden: &crate::hidden_chat::Snapshot,
        enforced: bool,
        filter: FilterTab,
        query: &str,
    ) -> bool {
        Self::thread_allowed(hidden, enforced, thread)
            && !(filter == FilterTab::Direct && thread.is_group)
            && !(filter == FilterTab::Group && !thread.is_group)
            && (query.is_empty()
                || thread.name.to_lowercase().contains(query)
                || thread.id.contains(query)
                || thread.preview.to_lowercase().contains(query)
                || thread
                    .messages
                    .iter()
                    .rev()
                    .take(20)
                    .any(|message| message.text.to_lowercase().contains(query)))
    }

    fn selected_thread_allowed(&self) -> bool {
        self.current_thread().is_some_and(|thread| {
            Self::thread_allowed(&self.hidden_view(), self.session.hidden.enforced(), thread)
        })
    }

    fn select_index(&mut self, thread_idx: usize) {
        let previous_hidden = self.current_thread().is_some_and(|thread| {
            let hidden = self.hidden_view();
            let identity = crate::server_pins::identity(&thread.id, thread.is_group).ok();
            !Self::thread_allowed(&hidden, self.session.hidden.enforced(), thread)
                || identity.is_some_and(|identity| hidden.ids.contains(&identity))
        });
        let pos = self.filtered.iter().position(|&i| i == thread_idx);
        self.list_state.select(pos);
        self.selected_thread = thread_idx;
        if let Some(t) = self.threads.get_mut(thread_idx) {
            t.unread.read_visible(&t.messages);
            self.unread.remove(&t.id);
            self.selected_msg = t.messages.len().checked_sub(1);
        } else {
            self.selected_msg = None;
        }
        self.msg_scroll = 0;
        self.follow_tail = true;
        self.reply_target = None;
        if previous_hidden {
            self.input_text.clear();
        }
        self.msg_ranges.clear();
    }

    fn move_thread(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let cur = self
            .list_state
            .selected()
            .unwrap_or(0)
            .min(self.filtered.len() - 1);
        let n = self.filtered.len() as isize;
        let next = if delta.abs() > 1 {
            (cur as isize + delta).clamp(0, n - 1) as usize
        } else {
            (cur as isize + delta).rem_euclid(n) as usize
        };
        let idx = self.filtered[next];
        self.select_index(idx);
    }

    fn select_first(&mut self) {
        if let Some(&i) = self.filtered.first() {
            self.select_index(i);
        }
    }

    fn select_last(&mut self) {
        if let Some(&i) = self.filtered.last() {
            self.select_index(i);
        }
    }

    fn cycle_filter(&mut self) {
        self.filter = self.filter.next();
        self.rebuild_filter();
        self.status = format!("Loc: {}", self.filter.label());
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Threads => Focus::Messages,
            Focus::Messages => Focus::Input,
            Focus::Input => Focus::Threads,
        };
        if self.focus == Focus::Messages {
            self.ensure_msg_selection();
        }
    }

    fn cycle_focus_rev(&mut self) {
        self.focus = match self.focus {
            Focus::Threads => Focus::Input,
            Focus::Messages => Focus::Threads,
            Focus::Input => Focus::Messages,
        };
        if self.focus == Focus::Messages {
            self.ensure_msg_selection();
        }
    }

    fn escape(&mut self) {
        if self.reply_target.is_some() {
            self.reply_target = None;
            self.status = "Da huy tra loi".into();
            return;
        }
        self.focus = match self.focus {
            Focus::Input => Focus::Messages,
            Focus::Messages => Focus::Threads,
            Focus::Threads => Focus::Threads,
        };
    }

    fn ensure_msg_selection(&mut self) {
        let len = self
            .threads
            .get(self.selected_thread)
            .map(|t| t.messages.len())
            .unwrap_or(0);
        if len == 0 {
            self.selected_msg = None;
            return;
        }
        if self.selected_msg.map(|i| i >= len).unwrap_or(true) {
            self.selected_msg = Some(len - 1);
        }
    }

    fn move_message(&mut self, delta: isize) {
        let Some(len) = self
            .threads
            .get(self.selected_thread)
            .map(|t| t.messages.len())
        else {
            return;
        };
        if len == 0 {
            return;
        }
        let cur = self.selected_msg.unwrap_or(len - 1).min(len - 1);
        let next = ((cur as isize + delta).clamp(0, (len - 1) as isize)) as usize;
        self.selected_msg = Some(next);
        self.follow_tail = false;
        self.ensure_msg_visible();
    }

    fn ensure_msg_visible(&mut self) {
        let Some(idx) = self.selected_msg else {
            return;
        };
        let Some(&(start, end)) = self.msg_ranges.get(idx) else {
            return;
        };
        let h = self.chat_view_h.max(1);
        if start < self.msg_scroll {
            self.msg_scroll = start;
        } else if end > self.msg_scroll + h {
            self.msg_scroll = end.saturating_sub(h);
        }
    }

    fn scroll_by(&mut self, delta: isize) {
        let step = delta.unsigned_abs();
        if delta < 0 {
            self.msg_scroll = self.msg_scroll.saturating_sub(step);
        } else {
            self.msg_scroll = self.msg_scroll.saturating_add(step);
        }
        self.follow_tail = false;
    }

    fn scroll_page(&mut self, dir: isize) {
        let step = self.chat_view_h.max(1);
        self.scroll_by(dir * step as isize);
    }

    fn bump_thread(&mut self, thread_id: &str) {
        let selected_id = self.current_thread().map(|t| t.id.clone());
        if let Some(pos) = self.threads.iter().position(|t| t.id == thread_id) {
            if pos != 0 {
                let t = self.threads.remove(pos);
                self.threads.insert(0, t);
            }
        }
        self.rebuild_filter_prefer(selected_id);
    }

    fn persist_snapshot(&mut self, threads: &[Thread]) {
        let session = self.session_clone();
        let stamp = RuntimeStamp::for_session(&session);
        let snapshot = threads.to_vec();
        let command = StoreCommand::Snapshot {
            session: session.clone(),
            stamp: stamp.clone(),
            threads: snapshot.clone(),
            completion: BgSender {
                tx: self.bg_tx.clone(),
                stamp: stamp.clone(),
            },
        };
        match self.writer.try_send(command) {
            // A full snapshot supersedes any older one still waiting for a slot;
            // keeping it would let stale state be written after this one.
            Ok(()) => self.pending_snapshot = None,
            Err(StoreAdmissionError::Full) => {
                self.pending_snapshot = Some((session, stamp, snapshot));
                self.status = "Đang chờ lưu lịch sử cục bộ".into();
            }
            Err(StoreAdmissionError::Disconnected) => {
                self.status = "Không thể lưu lịch sử cục bộ".into();
            }
        }
    }

    fn persist_async(&mut self) {
        let snapshot = self.threads.clone();
        self.persist_snapshot(&snapshot);
    }

    fn flush_pending_snapshot(&mut self) {
        let Some((session, stamp, threads)) = self.pending_snapshot.take() else {
            return;
        };
        let command = StoreCommand::Snapshot {
            session: session.clone(),
            stamp: stamp.clone(),
            threads: threads.clone(),
            completion: BgSender {
                tx: self.bg_tx.clone(),
                stamp: stamp.clone(),
            },
        };
        match self.writer.try_send(command) {
            Ok(()) => {}
            Err(StoreAdmissionError::Full) => {
                self.pending_snapshot = Some((session, stamp, threads))
            }
            Err(StoreAdmissionError::Disconnected) => {
                self.status = "Không thể lưu lịch sử cục bộ".into();
            }
        }
    }

    fn spawn_poll(&mut self) {
        if self.poll_inflight || self.logout_inflight || self.session.login.is_none() {
            return;
        }
        self.poll_inflight = true;
        let session = self.session_clone();
        let mut state = self.poll_state.clone();
        let mut threads = self.threads.clone();
        let tx = self.bg_sender();
        tokio::spawn(async move {
            match chat::receive_new_messages(&session, &mut state, &mut threads).await {
                Ok(()) => {
                    let _ = tx.send(BgEvent::PollDone { threads, state });
                }
                Err(e) => {
                    logger::log_error("tui::poll_new_messages", &e.to_string());
                    let _ = tx.send(BgEvent::PollFailed(e.to_string()));
                }
            }
        });
    }

    fn spawn_refresh(&mut self) {
        self.status = "Dang lam moi...".into();
        let session = self.session_clone();
        let tx = self.bg_sender();
        tokio::spawn(async move {
            let result = async {
                let mut list = chat::get_friends(&session).await?;
                let ids: Vec<String> = list.iter().map(|t| t.id.clone()).collect();
                match chat::get_last_msgs(&session, &ids).await {
                    Ok(resp) => chat::merge_previews_with_profile(
                        &mut list,
                        &resp,
                        Some(session.account_key()),
                    ),
                    Err(e) => logger::log_error("tui::refresh::get_last_msgs", &e.to_string()),
                }
                Ok::<Vec<Thread>, anyhow::Error>(list)
            }
            .await;
            match result {
                Ok(list) => {
                    let _ = tx.send(BgEvent::ThreadsRefreshed(list));
                }
                Err(e) => {
                    logger::log_error("tui::refresh::get_friends", &e.to_string());
                    let _ = tx.send(BgEvent::ActionFailed(format!("Loi lam moi: {e}")));
                }
            }
        });
    }

    fn spawn_history(&mut self) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let Some(t) = self.threads.get(self.selected_thread) else {
            return;
        };
        if !t.is_group {
            self.status = "Chi ap dung cho nhom chat".into();
            return;
        }
        let group_id = t.id.clone();
        let thread = t.clone();
        self.status = "Dang tai lich su nhom...".into();
        let session = self.session_clone();
        let tx = self.bg_sender();
        let gid = group_id.clone();
        tokio::spawn(async move {
            match chat::get_group_history(&session, &gid, 50).await {
                Ok(resp) => {
                    let mut loaded = thread;
                    chat::merge_group_history_with_profile(
                        &mut loaded,
                        &resp,
                        Some(session.account_key()),
                    );
                    let _ = tx.send(BgEvent::HistoryLoaded { thread: loaded });
                }
                Err(e) => {
                    logger::log_error("tui::get_group_history", &e.to_string());
                    let _ = tx.send(BgEvent::ActionFailed(format!("Loi tai lich su nhom: {e}")));
                }
            }
        });
    }

    fn spawn_hidden_refresh(&self) {
        let session = self.session_clone();
        let tx = self.bg_sender();
        tokio::spawn(async move {
            match crate::hidden_chat::restore_cached(&session).await {
                Ok(Some(_)) => {
                    let _ = tx.send(BgEvent::HiddenStateChanged(Ok(())));
                }
                Ok(None) => {}
                Err(error) => {
                    logger::log_error("tui::restore_hidden_membership", &error.to_string());
                }
            }
            match crate::hidden_chat::execute(&session, &crate::hidden_chat::Action::Read).await {
                Ok(_) => {
                    let _ = tx.send(BgEvent::HiddenStateChanged(Ok(())));
                }
                Err(error) => {
                    logger::log_error("tui::read_hidden_membership", &error.to_string());
                    let _ = tx.send(BgEvent::HiddenStateChanged(Err(error.to_string())));
                }
            }
        });
    }

    fn open_sticker_store(&mut self) {
        self.overlay = Overlay::Sticker;
        self.sticker.stage = 0;
        if !self.sticker.packs.is_empty() || self.sticker.loading {
            return;
        }
        self.sticker.loading = true;
        let session = self.session_clone();
        let tx = self.bg_sender();
        tokio::spawn(async move {
            match chat::fetch_sticker_store_packs(&session).await {
                Ok(packs) => {
                    let _ = tx.send(BgEvent::StickerPacks(packs));
                }
                Err(e) => {
                    logger::log_error("tui::fetch_sticker_store_packs", &e.to_string());
                    let _ = tx.send(BgEvent::ActionFailed(format!("Loi tai kho nhan dan: {e}")));
                }
            }
        });
    }

    fn sticker_move(&mut self, delta: isize) {
        let (len, idx) = if self.sticker.stage == 0 {
            (self.sticker.packs.len(), &mut self.sticker.pack_idx)
        } else {
            (self.sticker.items.len(), &mut self.sticker.item_idx)
        };
        if len == 0 {
            return;
        }
        let cur = (*idx).min(len - 1) as isize;
        *idx = ((cur + delta).clamp(0, (len - 1) as isize)) as usize;
    }

    fn sticker_enter(&mut self) {
        if self.sticker.stage == 0 {
            let Some(pack) = self.sticker.packs.get(self.sticker.pack_idx) else {
                return;
            };
            let cate_id = pack.id;
            let name = pack.name.clone();
            if self.sticker.loading {
                return;
            }
            self.sticker.loading = true;
            self.sticker.loaded_name = name.clone();
            let session = self.session_clone();
            let tx = self.bg_sender();
            tokio::spawn(async move {
                match chat::fetch_sticker_pack_items(&session, cate_id, &name).await {
                    Ok(items) => {
                        let _ = tx.send(BgEvent::StickerItems(items));
                    }
                    Err(e) => {
                        logger::log_error("tui::fetch_sticker_pack_items", &e.to_string());
                        let _ = tx.send(BgEvent::ActionFailed(format!("Loi tai bo nhan dan: {e}")));
                    }
                }
            });
        } else {
            let Some(item) = self.sticker.items.get(self.sticker.item_idx).cloned() else {
                return;
            };
            self.overlay = Overlay::None;
            self.send_sticker(item);
        }
    }

    fn handle_sticker_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::F(2) => {
                self.overlay = Overlay::None;
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                if !self.sticker.items.is_empty() {
                    self.sticker.stage = 1 - self.sticker.stage;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.sticker_move(-1),
            KeyCode::Down | KeyCode::Char('j') => self.sticker_move(1),
            KeyCode::PageUp => self.sticker_move(-10),
            KeyCode::PageDown => self.sticker_move(10),
            KeyCode::Enter => self.sticker_enter(),
            _ => {}
        }
    }

    fn send_sticker(&mut self, item: StickerItem) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let Some(t) = self.threads.get(self.selected_thread) else {
            return;
        };
        let to_id = t.id.clone();
        let is_group = t.is_group;

        let mut msg = crate::outgoing::pending(
            String::new(),
            is_group,
            crate::outgoing::Payload::Sticker {
                id: item.sticker_id,
                category: item.cate_id,
                kind: item.sticker_type,
            },
        );
        msg.sticker_id = Some(item.sticker_id);
        msg.sticker_cate_id = Some(item.cate_id);
        msg.sticker_type = Some(item.sticker_type);
        if let Some(t) = self.threads.get_mut(self.selected_thread) {
            t.messages.push(msg.clone());
            t.preview = "[Nhan dan]".into();
            t.time = msg.time.clone();
        }
        let thread_snapshot = self.threads[self.selected_thread].clone();
        self.selected_msg = None;
        self.follow_tail = true;
        let session = self.session_clone();
        let tx = self.bg_sender();
        let writer = self.writer.clone();
        let Some(active) = crate::outgoing::Active::start(&msg.client_message_id) else {
            self.status = "Tin nhắn này đang được gửi".into();
            return;
        };
        let critical = self.critical_task();
        tokio::spawn(deliver_outgoing(
            session,
            to_id,
            thread_snapshot,
            msg,
            writer,
            tx,
            active,
            critical,
        ));
        self.status = "Da gui nhan dan".into();
    }

    fn send_attachment(&mut self, path: PathBuf) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let Some(t) = self.threads.get(self.selected_thread) else {
            return;
        };
        let to_id = t.id.clone();
        let is_group = t.is_group;
        let caption = self.input_text.trim().to_string();
        self.input_text.clear();
        let mut pending = crate::outgoing::pending(
            caption.clone(),
            is_group,
            crate::outgoing::Payload::Attachment {
                path: path.clone(),
                caption,
                stamp: None,
                hd_source: None,
            },
        );
        pending.sender = "Toi".into();
        pending.kind = crate::message_model::MessageKind::File;
        if let Some(thread) = self.threads.get_mut(self.selected_thread) {
            thread.messages.push(pending.clone());
            thread.preview = if pending.text.is_empty() {
                "[Tep dinh kem]".into()
            } else {
                pending.text.clone()
            };
            thread.time = pending.time.clone();
        }
        let thread = self.threads[self.selected_thread].clone();
        let session = self.session_clone();
        let tx = self.bg_sender();
        let writer = self.writer.clone();
        let Some(active) = crate::outgoing::Active::start(&pending.client_message_id) else {
            self.status = "Tin nhắn này đang được gửi".into();
            return;
        };
        let critical = self.critical_task();
        tokio::spawn(deliver_outgoing(
            session, to_id, thread, pending, writer, tx, active, critical,
        ));
    }

    fn submit_prompt(&mut self, kind: PromptKind, text: String) {
        match kind {
            PromptKind::Attachment => {
                let trimmed = text.trim().to_string();
                if trimmed.is_empty() {
                    self.status = "Chua nhap duong dan tep".into();
                    return;
                }
                self.send_attachment(PathBuf::from(trimmed));
            }
            PromptKind::HiddenPin => {
                let pin = zeroize::Zeroizing::new(text);
                let session = self.session_clone();
                let tx = self.bg_sender();
                tokio::spawn(async move {
                    let result = crate::hidden_chat::execute(
                        &session,
                        &crate::hidden_chat::Action::Unlock(pin),
                    )
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string());
                    let _ = tx.send(BgEvent::HiddenStateChanged(result));
                });
                self.status = "Đang xác minh PIN hội thoại ẩn...".into();
            }
        }
    }

    fn send_current(&mut self) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let text = self.input_text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let Some(t) = self.threads.get(self.selected_thread) else {
            self.status = "Chua chon hoi thoai".into();
            return;
        };
        let to_id = t.id.clone();
        let is_group = t.is_group;

        self.input_text.clear();
        let reply = self.reply_target.take();
        let payload = reply
            .clone()
            .map(|quoted| crate::outgoing::Payload::Quote(Box::new(quoted)))
            .unwrap_or(crate::outgoing::Payload::Text);
        let mut msg = crate::outgoing::pending(text.clone(), is_group, payload);
        msg.reply_to_id = reply.as_ref().map(|quoted| quoted.id.clone());
        msg.reply_to_sender = reply.as_ref().map(|quoted| quoted.sender.clone());
        msg.reply_to_text = reply.as_ref().map(|quoted| quoted.text.clone());
        if let Some(t) = self.threads.get_mut(self.selected_thread) {
            t.messages.push(msg.clone());
            t.preview = text.clone();
            t.time = msg.time.clone();
        }
        let thread_snapshot = self.threads[self.selected_thread].clone();
        self.selected_msg = None;
        self.follow_tail = true;
        self.bump_thread(&to_id);

        let session = self.session_clone();
        let tx = self.bg_sender();
        let writer = self.writer.clone();
        let Some(active) = crate::outgoing::Active::start(&msg.client_message_id) else {
            self.status = "Tin nhắn này đang được gửi".into();
            return;
        };
        let critical = self.critical_task();
        tokio::spawn(deliver_outgoing(
            session,
            to_id,
            thread_snapshot,
            msg,
            writer,
            tx,
            active,
            critical,
        ));
        self.status = "Dang gui tin nhan...".into();
    }

    fn confirm_retry_selected(&mut self) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let (Some(message_idx), Some(thread)) =
            (self.selected_msg, self.threads.get(self.selected_thread))
        else {
            return;
        };
        let Some(message) = thread.messages.get(message_idx) else {
            return;
        };
        if message.outgoing_job.is_none()
            || !matches!(
                message.delivery,
                crate::message_model::DeliveryState::Failed
                    | crate::message_model::DeliveryState::DeliveryUnknown
            )
        {
            self.status = "Tin nhắn này chưa sẵn sàng để gửi lại".into();
            return;
        }
        self.confirm = Some(ConfirmAction {
            question: "Kiểm tra hội thoại trước khi gửi lại cùng mã tin nhắn?".into(),
            detail: message.text.chars().take(60).collect(),
            kind: ConfirmKind::RetryMessage,
            thread_id: thread.id.clone(),
            msg_id: message.id.clone(),
        });
    }

    fn retry_message(&mut self, thread_id: &str, msg_id: &str) {
        let Some(thread_idx) = self
            .threads
            .iter()
            .position(|thread| thread.id == thread_id)
        else {
            return;
        };
        let Some(message_idx) = self.threads[thread_idx]
            .messages
            .iter()
            .position(|message| message.id == msg_id)
        else {
            return;
        };
        let mut message = self.threads[thread_idx].messages[message_idx].clone();
        let Some(job) = message.outgoing_job.as_mut() else {
            self.status = "Thiếu dữ liệu gửi lại".into();
            return;
        };
        if crate::outgoing::is_active(&message.client_message_id) {
            self.status = "Tin nhắn này đang được gửi".into();
            return;
        }
        job.attempt = job.attempt.saturating_add(1);
        job.error.clear();
        message.delivery = crate::message_model::DeliveryState::Pending;
        self.threads[thread_idx].messages[message_idx] = message.clone();
        let thread = self.threads[thread_idx].clone();
        let Some(active) = crate::outgoing::Active::start(&message.client_message_id) else {
            self.status = "Tin nhắn này đang được gửi".into();
            return;
        };
        self.status = "Đang gửi lại cùng mã tin nhắn...".into();
        let critical = self.critical_task();
        tokio::spawn(deliver_outgoing(
            self.session_clone(),
            thread_id.to_owned(),
            thread,
            message,
            self.writer.clone(),
            self.bg_sender(),
            active,
            critical,
        ));
    }

    fn react_quick(&mut self, key: char) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let Some(idx) = key
            .to_digit(10)
            .map(|d| d as usize)
            .and_then(|d| d.checked_sub(1))
        else {
            return;
        };
        let Some(emoji) = QUICK_REACTIONS.get(idx).copied() else {
            return;
        };
        let Some(mi) = self.selected_msg else {
            return;
        };
        let (to_id, is_group, msg) = {
            let Some(t) = self.threads.get(self.selected_thread) else {
                return;
            };
            let Some(msg) = t.messages.get(mi) else {
                return;
            };
            (t.id.clone(), t.is_group, msg.clone())
        };
        if let Some(m) = self
            .threads
            .get_mut(self.selected_thread)
            .and_then(|t| t.messages.get_mut(mi))
        {
            let code = chat::reaction_code_and_type(emoji).0;
            if !m.reactions.iter().any(|r| r == code) {
                m.reactions.push(code.to_string());
            }
        }
        let session = self.session_clone();
        let tx = self.bg_sender();
        let msg_id = msg.id.clone();
        tokio::spawn(async move {
            match chat::send_reaction(&session, &to_id, is_group, &msg, emoji).await {
                Ok(_) => {
                    let _ = tx.send(BgEvent::ReactionDone {
                        thread_id: to_id,
                        msg_id,
                        icon: emoji.to_string(),
                    });
                }
                Err(e) => {
                    logger::log_error("tui::send_reaction", &e.to_string());
                    let _ = tx.send(BgEvent::ActionFailed(format!("Loi tha cam xuc: {e}")));
                }
            }
        });
        self.status = format!("Da tha cam xuc {emoji}");
    }

    fn quote_selected(&mut self) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let Some(mi) = self.selected_msg else {
            return;
        };
        let Some(t) = self.threads.get(self.selected_thread) else {
            return;
        };
        let Some(m) = t.messages.get(mi) else {
            return;
        };
        self.reply_target = Some(m.clone());
        self.focus = Focus::Input;
        self.status = "Dang tra loi tin nhan (Ctrl+X de huy)".into();
    }

    fn confirm_delete_selected(&mut self) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let Some(mi) = self.selected_msg else {
            return;
        };
        let Some(t) = self.threads.get(self.selected_thread) else {
            return;
        };
        let Some(m) = t.messages.get(mi) else {
            return;
        };
        if m.id.is_empty() || m.id.starts_with("local_") || m.id.starts_with("out_") {
            self.status = "Tin nhan chua dong bo, chua the xoa tren server".into();
            return;
        }
        let preview = m.text.chars().take(60).collect::<String>();
        self.confirm = Some(ConfirmAction {
            question: "Xoa tin nhan nay (chi phia ban)?".into(),
            detail: preview,
            kind: ConfirmKind::DeleteMessage,
            thread_id: t.id.clone(),
            msg_id: m.id.clone(),
        });
    }

    fn open_selected_link(&mut self) {
        if !self.selected_thread_allowed() {
            self.status = "Trạng thái hội thoại ẩn chưa được xác minh hoặc đang khóa".into();
            return;
        }
        let Some(mi) = self.selected_msg else {
            return;
        };
        let url = self
            .threads
            .get(self.selected_thread)
            .and_then(|t| t.messages.get(mi))
            .and_then(|m| m.link_url.clone().or_else(|| m.link_src.clone()));
        match url {
            Some(u) if !u.is_empty() => {
                open_external_url(&u);
                self.status = format!("Mo lien ket: {u}");
            }
            _ => self.status = "Tin nhan khong co lien ket".into(),
        }
    }

    fn run_confirm(&mut self, action: ConfirmAction) {
        match action.kind {
            ConfirmKind::Logout => {
                if self.logout_inflight {
                    self.status = "Đang xử lý đăng xuất; vui lòng đợi...".into();
                    return;
                }
                // An acknowledged delete must commit its tombstone under this runtime;
                // revoking it now would let the message reappear after next login.
                if self.critical_inflight.load(Ordering::Acquire) > 0 {
                    self.status =
                        "Đang chờ xác nhận gửi/xóa tin; hãy đăng xuất lại sau giây lát".into();
                    return;
                }
                let owner = self.session.account_key().to_owned();
                self.logout_inflight = true;
                self.session.hidden.lock();
                self.clear_private_draft();
                self.pending_snapshot = None;
                self.poll_inflight = false;
                self.delete_inflight = false;
                self.threads.clear();
                self.filtered.clear();
                self.list_state.select(None);
                self.unread.clear();
                self.selected_thread = 0;
                self.selected_msg = None;
                self.status = "Đang xóa phiên và dọn dẹp dữ liệu...".into();

                let sender = self.bg_sender();
                let session = self.session.clone();
                let critical = self.critical_task();
                std::thread::spawn(move || {
                    let _critical = critical;
                    let _write_barrier = session.close_account_writes_blocking();
                    let result =
                        crate::profiles::perform_account_logout(&owner).map_err(|e| format!("{e}"));
                    if result.is_ok() {
                        session.revoke_runtime();
                    }
                    let _ = sender.send(BgEvent::LogoutDone { owner, result });
                });
            }
            ConfirmKind::DeleteMessage => {
                if self.delete_inflight {
                    self.status = "Đang lưu một thao tác xóa khác".into();
                    return;
                }
                let thread_id = action.thread_id;
                let msg_id = action.msg_id;
                let Some(t) = self.threads.iter().find(|t| t.id == thread_id) else {
                    return;
                };
                let Some(msg) = t.messages.iter().find(|m| m.id == msg_id).cloned() else {
                    return;
                };
                let is_group = t.is_group;
                let session = self.session_clone();
                let tx = self.bg_sender();
                let db_msg = msg.clone();
                let deleted = DeletedIdentity::from_message(&msg);
                let writer = self.writer.clone();
                let threads = self.threads.clone();
                let permit = match self.writer.try_reserve_delete() {
                    Ok(permit) => permit,
                    Err(StoreAdmissionError::Full) => {
                        self.status =
                            "Hàng đợi lưu lịch sử đang đầy; thao tác xóa chưa được gửi".into();
                        return;
                    }
                    Err(StoreAdmissionError::Disconnected) => {
                        self.status = "Bộ lưu lịch sử đã dừng; thao tác xóa chưa được gửi".into();
                        return;
                    }
                };
                // Acquire capacity before the API call so an ACK can always enqueue its tombstone.
                self.delete_inflight = true;
                let critical = self.critical_task();
                tokio::spawn(async move {
                    let _critical = critical;
                    match chat::delete_message_for_me(&session, &thread_id, is_group, &msg).await {
                        Ok(_) => {
                            let (reply, wait) = tokio::sync::oneshot::channel();
                            let command = StoreCommand::Delete {
                                session: session.clone(),
                                stamp: RuntimeStamp::for_session(&session),
                                thread_id: thread_id.clone(),
                                message: Box::new(db_msg),
                                threads,
                                reply,
                            };
                            let admitted = writer.enqueue_reserved(command, permit).map_err(
                                |error| match error {
                                    StoreAdmissionError::Full => {
                                        "Hàng đợi lưu lịch sử đầy; dấu xóa cục bộ chưa được lưu"
                                            .to_owned()
                                    }
                                    StoreAdmissionError::Disconnected => {
                                        "Bộ lưu lịch sử đã dừng; dấu xóa cục bộ chưa được lưu"
                                            .to_owned()
                                    }
                                },
                            );
                            let db_result = match admitted {
                                Ok(()) => wait
                                    .await
                                    .unwrap_or_else(|_| Err("Bộ lưu lịch sử đã dừng".into())),
                                Err(error) => Err(error),
                            };
                            let _ = tx.send(BgEvent::DeleteDone {
                                thread_id,
                                deleted,
                                result: db_result,
                            });
                        }
                        Err(e) => {
                            logger::log_error("tui::delete_message_for_me", &e.to_string());
                            let _ =
                                tx.send(BgEvent::DeleteFailed(format!("Loi xoa tin nhan: {e}")));
                        }
                    }
                });
                self.status = "Dang xoa tin nhan...".into();
            }
            ConfirmKind::RetryMessage => self.retry_message(&action.thread_id, &action.msg_id),
        }
    }

    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('c') => {
                    self.should_quit = true;
                    return;
                }
                KeyCode::Char('u') => {
                    self.input_text.clear();
                    return;
                }
                KeyCode::Char('x') => {
                    if self.reply_target.is_some() {
                        self.reply_target = None;
                        self.status = "Da huy tra loi".into();
                    }
                    return;
                }
                _ => {}
            }
        }

        if let Some(action) = self.confirm.take() {
            match code {
                KeyCode::Char('y') | KeyCode::Enter => self.run_confirm(action),
                KeyCode::Char('n') | KeyCode::Esc => self.status = "Da huy thao tac".into(),
                _ => self.confirm = Some(action),
            }
            return;
        }

        if let Some(mut prompt) = self.prompt.take() {
            match code {
                KeyCode::Esc => {
                    use zeroize::Zeroize;
                    prompt.text.zeroize();
                    self.status = "Da huy nhap".into();
                }
                KeyCode::Enter => {
                    let text = std::mem::take(&mut prompt.text);
                    self.submit_prompt(prompt.kind, text);
                }
                KeyCode::Backspace => {
                    prompt.text.pop();
                    self.prompt = Some(prompt);
                }
                KeyCode::Char(c) => {
                    if !c.is_control() && !mods.contains(KeyModifiers::CONTROL) {
                        prompt.text.push(c);
                    }
                    self.prompt = Some(prompt);
                }
                _ => self.prompt = Some(prompt),
            }
            return;
        }

        match self.overlay {
            Overlay::Help => {
                if code == KeyCode::Esc
                    || code == KeyCode::F(1)
                    || code == KeyCode::Char('?')
                    || code == KeyCode::Char('q')
                {
                    self.overlay = Overlay::None;
                }
                return;
            }
            Overlay::Log => {
                match code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l') => {
                        self.overlay = Overlay::None;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.log_scroll = self.log_scroll.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.log_scroll = self.log_scroll.saturating_add(1);
                    }
                    KeyCode::PageUp => self.log_scroll = self.log_scroll.saturating_sub(10),
                    KeyCode::PageDown => self.log_scroll = self.log_scroll.saturating_add(10),
                    _ => {}
                }
                return;
            }
            Overlay::Sticker => {
                self.handle_sticker_key(code);
                return;
            }
            Overlay::None => {}
        }

        if self.search_mode {
            match code {
                KeyCode::Esc => {
                    self.search.clear();
                    self.search_mode = false;
                    self.rebuild_filter();
                }
                KeyCode::Enter => self.search_mode = false,
                KeyCode::Backspace => {
                    self.search.pop();
                    self.rebuild_filter();
                }
                KeyCode::Up | KeyCode::Char('k') => self.move_thread(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_thread(1),
                KeyCode::Char(c) => {
                    self.search.push(c);
                    self.rebuild_filter();
                }
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::F(1) => {
                self.overlay = Overlay::Help;
                return;
            }
            KeyCode::Char('?') if self.focus != Focus::Input => {
                self.overlay = Overlay::Help;
                return;
            }
            KeyCode::F(2) => {
                self.open_sticker_store();
                return;
            }
            KeyCode::F(3) => {
                self.prompt = Some(PromptState {
                    kind: PromptKind::Attachment,
                    text: String::new(),
                });
                return;
            }
            KeyCode::Char('/') if self.focus != Focus::Input => {
                self.search_mode = true;
                self.status = "Tim kiem: go tu khoa, Enter de xong, Esc de xoa".into();
                return;
            }
            KeyCode::Char('f') if self.focus != Focus::Input => {
                self.cycle_filter();
                return;
            }
            KeyCode::Char('p') if self.focus != Focus::Input => {
                let hidden = self.hidden_view();
                if hidden.unlocked {
                    let session = self.session_clone();
                    let tx = self.bg_sender();
                    tokio::spawn(async move {
                        let result = crate::hidden_chat::execute(
                            &session,
                            &crate::hidden_chat::Action::Lock,
                        )
                        .await
                        .map(|_| ())
                        .map_err(|error| error.to_string());
                        let _ = tx.send(BgEvent::HiddenStateChanged(result));
                    });
                    self.status = "Đang khóa hội thoại ẩn...".into();
                } else if !hidden.ready || !hidden.pin_known {
                    self.spawn_hidden_refresh();
                    self.status = "Chưa tải được trạng thái PIN; đang thử đồng bộ lại".into();
                } else if !hidden.has_pin {
                    self.status =
                        "Tài khoản chưa cấu hình PIN hội thoại ẩn; nhấn R để tải lại".into();
                } else {
                    self.prompt = Some(PromptState {
                        kind: PromptKind::HiddenPin,
                        text: String::new(),
                    });
                    self.status = "Nhập PIN hội thoại ẩn".into();
                }
                return;
            }
            KeyCode::Char('u') if self.focus != Focus::Input => {
                self.unread.clear();
                self.status = "Da xoa thong bao chua doc".into();
                return;
            }
            KeyCode::Char('r') if self.focus != Focus::Input => {
                self.spawn_refresh();
                return;
            }
            KeyCode::Char('R') if self.focus != Focus::Input => {
                self.spawn_hidden_refresh();
                self.status = "Đang xác minh trạng thái hội thoại ẩn...".into();
                return;
            }
            // `y` stays "reply"; retry has its own key so neither shadows the other.
            KeyCode::Char('Y') if self.focus != Focus::Input => {
                self.confirm_retry_selected();
                return;
            }
            KeyCode::Char('h') if self.focus != Focus::Input => {
                self.spawn_history();
                return;
            }
            KeyCode::Char('l') if self.focus != Focus::Input => {
                if logger::is_dev_mode() {
                    self.overlay = Overlay::Log;
                    self.log_scroll = 0;
                } else {
                    self.status = "Nhat ky chi co trong che do --dev".into();
                }
                return;
            }
            KeyCode::Char('x') if self.focus != Focus::Input => {
                self.confirm = Some(ConfirmAction {
                    question: "Dang xuat va xoa session.json, threads_cache.json?".into(),
                    detail: "Thao tac nay khong the hoan tac.".into(),
                    kind: ConfirmKind::Logout,
                    thread_id: String::new(),
                    msg_id: String::new(),
                });
                return;
            }
            KeyCode::Char('q') if self.focus != Focus::Input => {
                self.should_quit = true;
                return;
            }
            KeyCode::Tab => {
                self.cycle_focus();
                return;
            }
            KeyCode::BackTab => {
                self.cycle_focus_rev();
                return;
            }
            KeyCode::Esc => {
                self.escape();
                return;
            }
            _ => {}
        }

        match self.focus {
            Focus::Threads => match code {
                KeyCode::Up | KeyCode::Char('k') => self.move_thread(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_thread(1),
                KeyCode::PageUp => self.move_thread(-10),
                KeyCode::PageDown => self.move_thread(10),
                KeyCode::Home => self.select_first(),
                KeyCode::End => self.select_last(),
                KeyCode::Enter => {
                    self.focus = Focus::Messages;
                    self.ensure_msg_selection();
                }
                KeyCode::Char(c) if !c.is_control() && !mods.contains(KeyModifiers::CONTROL) => {
                    self.focus = Focus::Input;
                    self.input_text.push(c);
                }
                _ => {}
            },
            Focus::Messages => match code {
                KeyCode::Up | KeyCode::Char('k') => self.move_message(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_message(1),
                KeyCode::PageUp => self.scroll_page(-1),
                KeyCode::PageDown => self.scroll_page(1),
                KeyCode::Home => {
                    self.msg_scroll = 0;
                    self.follow_tail = false;
                }
                KeyCode::End => self.follow_tail = true,
                KeyCode::Enter => self.focus = Focus::Input,
                KeyCode::Char(c @ '1'..='6') => self.react_quick(c),
                KeyCode::Char('y') => self.quote_selected(),
                KeyCode::Char('d') => self.confirm_delete_selected(),
                KeyCode::Char('o') => self.open_selected_link(),
                KeyCode::Char(c) if !c.is_control() && !mods.contains(KeyModifiers::CONTROL) => {
                    self.focus = Focus::Input;
                    self.input_text.push(c);
                }
                _ => {}
            },
            Focus::Input => match code {
                KeyCode::Enter => self.send_current(),
                KeyCode::Backspace => {
                    self.input_text.pop();
                }
                KeyCode::Char(c) if !c.is_control() && !mods.contains(KeyModifiers::CONTROL) => {
                    self.input_text.push(c);
                }
                _ => {}
            },
        }
    }

    fn handle_mouse(&mut self, m: MouseEvent) {
        let in_threads = (m.column as usize) < self.threads_width as usize;
        match m.kind {
            MouseEventKind::ScrollUp => {
                if in_threads {
                    self.move_thread(-1);
                } else {
                    self.scroll_by(-3);
                }
            }
            MouseEventKind::ScrollDown => {
                if in_threads {
                    self.move_thread(1);
                } else {
                    self.scroll_by(3);
                }
            }
            _ => {}
        }
    }

    fn apply_polled(&mut self, mut polled: Vec<Thread>, state: chat::PollState) -> bool {
        for thread in &mut polled {
            if let Some(ids) = self.deleted_messages.get(&thread.id) {
                Self::filter_deleted_replays(thread, ids);
            }
        }
        self.poll_state = state;
        let current_id = self.current_thread().map(|t| t.id.clone());

        let mut old: HashMap<String, (HashSet<String>, i64, usize, String)> = HashMap::new();
        for t in &self.threads {
            old.insert(
                t.id.clone(),
                (
                    t.messages.iter().map(|m| m.id.clone()).collect(),
                    t.messages.iter().map(|m| m.ts).max().unwrap_or(0),
                    t.messages.len(),
                    t.preview.clone(),
                ),
            );
        }

        let mut changed = false;
        for t in &polled {
            match old.get(&t.id) {
                Some((prev_ids, prev_max_ts, prev_len, prev_preview)) => {
                    if t.messages.len() != *prev_len || &t.preview != prev_preview {
                        changed = true;
                    }
                    let fresh = t
                        .messages
                        .iter()
                        .filter(|m| {
                            !m.is_outgoing
                                && m.unread_policy.counts_unread()
                                && m.kind != crate::message_model::MessageKind::System
                                && !m.id.is_empty()
                                && !m.id.starts_with("local_")
                                && !prev_ids.contains(&m.id)
                                && !(*prev_max_ts > 0 && m.ts > 0 && m.ts <= *prev_max_ts)
                        })
                        .count();
                    if fresh > 0 && current_id.as_deref() != Some(t.id.as_str()) {
                        *self.unread.entry(t.id.clone()).or_insert(0) += fresh;
                        changed = true;
                    }
                }
                None => changed = true,
            }
        }

        // Merge into the live threads instead of replacing them, so optimistic
        // messages and history loaded while the poll was in flight survive.
        for mut incoming in polled {
            if let Some(pos) = self.threads.iter().position(|x| x.id == incoming.id) {
                chat::merge_thread_into(&mut self.threads[pos], incoming, false);
            } else {
                incoming.unread.advance_seen("0");
                incoming.unread.observe(&incoming.messages);
                self.threads.push(incoming);
                changed = true;
            }
        }

        for thread in &mut self.threads {
            if thread.unread.last_seen_id.is_some() {
                thread.unread.observe(&thread.messages);
                if current_id.as_deref() == Some(&thread.id) {
                    thread.unread.read_visible(&thread.messages);
                }
                if thread.unread.count() == 0 {
                    self.unread.remove(&thread.id);
                } else {
                    self.unread.insert(thread.id.clone(), thread.unread.count());
                }
            }
        }
        if let Some(id) = &current_id {
            self.unread.remove(id);
        }
        self.rebuild_filter_prefer(current_id);
        self.msg_ranges.clear();
        changed
    }

    fn apply_refresh(&mut self, list: Vec<Thread>) {
        if list.is_empty() {
            self.status = "Khong lay duoc danh sach hoi thoai".into();
            return;
        }
        for ft in list {
            if let Some(pos) = self.threads.iter().position(|t| t.id == ft.id) {
                chat::merge_thread_into(&mut self.threads[pos], ft, false);
            } else {
                self.threads.push(ft);
            }
        }
        self.rebuild_filter();
        self.persist_async();
    }

    fn check_background_events(&mut self) {
        self.flush_pending_snapshot();
        while let Ok(envelope) = self.bg_rx.try_recv() {
            if self.logout_inflight {
                // When logout is in flight, discard all incoming background events
                // (polls, hydration, history loads, send outcomes, etc.) except LogoutDone.
                if let BgEvent::LogoutDone { owner, result } = envelope.event {
                    if owner == self.session.account_key() {
                        self.logout_inflight = false;
                        match result {
                            Ok(()) => {
                                self.session.logout();
                                self.status = "Đã đăng xuất. Bấm 'q' để thoát.".into();
                            }
                            Err(e) => {
                                self.status =
                                    format!("Không thể xóa phiên: {e}. Bấm 'q' để thoát.");
                                crate::logger::log_error(
                                    "tui logout",
                                    &format!("Lỗi xóa phiên: {e}"),
                                );
                            }
                        }
                    }
                }
                continue;
            }
            if !self.session.is_active() || !envelope.stamp.matches(self.session) {
                continue;
            }
            let event = envelope.event;
            match event {
                BgEvent::Hydrated(threads) => {
                    // Hydration is async: merge so sends, polls and history that
                    // arrived meanwhile are kept, then persist the startup batch.
                    merge_hydrated(&mut self.threads, threads);
                    // Independent of writer ordering: never resurrect a local delete.
                    for thread in &mut self.threads {
                        if let Some(ids) = self.deleted_messages.get(&thread.id) {
                            Self::filter_deleted_replays(thread, ids);
                        }
                    }
                    self.rebuild_filter();
                    self.ensure_msg_selection();
                    self.persist_async();
                }
                BgEvent::PollDone { threads, state } => {
                    self.poll_inflight = false;
                    let changed = self.apply_polled(threads, state);
                    self.status = format!("Cap nhat {}", crate::ui::clock());
                    if changed {
                        self.persist_async();
                    }
                }
                BgEvent::PollFailed(err) => {
                    self.poll_inflight = false;
                    self.status = format!("Loi cap nhat: {err}");
                }
                BgEvent::ThreadsRefreshed(list) => {
                    let n = list.len();
                    self.apply_refresh(list);
                    self.status = format!("Da lam moi ({n} hoi thoai)");
                }
                BgEvent::HistoryLoaded { thread } => {
                    let thread_id = thread.id.clone();
                    let allowed = self
                        .threads
                        .iter()
                        .find(|existing| existing.id == thread_id)
                        .is_some_and(|existing| {
                            !self.session.hidden.enforced()
                                || self.hidden_view().allows(&existing.id, existing.is_group)
                        });
                    if !allowed {
                        continue;
                    }
                    let mut count = 0usize;
                    let deleted_ids = self
                        .deleted_messages
                        .get(&thread_id)
                        .cloned()
                        .unwrap_or_default();
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        chat::merge_thread_into(t, thread, false);
                        Self::filter_deleted_replays(t, &deleted_ids);
                        count = t.messages.len();
                    }
                    if count > 0 {
                        self.persist_async();
                        self.selected_msg = None;
                        self.follow_tail = true;
                        self.status = format!("Da tai lich su nhom ({count} tin)");
                    } else {
                        self.status = "Khong co tin nhan moi tu nhom".into();
                    }
                }
                BgEvent::SendOutcome {
                    thread_id,
                    client_message_id,
                    outcome,
                } => {
                    let mut accepted = false;
                    if let Some(message) = self
                        .threads
                        .iter_mut()
                        .find(|thread| thread.id == thread_id)
                        .and_then(|thread| {
                            thread
                                .messages
                                .iter_mut()
                                .find(|message| message.client_message_id == client_message_id)
                        })
                    {
                        let echoed = !message.id.is_empty()
                            && !message.id.starts_with("out_")
                            && !message.id.starts_with("local_");
                        if echoed {
                            message.delivery = message
                                .delivery
                                .merge(crate::message_model::DeliveryState::ServerAccepted);
                            message.outgoing_job = None;
                            self.status = "Đã được máy chủ xác nhận".into();
                            accepted = true;
                        } else {
                            match outcome {
                                SendDisposition::Accepted(mut sent) => {
                                    let state = message.delivery.merge(sent.delivery);
                                    if message.id.starts_with("out_")
                                        || message.id.starts_with("local_")
                                    {
                                        sent.delivery = state;
                                        *message = *sent;
                                    } else {
                                        message.delivery = state;
                                    }
                                    message.outgoing_job = None;
                                    self.status = "Đã được máy chủ xác nhận".into();
                                    accepted = true;
                                }
                                SendDisposition::Rejected(error) => {
                                    message.delivery = message
                                        .delivery
                                        .merge(crate::message_model::DeliveryState::Failed);
                                    if let Some(job) = message.outgoing_job.as_mut() {
                                        job.error = error.clone();
                                    }
                                    self.status = format!("Máy chủ từ chối gửi: {error}");
                                }
                                SendDisposition::Unknown(error) => {
                                    message.delivery = message.delivery.merge(
                                        crate::message_model::DeliveryState::DeliveryUnknown,
                                    );
                                    if let Some(job) = message.outgoing_job.as_mut() {
                                        job.error = error.clone();
                                    }
                                    self.status = format!("Chưa rõ máy chủ đã nhận tin: {error}");
                                }
                                SendDisposition::NotSent(error) => {
                                    message.delivery = message
                                        .delivery
                                        .merge(crate::message_model::DeliveryState::Failed);
                                    if let Some(job) = message.outgoing_job.as_mut() {
                                        job.error = error.clone();
                                    }
                                    self.status = format!("Chưa gửi được tin: {error}");
                                }
                            }
                        }
                    }
                    if accepted {
                        self.bump_thread(&thread_id);
                    }
                    self.persist_async();
                }
                BgEvent::ReactionDone {
                    thread_id,
                    msg_id,
                    icon,
                } => {
                    if let Some(m) = self
                        .threads
                        .iter_mut()
                        .find(|t| t.id == thread_id)
                        .and_then(|t| t.messages.iter_mut().find(|m| m.id == msg_id))
                    {
                        let code = chat::reaction_code_and_type(&icon).0;
                        if !m.reactions.iter().any(|r| r == code) {
                            m.reactions.push(code.to_string());
                        }
                    }
                    self.status = "Da tha cam xuc".into();
                }
                BgEvent::DeleteDone {
                    thread_id,
                    deleted,
                    result,
                } => {
                    self.delete_inflight = false;
                    let ids = self.deleted_messages.entry(thread_id.clone()).or_default();
                    ids.push(deleted.clone());
                    let deleted_ids = ids.clone();
                    if let Some(t) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                        Self::filter_deleted_replays(t, &deleted_ids);
                    }
                    self.selected_msg = None;
                    match result {
                        Ok(()) => {
                            self.persist_async();
                            self.status = "Đã xóa tin nhắn trên máy chủ và lưu cục bộ".into();
                        }
                        Err(error) => {
                            self.status = format!("Máy chủ đã xác nhận xóa, nhưng chưa lưu được bản xóa cục bộ: {error}");
                        }
                    }
                }
                BgEvent::DeleteFailed(error) => {
                    self.delete_inflight = false;
                    self.status = error;
                }
                BgEvent::HiddenStateChanged(result) => {
                    self.rebuild_filter();
                    self.clear_private_draft();
                    match result {
                        Ok(()) => {
                            self.status = if self.hidden_view().unlocked {
                                "Danh sách hội thoại ẩn đã được mở khóa".into()
                            } else {
                                "Đã cập nhật trạng thái hội thoại ẩn".into()
                            }
                        }
                        Err(_) => {
                            self.status =
                                "Không xác minh được hội thoại ẩn; đang chặn hiển thị".into()
                        }
                    }
                }
                BgEvent::StoreFailed(error) => {
                    self.status = format!("Không lưu được lịch sử cục bộ: {error}");
                }
                BgEvent::StoreSaved => {
                    if self.status == "Đang chờ lưu lịch sử cục bộ" {
                        self.status = "Đã lưu lịch sử cục bộ".into();
                    }
                }
                BgEvent::LogoutDone { .. } => {
                    // Handled above when logout_inflight is true.
                }
                BgEvent::StickerPacks(packs) => {
                    self.sticker.loading = false;
                    self.sticker.items.clear();
                    self.sticker.packs = packs;
                    self.sticker.pack_idx = 0;
                    self.sticker.item_idx = 0;
                    self.sticker.stage = 0;
                    self.status = format!("Kho nhan dan: {} bo", self.sticker.packs.len());
                }
                BgEvent::StickerItems(items) => {
                    self.sticker.loading = false;
                    let n = items.len();
                    self.sticker.items = items;
                    self.sticker.item_idx = 0;
                    self.sticker.stage = 1;
                    self.status = if n == 0 {
                        "Bo nhan dan trong".into()
                    } else {
                        format!("Bo nhan dan: {n} sticker")
                    };
                }
                BgEvent::ActionFailed(err) => {
                    self.status = err;
                }
            }
        }
    }
}

pub async fn run_tui(mut app: TuiApp<'_>) -> Result<()> {
    app.session.hidden.enable();
    app.status = "Đang xác minh trạng thái hội thoại ẩn...".into();
    app.rebuild_filter();
    app.spawn_hidden_refresh();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app).await;

    let terminal_restore = (|| -> Result<()> {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        Ok(())
    })();
    let drained = app.shutdown_writer().await;
    if !drained {
        eprintln!("TUI đã hết thời gian chờ lưu. Một thao tác gửi hoặc xóa có thể chưa được xác nhận cục bộ; hãy kiểm tra hội thoại trước khi thử lại. Tin chưa xác nhận được giữ ở trạng thái cần kiểm tra và không tự gửi lại.");
    }
    terminal_restore?;
    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiApp<'_>,
) -> Result<()> {
    let mut last_poll = std::time::Instant::now();

    while !app.should_quit {
        app.check_background_events();
        terminal.draw(|f| ui_draw(f, app))?;

        if event::poll(Duration::from_millis(30))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key.code, key.modifiers);
                }
                Event::Mouse(m) => app.handle_mouse(m),
                _ => {}
            }
        }

        if last_poll.elapsed() >= Duration::from_secs(POLL_SECS) {
            last_poll = std::time::Instant::now();
            app.spawn_poll();
        }
    }
    Ok(())
}

fn ui_draw(f: &mut Frame, app: &mut TuiApp<'_>) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(4),
        ])
        .split(size);

    // 1. Header bar
    let user_name = app
        .session
        .login
        .as_ref()
        .map(|l| {
            if l.display_name.is_empty() {
                format!("UIN: {}", l.uin)
            } else {
                format!("{} (UIN: {})", l.display_name, l.uin)
            }
        })
        .unwrap_or_else(|| "Chua dang nhap".into());

    let mode_span = if logger::is_dev_mode() {
        Span::styled(
            " [DEV] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " [PROD] ",
            Style::default().fg(Color::White).bg(Color::Green),
        )
    };

    let hidden = app.hidden_view();
    let total_unread: usize = app
        .threads
        .iter()
        .filter(|thread| TuiApp::thread_allowed(&hidden, app.session.hidden.enforced(), thread))
        .map(|thread| app.unread.get(&thread.id).copied().unwrap_or_default())
        .sum();
    let mut header_spans = vec![
        Span::styled(
            " Zalo Rust Client ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("| Tai khoan: "),
        Span::styled(
            user_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Chua doc: "),
        Span::styled(
            total_unread.to_string(),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(&app.status, Style::default().fg(Color::Yellow)),
        Span::raw(" "),
        mode_span,
    ];
    if app.poll_inflight {
        header_spans.push(Span::styled(
            " [dang cap nhat] ",
            Style::default().fg(Color::Yellow),
        ));
    }

    let header = Paragraph::new(Line::from(header_spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Left);
    f.render_widget(header, chunks[0]);

    // 2. Body: thread list | chat area
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(chunks[1]);
    app.threads_width = body_chunks[0].width;

    let selected_pos = app.list_state.selected();
    let thread_items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(pos, &i)| {
            let t = &app.threads[i];
            let is_sel = Some(pos) == selected_pos;
            let tag = if t.name.contains("Cloud") {
                "[Cloud] "
            } else if t.is_group {
                "[Group] "
            } else {
                "[User]  "
            };
            let cursor = if is_sel { "> " } else { "  " };
            let name_style = if is_sel {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let preview_text = if t.preview.is_empty() {
                "- chua co tin nhan -".to_string()
            } else {
                let p = t.preview.chars().take(22).collect::<String>();
                format!("{p}...")
            };
            let mut line1 = vec![
                Span::raw(cursor),
                Span::styled(tag, Style::default().fg(Color::DarkGray)),
                Span::styled(t.name.clone(), name_style),
                Span::raw("  "),
                Span::styled(t.time.clone(), Style::default().fg(Color::DarkGray)),
            ];
            if let Some(n) = app.unread.get(&t.id).copied().filter(|n| *n > 0) {
                line1.push(Span::raw(" "));
                line1.push(Span::styled(
                    format!("({n})"),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            let line2 = Line::from(vec![
                Span::raw("    "),
                Span::styled(preview_text, Style::default().fg(Color::Gray)),
            ]);
            ListItem::new(vec![Line::from(line1), line2])
        })
        .collect();

    let visible_thread_count = app
        .threads
        .iter()
        .filter(|thread| TuiApp::thread_allowed(&hidden, app.session.hidden.enforced(), thread))
        .count();
    let threads_title = if app.search_mode {
        format!(" Tim: {}_ ", app.search)
    } else if !app.search.is_empty() {
        format!(
            " Hoi thoai {}/{} [{}] tim '{}' ",
            app.filtered.len(),
            visible_thread_count,
            app.filter.label(),
            app.search
        )
    } else {
        format!(
            " Hoi thoai {}/{} [{}] ",
            app.filtered.len(),
            visible_thread_count,
            app.filter.label()
        )
    };

    let threads_border_style = if app.focus == Focus::Threads {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let threads_list = List::new(thread_items)
        .block(
            Block::default()
                .title(threads_title)
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(threads_border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(25, 40, 65))
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(threads_list, body_chunks[0], &mut app.list_state);

    // 2b. Right column: contact header, messages, reply bar, input
    let selected_allowed = app.selected_thread_allowed();
    let reply_h = if app.reply_target.is_some() && selected_allowed {
        2
    } else {
        0
    };
    let chat_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(reply_h),
            Constraint::Length(3),
        ])
        .split(body_chunks[1]);

    let (name, id, is_group, msg_count) = match app
        .threads
        .get(app.selected_thread)
        .filter(|_| app.selected_thread_allowed())
    {
        Some(t) => (t.name.clone(), t.id.clone(), t.is_group, t.messages.len()),
        None => ("Chua chon hoi thoai".to_string(), String::new(), false, 0),
    };
    let kind = if is_group { "Nhom" } else { "Ca nhan" };
    let chat_header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {name} "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("[{kind}] "), Style::default().fg(Color::DarkGray)),
        Span::raw("| ID: "),
        Span::styled(id, Style::default().fg(Color::DarkGray)),
        Span::raw(format!(" | {msg_count} tin | ")),
        if app.follow_tail {
            Span::styled("theo doi cuoi", Style::default().fg(Color::Green))
        } else {
            Span::styled("dang xem lich su", Style::default().fg(Color::Yellow))
        },
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(chat_header, chat_chunks[0]);

    if app.focus == Focus::Messages {
        app.ensure_msg_selection();
    }

    let inner_w = chat_chunks[1].width.saturating_sub(2) as usize;
    let inner_h = chat_chunks[1].height.saturating_sub(2) as usize;
    let highlight = app.focus == Focus::Messages;
    let (lines, ranges) = match app
        .threads
        .get(app.selected_thread)
        .filter(|_| app.selected_thread_allowed())
    {
        Some(t) => build_chat_lines(t, inner_w, app.selected_msg, highlight),
        None => (
            vec![Line::from(Span::styled(
                "  Chon mot hoi thoai de bat dau.",
                Style::default().fg(Color::DarkGray),
            ))],
            Vec::new(),
        ),
    };
    let max_scroll = lines.len().saturating_sub(inner_h);
    if app.follow_tail {
        app.msg_scroll = max_scroll;
    } else {
        app.msg_scroll = app.msg_scroll.min(max_scroll);
        if app.msg_scroll >= max_scroll {
            app.follow_tail = true;
        }
    }
    app.msg_ranges = ranges;
    app.chat_view_h = inner_h;

    let pct = if lines.len() <= inner_h {
        "100%".to_string()
    } else {
        format!(
            "{}%",
            ((app.msg_scroll + inner_h) * 100 / lines.len().max(1)).min(100)
        )
    };

    let chat_border_style = if app.focus == Focus::Messages {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let chat_body = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Noi dung tin nhan [{pct}] "))
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(chat_border_style),
        )
        .scroll(((app.msg_scroll.min(u16::MAX as usize)) as u16, 0));
    f.render_widget(chat_body, chat_chunks[1]);

    if selected_allowed {
        if let Some(q) = &app.reply_target {
            let quote = format!(
                " {}: {}",
                q.sender,
                q.text
                    .replace('\n', " ")
                    .chars()
                    .take(120)
                    .collect::<String>()
            );
            let reply = Paragraph::new(quote)
                .block(
                    Block::default()
                        .title(" Dang tra loi [Ctrl+X huy] ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Plain)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .style(Style::default().fg(Color::Gray));
            f.render_widget(reply, chat_chunks[2]);
        }
    }

    let input_title = if app.focus == Focus::Input {
        " Soan tin nhan: [DANG GO] (Enter: Gui | Esc: Lui | Ctrl+X: Huy tra loi) "
    } else {
        " Soan tin nhan: [Tab de nhap] "
    };
    let input_border_style = if app.focus == Focus::Input {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let cursor = if app.focus == Focus::Input { "_" } else { "" };
    let input_text = if selected_allowed {
        app.input_text.as_str()
    } else {
        ""
    };
    let input_widget = Paragraph::new(format!(" {input_text}{cursor}")).block(
        Block::default()
            .title(input_title)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(input_border_style),
    );
    f.render_widget(input_widget, chat_chunks[3]);

    // 3. Footer
    let (hint1, hint2) = match app.focus {
        Focus::Threads => (
            "[Up/Down j/k] chon | [PgUp/PgDn] nhay 10 | [Enter] xem tin | [Tab] doi vung | [/] tim | [f] loc",
            "[r] lam moi | [h] lich su nhom | [u] xoa chua doc | [F2] nhan dan | [F3] tep | [x] dang xuat | [F1] tro giup | [q] thoat",
        ),
        Focus::Messages => (
            "[Up/Down j/k] chon tin | [PgUp/PgDn] cuon | [Home/End] dau/cuoi | [Enter] soan | [Tab] doi vung",
            "[1-6] cam xuc | [y] tra loi | [d] xoa | [o] mo link | [F2] nhan dan | [F3] tep | [F1] tro giup | [q] thoat",
        ),
        Focus::Input => (
            "[Enter] gui | [Ctrl+U] xoa het | [Ctrl+X] huy tra loi | [Esc] lui | [Tab] doi vung",
            "[F2] nhan dan | [F3] tep dinh kem | [F1] tro giup | [Ctrl+C] thoat",
        ),
    };
    let footer_text = if app.search_mode {
        format!("Tim kiem: {}_   (Enter: xong | Esc: xoa)", app.search)
    } else {
        String::new()
    };
    let footer = Paragraph::new(vec![
        Line::from(Span::styled(hint1, Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            footer_text,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(hint2, Style::default().fg(Color::DarkGray))),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .title(" Phim tat "),
    )
    .alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);

    // 4. Overlays
    match app.overlay {
        Overlay::Help => render_help(f, size),
        Overlay::Log => render_log_modal(f, size, app.log_scroll),
        Overlay::Sticker => render_sticker_overlay(f, size, app),
        Overlay::None => {}
    }
    if let Some(action) = &app.confirm {
        render_confirm(f, size, action);
    }
    if let Some(prompt) = &app.prompt {
        render_prompt(f, size, prompt);
    }
}

fn build_chat_lines(
    thread: &Thread,
    width: usize,
    selected: Option<usize>,
    highlight: bool,
) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
    let width = width.max(20);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    if thread.messages.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            format!("  Hoi thoai '{}' da san sang.", thread.name),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Bam Tab de soan tin nhan, Enter de gui.",
            Style::default().fg(Color::Cyan),
        )));
        return (lines, ranges);
    }

    let mut last_day: Option<i64> = None;
    for (idx, m) in thread.messages.iter().enumerate() {
        if let Some(day) = message_day(m.ts) {
            if last_day != Some(day) {
                if !lines.is_empty() {
                    lines.push(Line::raw(""));
                }
                lines.push(Line::from(Span::styled(
                    format!("--- {} ---", day_label(day)),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )));
                last_day = Some(day);
            }
        }

        let start = lines.len();
        let sel = highlight && selected == Some(idx);

        let sender_name = if m.is_outgoing {
            "Toi".to_string()
        } else if !m.sender.trim().is_empty() {
            m.sender.clone()
        } else if thread.is_group {
            "Thanh vien".to_string()
        } else {
            thread.name.clone()
        };
        let time_disp = if m.time.is_empty() {
            "--:--".to_string()
        } else {
            m.time.clone()
        };
        let pin_part = if m.is_pinned { "* " } else { "" };
        let time_part = format!("[{time_disp}] ");
        let name_part = format!("{sender_name}: ");
        let prefix_len =
            pin_part.chars().count() + time_part.chars().count() + name_part.chars().count();

        let mut prefix: Vec<Span<'static>> = Vec::new();
        if m.is_pinned {
            prefix.push(Span::styled("* ", Style::default().fg(Color::Yellow)));
        }
        prefix.push(Span::styled(
            time_part,
            Style::default().fg(Color::DarkGray),
        ));
        let sender_style = if m.is_outgoing {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };
        prefix.push(Span::styled(name_part, sender_style));

        if let (Some(qs), Some(qt)) = (&m.reply_to_sender, &m.reply_to_text) {
            let quote = format!("| {}: {}", qs, qt.replace('\n', " "));
            for chunk in wrap_text(&quote, width.saturating_sub(prefix_len).max(16)) {
                lines.push(Line::from(vec![
                    Span::styled(" ".repeat(prefix_len), Style::default()),
                    Span::styled(
                        chunk,
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        }

        let mut body = if m.sticker_id.unwrap_or(0) > 0 {
            format!("[Nhan dan #{}]", m.sticker_id.unwrap_or(0))
        } else {
            m.text.clone()
        };
        if let Some(url) = &m.link_url {
            if !url.is_empty() && !body.contains(url.as_str()) {
                if !body.is_empty() {
                    body.push(' ');
                }
                body.push_str(url);
            }
        }
        if !m.reactions.is_empty() {
            let icons: Vec<&str> = m.reactions.iter().map(|r| chat::icon_to_emoji(r)).collect();
            body.push_str("  ");
            body.push_str(&icons.join(" "));
        }
        if body.trim().is_empty() {
            body = "(khong co noi dung)".to_string();
        }

        let avail = width.saturating_sub(prefix_len).max(10);
        let chunks = wrap_text(&body, avail);
        for (ci, chunk) in chunks.iter().enumerate() {
            if ci == 0 {
                let mut spans = prefix.clone();
                spans.push(Span::styled(
                    chunk.clone(),
                    Style::default().fg(Color::White),
                ));
                lines.push(Line::from(spans));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(" ".repeat(prefix_len), Style::default()),
                    Span::styled(chunk.clone(), Style::default().fg(Color::White)),
                ]));
            }
        }

        if sel {
            let bg = Style::default().bg(Color::Rgb(35, 48, 70));
            for line in lines.iter_mut().skip(start) {
                *line = line.clone().style(bg);
            }
        }
        ranges.push((start, lines.len()));
    }

    (lines, ranges)
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(4);
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0usize;

    for word in text.split_whitespace() {
        let wl = word.chars().count();
        if wl > width {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
                cur_len = 0;
            }
            let mut buf = String::new();
            let mut n = 0usize;
            for ch in word.chars() {
                buf.push(ch);
                n += 1;
                if n == width {
                    out.push(std::mem::take(&mut buf));
                    n = 0;
                }
            }
            if !buf.is_empty() {
                cur = buf;
                cur_len = n;
            }
            continue;
        }
        if cur_len > 0 && cur_len + 1 + wl > width {
            out.push(std::mem::take(&mut cur));
            cur_len = 0;
        }
        if cur_len > 0 {
            cur.push(' ');
            cur_len += 1;
        }
        cur.push_str(word);
        cur_len += wl;
    }

    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn normalize_ts_ms(ts: i64) -> i64 {
    if ts <= 0 {
        0
    } else if ts < 10_000_000_000 {
        ts * 1000
    } else {
        ts
    }
}

fn message_day(ts: i64) -> Option<i64> {
    let ms = normalize_ts_ms(ts);
    if ms <= 0 {
        return None;
    }
    Some((ms / 1000 + 7 * 3600).div_euclid(86_400))
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
    (year, m, d)
}

fn day_label(day: i64) -> String {
    let today = (crate::login::unix_now() + 7 * 3600).div_euclid(86_400);
    let (y, m, d) = civil_from_days(day);
    if day == today {
        format!("Hom nay {d:02}/{m:02}/{y}")
    } else if day == today - 1 {
        format!("Hom qua {d:02}/{m:02}/{y}")
    } else {
        format!("{d:02}/{m:02}/{y}")
    }
}

fn persist_acknowledged_delete(
    store: &mut crate::store::MessageStore,
    thread_id: &str,
    message: &ChatMessage,
) -> Result<()> {
    store.delete_chat_message(thread_id, message)
}

fn open_external_url(url: &str) {
    let _ = crate::platform::open_url(url);
}

fn help_line(keys: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {keys:<14}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc.to_string(), Style::default().fg(Color::White)),
    ])
}

// Sequential pushes keep the key/description pairs readable beside their styles.
#[allow(clippy::vec_init_then_push)]
fn render_help(f: &mut Frame, area: Rect) {
    let modal_area = centered_rect(80, 80, area);
    f.render_widget(Clear, modal_area);

    let mut content: Vec<Line<'static>> = Vec::new();
    content.push(Line::from(Span::styled(
        "=== PHIM TAT ZALO TUI ===",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    content.push(Line::raw(""));
    content.push(help_line(
        "Tab / BackTab",
        "Chuyen vung: Danh sach -> Tin nhan -> Soan tin",
    ));
    content.push(help_line("F1 / ?", "Mo tro giup nay"));
    content.push(help_line("q / Ctrl+C", "Thoat (q chi khi khong go tin)"));
    content.push(Line::raw(""));
    content.push(Line::from(Span::styled(
        "-- Danh sach hoi thoai --",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));
    content.push(help_line(
        "Up/Down j/k",
        "Chon hoi thoai (cuon chuot cung duoc)",
    ));
    content.push(help_line("PgUp/PgDn", "Nhay 10 hoi thoai"));
    content.push(help_line("Home / End", "Hoi thoai dau / cuoi"));
    content.push(help_line("/", "Tim theo ten, ID, noi dung, tin nhan"));
    content.push(help_line("f", "Loc: Tat ca -> Ca nhan -> Nhom"));
    content.push(help_line("u", "Xoa tat ca thong bao chua doc"));
    content.push(help_line("r", "Lam moi danh sach tu server (nen)"));
    content.push(help_line("R", "Tai lai trang thai/PIN hoi thoai an"));
    content.push(help_line("p", "Mo khoa PIN hoi thoai an; nhan lai de khoa"));
    content.push(help_line("h", "Tai lich su nhom (50 tin)"));
    content.push(help_line("x", "Dang xuat (co xac nhan)"));
    content.push(Line::raw(""));
    content.push(Line::from(Span::styled(
        "-- Tin nhan --",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));
    content.push(help_line("Up/Down j/k", "Chon tin nhan"));
    content.push(help_line("PgUp/PgDn", "Cuon nhanh"));
    content.push(help_line("Home / End", "Dau / cuoi hoi thoai"));
    content.push(help_line(
        "1 .. 6",
        "Tha cam xuc nhanh (Thich, Yeu, Haha, Wow, Buon, Phan no)",
    ));
    content.push(help_line("y", "Tra loi tin dang chon"));
    content.push(help_line("d", "Xoa tin nhan (chi phia ban)"));
    content.push(help_line(
        "Y",
        "Gửi lại tin thất bại hoặc chưa rõ (giữ nguyên mã)",
    ));
    content.push(help_line("o", "Mo lien ket cua tin nhan"));
    content.push(Line::raw(""));
    content.push(Line::from(Span::styled(
        "-- Soan tin / dinh kem --",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));
    content.push(help_line("Enter", "Gui tin nhan"));
    content.push(help_line("Ctrl+U", "Xoa toan bo o soan"));
    content.push(help_line("Ctrl+X", "Huy tra loi"));
    content.push(help_line("Esc", "Lui ve vung truoc"));
    content.push(help_line(
        "F2",
        "Kho nhan dan: Enter chon bo, Enter lan 2 de gui",
    ));
    content.push(help_line("F3", "Gui tep dinh kem bang duong dan"));
    content.push(help_line("l", "Dev log inspector (chi che do --dev)"));
    content.push(Line::raw(""));
    content.push(Line::from(Span::styled(
        "Esc hoac F1 de dong tro giup.",
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .title(" Tro giup (Esc/F1 de dong) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, modal_area);
}

fn render_sticker_overlay(f: &mut Frame, area: Rect, app: &TuiApp<'_>) {
    let modal_area = centered_rect(84, 80, area);
    f.render_widget(Clear, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(modal_area);

    let loading = if app.sticker.loading {
        " (dang tai...)"
    } else {
        ""
    };
    let packs_title = if app.sticker.stage == 0 {
        format!(
            " Kho nhan dan ({} bo){loading} [DANG CHON] — Enter de mo bo ",
            app.sticker.packs.len()
        )
    } else {
        format!(
            " Kho nhan dan ({} bo){loading} — Left/Right doi cot ",
            app.sticker.packs.len()
        )
    };
    let pack_items: Vec<ListItem> = app
        .sticker
        .packs
        .iter()
        .map(|p| {
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", p.name), Style::default().fg(Color::White)),
                Span::styled(
                    format!("({} sticker)", p.total_image),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let mut packs_state = ListState::default();
    if !app.sticker.packs.is_empty() {
        packs_state.select(Some(app.sticker.pack_idx.min(app.sticker.packs.len() - 1)));
    }
    let packs_border = if app.sticker.stage == 0 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let packs_list = List::new(pack_items)
        .block(
            Block::default()
                .title(packs_title)
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(packs_border),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(25, 40, 65))
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(packs_list, chunks[0], &mut packs_state);

    let items_title = if app.sticker.items.is_empty() {
        format!(
            " Bo '{}' — chon mot bo ben tren roi Enter ",
            app.sticker.loaded_name
        )
    } else if app.sticker.stage == 1 {
        format!(
            " Bo '{}' ({} sticker) [DANG CHON] — Enter de gui ",
            app.sticker.loaded_name,
            app.sticker.items.len()
        )
    } else {
        format!(
            " Bo '{}' ({} sticker) ",
            app.sticker.loaded_name,
            app.sticker.items.len()
        )
    };
    let item_rows: Vec<ListItem> = app
        .sticker
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let label = if it.label.trim().is_empty() {
                format!("Sticker #{}", it.sticker_id)
            } else {
                it.label.clone()
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {:>3}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(label, Style::default().fg(Color::White)),
                Span::styled(
                    format!("  id={} cate={}", it.sticker_id, it.cate_id),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let mut items_state = ListState::default();
    if !app.sticker.items.is_empty() {
        items_state.select(Some(app.sticker.item_idx.min(app.sticker.items.len() - 1)));
    }
    let items_border = if app.sticker.stage == 1 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let items_list = List::new(item_rows)
        .block(
            Block::default()
                .title(items_title)
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(items_border),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(25, 40, 65))
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(items_list, chunks[1], &mut items_state);
}

fn render_confirm(f: &mut Frame, area: Rect, action: &ConfirmAction) {
    let modal_area = centered_rect(60, 26, area);
    f.render_widget(Clear, modal_area);

    let content = vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!("  {}", action.question),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("  {}", action.detail),
            Style::default().fg(Color::White),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "  [y/Enter] Xac nhan    [n/Esc] Huy",
            Style::default().fg(Color::Cyan),
        )),
    ];
    let paragraph = Paragraph::new(content).block(
        Block::default()
            .title(" Xac nhan ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    f.render_widget(paragraph, modal_area);
}

fn render_prompt(f: &mut Frame, area: Rect, prompt: &PromptState) {
    let modal_area = centered_rect(70, 24, area);
    f.render_widget(Clear, modal_area);

    let title = match prompt.kind {
        PromptKind::Attachment => " Gui tep dinh kem: nhap duong dan tep ",
        PromptKind::HiddenPin => " Nhap PIN hoi thoai an ",
    };
    let content = vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!(
                "  {}_",
                if prompt.kind == PromptKind::HiddenPin {
                    "•".repeat(prompt.text.chars().count())
                } else {
                    prompt.text.clone()
                }
            ),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            if prompt.kind == PromptKind::HiddenPin {
                "  [Enter] Mo khoa    [Esc] Huy"
            } else {
                "  [Enter] Gui    [Esc] Huy"
            },
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let paragraph = Paragraph::new(content).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(paragraph, modal_area);
}

fn render_log_modal(f: &mut Frame, area: Rect, scroll: usize) {
    let modal_area = centered_rect(88, 80, area);
    f.render_widget(Clear, modal_area);

    let findings = logger::scan_logs();
    let entries = logger::get_entries();

    let mut content = Vec::new();

    content.push(Line::from(vec![Span::styled(
        "=== KET QUA QUET LOG & CHAN DOAN GIAO THUC ===",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )]));
    for finding in &findings {
        let (icon, color) = match finding.level {
            FindingLevel::Info => ("[INFO] ", Color::Green),
            FindingLevel::Warning => ("[WARN] ", Color::Yellow),
            FindingLevel::Critical => ("[CRIT] ", Color::Red),
        };
        content.push(Line::from(vec![
            Span::styled(
                icon,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                finding.title.clone(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));
        content.push(Line::from(vec![
            Span::raw("  Chi tiet: "),
            Span::styled(finding.detail.clone(), Style::default().fg(Color::White)),
        ]));
        content.push(Line::from(vec![
            Span::raw("  Goi y: "),
            Span::styled(finding.suggestion.clone(), Style::default().fg(Color::Cyan)),
        ]));
        content.push(Line::raw(""));
    }

    content.push(Line::from(vec![Span::styled(
        format!(
            "=== LUU LUONG MANG THOI GIAN THUC ({} requests) ===",
            entries.len()
        ),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    for e in entries.iter().rev() {
        let status_color = if e.is_error { Color::Red } else { Color::Green };
        let status_str = e
            .status
            .map(|s| s.to_string())
            .unwrap_or_else(|| "ERR".into());

        content.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", e.timestamp),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:<4} ", e.method),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<3} ", status_str),
                Style::default().fg(status_color),
            ),
            Span::styled(
                format!("({}ms) ", e.latency_ms),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(e.url.clone(), Style::default().fg(Color::White)),
        ]));
        if let Some(dec) = &e.decrypted_preview {
            let preview = dec.chars().take(80).collect::<String>();
            content.push(Line::from(vec![
                Span::raw("  + Decrypted: "),
                Span::styled(format!("{preview}..."), Style::default().fg(Color::Gray)),
            ]));
        }
        if let Some(err) = &e.error_msg {
            content.push(Line::from(vec![Span::styled(
                format!("  + Loi: {err}"),
                Style::default().fg(Color::Red),
            )]));
        }
    }

    let block = Block::default()
        .title(" Dev Inspector & Protocol Scanner (Esc/q de dong, Up/Down de cuon) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::Yellow));

    let p = Paragraph::new(content)
        .block(block)
        .scroll((scroll.min(u16::MAX as usize) as u16, 0))
        .wrap(Wrap { trim: true });

    f.render_widget(p, modal_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_stamp_rejects_events_from_revoked_session() {
        let mut session = crate::session::Session::new(false, false, false).unwrap();
        let stamp = RuntimeStamp::for_session(&session);
        assert!(stamp.matches(&session));

        session.logout();
        assert!(!stamp.matches(&session));
    }

    #[test]
    fn history_writer_admission_is_bounded_and_nonblocking() {
        let (writer, rx) = StoreWriter::channel();
        let session = crate::session::Session::new(false, false, false).unwrap();
        let (event_tx, _event_rx) = channel();
        let snapshot = || {
            let stamp = RuntimeStamp::for_session(&session);
            StoreCommand::Snapshot {
                session: session.clone(),
                stamp: stamp.clone(),
                threads: Vec::new(),
                completion: BgSender {
                    tx: event_tx.clone(),
                    stamp,
                },
            }
        };

        // Reserve the final slot for a delete that may be acknowledged remotely.
        let delete_reservation = writer.try_reserve_delete().unwrap();
        for _ in 1..StoreWriter::CAPACITY {
            writer.try_send(snapshot()).unwrap();
        }
        assert!(matches!(
            writer.try_reserve_delete(),
            Err(StoreAdmissionError::Full)
        ));
        writer
            .enqueue_reserved(snapshot(), delete_reservation)
            .expect("reserved admission remains available after remote ACK");
        assert_eq!(rx.try_iter().count(), StoreWriter::CAPACITY);
    }

    #[test]
    fn hydration_merges_history_without_dropping_live_sends_or_new_threads() {
        let message = |id: &str, client: &str, ts: i64| ChatMessage {
            id: id.into(),
            client_message_id: client.into(),
            ts,
            ..Default::default()
        };
        let thread = |id: &str, messages: Vec<ChatMessage>| Thread {
            id: id.into(),
            messages,
            ..Default::default()
        };
        // Live state: an optimistic send typed before hydration completed, plus a
        // thread first seen by an early poll.
        let mut live = vec![
            thread("peer-1", vec![message("out_1", "cli-1", 300)]),
            thread("polled-only", vec![message("srv-9", "", 400)]),
        ];
        let hydrated = vec![
            thread(
                "peer-1",
                vec![message("srv-1", "", 100), message("srv-2", "", 200)],
            ),
            thread("history-only", vec![message("srv-5", "", 50)]),
        ];
        merge_hydrated(&mut live, hydrated);

        let ids = |id: &str| -> Vec<String> {
            live.iter()
                .find(|t| t.id == id)
                .map(|t| t.messages.iter().map(|m| m.id.clone()).collect())
                .unwrap_or_default()
        };
        let peer = ids("peer-1");
        assert!(
            peer.contains(&"out_1".to_owned()),
            "optimistic send survives: {peer:?}"
        );
        assert!(peer.contains(&"srv-1".to_owned()) && peer.contains(&"srv-2".to_owned()));
        assert_eq!(ids("polled-only"), vec!["srv-9".to_owned()]);
        assert_eq!(ids("history-only"), vec!["srv-5".to_owned()]);
        let pending = live[0]
            .messages
            .iter()
            .find(|m| m.client_message_id == "cli-1");
        assert!(
            pending.is_some(),
            "SendOutcome can still find the client ID"
        );
    }

    fn unauthenticated_snapshot(
        session: &crate::session::Session,
        event_tx: &Sender<BgEnvelope>,
    ) -> StoreCommand {
        // No login: the worker must reply with an error and never open a real DB.
        let stamp = RuntimeStamp::for_session(session);
        StoreCommand::Snapshot {
            session: session.clone(),
            stamp: stamp.clone(),
            threads: Vec::new(),
            completion: BgSender {
                tx: event_tx.clone(),
                stamp,
            },
        }
    }

    #[tokio::test]
    async fn history_writer_shutdown_drains_queue_then_rejects_admission() {
        let writer = StoreWriter::new();
        let session = crate::session::Session::new(false, false, false).unwrap();
        let (event_tx, event_rx) = channel();
        for _ in 0..4 {
            writer
                .try_send(unauthenticated_snapshot(&session, &event_tx))
                .unwrap();
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        assert!(
            writer.shutdown_until(deadline).await,
            "clean drain must report success"
        );
        let outcomes: Vec<_> = event_rx.try_iter().collect();
        assert_eq!(
            outcomes.len(),
            4,
            "every queued command gets an explicit outcome"
        );
        assert!(outcomes
            .iter()
            .all(|envelope| matches!(envelope.event, BgEvent::StoreFailed(_))));
        assert!(matches!(
            writer.try_send(unauthenticated_snapshot(&session, &event_tx)),
            Err(StoreAdmissionError::Disconnected)
        ));
    }

    #[tokio::test]
    async fn history_writer_shutdown_reports_failure_after_worker_panic() {
        let writer = StoreWriter::new();
        writer.try_send(StoreCommand::PanicForTest).unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        assert!(
            !writer.shutdown_until(deadline).await,
            "panicked worker is not a clean save"
        );
        assert!(
            tokio::time::Instant::now() < deadline,
            "failure is detected before the budget"
        );
    }

    #[tokio::test]
    async fn history_writer_shutdown_is_bounded_when_worker_is_stuck() {
        // Receiver kept alive but never read: simulates a blocked worker.
        let (writer, _rx) = StoreWriter::channel();
        let started = tokio::time::Instant::now();
        let deadline = started + Duration::from_millis(200);
        assert!(!writer.shutdown_until(deadline).await);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "exit must not wait unbounded"
        );
    }

    #[test]
    fn wraps_words_to_width() {
        assert_eq!(
            wrap_text("a b c", 3),
            vec!["a b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn breaks_long_words() {
        assert_eq!(
            wrap_text("abcdef", 4),
            vec!["abcd".to_string(), "ef".to_string()]
        );
    }

    #[test]
    fn message_day_skips_missing_ts() {
        assert_eq!(message_day(0), None);
        assert!(message_day(1_700_000_000_000).is_some());
    }

    #[test]
    fn hidden_membership_gates_tui_list_search_and_unlock_visibility() {
        let ordinary = Thread {
            id: "303".into(),
            name: "Ordinary contact".into(),
            preview: "ordinary preview".into(),
            ..Default::default()
        };
        let hidden = Thread {
            id: "101".into(),
            name: "Secret contact".into(),
            preview: "secret preview".into(),
            messages: vec![ChatMessage {
                text: "secret body".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let membership = crate::hidden_chat::Snapshot {
            ready: true,
            has_pin: true,
            pin_known: true,
            ids: HashSet::from(["u101".into()]),
            ..Default::default()
        };

        assert!(TuiApp::thread_matches_filter(
            &ordinary,
            &membership,
            true,
            FilterTab::All,
            "ordinary"
        ));
        assert!(!TuiApp::thread_matches_filter(
            &hidden,
            &membership,
            true,
            FilterTab::All,
            "secret"
        ));
        assert!(!TuiApp::thread_matches_filter(
            &ordinary,
            &crate::hidden_chat::Snapshot::default(),
            true,
            FilterTab::All,
            "ordinary"
        ));
        let unlocked = crate::hidden_chat::Snapshot {
            unlocked: true,
            ..membership
        };
        assert!(TuiApp::thread_matches_filter(
            &hidden,
            &unlocked,
            true,
            FilterTab::All,
            "secret body"
        ));
    }

    #[test]
    fn tui_acknowledged_delete_persists_tombstone_and_rejects_hydrate_and_replay() {
        let mut store = crate::store::MessageStore::open(":memory:").unwrap();
        let message = ChatMessage {
            id: "server-1".into(),
            client_message_id: "client-1".into(),
            text: "deleted content".into(),
            wire_ref: crate::message_model::WireMessageRef {
                sender: Some(serde_json::json!("sender-a")),
                ..Default::default()
            },
            ..Default::default()
        };
        let deleted_thread = Thread {
            id: "thread-a".into(),
            messages: vec![message.clone()],
            ..Default::default()
        };
        crate::store::persist_threads(&mut store, std::slice::from_ref(&deleted_thread)).unwrap();
        persist_acknowledged_delete(&mut store, "thread-a", &message).unwrap();

        let mut hydrated = vec![deleted_thread.clone()];
        store.hydrate_threads(&mut hydrated, 20).unwrap();
        assert!(hydrated[0].messages.is_empty());

        let mut aliased_replay = message.clone();
        aliased_replay.id = "phone-client:8:sender-a:client-1".into();
        let stale_snapshot = Thread {
            messages: vec![aliased_replay.clone()],
            ..deleted_thread.clone()
        };
        let replay = crate::store::persist_threads_counted(&mut store, &[stale_snapshot]).unwrap();
        assert_eq!(replay.deleted, 1);

        let mut sender_collision = aliased_replay.clone();
        sender_collision.wire_ref.sender = Some(serde_json::json!("sender-b"));
        let collision_thread = Thread {
            messages: vec![sender_collision],
            ..deleted_thread.clone()
        };
        let collision =
            crate::store::persist_threads_counted(&mut store, &[collision_thread]).unwrap();
        assert_eq!(collision.inserted, 1);

        let mut distinct_server_id = message.clone();
        distinct_server_id.id = "server-2".into();
        let distinct_thread = Thread {
            messages: vec![distinct_server_id.clone()],
            ..deleted_thread.clone()
        };
        let distinct =
            crate::store::persist_threads_counted(&mut store, &[distinct_thread]).unwrap();
        assert_eq!(distinct.inserted, 1);

        let other_thread = Thread {
            id: "thread-b".into(),
            messages: vec![distinct_server_id],
            ..Default::default()
        };
        let independent =
            crate::store::persist_threads_counted(&mut store, &[other_thread]).unwrap();
        assert_eq!(independent.inserted, 1);
    }

    #[test]
    fn tui_delete_alias_filter_keeps_distinct_ids_and_sender_collisions() {
        let deleted = DeletedIdentity::from_message(&ChatMessage {
            id: "server-1".into(),
            client_message_id: "client-1".into(),
            wire_ref: crate::message_model::WireMessageRef {
                sender: Some(serde_json::json!("sender-a")),
                ..Default::default()
            },
            ..Default::default()
        });
        let provisional_alias = ChatMessage {
            id: "phone-client:8:sender-a:client-1".into(),
            client_message_id: "client-1".into(),
            wire_ref: crate::message_model::WireMessageRef {
                sender: Some(serde_json::json!("sender-a")),
                ..Default::default()
            },
            ..Default::default()
        };
        let sender_collision = ChatMessage {
            wire_ref: crate::message_model::WireMessageRef {
                sender: Some(serde_json::json!("sender-b")),
                ..provisional_alias.wire_ref.clone()
            },
            ..provisional_alias.clone()
        };
        let direction_collision = ChatMessage {
            is_outgoing: true,
            ..provisional_alias.clone()
        };
        let distinct_server_id = ChatMessage {
            id: "server-2".into(),
            ..provisional_alias.clone()
        };

        assert!(deleted.matches(&provisional_alias));
        assert!(!deleted.matches(&sender_collision));
        assert!(!deleted.matches(&direction_collision));
        assert!(!deleted.matches(&distinct_server_id));
    }

    #[test]
    fn draws_thread_and_messages_without_panicking() {
        let mut session = crate::session::Session::new(false, false, false).unwrap();
        let (tx, rx) = channel();
        let thread = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "u1".into(),
            name: "Test".into(),
            preview: "xin chao".into(),
            time: "12:00".into(),
            avatar: None,
            is_group: true,
            messages: vec![
                ChatMessage {
                    id: "m1".into(),
                    sender: "An".into(),
                    text: "xin chao".into(),
                    time: "12:00".into(),
                    is_outgoing: false,
                    ts: 1_700_000_000_000,
                    ..Default::default()
                },
                ChatMessage {
                    id: "m2".into(),
                    sender: "Toi".into(),
                    text: "chao ban".into(),
                    time: "12:01".into(),
                    is_outgoing: true,
                    ts: 1_700_000_060_000,
                    reply_to_sender: Some("An".into()),
                    reply_to_text: Some("xin chao".into()),
                    reactions: vec!["/-strong".into()],
                    ..Default::default()
                },
            ],
        };
        let mut app = TuiApp {
            session: &mut session,
            threads: vec![thread],
            list_state: ListState::default(),
            selected_thread: 0,
            input_text: "hello".into(),
            focus: Focus::Messages,
            status: "test".into(),
            should_quit: false,
            poll_state: chat::PollState::default(),
            filtered: vec![0],
            filter: FilterTab::All,
            search: String::new(),
            search_mode: false,
            unread: HashMap::new(),
            deleted_messages: HashMap::new(),
            selected_msg: Some(1),
            msg_scroll: 0,
            follow_tail: true,
            msg_ranges: Vec::new(),
            chat_view_h: 10,
            threads_width: 0,
            reply_target: None,
            overlay: Overlay::None,
            log_scroll: 0,
            sticker: StickerState::default(),
            prompt: None,
            confirm: None,
            poll_inflight: false,
            delete_inflight: false,
            logout_inflight: false,
            critical_inflight: Arc::new(AtomicUsize::new(0)),
            bg_rx: rx,
            bg_tx: tx,
            writer: StoreWriter::new(),
            pending_snapshot: None,
        };
        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui_draw(f, &mut app)).unwrap();
        assert!(app.msg_ranges.len() >= 2);
    }

    #[test]
    fn test_tui_logout_inflight_discards_stale_events() {
        let mut session = crate::session::Session::new(false, false, false).unwrap();
        session.login = Some(crate::session::LoginInfo {
            uin: "test_owner_tui".into(),
            ..Default::default()
        });
        let (tx, rx) = std::sync::mpsc::channel();
        let mut app = TuiApp {
            session: &mut session,
            threads: Vec::new(),
            list_state: ListState::default(),
            selected_thread: 0,
            input_text: String::new(),
            focus: Focus::Threads,
            status: "Đang xóa phiên và dọn dẹp dữ liệu...".into(),
            should_quit: false,
            poll_state: chat::PollState::default(),
            filtered: Vec::new(),
            filter: FilterTab::All,
            search: String::new(),
            search_mode: false,
            unread: HashMap::new(),
            deleted_messages: HashMap::new(),
            selected_msg: None,
            msg_scroll: 0,
            follow_tail: true,
            msg_ranges: Vec::new(),
            chat_view_h: 10,
            threads_width: 0,
            reply_target: None,
            overlay: Overlay::None,
            log_scroll: 0,
            sticker: StickerState::default(),
            prompt: None,
            confirm: None,
            poll_inflight: false,
            delete_inflight: false,
            logout_inflight: true,
            critical_inflight: Arc::new(AtomicUsize::new(0)),
            bg_rx: rx,
            bg_tx: tx.clone(),
            writer: StoreWriter::new(),
            pending_snapshot: None,
        };

        // 1. Send stale PollDone while logout_inflight == true
        let stale_thread = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "stale_thread".into(),
            name: "Stale Thread".into(),
            preview: "stale".into(),
            time: "12:00".into(),
            avatar: None,
            is_group: false,
            messages: vec![ChatMessage {
                id: "stale_m1".into(),
                sender: "Other".into(),
                text: "Should be dropped".into(),
                ..Default::default()
            }],
        };
        let _ = tx.send(BgEnvelope {
            stamp: RuntimeStamp::for_session(app.session),
            event: BgEvent::PollDone {
                threads: vec![stale_thread],
                state: chat::PollState::default(),
            },
        });

        // Check events: stale PollDone must be completely discarded
        app.check_background_events();
        assert!(
            app.threads.is_empty(),
            "Stale thread must NOT be added to threads while logout is in flight"
        );
        assert_eq!(
            app.status, "Đang xóa phiên và dọn dẹp dữ liệu...",
            "Status must NOT be overwritten by stale poll"
        );
        assert!(
            app.logout_inflight,
            "logout_inflight must remain true until LogoutDone"
        );

        // 2. Now send LogoutDone
        let _ = tx.send(BgEnvelope {
            stamp: RuntimeStamp::for_session(app.session),
            event: BgEvent::LogoutDone {
                owner: "test_owner_tui".into(),
                result: Ok(()),
            },
        });

        app.check_background_events();
        assert!(
            !app.logout_inflight,
            "logout_inflight must be cleared upon LogoutDone"
        );
        assert!(
            app.session.login.is_none(),
            "Session login credentials must be cleared"
        );
        assert_eq!(app.session.account_key(), "");
        assert_eq!(app.status, "Đã đăng xuất. Bấm 'q' để thoát.");
    }
}
