//! Versioned label editing and per-conversation assignment; no changes before server acknowledgement.
use super::*;
#[derive(Default)]
pub(super) struct LabelsPanel {
    pub open: bool,
    pub target: Option<(String, bool)>,
    pub snapshot: Option<crate::labels::Snapshot>,
    pub pending: bool,
    pub uncertain: bool,
    pub generation: u64,
    pub name: String,
    pub status: String,
    pub confirm: Option<crate::labels::Action>,
    pub filter: Option<i64>,
}
impl ZaloGuiApp {
    pub(super) fn open_labels(&mut self, target: Option<(String, bool)>) {
        self.labels_panel.open = true;
        self.labels_panel.target = target;
        self.labels_panel.confirm = None;
        if !self.labels_panel.pending {
            self.labels_command(crate::labels::Action::Read);
        }
    }
    fn labels_command(&mut self, action: crate::labels::Action) {
        if self.labels_panel.pending {
            return;
        }
        self.labels_panel.generation = crate::search::generation();
        self.labels_panel.pending = true;
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Labels {
                owner: self.user_uin.clone(),
                generation: self.labels_panel.generation,
                action,
            })
            .is_ok()
        }) {
            self.labels_panel.pending = false;
            self.labels_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn render_labels_panel(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.labels_panel = Default::default();
            return;
        }
        if !self.labels_panel.open {
            return;
        }
        let mut open = true;
        let mut intent = None;
        egui::Window::new("Phân loại hội thoại")
            .id(Id::new("server-labels"))
            .open(&mut open)
            .vscroll(true)
            .min_width(120.0)
            .max_width((ctx.screen_rect().width() - 32.0).max(180.0))
            .max_height((ctx.screen_rect().height() - 64.0).max(100.0))
            .default_width(480.0_f32.min((ctx.screen_rect().width() - 48.0).max(160.0)))
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                if ui
                    .add_enabled(
                        !self.labels_panel.pending,
                        egui::Button::new("Tải lại phân loại"),
                    )
                    .clicked()
                {
                    intent = Some(crate::labels::Action::Read);
                }
                ui.label(&self.labels_panel.status);
                if self.labels_panel.pending {
                    ui.spinner();
                }
                if let Some((id, group)) = &self.labels_panel.target {
                    ui.label(format!(
                        "Hội thoại: {} {id}",
                        if *group { "nhóm" } else { "cá nhân" }
                    ));
                }
                ui.add_enabled_ui(
                    !self.labels_panel.pending && !self.labels_panel.uncertain,
                    |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.labels_panel.name)
                                .desired_width(ui.available_width()),
                        )
                        .on_hover_text("Tên cho phân loại mới hoặc để đổi tên phân loại đã chọn");
                        if self.labels_panel.snapshot.is_some()
                            && ui.button("Tạo phân loại mới").clicked()
                        {
                            self.labels_panel.confirm = Some(crate::labels::Action::Create {
                                name: self.labels_panel.name.clone(),
                            });
                        }
                        if let Some(snapshot) = &self.labels_panel.snapshot {
                            egui::ScrollArea::vertical()
                                .max_height(350.0)
                                .show(ui, |ui| {
                                    for label in &snapshot.labels {
                                        ui.push_id(label.id, |ui| {
                                            ui.label(format!(
                                                "{} • {} hội thoại",
                                                label.text,
                                                label.conversations.len()
                                            ));
                                            ui.horizontal_wrapped(|ui| {
                                                if ui.button("Lọc danh sách").clicked() {
                                                    self.labels_panel.filter = Some(label.id);
                                                }
                                                if ui.button("Đổi tên").clicked() {
                                                    self.labels_panel.confirm =
                                                        Some(crate::labels::Action::Rename {
                                                            id: label.id,
                                                            name: self.labels_panel.name.clone(),
                                                        });
                                                }
                                                if ui.button("Xóa phân loại").clicked() {
                                                    self.labels_panel.confirm =
                                                        Some(crate::labels::Action::Delete {
                                                            id: label.id,
                                                        });
                                                }
                                                if let Some((target, group)) =
                                                    &self.labels_panel.target
                                                {
                                                    if ui.button("Áp dụng cho hội thoại").clicked()
                                                    {
                                                        self.labels_panel.confirm =
                                                            Some(crate::labels::Action::Assign {
                                                                target: target.clone(),
                                                                group: *group,
                                                                label: Some(label.id),
                                                            });
                                                    }
                                                }
                                            });
                                            ui.separator();
                                        });
                                    }
                                });
                            if let Some((target, group)) = &self.labels_panel.target {
                                if ui.button("Bỏ phân loại khỏi hội thoại").clicked() {
                                    self.labels_panel.confirm =
                                        Some(crate::labels::Action::Assign {
                                            target: target.clone(),
                                            group: *group,
                                            label: None,
                                        });
                                }
                            }
                        }
                    },
                );
                if self.labels_panel.filter.is_some() && ui.button("Bỏ bộ lọc phân loại").clicked()
                {
                    self.labels_panel.filter = None;
                }
                if let Some(action) = self.labels_panel.confirm.clone() {
                    ui.separator();
                    ui.label(match action {
                        crate::labels::Action::Delete { .. } => {
                            "Xóa phân loại trên máy chủ? Các hội thoại và tin nhắn được giữ nguyên."
                        }
                        _ => "Áp dụng thay đổi phân loại trên máy chủ cho tài khoản?",
                    });
                    if ui
                        .add_enabled(!self.labels_panel.pending, egui::Button::new("Xác nhận"))
                        .clicked()
                    {
                        intent = Some(action);
                        self.labels_panel.confirm = None;
                    }
                    if ui.button("Hủy").clicked() {
                        self.labels_panel.confirm = None;
                    }
                }
                if self.labels_panel.uncertain {
                    ui.label("Chưa rõ kết quả; tải lại phân loại để kiểm tra trước khi sửa tiếp.");
                }
            });
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            open = false;
        }
        self.labels_panel.open = open;
        if let Some(action) = intent {
            self.labels_command(action);
        }
    }
}
