//! Explicit server PIN operations; the local database and downloaded files stay unencrypted.
use super::*;
#[derive(Default)]
pub(super) struct HiddenPanel {
    pub open: bool,
    pub pending: bool,
    pub locking: bool,
    pub show_in_sidebar: bool,
    pub generation: u64,
    pub view: crate::hidden_chat::Snapshot,
    pub pin: zeroize::Zeroizing<String>,
    pub new_pin: zeroize::Zeroizing<String>,
    pub status: String,
    pub target: Option<(String, bool, bool)>,
}
impl ZaloGuiApp {
    pub(super) fn hidden_command(&mut self, action: crate::hidden_chat::Action) {
        if self.hidden_panel.pending {
            return;
        }
        self.hidden_panel.pending = true;
        self.hidden_panel.generation = crate::search::generation();
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Hidden {
                owner: self.user_uin.clone(),
                generation: self.hidden_panel.generation,
                action,
            })
            .is_ok()
        }) {
            self.hidden_panel.pending = false;
            self.hidden_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn apply_hidden_view(&mut self, mut view: crate::hidden_chat::Snapshot) {
        if self.hidden_panel.locking {
            view.unlocked = false;
        }
        let previous = &self.hidden_panel.view;
        let restrictive = !view.unlocked
            && ((!view.ready && previous.ready)
                || previous.unlocked
                || (view.ids != previous.ids && !view.ids.is_empty()));
        self.hidden_panel.view = view;
        // Invalidate late search responses, close decrypted previews, and drop GPU copies on relock.
        if restrictive {
            self.hidden_panel.show_in_sidebar = false;
            self.clear_downloads();
            self.quick_schedule = None;
            self.server_board.open = false;
            self.group_content.open = false;
            self.groups_panel.open = false;
            self.contact_panel.open = false;
            self.labels_panel.open = false;
            crate::desktop_notice::clear_previews(&self.user_uin);
            self.invalidate_history_search();
            self.history_search.open = false;
            self.tools.hits.clear();
            self.image_viewer = None;
            self.forward_msg = None;
            self.replying_to = None;
            self.sticker_textures.clear();
            self.loading_stickers.clear();
            self.image_retry_at.clear();
            self.image_failures.clear();
            self.texture_budget =
                crate::media_resources::ResourceLru::new(crate::media_resources::TEXTURE_BUDGET);
        }
    }
    pub(super) fn render_hidden_panel(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in || !self.hidden_panel.open {
            return;
        }
        let mut open = true;
        let mut action = None;
        let mut open_target = None;
        egui::Window::new("Hội thoại ẩn và PIN").id(Id::new("hidden-pin")).open(&mut open).vscroll(true).min_width(120.0).max_width((ctx.screen_rect().width()-32.0).max(180.0)).max_height((ctx.screen_rect().height()-64.0).max(100.0)).show(ctx,|ui| {
            ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
            ui.label("PIN khóa việc hiển thị trong ứng dụng. CSDL và file tải xuống trên đĩa chưa được mã hóa; không dùng PIN này thay khóa Windows.");
            ui.label(&self.hidden_panel.status);if self.hidden_panel.pending {ui.spinner();}
            ui.label(if !self.hidden_panel.view.ready {"Chưa xác minh danh sách ẩn; đang chặn mở/tìm kiếm để bảo vệ dữ liệu"}else if self.hidden_panel.view.unlocked {"Đã mở khóa"}else{"Đang khóa"});
            if self.hidden_panel.view.ready && !self.hidden_panel.view.pin_known {ui.label("Máy chủ chưa cung cấp thông tin PIN: vẫn mở được chat thường; chat ẩn tiếp tục khóa.");}
            ui.add_enabled_ui(!self.hidden_panel.pending,|ui| {
                if ui.button("Tải lại trạng thái ẩn").clicked(){action=Some(crate::hidden_chat::Action::Read);}
                ui.add(egui::TextEdit::singleline(&mut *self.hidden_panel.pin).password(true).char_limit(4).hint_text("PIN hiện tại (4 số)").desired_width(ui.available_width()));
                if self.hidden_panel.view.has_pin && ui.button("Mở khóa hội thoại ẩn").clicked(){action=Some(crate::hidden_chat::Action::Unlock(std::mem::take(&mut self.hidden_panel.pin)));}
                if self.hidden_panel.view.unlocked && ui.button("Khóa lại ngay").clicked(){action=Some(crate::hidden_chat::Action::Lock);}
                ui.collapsing("Đặt hoặc đổi PIN trên máy chủ",|ui| {
                    ui.add(egui::TextEdit::singleline(&mut *self.hidden_panel.new_pin).password(true).char_limit(4).hint_text("PIN mới (4 số)").desired_width(ui.available_width()));
                    ui.label("Thay đổi này áp dụng cho tài khoản trên các thiết bị khác.");
                    if ui.add_enabled(self.hidden_panel.view.pin_known,egui::Button::new("Xác nhận đặt/đổi PIN")).clicked(){action=Some(crate::hidden_chat::Action::SetPin {old:std::mem::take(&mut self.hidden_panel.pin),new:std::mem::take(&mut self.hidden_panel.new_pin)});}
                });
                if let Some((id,group,hidden))=self.hidden_panel.target.clone() {
                    ui.label(format!("{} hội thoại {} trên máy chủ?",if hidden {"Ẩn"}else{"Bỏ ẩn"},id));
                    if ui.add_enabled(self.hidden_panel.view.unlocked,egui::Button::new("Xác nhận thay đổi ẩn")).clicked(){action=Some(crate::hidden_chat::Action::SetHidden {target:id,group,hidden});self.hidden_panel.target=None;}
                    if ui.button("Hủy thay đổi ẩn").clicked(){self.hidden_panel.target=None;}
                }
                if self.hidden_panel.view.unlocked {
                    ui.label(format!("{} hội thoại ẩn",self.hidden_panel.view.ids.len()));
                    ui.checkbox(&mut self.hidden_panel.show_in_sidebar,"Hiện hội thoại ẩn trong danh sách khi đang mở khóa");
                    for id in &self.hidden_panel.view.ids {
                        ui.horizontal_wrapped(|ui| {let name=self.threads.iter().find(|thread|crate::server_pins::identity(&thread.id,thread.is_group).ok().as_ref()==Some(id)).map(|thread|thread.name.as_str()).unwrap_or(id);ui.label(name);
                            if ui.button("Mở hội thoại").clicked(){open_target=Some(id.clone());}
                            if ui.button("Bỏ ẩn…").clicked(){self.hidden_panel.target=Some((id.clone(),id.starts_with('g'),false));}
                        });
                    }
                }
            });
        });
        self.hidden_panel.open = open;
        if !open {
            use zeroize::Zeroize;
            self.hidden_panel.pin.zeroize();
            self.hidden_panel.new_pin.zeroize();
        }
        if let Some(id) = open_target {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::Tools {
                    owner: self.user_uin.clone(),
                    command: tools_panel::ToolsCommand::OpenThread {
                        thread: id.clone(),
                        group: id.starts_with('g'),
                    },
                });
            }
        }
        if let Some(action) = action {
            if matches!(action, crate::hidden_chat::Action::Lock) {
                self.hidden_panel.locking = true;
                let mut view = self.hidden_panel.view.clone();
                view.unlocked = false;
                self.apply_hidden_view(view);
            }
            self.hidden_command(action);
        }
    }
}
