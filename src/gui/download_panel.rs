use super::*;
pub(super) struct DownloadView {
    pub name: String,
    pub bytes: u64,
    pub total: Option<u64>,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub result: Option<Result<std::path::PathBuf, String>>,
}
impl ZaloGuiApp {
    pub(super) fn render_downloads(&mut self, ctx: &egui::Context) {
        if self.downloads.is_empty() {
            return;
        }
        egui::Window::new("Tải tệp")
            .id(Id::new("media_downloads"))
            .default_width(340.0)
            .resizable(true)
            .show(ctx, |ui| {
                let mut remove = Vec::new();
                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .show(ui, |ui| {
                        for (id, d) in &self.downloads {
                            ui.label(&d.name);
                            match &d.result {
                                None => {
                                    if let Some(total) = d.total.filter(|n| *n > 0) {
                                        ui.add(
                                            egui::ProgressBar::new(
                                                (d.bytes as f64 / total as f64).min(1.0) as f32,
                                            )
                                            .text(
                                                format!(
                                                    "{} / {} KiB",
                                                    d.bytes / 1024,
                                                    total / 1024
                                                ),
                                            ),
                                        );
                                    } else {
                                        ui.label(format!("Đã tải {} KiB", d.bytes / 1024));
                                    }
                                    if ui.button("Hủy tải").clicked() {
                                        d.cancel.store(true, std::sync::atomic::Ordering::Release);
                                    }
                                }
                                Some(Ok(path)) => {
                                    ui.horizontal(|ui| {
                                        if ui.button("Mở tệp").clicked() {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::OpenDownloaded {
                                                    path: path.clone(),
                                                    folder: false,
                                                });
                                            }
                                        }
                                        if ui.button("Mở thư mục").clicked() {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::OpenDownloaded {
                                                    path: path.clone(),
                                                    folder: true,
                                                });
                                            }
                                        }
                                        if ui.small_button("×").clicked() {
                                            remove.push(id.clone());
                                        }
                                    });
                                }
                                Some(Err(error)) => {
                                    ui.label(error);
                                    if ui.small_button("Đóng").clicked() {
                                        remove.push(id.clone());
                                    }
                                }
                            }
                            ui.separator();
                        }
                    });
                for id in remove {
                    self.downloads.remove(&id);
                }
            });
    }
    pub(super) fn clear_downloads(&mut self) {
        for d in self.downloads.values() {
            d.cancel.store(true, std::sync::atomic::Ordering::Release);
        }
        self.downloads.clear();
    }
}
