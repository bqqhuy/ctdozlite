//! A bounded downloader runs while the single ordered decoder/importer is busy.
use super::{assets, transfer::Batch};
use crate::session::Session;
use anyhow::{ensure, Result};
use futures_util::{stream, StreamExt};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
pub(super) struct Staged {
    pub batch: Batch,
    pub path: PathBuf,
}
impl Drop for Staged {
    fn drop(&mut self) {
        // Best-effort cleanup owns only this transfer's file.
        let _ = std::fs::remove_file(&self.path);
    }
}
struct Feed {
    receiver: tokio::sync::mpsc::Receiver<Batch>,
    ready: tokio::sync::watch::Receiver<bool>,
    pending: std::collections::VecDeque<Batch>,
    closed: bool,
    metadata_seen: bool,
}
fn batch_feed(
    receiver: tokio::sync::mpsc::Receiver<Batch>,
    ready: tokio::sync::watch::Receiver<bool>,
) -> impl futures_util::Stream<Item = Result<Batch>> {
    stream::unfold(
        Feed {
            receiver,
            ready,
            pending: Default::default(),
            closed: false,
            metadata_seen: false,
        },
        |mut state| async move {
            loop {
                let ready = *state.ready.borrow();
                if let Some(index) = state.pending.iter().position(|b| b.metadata || ready) {
                    return state.pending.remove(index).map(|batch| (Ok(batch), state));
                }
                if state.closed && state.pending.is_empty() {
                    return None;
                }
                if state.closed && !state.metadata_seen {
                    state.pending.clear();
                    return Some((
                        Err(anyhow::anyhow!("Không nhận được metadata giải mã")),
                        state,
                    ));
                }
                tokio::select! {
                    next=state.receiver.recv(),if !state.closed=>match next {
                        Some(batch)=>{state.metadata_seen|=batch.metadata; if state.pending.len()>=2048 {state.pending.clear();state.closed=true;return Some((Err(anyhow::anyhow!("Hàng đợi descriptor vượt giới hạn")),state));}state.pending.push_back(batch);},
                        None=>state.closed=true,
                    },
                    changed=state.ready.changed(),if !ready=>{if changed.is_err(){return None;}},
                }
            }
        },
    )
}
pub(super) fn start<F: FnMut(String) + Send + 'static>(
    session: Session,
    batches: tokio::sync::mpsc::Receiver<Batch>,
    cancel: Arc<AtomicBool>,
    progress: Arc<std::sync::Mutex<F>>,
    round: usize,
    ready: tokio::sync::watch::Receiver<bool>,
    sync_id: String,
) -> (
    tokio::sync::mpsc::Receiver<Result<Staged>>,
    tokio::task::JoinHandle<()>,
) {
    let (tx, rx) = tokio::sync::mpsc::channel(2);
    let task = tokio::spawn(async move {
        let directory = session.history_path().with_extension("phone-staging");
        let create = directory.clone();
        let prepared = tokio::task::spawn_blocking(move || -> Result<()> {
            std::fs::create_dir_all(&create)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&create, std::fs::Permissions::from_mode(0o700))?;
            }
            Ok(())
        })
        .await;
        if !matches!(prepared, Ok(Ok(()))) {
            let _ = tx
                .send(Err(anyhow::anyhow!("Không tạo được vùng tạm đồng bộ")))
                .await;
            return;
        }
        let feed = batch_feed(batches, ready);
        let jobs = feed
            .map(|batch| {
                let directory = directory.clone();
                let session = session.clone();
                let cancel = cancel.clone();
                let progress = progress.clone();
                let sync_id = sync_id.clone();
                async move {
                    let batch = batch?;
                    ensure!(
                        session.is_active() && !cancel.load(Ordering::Acquire),
                        "Đã dừng tải lịch sử"
                    );
                    let staged = Staged {
                        batch,
                        path: directory
                            .join(format!("{}.part", crate::chat::next_client_message_id())),
                    };
                    let mut trace = crate::transfer_diagnostics::BatchWork::start(
                        &sync_id,
                        round,
                        staged.batch.index,
                        crate::transfer_diagnostics::BatchStage::Download,
                    );
                    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
                    assets::download_to(
                        &staged.batch.url,
                        &staged.path,
                        &session,
                        &cancel,
                        |bytes, total| {
                            if last.elapsed() >= std::time::Duration::from_millis(200) {
                                if let Ok(mut p) = progress.lock() {
                                    p(format!(
                                        "Tải về: lượt {round} • gói {} • {} / {} KiB",
                                        staged.batch.index,
                                        bytes / 1024,
                                        total
                                            .map(|n| (n / 1024).to_string())
                                            .unwrap_or_else(|| "?".into())
                                    ));
                                }
                                last = std::time::Instant::now();
                            }
                        },
                    )
                    .await?;
                    trace.finish(0);
                    Ok(staged)
                }
            })
            .buffer_unordered(2);
        let mut jobs = std::pin::pin!(jobs);
        while let Some(result) = tokio::select! {result=jobs.next()=>result,_=tx.closed()=>None} {
            let failed = result.is_err();
            if tx.send(result).await.is_err() || failed {
                break;
            }
        }
    });
    (rx, task)
}

struct ImportWork {
    object: serde_json::Value,
    senders: Arc<std::collections::HashMap<String, String>>,
    done: tokio::sync::oneshot::Sender<Result<super::import::Imported>>,
}
pub(super) struct Writer(tokio::sync::mpsc::Sender<ImportWork>);
impl Writer {
    pub fn start(
        session: Session,
        threads: Arc<std::collections::HashMap<String, crate::chat::Thread>>,
    ) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ImportWork>(2);
        tokio::task::spawn_blocking(move || {
            let mut store = match crate::store::MessageStore::open(session.history_path()) {
                Ok(s) => s,
                Err(_) => {
                    if let Some(work) = rx.blocking_recv() {
                        let _ = work
                            .done
                            .send(Err(anyhow::anyhow!("Không mở được CSDL lịch sử")));
                    }
                    return;
                }
            };
            while let Some(work) = rx.blocking_recv() {
                let result = if session.is_active() {
                    super::import::save(
                        &mut store,
                        &work.object,
                        &threads,
                        &work.senders,
                        session.account_key(),
                    )
                } else {
                    Err(anyhow::anyhow!("Đã đăng xuất; dừng nhập lịch sử"))
                };
                let _ = work.done.send(result);
            }
        });
        Self(tx)
    }
    pub async fn save(
        &self,
        object: serde_json::Value,
        senders: Arc<std::collections::HashMap<String, String>>,
    ) -> Result<super::import::Imported> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.0
            .send(ImportWork {
                object,
                senders,
                done: tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("Hàng đợi nhập lịch sử đã đóng"))?;
        rx.await
            .map_err(|_| anyhow::anyhow!("Không hoàn tất ghi lịch sử"))?
    }
}

#[cfg(test)]
mod dependency_tests {
    use super::*;
    fn batch(index: u64, metadata: bool) -> Batch {
        Batch {
            metadata,
            index,
            url: "https://example.invalid/file".into(),
        }
    }
    #[tokio::test]
    async fn late_metadata_bypasses_messages_and_decoding_opens_message_queue() {
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let (ready, state) = tokio::sync::watch::channel(false);
        for i in 0..20 {
            tx.send(batch(i, false)).await.unwrap();
        }
        tx.send(batch(20, true)).await.unwrap();
        drop(tx);
        let feed = batch_feed(rx, state);
        let mut feed = std::pin::pin!(feed);
        assert!(feed.next().await.unwrap().unwrap().metadata);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(5), feed.next())
                .await
                .is_err()
        );
        ready.send(true).unwrap();
        let mut n = 0;
        while let Some(batch) = feed.next().await {
            assert!(!batch.unwrap().metadata);
            n += 1;
        }
        assert_eq!(n, 20);
    }
    #[tokio::test]
    async fn message_only_manifest_reports_missing_metadata_instead_of_success() {
        let (tx, rx) = tokio::sync::mpsc::channel(2);
        let (_ready, state) = tokio::sync::watch::channel(false);
        tx.send(batch(0, false)).await.unwrap();
        drop(tx);
        let feed = batch_feed(rx, state);
        let mut feed = std::pin::pin!(feed);
        assert!(feed.next().await.unwrap().is_err());
    }
}
