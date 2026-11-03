//! User-facing account tools; all persistence is dispatched to the worker.
use super::*;

#[derive(Default)]
pub(super) struct ToolsPanel {
    pub open: bool,
    pub loaded_for: String,
    pub config: crate::tools::Preferences,
    pub tasks: Vec<crate::tools::Scheduled>,
    pub query: String,
    pub hits: Vec<crate::tools::SearchHit>,
    pub last_query: String,
    pub tab: usize,
    pub schedule_text: String,
    pub schedule_minutes: i64,
    pub editing: Option<String>,
    pub when_local: String,
    pub import_owned: bool,
    pub status: String,
    pub paused: bool,
}

#[derive(Clone)]
pub enum ToolsCommand {
    OpenHit {
        thread: String,
        id: String,
    },
    Resume,
    Update {
        id: String,
        text: String,
        at: i64,
    },
    Load,
    Save(crate::tools::Preferences),
    Search {
        query: String,
        hidden: bool,
    },
    ExportHistory,
    Import {
        directory: bool,
        confirmed: bool,
    },
    ReadConfig {
        stickers: bool,
    },
    ExportConfig {
        stickers: bool,
    },
    Schedule {
        request: Option<u64>,
        thread: String,
        group: bool,
        text: String,
        at: i64,
    },
    Edit {
        id: String,
        at: Option<i64>,
    },
}

pub enum ToolsResult {
    ScheduleSaved {
        request: Option<u64>,
        thread: String,
        text: String,
        at: i64,
        result: Result<(), String>,
    },
    Tasks {
        tasks: Vec<crate::tools::Scheduled>,
        paused: bool,
    },
    Hit {
        thread: Box<Thread>,
        id: String,
    },
    Loaded {
        config: crate::tools::Preferences,
        tasks: Vec<crate::tools::Scheduled>,
        paused: bool,
    },
    Search {
        query: String,
        hits: Vec<crate::tools::SearchHit>,
    },
    Status(String),
}

impl ZaloGuiApp {
    pub(super) fn tools_command(&self, command: ToolsCommand) {
        if let Some(tx) = WORKER_TX.get() {
            let _ = tx.send(WorkerCmd::Tools {
                owner: self.user_uin.clone(),
                command,
            });
        }
    }
    pub(super) fn render_tools_panel(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.tools = Default::default();
            return;
        }
        if !self.tools.open {
            return;
        }
        if self.tools.loaded_for != self.user_uin {
            self.tools.loaded_for = self.user_uin.clone();
            self.tools_command(ToolsCommand::Load);
        }
        let mut open = true;
        egui::Window::new("Tiện ích hội thoại")
            .id(Id::new("account-tools"))
            .open(&mut open)
            .default_width(600.0)
            .max_width((ctx.screen_rect().width() - 32.0).max(240.0))
            .max_height((ctx.screen_rect().height() - 60.0).max(200.0))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (i, label) in [
                        "Tìm lịch sử",
                        "Nhập lịch sử",
                        "Giao diện & sticker",
                        "Hẹn giờ gửi",
                        "Tự trả lời",
                    ]
                    .iter()
                    .enumerate()
                    {
                        ui.selectable_value(&mut self.tools.tab, i, *label);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| match self.tools.tab {
                    0 => self.render_tools_search(ui),
                    1 => self.render_tools_import(ui),
                    2 => self.render_tools_appearance(ui),
                    3 => self.render_tools_schedule(ui),
                    _ => self.render_tools_rules(ui),
                });
                if !self.tools.status.is_empty() {
                    ui.separator();
                    ui.label(&self.tools.status);
                }
            });
        self.tools.open = open;
    }
    fn render_tools_search(&mut self, ui: &mut egui::Ui) {
        ui.label("Tìm trong toàn bộ lịch sử đã lưu trên máy, kể cả tin chưa nạp vào cửa sổ chat.");
        ui.horizontal(|ui| {
            let response = ui.text_edit_singleline(&mut self.tools.query);
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if ui.button("Tìm kiếm").clicked() || enter {
                self.tools.last_query = self.tools.query.clone();
                self.tools_command(ToolsCommand::Search {
                    query: self.tools.query.clone(),
                    hidden: self.show_hidden_conversations,
                });
            }
        });
        ui.small(format!(
            "{} kết quả mới nhất (tối đa 50)",
            self.tools.hits.len()
        ));
        let mut action = None;
        for hit in &self.tools.hits {
            ui.group(|ui| {
                ui.strong(&hit.thread_name);
                ui.small(crate::clock::message_time(hit.time));
                ui.label(&hit.text);
                if ui.button("Mở tin trong hội thoại").clicked() {
                    action = Some(ToolsCommand::OpenHit {
                        thread: hit.thread_id.clone(),
                        id: hit.message_id.clone(),
                    });
                }
            });
        }
        if let Some(action) = action {
            self.tools_command(action);
        }
    }
    fn render_tools_import(&mut self, ui: &mut egui::Ui) {
        if ui.button("Xuất lịch sử SQLite…").clicked() {
            self.tools_command(ToolsCommand::ExportHistory);
        }
        ui.label(
            "Bản xuất chỉ gồm lịch sử tin nhắn; không chứa lịch gửi hoặc cấu hình tự trả lời.",
        );
        ui.label(
            "Nhập lịch sử SQLite/JSON tương thích từ máy khác. Dữ liệu nguồn được mở chỉ đọc.",
        );
        ui.checkbox(
            &mut self.tools.import_owned,
            "Tôi xác nhận dữ liệu nguồn thuộc tài khoản đang đăng nhập",
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.tools.import_owned, egui::Button::new("Chọn tập tin"))
                .clicked()
            {
                self.tools_command(ToolsCommand::Import {
                    directory: false,
                    confirmed: true,
                });
            }
            if ui
                .add_enabled(self.tools.import_owned, egui::Button::new("Chọn thư mục"))
                .clicked()
            {
                self.tools_command(ToolsCommand::Import {
                    directory: true,
                    confirmed: true,
                });
            }
        });
    }
    fn render_tools_appearance(&mut self, ui: &mut egui::Ui) {
        ui.label("Theme hỗ trợ bảng màu dark/light trong JSON; sticker tùy chỉnh dùng các ID sticker Zalo có sẵn.");
        ui.horizontal(|ui| {
            if ui.button("Nạp theme JSON").clicked() {
                self.tools_command(ToolsCommand::ReadConfig { stickers: false });
            }
            if ui.button("Xuất theme / mẫu").clicked() {
                self.tools_command(ToolsCommand::ExportConfig { stickers: false });
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Nạp sticker JSON").clicked() {
                self.tools_command(ToolsCommand::ReadConfig { stickers: true });
            }
            if ui.button("Xuất sticker / mẫu").clicked() {
                self.tools_command(ToolsCommand::ExportConfig { stickers: true });
            }
        });
        ui.label(format!(
            "Đã nạp {} bộ sticker riêng",
            self.tools.config.stickers.len()
        ));
        if ui.button("Khôi phục theme mặc định").clicked() {
            let mut config = self.tools.config.clone();
            config.theme = serde_json::Value::Null;
            self.tools_command(ToolsCommand::Save(config));
        }
    }
    fn render_tools_schedule(&mut self, ui: &mut egui::Ui) {
        if let Some((id, name, group)) = self
            .threads
            .get(self.selected_thread_index)
            .map(|t| (t.id.clone(), t.name.clone(), t.is_group))
        {
            ui.label(format!("Gửi tới: {name}"));
            ui.text_edit_multiline(&mut self.tools.schedule_text);
            ui.horizontal_wrapped(|ui| {
                for (minutes, label) in [(15, "Sau 15 phút"), (60, "Sau 1 giờ")] {
                    if ui.button(label).clicked() {
                        self.tools.schedule_minutes = minutes;
                        self.tools.when_local.clear();
                    }
                }
                let now = chrono::Local::now();
                if ui
                    .add_enabled(now.hour() < 18, egui::Button::new("18:00 hôm nay"))
                    .clicked()
                {
                    self.tools.when_local = now
                        .date_naive()
                        .and_hms_opt(18, 0, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default();
                }
                if ui.button("08:00 ngày mai").clicked() {
                    self.tools.when_local = now
                        .date_naive()
                        .succ_opt()
                        .and_then(|d| d.and_hms_opt(8, 0, 0))
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Sau (phút):");
                ui.add(egui::DragValue::new(&mut self.tools.schedule_minutes).range(1..=527040));
            });
            ui.label("Hoặc giờ cụ thể (YYYY-MM-DD HH:MM):");
            ui.text_edit_singleline(&mut self.tools.when_local);
            if ui
                .button(if self.tools.editing.is_some() {
                    "Lưu thay đổi"
                } else {
                    "Hẹn giờ gửi"
                })
                .clicked()
            {
                match scheduled_time(&self.tools.when_local, self.tools.schedule_minutes) {
                    Some(at) => {
                        let command = if let Some(id) = self.tools.editing.take() {
                            ToolsCommand::Update {
                                id,
                                text: self.tools.schedule_text.clone(),
                                at,
                            }
                        } else {
                            ToolsCommand::Schedule {
                                request: None,
                                thread: id,
                                group,
                                text: self.tools.schedule_text.clone(),
                                at,
                            }
                        };
                        self.tools_command(command);
                    }
                    None => {
                        self.tools.status =
                            "Giờ hẹn không hợp lệ hoặc không xác định được múi giờ".into()
                    }
                }
            }
            if self.tools.editing.is_some() && ui.button("Hủy sửa").clicked() {
                self.tools.editing = None;
                self.tools.schedule_text.clear();
            }
            ui.label(RichText::new("Tin đang chờ gửi khi app chạy và tài khoản đăng nhập. Nếu app đã đóng, tin sẽ gửi khi mở lại, kể cả đã qua giờ hẹn.").size(12.0));
        }
        if ui.button("Làm mới danh sách").clicked() {
            self.tools_command(ToolsCommand::Load);
        }
        if self.tools.paused {
            ui.colored_label(Color32::from_rgb(210,80,80),"Lịch gửi tạm dừng sau lỗi/gián đoạn. Kiểm tra kết nối và hội thoại trước khi tiếp tục.");
            if ui
                .button("Đã kiểm tra — tiếp tục lịch gửi đang chờ")
                .clicked()
            {
                self.tools_command(ToolsCommand::Resume);
            }
        }
        let mut action = None;
        let mut edit = None;
        for task in &self.tools.tasks {
            let name = self
                .threads
                .iter()
                .find(|t| t.id == task.thread_id)
                .map(|t| t.name.as_str())
                .unwrap_or(&task.thread_id);
            ui.group(|ui| {
                let when = chrono::Local
                    .timestamp_opt(task.deliver_at, 0)
                    .single()
                    .map(|d| d.format("%d/%m/%Y %H:%M").to_string())
                    .unwrap_or_default();
                let status = match task.status.as_str() {
                    "pending" => "Đang chờ",
                    "sending" => "Đang gửi",
                    "sent" => "Đã gửi",
                    "uncertain" => "Chưa rõ kết quả",
                    "cancelled" => "Đã hủy",
                    _ => "Gửi lỗi",
                };
                ui.label(format!("{name} · {when} · {status}"));
                ui.label(&task.message.text);
                if !task.error.is_empty() {
                    ui.colored_label(Color32::from_rgb(210, 80, 80), &task.error);
                }
                if matches!(task.status.as_str(), "pending" | "failed" | "uncertain") {
                    ui.horizontal_wrapped(|ui| {
                        if task.status == "pending"
                            && task
                                .message
                                .outgoing_job
                                .as_ref()
                                .is_some_and(|j| j.attempt == 1)
                            && ui.button("Sửa nội dung / giờ").clicked()
                        {
                            edit =
                                Some((task.id.clone(), task.message.text.clone(), task.deliver_at));
                        }
                        if ui.button("Hủy").clicked() {
                            action = Some(ToolsCommand::Edit {
                                id: task.id.clone(),
                                at: None,
                            });
                        }
                        let label = if task.status == "pending" {
                            "Đổi giờ: sau 15 phút"
                        } else {
                            "Đã kiểm tra hội thoại — gửi lại sau 1 phút"
                        };
                        if ui.button(label).clicked() {
                            action = Some(ToolsCommand::Edit {
                                id: task.id.clone(),
                                at: Some(
                                    crate::login::unix_now()
                                        + if task.status == "pending" { 900 } else { 60 },
                                ),
                            });
                        }
                    });
                }
            });
        }
        if let Some((id, text, at)) = edit {
            self.tools.editing = Some(id);
            self.tools.schedule_text = text;
            self.tools.when_local = chrono::Local
                .timestamp_opt(at, 0)
                .single()
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default();
        }
        if let Some(action) = action {
            self.tools_command(action);
        }
    }
    fn render_tools_rules(&mut self, ui: &mut egui::Ui) {
        ui.label("Quy tắc mặc định tắt. Chỉ áp dụng cho tin mới sau lúc lưu/bật, tối đa 5 trả lời tự động mỗi phút.");
        let mut remove = None;
        for (i, rule) in self.tools.config.rules.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut rule.enabled, "Bật");
                    if ui.button("Xóa quy tắc").clicked() {
                        remove = Some(i);
                    }
                });
                ui.label("Từ khóa");
                ui.text_edit_singleline(&mut rule.keyword);
                ui.label("Nội dung trả lời");
                ui.text_edit_multiline(&mut rule.reply_text);
                ui.horizontal(|ui| {
                    let mode = match rule.mode.as_str() {
                        "exact" => "Chính xác",
                        "prefix" => "Bắt đầu bằng",
                        _ => "Có chứa",
                    };
                    egui::ComboBox::from_id_salt(("mode", i))
                        .selected_text(mode)
                        .show_ui(ui, |ui| {
                            for (mode, name) in [
                                ("contains", "Có chứa"),
                                ("exact", "Chính xác"),
                                ("prefix", "Bắt đầu bằng"),
                            ] {
                                ui.selectable_value(&mut rule.mode, mode.into(), name);
                            }
                        });
                    let scope = match rule.scope.as_str() {
                        "group" => "Nhóm",
                        "all" => "Tất cả",
                        _ => "Cá nhân",
                    };
                    egui::ComboBox::from_id_salt(("scope", i))
                        .selected_text(scope)
                        .show_ui(ui, |ui| {
                            for (scope, name) in
                                [("all", "Tất cả"), ("direct", "Cá nhân"), ("group", "Nhóm")]
                            {
                                ui.selectable_value(&mut rule.scope, scope.into(), name);
                            }
                        });
                });
            });
        }
        if let Some(i) = remove {
            self.tools.config.rules.remove(i);
        }
        if self.tools.config.rules.len() < 64 && ui.button("Thêm quy tắc").clicked() {
            self.tools.config.rules.push(crate::tools::ReplyRule {
                id: crate::param_cipher::random_hex(12),
                keyword: String::new(),
                reply_text: String::new(),
                mode: "contains".into(),
                scope: "direct".into(),
                enabled: false,
                enabled_since: 0,
            });
        }
        if ui.button("Lưu các quy tắc").clicked() {
            self.tools_command(ToolsCommand::Save(self.tools.config.clone()));
        }
    }
}
fn scheduled_time(text: &str, minutes: i64) -> Option<i64> {
    if text.trim().is_empty() {
        Some(crate::login::unix_now() + minutes.max(1) * 60)
    } else {
        chrono::NaiveDateTime::parse_from_str(text.trim(), "%Y-%m-%d %H:%M")
            .ok()
            .and_then(|date| chrono::Local.from_local_datetime(&date).single())
            .map(|d| d.timestamp())
    }
}
use chrono::{TimeZone, Timelike};
