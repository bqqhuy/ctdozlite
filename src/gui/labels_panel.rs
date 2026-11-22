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
        crate::gui::modals::render_modal_backdrop(ctx, "labels_panel_backdrop");
        let mut open = true;
        let mut intent = None;
        egui::Window::new("Phân loại hội thoại")
            .id(Id::new("server-labels"))
            // The full-screen backdrop is foreground too; the dialog must be
            // painted after it on that same layer to receive pointer input.
            .order(egui::Order::Foreground)
            .open(&mut open)
            .vscroll(true)
            .min_width(120.0)
            .max_width((ctx.screen_rect().width() - 32.0).max(180.0))
            .max_height((ctx.screen_rect().height() - 64.0).max(100.0))
            .default_width(480.0_f32.min((ctx.screen_rect().width() - 48.0).max(160.0)))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_button_receives_click_above_backdrop_and_ungates_workspace() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![],
            false,
        );
        app.is_logged_in = true;
        app.labels_panel.open = true;
        // An unanswered request must not prevent closing the dialog.
        app.labels_panel.pending = true;
        assert!(app.is_workspace_input_gated(&ctx));
        let input = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))),
            ..Default::default()
        };
        let mut output = ctx.run(input(), |ctx| app.render_labels_panel(ctx));
        for _ in 0..3 {
            output = ctx.run(input(), |ctx| app.render_labels_panel(ctx));
        }
        let window = ctx
            .memory(|m| m.area_rect(Id::new("server-labels")))
            .unwrap();
        // Find the painted X, so the test does not depend on theme margins.
        fn diagonal_centers(shape: &egui::Shape, points: &mut Vec<Pos2>) {
            match shape {
                egui::Shape::LineSegment { points: ends, .. }
                    if (ends[0].x - ends[1].x).abs() > 1.0
                        && (ends[0].y - ends[1].y).abs() > 1.0 =>
                {
                    points.push(ends[0].lerp(ends[1], 0.5));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        diagonal_centers(shape, points);
                    }
                }
                _ => {}
            }
        }
        let mut points = Vec::new();
        for shape in &output.shapes {
            diagonal_centers(&shape.shape, &mut points);
        }
        let close = points
            .into_iter()
            .filter(|p| window.contains(*p) && p.y < window.top() + 50.0)
            .max_by(|a, b| a.x.total_cmp(&b.x))
            .expect("window close icon is painted");
        assert_eq!(
            ctx.layer_id_at(close),
            Some(egui::LayerId::new(
                egui::Order::Foreground,
                Id::new("server-labels")
            )),
            "dialog must receive input above its full-screen backdrop",
        );
        for pressed in [true, false] {
            let mut raw = input();
            raw.events = vec![
                egui::Event::PointerMoved(close),
                egui::Event::PointerButton {
                    pos: close,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            let _ = ctx.run(raw, |ctx| app.render_labels_panel(ctx));
        }
        assert!(
            !app.labels_panel.open,
            "pointer click must close the dialog"
        );
        assert!(!app.is_workspace_input_gated(&ctx));
    }
}
