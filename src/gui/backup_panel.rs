//! Explicit backup dry-run and import, with keys kept in memory only.
use super::*;
#[derive(Default)]
pub(super) struct BackupPanel {
    pub open: bool,
    pub generation: u64,
    pub pending: bool,
    pub path: String,
    pub key: zeroize::Zeroizing<String>,
    pub report: Option<crate::desktop_backup::Report>,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub status: String,
}
impl ZaloGuiApp {
    pub(super) fn render_backup_panel(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.backup_panel
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
            self.backup_panel = Default::default();
            return;
        }
        if !self.backup_panel.open {
            return;
        }
        let mut open = true;
        let mut command = None;
        egui::Window::new("Nhập backup Zalo Desktop V2").id(Id::new("desktop-backup")).open(&mut open).default_width(520.0).show(ctx,|ui|{
            ui.label("Backup .zl.zip V2 được kiểm tra trước khi nhập; khác với đồng bộ V1 từ điện thoại.");
            ui.add_enabled_ui(!self.backup_panel.pending,|ui|{
                ui.horizontal(|ui|{
                    if ui.text_edit_singleline(&mut self.backup_panel.path).changed(){self.backup_panel.report=None;}
                    if ui.button("Chọn tệp…").clicked(){
                        self.backup_panel.generation=crate::search::generation();
                        command=Some(WorkerCmd::PickDesktopBackup{owner:self.user_uin.clone(),generation:self.backup_panel.generation});
                    }
                });
                ui.label("Khóa UIN của backup (32 ký tự, không phải UID số của tài khoản)");
                if ui.add(egui::TextEdit::singleline(&mut *self.backup_panel.key).password(true)).changed(){self.backup_panel.report=None;}
                ui.small("Khóa chỉ giữ trong bộ nhớ, không ghi vào cấu hình. Bản này không tự khởi chạy hoặc điều khiển Zalo gốc để lấy khóa.");
                if ui.button("Kiểm tra backup").clicked(){self.backup_panel.report=None;command=Some(self.backup_command(false));}
                if let Some(report)=&self.backup_panel.report{
                    ui.label(format!("{} hội thoại · {} tin · {} tệp · {} bản ghi chưa hỗ trợ",report.threads,report.records,report.media,report.unsupported));
                    ui.small("Nhập theo lô ngắn để không khóa ghi tin trực tiếp lâu. Dừng/lỗi giữ các lô đã nhập, có thể chạy lại để khử trùng.");
                    if ui.button("Nhập bản đã kiểm tra").clicked(){command=Some(self.backup_command(true));}
                }
            });
            if self.backup_panel.pending{ui.spinner();if ui.button("Dừng nhập").clicked(){self.backup_panel.cancel.store(true,std::sync::atomic::Ordering::Release);}}
            ui.label(&self.backup_panel.status);
        });
        if let Some(command) = command {
            self.backup_panel.pending = true;
            if !WORKER_TX.get().is_some_and(|tx| tx.send(command).is_ok()) {
                self.backup_panel.pending = false;
                self.backup_panel.status = "Worker chưa sẵn sàng".into();
            }
        }
        self.backup_panel.open = open;
        if !open {
            self.backup_panel
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
            self.backup_panel.key.clear();
        }
    }
    fn backup_command(&mut self, import: bool) -> WorkerCmd {
        self.backup_panel.cancel = Default::default();
        self.backup_panel.generation = crate::search::generation();
        WorkerCmd::DesktopBackup {
            owner: self.user_uin.clone(),
            generation: self.backup_panel.generation,
            path: self.backup_panel.path.clone().into(),
            key: self.backup_panel.key.clone(),
            cancel: self.backup_panel.cancel.clone(),
            expected: if import {
                self.backup_panel.report.clone()
            } else {
                None
            },
        }
    }
}
