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
    pub quick_messages: crate::quick_message::QuickMessageStore,
    pub devices: Vec<crate::device::DeviceSession>,
    pub group_todos: Vec<crate::todo::TodoItem>,
    pub new_todo_content: String,
    pub new_todo_assignee: String,
    pub new_quick_shortcut: String,
    pub new_quick_title: String,
    pub new_quick_text: String,
    pub server_media_items: Vec<crate::server_media::ServerMediaItem>,
    pub cloud_viewer_keys: std::collections::HashMap<String, crate::zcloud::CloudViewerKey>,
    pub catalog_items: Vec<crate::business::ProductCatalogItem>,
    pub two_factor_status: Option<crate::account_recovery::TwoFactorStatus>,
    pub group_polls: Vec<crate::poll::PollItem>,
    pub lan_peers: Vec<crate::lan_transfer::LanPeer>,
    pub new_poll_question: String,
    pub new_poll_option_1: String,
    pub new_poll_option_2: String,
}

#[derive(Clone)]
pub enum ToolsCommand {
    SearchPage(crate::search::Request),
    OpenSearchHit {
        generation: u64,
        thread: String,
        id: String,
    },
    OpenThread {
        thread: String,
        group: bool,
    },
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
    SearchOpenFailed {
        generation: u64,
        thread: String,
        id: String,
        error: String,
    },
    SearchPage {
        request: crate::search::Request,
        result: Result<crate::search::Page, String>,
        senders: Vec<crate::search::Sender>,
    },
    SearchOpened {
        generation: u64,
        thread: Box<Thread>,
        id: String,
    },
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
        egui::Window::new(crate::i18n::tr("Tiện ích hội thoại"))
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
                        "Giao việc & Bình chọn",
                        "Tin nhắn nhanh",
                        "Thiết bị & Bảo mật",
                        "Kho Media Server & zCloud",
                        "Doanh nghiệp & Truyền LAN",
                    ]
                    .iter()
                    .enumerate()
                    {
                        ui.selectable_value(&mut self.tools.tab, i, crate::i18n::tr(label));
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| match self.tools.tab {
                    0 => self.render_tools_search(ui),
                    1 => self.render_tools_import(ui),
                    2 => self.render_tools_appearance(ui),
                    3 => self.render_tools_schedule(ui),
                    4 => self.render_tools_rules(ui),
                    5 => self.render_tools_todo_poll(ui),
                    6 => self.render_tools_quick_messages(ui),
                    7 => self.render_tools_devices(ui),
                    8 => self.render_tools_server_media_zcloud(ui),
                    _ => self.render_tools_business_lan(ui),
                });
                if !self.tools.status.is_empty() {
                    ui.separator();
                    ui.label(&self.tools.status);
                }
            });
        self.tools.open = open;
    }
    fn render_tools_search(&mut self, ui: &mut egui::Ui) {
        ui.label(crate::i18n::tr(
            "Tìm trong toàn bộ lịch sử đã lưu trên máy, kể cả tin chưa nạp vào cửa sổ chat.",
        ));
        ui.horizontal(|ui| {
            let response = ui.text_edit_singleline(&mut self.tools.query);
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if ui.button(crate::i18n::tr("Tìm kiếm")).clicked() || enter {
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
                if ui
                    .button(crate::i18n::tr("Mở tin trong hội thoại"))
                    .clicked()
                {
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
        if ui.button(crate::i18n::tr("Xuất lịch sử SQLite…")).clicked() {
            self.tools_command(ToolsCommand::ExportHistory);
        }
        ui.label(crate::i18n::tr(
            "Bản xuất chỉ gồm lịch sử tin nhắn; không chứa lịch gửi hoặc cấu hình tự trả lời.",
        ));
        ui.label(crate::i18n::tr(
            "Nhập lịch sử SQLite/JSON tương thích từ máy khác. Dữ liệu nguồn được mở chỉ đọc.",
        ));
        ui.checkbox(
            &mut self.tools.import_owned,
            "Tôi xác nhận dữ liệu nguồn thuộc tài khoản đang đăng nhập",
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.tools.import_owned,
                    egui::Button::new(crate::i18n::tr("Chọn tập tin")),
                )
                .clicked()
            {
                self.tools_command(ToolsCommand::Import {
                    directory: false,
                    confirmed: true,
                });
            }
            if ui
                .add_enabled(
                    self.tools.import_owned,
                    egui::Button::new(crate::i18n::tr("Chọn thư mục")),
                )
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
        ui.label(crate::i18n::tr("Theme hỗ trợ bảng màu dark/light trong JSON; sticker tùy chỉnh dùng các ID sticker Zalo có sẵn."));
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::tr("Nạp theme JSON")).clicked() {
                self.tools_command(ToolsCommand::ReadConfig { stickers: false });
            }
            if ui.button(crate::i18n::tr("Xuất theme / mẫu")).clicked() {
                self.tools_command(ToolsCommand::ExportConfig { stickers: false });
            }
        });
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::tr("Nạp sticker JSON")).clicked() {
                self.tools_command(ToolsCommand::ReadConfig { stickers: true });
            }
            if ui.button(crate::i18n::tr("Xuất sticker / mẫu")).clicked() {
                self.tools_command(ToolsCommand::ExportConfig { stickers: true });
            }
        });
        ui.label(format!(
            "Đã nạp {} bộ sticker riêng",
            self.tools.config.stickers.len()
        ));
        if ui
            .button(crate::i18n::tr("Khôi phục theme mặc định"))
            .clicked()
        {
            let mut config = self.tools.config.clone();
            config.theme = serde_json::Value::Null;
            self.tools_command(ToolsCommand::Save(config));
        }
    }
    fn render_tools_schedule(&mut self, ui: &mut egui::Ui) {
        if let Some((id, name, group)) = self
            .threads
            .get(self.selected_thread_index)
            .filter(|t| self.private_access(&t.id, t.is_group))
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
                    .add_enabled(
                        now.hour() < 18,
                        egui::Button::new(crate::i18n::tr("18:00 hôm nay")),
                    )
                    .clicked()
                {
                    self.tools.when_local = now
                        .date_naive()
                        .and_hms_opt(18, 0, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default();
                }
                if ui.button(crate::i18n::tr("08:00 ngày mai")).clicked() {
                    self.tools.when_local = now
                        .date_naive()
                        .succ_opt()
                        .and_then(|d| d.and_hms_opt(8, 0, 0))
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default();
                }
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Sau (phút):"));
                ui.add(egui::DragValue::new(&mut self.tools.schedule_minutes).range(1..=527040));
            });
            ui.label(crate::i18n::tr("Hoặc giờ cụ thể (YYYY-MM-DD HH:MM):"));
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
            if self.tools.editing.is_some() && ui.button(crate::i18n::tr("Hủy sửa")).clicked() {
                self.tools.editing = None;
                self.tools.schedule_text.clear();
            }
            ui.label(RichText::new(crate::i18n::tr("Tin đang chờ gửi khi app chạy và tài khoản đăng nhập. Nếu app đã đóng, tin sẽ gửi khi mở lại, kể cả đã qua giờ hẹn.")).size(12.0));
        }
        if ui.button(crate::i18n::tr("Làm mới danh sách")).clicked() {
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
            if !self.private_access(&task.thread_id, task.thread_id.starts_with('g')) {
                continue;
            }
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
                            && ui.button(crate::i18n::tr("Sửa nội dung / giờ")).clicked()
                        {
                            edit =
                                Some((task.id.clone(), task.message.text.clone(), task.deliver_at));
                        }
                        if ui.button(crate::i18n::tr("Hủy")).clicked() {
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
        ui.label(crate::i18n::tr("Quy tắc mặc định tắt. Chỉ áp dụng cho tin mới sau lúc lưu/bật, tối đa 5 trả lời tự động mỗi phút."));
        let mut remove = None;
        for (i, rule) in self.tools.config.rules.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut rule.enabled, crate::i18n::tr("Bật"));
                    if ui.button(crate::i18n::tr("Xóa quy tắc")).clicked() {
                        remove = Some(i);
                    }
                });
                ui.label(crate::i18n::tr("Từ khóa"));
                ui.text_edit_singleline(&mut rule.keyword);
                ui.label(crate::i18n::tr("Nội dung trả lời"));
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
        if self.tools.config.rules.len() < 64
            && ui.button(crate::i18n::tr("Thêm quy tắc")).clicked()
        {
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
        if ui.button(crate::i18n::tr("Lưu các quy tắc")).clicked() {
            self.tools_command(ToolsCommand::Save(self.tools.config.clone()));
        }
    }

    fn render_tools_todo_poll(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("📋 Giao việc & Nhắc việc nhóm (Zalo Parity A18)").strong().size(14.0));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Nội dung việc:");
            ui.text_edit_singleline(&mut self.tools.new_todo_content);
        });
        ui.horizontal(|ui| {
            ui.label("Người được giao (ID):");
            ui.text_edit_singleline(&mut self.tools.new_todo_assignee);
        });
        ui.horizontal(|ui| {
            if ui.button("➕ Tạo việc cần làm").clicked() && !self.tools.new_todo_content.trim().is_empty() {
                let content = self.tools.new_todo_content.trim().to_string();
                let assignee = self.tools.new_todo_assignee.trim().to_string();
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::CreateTodo {
                            thread_id: thread.id.clone(),
                            content: content.clone(),
                            assignee_id: assignee.clone(),
                            deadline_ts: 0,
                        });
                    }
                }
                self.tools.new_todo_content.clear();
                self.tools.new_todo_assignee.clear();
                self.status_msg = format!("Đang gửi lệnh tạo việc: {content}...");
            }
            if ui.button("🔄 Tải danh sách việc").clicked() {
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::FetchTodos {
                            thread_id: thread.id.clone(),
                        });
                    }
                }
            }
        });

        if !self.tools.group_todos.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("Danh sách việc ({}) :", self.tools.group_todos.len())).strong());
            for todo in &self.tools.group_todos {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let icon = if todo.status == crate::todo::TodoStatus::Done { "✅" } else { "⏳" };
                        ui.label(icon);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&todo.content).strong());
                            ui.label(RichText::new(format!("Người tạo: {} • Người nhận: {}", todo.creator_id, todo.assignee_id)).size(11.0));
                        });
                    });
                });
            }
        }

        ui.separator();
        ui.label(RichText::new("📊 Bình chọn nhóm (Zalo Parity A19)").strong().size(14.0));
        ui.label("Tạo và theo dõi kết quả các cuộc bình chọn trong nhóm trò chuyện.");
        ui.horizontal(|ui| {
            ui.label("Câu hỏi:");
            ui.text_edit_singleline(&mut self.tools.new_poll_question);
        });
        ui.horizontal(|ui| {
            ui.label("Lựa chọn 1:");
            ui.text_edit_singleline(&mut self.tools.new_poll_option_1);
        });
        ui.horizontal(|ui| {
            ui.label("Lựa chọn 2:");
            ui.text_edit_singleline(&mut self.tools.new_poll_option_2);
        });
        ui.horizontal(|ui| {
            if ui.button("➕ Tạo bình chọn").clicked() && !self.tools.new_poll_question.trim().is_empty() {
                let question = self.tools.new_poll_question.trim().to_string();
                let mut options = Vec::new();
                if !self.tools.new_poll_option_1.trim().is_empty() {
                    options.push(self.tools.new_poll_option_1.trim().to_string());
                }
                if !self.tools.new_poll_option_2.trim().is_empty() {
                    options.push(self.tools.new_poll_option_2.trim().to_string());
                }
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::CreatePoll {
                            thread_id: thread.id.clone(),
                            question: question.clone(),
                            options,
                        });
                    }
                }
                self.tools.new_poll_question.clear();
                self.tools.new_poll_option_1.clear();
                self.tools.new_poll_option_2.clear();
                self.status_msg = format!("Đang gửi lệnh tạo bình chọn: {question}...");
            }
            if ui.button("🔄 Tải bình chọn").clicked() {
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::FetchPolls {
                            thread_id: thread.id.clone(),
                        });
                    }
                }
            }
        });

        if !self.tools.group_polls.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new("Các cuộc bình chọn hiện tại:").strong());
            for poll in &self.tools.group_polls {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&poll.question).strong().color(Color32::from_rgb(0, 120, 215)));
                        ui.label(RichText::new(format!("Tổng số phiếu: {}", poll.total_votes)).size(11.0));
                        for opt in &poll.options {
                            ui.horizontal(|ui| {
                                ui.label(format!("• {} ({} phiếu)", opt.content, opt.votes));
                            });
                        }
                    });
                });
            }
        }
    }

    fn render_tools_quick_messages(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("⚡ Tin nhắn nhanh & Phím tắt gõ nhanh (Zalo Parity B07)").strong().size(14.0));
        ui.label("Gõ phím tắt (bắt đầu bằng '/') trong khung chat để gợi ý và chèn nhanh nội dung.");
        ui.add_space(6.0);

        let mut remove_shortcut = None;
        for (idx, qm) in self.tools.quick_messages.items.iter().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&qm.shortcut).strong().color(Color32::from_rgb(0, 120, 215)));
                    ui.label(format!("({})", qm.title));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Xoá").clicked() {
                            remove_shortcut = Some(idx);
                        }
                    });
                });
                ui.label(RichText::new(&qm.text).size(12.5));
            });
        }
        if let Some(idx) = remove_shortcut {
            self.tools.quick_messages.items.remove(idx);
        }

        ui.separator();
        ui.label(RichText::new("Thêm tin nhắn nhanh mới:").strong());
        ui.horizontal(|ui| {
            ui.label("Phím tắt:");
            ui.text_edit_singleline(&mut self.tools.new_quick_shortcut);
            ui.label("Tiêu đề:");
            ui.text_edit_singleline(&mut self.tools.new_quick_title);
        });
        ui.horizontal(|ui| {
            ui.label("Nội dung:");
            ui.text_edit_singleline(&mut self.tools.new_quick_text);
        });
        if ui.button("➕ Thêm tin nhắn nhanh").clicked() {
            let sc = self.tools.new_quick_shortcut.trim();
            let title = self.tools.new_quick_title.trim();
            let text = self.tools.new_quick_text.trim();
            if !sc.is_empty() && !text.is_empty() {
                let shortcut = if sc.starts_with('/') { sc.to_string() } else { format!("/{sc}") };
                self.tools.quick_messages.upsert(shortcut, title.into(), text.into());
                self.tools.new_quick_shortcut.clear();
                self.tools.new_quick_title.clear();
                self.tools.new_quick_text.clear();
            }
        }
    }

    fn render_tools_business_lan(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("🏢 Doanh nghiệp & Truyền tệp LAN (Zalo Parity C01 / C03)").strong().size(14.0));
        ui.label("Quản lý danh mục sản phẩm OA/Doanh nghiệp và tính năng truyền tệp siêu tốc trong mạng nội bộ LAN.");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui.button("📋 Tải Danh mục sản phẩm").clicked() {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::GetBusinessCatalog);
                }
            }
            if ui.button("🔍 Quét thiết bị cùng mạng LAN").clicked() {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::DiscoverLanPeers);
                }
            }
        });

        if !self.tools.catalog_items.is_empty() {
            ui.separator();
            ui.label(RichText::new(format!("📦 Danh mục sản phẩm ({}) :", self.tools.catalog_items.len())).strong());
            for prod in &self.tools.catalog_items {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("🛍️");
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&prod.name).strong());
                            ui.label(RichText::new(format!("Giá: {} đ • {}", prod.price, prod.description)).size(12.0));
                        });
                    });
                });
            }
        }

        ui.separator();
        ui.label(RichText::new("📡 Thiết bị ngang hàng trong mạng LAN:").strong());
        if self.tools.lan_peers.is_empty() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("💻");
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Sẵn sàng truyền nhận tệp P2P qua Wi-Fi / Ethernet").strong());
                        ui.label("Cổng mặc định: 8998 • Bấm quét để tìm kiếm thiết bị Zalo lân cận");
                    });
                });
            });
        } else {
            for peer in &self.tools.lan_peers {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("💻");
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&peer.device_name).strong());
                            ui.label(RichText::new(format!("IP: {}:{} • ID: {}", peer.ip_address, peer.port, peer.peer_id)).size(12.0));
                        });
                    });
                });
            }
        }
    }

    fn render_tools_server_media_zcloud(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("📦 Kho Lưu Trữ Media Server & zCloud (Zalo Parity A20 / B05)").strong().size(14.0));
        ui.label("Truy xuất thư viện hình ảnh, video, tệp tin và liên kết từ máy chủ Zalo cùng dữ liệu lưu trữ đám mây zCloud.");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui.button("🖼️ Tải Ảnh & Video").clicked() {
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::FetchServerMedia {
                            thread_id: thread.id.clone(),
                            is_group: thread.is_group,
                            media_type: 1,
                        });
                    }
                }
            }
            if ui.button("📁 Tải Tệp tin").clicked() {
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::FetchServerMedia {
                            thread_id: thread.id.clone(),
                            is_group: thread.is_group,
                            media_type: 3,
                        });
                    }
                }
            }
            if ui.button("🔗 Tải Liên kết").clicked() {
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::FetchServerMedia {
                            thread_id: thread.id.clone(),
                            is_group: thread.is_group,
                            media_type: 4,
                        });
                    }
                }
            }
        });

        ui.separator();
        ui.label(RichText::new("☁️ Khôi phục & Mở tệp zCloud (A20):").strong());
        ui.label("Nhập mã file_id để kiểm tra trạng thái và nhận khoá giải mã xem trước.");
        ui.horizontal(|ui| {
            ui.label("File ID:");
            ui.text_edit_singleline(&mut self.tools.query);
            if ui.button("🔑 Lấy khoá zCloud").clicked() && !self.tools.query.trim().is_empty() {
                let fid = self.tools.query.trim().to_string();
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::GetCloudViewerKey { file_id: fid });
                }
            }
        });

        if !self.tools.cloud_viewer_keys.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Khoá giải mã zCloud đã lưu:").strong());
            for (fid, k) in &self.tools.cloud_viewer_keys {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("Tệp: {fid}")).strong());
                        ui.label(RichText::new(format!("Thuật toán: {} • Hết hạn: {}", k.algorithm, k.expires_at)).size(11.0));
                    });
                });
            }
        }

        if !self.tools.server_media_items.is_empty() {
            ui.separator();
            ui.label(RichText::new(format!("Thư viện media từ máy chủ ({}) :", self.tools.server_media_items.len())).strong());
            for item in &self.tools.server_media_items {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let icon = match item.kind {
                            crate::server_media::ServerMediaType::Photo => "🖼️",
                            crate::server_media::ServerMediaType::Video => "🎬",
                            crate::server_media::ServerMediaType::File => "📁",
                            crate::server_media::ServerMediaType::Link => "🔗",
                        };
                        ui.label(icon);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&item.name).strong());
                            ui.label(RichText::new(format!("Kích thước: {} bytes • Người gửi: {}", item.file_size, item.sender_name)).size(11.0));
                        });
                    });
                });
            }
        }
    }

    fn render_tools_devices(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("🔒 Quản lý thiết bị đăng nhập & Bảo mật (Zalo Parity C02)").strong().size(14.0));
        ui.label("Danh sách các phiên và thiết bị đang đăng nhập tài khoản Zalo cùng trạng thái bảo mật.");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui.button("🔄 Làm mới thiết bị").clicked() {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::FetchDevices);
                }
            }
            if ui.button("🛡️ Kiểm tra 2FA").clicked() {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::CheckTwoFactor);
                }
            }
        });

        if let Some(status) = &self.tools.two_factor_status {
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("🛡️");
                    ui.vertical(|ui| {
                        let st_badge = if status.is_enabled {
                            RichText::new("Xác thực hai lớp (2FA): ĐÃ KÍCH HOẠT").color(Color32::from_rgb(0, 180, 80)).strong()
                        } else {
                            RichText::new("Xác thực hai lớp (2FA): CHƯA KÍCH HOẠT").color(Color32::from_rgb(220, 100, 0)).strong()
                        };
                        ui.label(st_badge);
                        ui.label(RichText::new(format!("Số điện thoại bảo vệ: {} • Ứng dụng OTP: {}", status.phone_masked, if status.has_authenticator_app { "Có" } else { "Không" })).size(12.0));
                    });
                });
            });
        }

        if self.tools.devices.is_empty() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("💻");
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Thiết bị hiện tại (Windows 11 ARM64)").strong());
                        ui.label(RichText::new("Đang hoạt động • Phiên làm việc này").size(12.0).color(Color32::from_rgb(0, 180, 80)));
                    });
                });
            });
        } else {
            for dev in &self.tools.devices {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(if dev.platform.contains("Windows") { "💻" } else if dev.platform.contains("Web") { "🌐" } else { "📱" });
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&dev.name).strong());
                            ui.label(RichText::new(format!("Hệ điều hành: {} • Vị trí: {}", dev.platform, dev.location)).size(12.0));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if dev.is_current {
                                ui.label(RichText::new("Thiết bị này").color(Color32::from_rgb(0, 180, 80)).strong());
                            } else if ui.button("Đăng xuất từ xa").clicked() {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::KickDevice { device_id: dev.device_id.clone() });
                                }
                            }
                        });
                    });
                });
            }
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_panel_initial_state() {
        let tools = ToolsPanel::default();
        assert!(tools.server_media_items.is_empty());
        assert!(tools.cloud_viewer_keys.is_empty());
        assert!(tools.catalog_items.is_empty());
        assert!(tools.devices.is_empty());
        assert!(tools.two_factor_status.is_none());
        assert!(tools.group_todos.is_empty());
        assert!(tools.group_polls.is_empty());
        assert!(tools.lan_peers.is_empty());
        assert!(tools.new_poll_question.is_empty());
    }
}
