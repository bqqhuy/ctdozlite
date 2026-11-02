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
pub(super) fn start<F: FnMut(String) + Send + 'static>(
    session: Session,
    batches: Vec<Batch>,
    cancel: Arc<AtomicBool>,
    progress: Arc<std::sync::Mutex<F>>,
    round: usize,
) -> tokio::sync::mpsc::Receiver<Result<Staged>> {
    let (tx, rx) = tokio::sync::mpsc::channel(2);
    tokio::spawn(async move {
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
        let mut jobs = stream::iter(batches.into_iter().map(|batch| {
            let directory = directory.clone();
            let session = session.clone();
            let cancel = cancel.clone();
            let progress = progress.clone();
            async move {
                ensure!(
                    session.is_active() && !cancel.load(Ordering::Acquire),
                    "Đã dừng tải lịch sử"
                );
                let staged = Staged {
                    batch,
                    path: directory.join(format!("{}.part", crate::chat::next_client_message_id())),
                };
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
                Ok(staged)
            }
        }))
        .buffered(2);
        while let Some(result) = jobs.next().await {
            let failed = result.is_err();
            if tx.send(result).await.is_err() || failed {
                break;
            }
        }
    });
    rx
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
