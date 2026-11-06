//! Explicit own-profile edits with a review step and account-scoped results.
use super::*;
#[derive(Default)]
pub(super) struct ProfilePanel {
    pub open: bool,
    pub pending: bool,
    pub uncertain: bool,
    pub generation: u64,
    pub loaded: bool,
    pub name: String,
    pub gender: Option<i64>,
    pub dob: String,
    pub status: String,
    pub confirm: bool,
}
impl ZaloGuiApp {
    pub(super) fn open_own_profile(&mut self) {
        if self.profile_panel.pending
            || (self.avatar_panel.pending
                && matches!(self.avatar_panel.target, Some(crate::avatar::Target::Own)))
        {
            self.profile_panel.open = true;
            self.show_toast("Chờ thao tác hồ sơ hoặc ảnh đại diện hiện tại hoàn tất");
            return;
        }
        self.profile_panel = ProfilePanel {
            open: true,
            ..Default::default()
        };
        self.profile_command(crate::profile::Action::Read);
    }
    fn profile_command(&mut self, action: crate::profile::Action) {
        if self.avatar_panel.pending
            && matches!(self.avatar_panel.target, Some(crate::avatar::Target::Own))
        {
            self.profile_panel.status =
                "Đang xử lý ảnh đại diện; chờ kết quả rồi tải lại hồ sơ".into();
            return;
        }
        if self.profile_panel.pending {
            return;
        }
        self.profile_panel.pending = true;
        self.profile_panel.generation = crate::search::generation();
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Profile {
                owner: self.user_uin.clone(),
                generation: self.profile_panel.generation,
                action,
            })
            .is_ok()
        }) {
            self.profile_panel.pending = false;
            self.profile_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn render_own_profile(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.profile_panel = Default::default();
            return;
        }
        if !self.profile_panel.open {
            return;
        }
        let mut open = true;
        let mut intent = None;
        egui::Window::new("Hồ sơ của tôi")
            .id(Id::new("own-profile"))
            .open(&mut open)
            .vscroll(true)
            .min_width(120.0)
            .max_width((ctx.screen_rect().width() - 32.0).max(180.0))
            .max_height((ctx.screen_rect().height() - 64.0).max(100.0))
            .default_width(400.0_f32.min((ctx.screen_rect().width() - 48.0).max(160.0)))
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                if ui.button("Thay ảnh đại diện…").clicked() {
                    self.open_avatar(crate::avatar::Target::Own);
                }
                ui.label(&self.profile_panel.status);
                if self.profile_panel.pending {
                    ui.spinner();
                }
                if ui
                    .add_enabled(
                        !self.profile_panel.pending,
                        egui::Button::new("Tải lại hồ sơ"),
                    )
                    .clicked()
                {
                    intent = Some(crate::profile::Action::Read);
                }
                ui.add_enabled_ui(
                    self.profile_panel.loaded
                        && !self.profile_panel.pending
                        && !self.profile_panel.uncertain,
                    |ui| {
                        ui.label("Tên hiển thị");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.profile_panel.name)
                                .desired_width(ui.available_width()),
                        );
                        ui.label("Giới tính");
                        egui::ComboBox::from_id_salt("own-gender")
                            .selected_text(match self.profile_panel.gender {
                                Some(0) => "Nam",
                                Some(1) => "Nữ",
                                _ => "Chưa xác định",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.profile_panel.gender, Some(0), "Nam");
                                ui.selectable_value(&mut self.profile_panel.gender, Some(1), "Nữ");
                            });
                        ui.label("Ngày sinh (YYYY-MM-DD)");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.profile_panel.dob)
                                .desired_width(ui.available_width()),
                        );
                        if ui.button("Xem lại thay đổi").clicked() {
                            self.profile_panel.confirm = true;
                        }
                    },
                );
                if self.profile_panel.confirm {
                    ui.separator();
                    ui.label(format!(
                        "Cập nhật tài khoản: {} • {} • {}?",
                        self.profile_panel.name,
                        match self.profile_panel.gender {
                            Some(0) => "Nam",
                            Some(1) => "Nữ",
                            _ => "Chưa chọn giới tính",
                        },
                        self.profile_panel.dob
                    ));
                    if ui
                        .add_enabled(
                            !self.profile_panel.pending && self.profile_panel.gender.is_some(),
                            egui::Button::new("Xác nhận lưu hồ sơ"),
                        )
                        .clicked()
                    {
                        intent = Some(crate::profile::Action::Save {
                            name: self.profile_panel.name.clone(),
                            gender: self.profile_panel.gender.unwrap_or(-1),
                            dob: self.profile_panel.dob.clone(),
                        });
                        self.profile_panel.confirm = false;
                    }
                    if ui.button("Hủy").clicked() {
                        self.profile_panel.confirm = false;
                    }
                }
                if self.profile_panel.uncertain {
                    ui.label(
                        "Chưa rõ kết quả thay đổi. Tải lại hồ sơ để kiểm tra trước khi gửi tiếp.",
                    );
                }
            });
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            open = false;
        }
        self.profile_panel.open = open;
        if let Some(action) = intent {
            self.profile_command(action);
        }
    }
}
