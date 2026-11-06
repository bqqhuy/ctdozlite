//! Server privacy controls remain unknown until read for the active account.
use super::*;
#[derive(Default)]
pub(super) struct PrivacyPanel {
    pub snapshot: Option<crate::privacy::Snapshot>,
    pub pending: bool,
    pub uncertain: bool,
    pub generation: u64,
    pub status: String,
    pub confirm: Option<(crate::privacy::Setting, i64)>,
}
impl ZaloGuiApp {
    fn privacy_command(&mut self, action: crate::privacy::Action) {
        if self.privacy_panel.pending {
            return;
        }
        self.privacy_panel.pending = true;
        self.privacy_panel.generation = crate::search::generation();
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Privacy {
                owner: self.user_uin.clone(),
                generation: self.privacy_panel.generation,
                action,
            })
            .is_ok()
        }) {
            self.privacy_panel.pending = false;
            self.privacy_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn render_server_privacy(&mut self, ui: &mut egui::Ui) {
        use crate::privacy::{Action, Setting};
        ui.heading("Quyền riêng tư trên máy chủ");
        ui.label("Đây là thiết lập tài khoản, áp dụng cả trên các thiết bị khác. Trường chưa được máy chủ cung cấp sẽ không hiện để sửa.");
        let mut intent = None;
        if ui.button("Danh sách chặn tin nhắn…").clicked() {
            self.open_blocklist();
        }
        if ui
            .add_enabled(
                !self.privacy_panel.pending,
                egui::Button::new("Tải lại quyền riêng tư"),
            )
            .clicked()
        {
            intent = Some(Action::Read);
        }
        if self.privacy_panel.pending {
            ui.spinner();
        }
        ui.label(&self.privacy_panel.status);
        if let Some(snapshot) = &self.privacy_panel.snapshot {
            ui.add_enabled_ui(
                !self.privacy_panel.pending && !self.privacy_panel.uncertain,
                |ui| {
                    for (setting, label) in [
                        (Setting::StrangerDirect, "Hiện tin người lạ trực tiếp"),
                        (Setting::ShowOnline, "Hiện trạng thái truy cập"),
                        (Setting::DisplaySeen, "Hiện trạng thái đã xem"),
                    ] {
                        if let Some(value) = snapshot.values.get(&setting) {
                            let mut enabled = *value == 1;
                            if ui.checkbox(&mut enabled, label).changed() {
                                self.privacy_panel.confirm = Some((setting, i64::from(enabled)));
                            }
                        }
                    }
                    for (setting, label, options) in [
                        (
                            Setting::ReceiveMessages,
                            "Cho phép nhắn tin",
                            vec![(1, "Mọi người"), (2, "Bạn bè")],
                        ),
                        (
                            Setting::ReceiveCalls,
                            "Cho phép cuộc gọi",
                            vec![
                                (2, "Bạn bè"),
                                (4, "Bạn bè và người lạ từng liên hệ"),
                                (3, "Mọi người"),
                            ],
                        ),
                    ] {
                        if let Some(value) = snapshot.values.get(&setting) {
                            let mut selected = *value;
                            egui::ComboBox::from_id_salt(("privacy-setting", setting.key()))
                                .selected_text(
                                    options
                                        .iter()
                                        .find(|(value, _)| *value == selected)
                                        .map(|(_, label)| *label)
                                        .unwrap_or("Chưa xác định"),
                                )
                                .show_ui(ui, |ui| {
                                    for (value, label) in &options {
                                        ui.selectable_value(&mut selected, *value, *label);
                                    }
                                });
                            ui.label(label);
                            if selected != *value {
                                self.privacy_panel.confirm = Some((setting, selected));
                            }
                        }
                    }
                },
            );
        }
        if let Some((setting, value)) = self.privacy_panel.confirm {
            ui.separator();
            ui.label("Áp dụng thay đổi quyền riêng tư này cho tài khoản trên máy chủ?");
            if ui
                .add_enabled(
                    !self.privacy_panel.pending,
                    egui::Button::new("Xác nhận thay đổi"),
                )
                .clicked()
            {
                intent = Some(Action::Set { setting, value });
                self.privacy_panel.confirm = None;
            }
            if ui.button("Hủy").clicked() {
                self.privacy_panel.confirm = None;
            }
        }
        if self.privacy_panel.uncertain {
            ui.label("Chưa rõ kết quả thay đổi. Tải lại quyền riêng tư trước khi sửa tiếp.");
        }
        if let Some(action) = intent {
            self.privacy_command(action);
        }
    }
}
