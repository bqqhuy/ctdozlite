//! Safety number verification dialog (WP16 Sprint 3-4).

use egui::{Context, Id, Window};

pub struct SafetyNumberDialog {
    pub peer_id: String,
    pub peer_name: String,
    pub safety_number: String,
    pub identity_changed: bool,
}

impl SafetyNumberDialog {
    pub fn render(&mut self, ctx: &Context, open: &mut bool) {
        Window::new(format!("🔒 Mã an toàn - {}", self.peer_name))
            .id(Id::new("safety_number_dialog"))
            .open(open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if self.identity_changed {
                        ui.colored_label(
                            egui::Color32::from_rgb(239, 68, 68),
                            "⚠️ Thiết bị của đối phương đã thay đổi"
                        );
                        ui.label("Mã an toàn trước đây không còn khớp.");
                        ui.add_space(10.0);
                    }

                    ui.label("Mã an toàn (12 nhóm, 5 chữ số mỗi nhóm):");
                    ui.add_space(5.0);

                    let groups: Vec<&str> = self.safety_number.split_whitespace().collect();
                    for chunk in groups.chunks(3) {
                        ui.horizontal(|ui| {
                            for group in chunk {
                                ui.monospace(*group);
                                ui.add_space(8.0);
                            }
                        });
                        ui.add_space(5.0);
                    }

                    ui.add_space(15.0);
                    ui.label("Đọc mã này qua điện thoại hoặc so sánh trực tiếp.");
                    
                    if ui.button("Đóng").clicked() {
                        *open = false;
                    }
                });
            });
    }
}
