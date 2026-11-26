//! User-facing account tools; all persistence is dispatched to the worker.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerMediaFetchMode {
    Reload,
    Append,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingServerMediaRequest {
    pub runtime_id: usize,
    pub thread_id: String,
    pub media_type: u8,
    pub generation: u64,
    pub mode: ServerMediaFetchMode,
    pub requested_page: u32,
    pub requested_cursor: Option<String>,
}

#[derive(Default)]
pub(super) struct ToolsPanel {
    pub open: bool,
    /// One diagnostics-folder open at a time; cleared by its completion event.
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
    pub data_pending: Option<u64>,
    pub paused: bool,
    pub devices: Vec<crate::device::DeviceSession>,
    pub group_todos: Vec<crate::todo::TodoItem>,
    pub todo_poll_thread_id: String,
    pub new_todo_content: String,
    pub new_todo_assignee: String,
    pub new_quick_shortcut: String,
    pub new_quick_title: String,
    pub new_quick_text: String,
    pub server_media_items: Vec<crate::server_media::ServerMediaItem>,
    pub server_media_thread_id: String,
    pub server_media_type: u8,
    pub server_media_page: u32,
    pub server_media_has_more: bool,
    pub server_media_next_cursor: Option<String>,
    pub server_media_generation: u64,
    pub server_media_loading: bool,
    pub server_media_pending: Option<PendingServerMediaRequest>,
    pub cloud_viewer_keys: std::collections::HashMap<String, crate::zcloud::CloudViewerKey>,
    pub catalog_items: Vec<crate::business::ProductCatalogItem>,
    pub two_factor_status: Option<crate::account_recovery::TwoFactorStatus>,
    pub group_polls: Vec<crate::poll::PollItem>,
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
        /// Dispatch-time identity stamped by the worker. `None` never hydrates Quick Messages.
        origin: Option<ToolsOrigin>,
    },
    Search {
        query: String,
        hits: Vec<crate::tools::SearchHit>,
    },
    Status(String),
}

/// Identity captured when a config-bearing Tools command is dispatched, never at completion.
/// It lets the consumer reject a config snapshot whose Quick Messages are older than the
/// confirmed state (server list, server mutation, or local edit applied in between).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolsOrigin {
    pub runtime_id: usize,
    pub request_id: u64,
    pub quick_message_revision: u64,
}

impl ZaloGuiApp {
    pub(super) fn tools_command(&mut self, command: ToolsCommand) -> bool {
        let data_operation = matches!(
            command,
            ToolsCommand::ExportHistory | ToolsCommand::Import { .. }
        );
        if data_operation && self.tools.data_pending.is_some() {
            return false;
        }
        let generation = crate::search::generation();
        let command = if data_operation {
            WorkerCmd::ToolsData {
                owner: self.user_uin.clone(),
                runtime_id: self.account_runtime_id,
                generation,
                command,
            }
        } else {
            WorkerCmd::Tools {
                owner: self.user_uin.clone(),
                origin: Some(ToolsOrigin {
                    runtime_id: self.account_runtime_id,
                    request_id: generation,
                    quick_message_revision: self.quick_message_state_revision,
                }),
                command,
            }
        };
        if WORKER_TX.get().is_some_and(|tx| tx.send(command).is_ok()) {
            if data_operation {
                self.tools.data_pending = Some(generation);
            }
            true
        } else {
            self.show_toast("Không thể gửi lệnh tới tác vụ nền");
            false
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
                        "Doanh nghiệp & LAN",
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
        let current_thread = self
            .threads
            .get(self.selected_thread_index)
            .filter(|thread| thread.is_group)
            .map(|thread| thread.id.as_str())
            .unwrap_or("");
        if self.tools.todo_poll_thread_id != current_thread {
            self.tools.group_todos.clear();
            self.tools.group_polls.clear();
            self.tools.todo_poll_thread_id = current_thread.to_owned();
        }
        ui.label(RichText::new("📋 Giao việc nhóm").strong().size(14.0));
        ui.label("Giao việc chưa khả dụng: đang xác minh giao thức Zalo PC.");
        ui.add_enabled_ui(false, |ui| {
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
                if ui.button("➕ Tạo việc cần làm").clicked()
                    && !self.tools.new_todo_content.trim().is_empty()
                {
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
                ui.label(
                    RichText::new(format!(
                        "Danh sách việc ({}) :",
                        self.tools.group_todos.len()
                    ))
                    .strong(),
                );
                for todo in &self.tools.group_todos {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let icon = if todo.status == crate::todo::TodoStatus::Done {
                                "✅"
                            } else {
                                "⏳"
                            };
                            ui.label(icon);
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&todo.content).strong());
                                ui.label(
                                    RichText::new(format!(
                                        "Người tạo: {} • Người nhận: {}",
                                        todo.creator_id, todo.assignee_id
                                    ))
                                    .size(11.0),
                                );
                            });
                        });
                    });
                }
            }
        });
        ui.separator();
        ui.label(
            RichText::new("📊 Bình chọn nhóm (Zalo Parity A19)")
                .strong()
                .size(14.0),
        );
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
            if ui.button("➕ Tạo bình chọn").clicked()
                && !self.tools.new_poll_question.trim().is_empty()
            {
                let question = self.tools.new_poll_question.trim().to_string();
                let mut options = Vec::new();
                if !self.tools.new_poll_option_1.trim().is_empty() {
                    options.push(self.tools.new_poll_option_1.trim().to_string());
                }
                if !self.tools.new_poll_option_2.trim().is_empty() {
                    options.push(self.tools.new_poll_option_2.trim().to_string());
                }
                let active_thread_id = self.current_thread().map(|t| t.id.clone());
                if let Some(thread_id) = active_thread_id {
                    let intent_key = (thread_id.clone(), question.clone());
                    if let Some(&locked_req_id) = self.poll_intent_lock.get(&intent_key) {
                        if let Some(op) = self.poll_operations.get(&locked_req_id) {
                            match &op.phase {
                                crate::gui::PollOpPhase::Pending => {
                                    self.show_toast(
                                        "Bình chọn này đang được gửi, vui lòng chờ máy chủ xác nhận",
                                    );
                                    return;
                                }
                                crate::gui::PollOpPhase::Indeterminate { error } => {
                                    self.show_toast(format!(
                                        "Bình chọn này chưa rõ kết quả ({error}). Vui lòng bấm '🔄 Tải bình chọn' để kiểm tra trước khi gửi lại."
                                    ));
                                    return;
                                }
                                _ => {
                                    self.poll_intent_lock.remove(&intent_key);
                                }
                            }
                        } else {
                            self.poll_intent_lock.remove(&intent_key);
                        }
                    }

                    let req_id = crate::search::generation();
                    self.poll_operations.insert(
                        req_id,
                        crate::gui::PollOperation {
                            owner: self.user_uin.clone(),
                            runtime_id: self.account_runtime_id,
                            thread_id: thread_id.clone(),
                            request_id: req_id,
                            question: question.clone(),
                            options: options.clone(),
                            phase: crate::gui::PollOpPhase::Pending,
                            created_at: std::time::Instant::now(),
                        },
                    );
                    self.poll_intent_lock.insert(intent_key.clone(), req_id);
                    self.poll_pending_intents.insert(intent_key.clone());
                    let mut sent = false;
                    if let Some(tx) = WORKER_TX.get() {
                        if tx
                            .send(WorkerCmd::CreatePoll {
                                owner: self.user_uin.clone(),
                                runtime_id: self.account_runtime_id,
                                request_id: req_id,
                                thread_id,
                                question: question.clone(),
                                options,
                                multi: false,
                                anonymous: false,
                                allow_add: false,
                            })
                            .is_ok()
                        {
                            sent = true;
                        }
                    }
                    if !sent {
                        self.poll_operations.remove(&req_id);
                        self.poll_intent_lock.remove(&intent_key);
                        self.poll_pending_intents.remove(&intent_key);
                        self.status_msg =
                            "Không thể gửi lệnh tạo bình chọn: worker chưa sẵn sàng".into();
                    } else {
                        self.tools.new_poll_question.clear();
                        self.tools.new_poll_option_1.clear();
                        self.tools.new_poll_option_2.clear();
                        self.status_msg = format!("Đang gửi lệnh tạo bình chọn: {question}...");
                    }
                }
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

        let current_tid = self.current_thread().map(|t| t.id.clone());
        if let Some(ref tid) = current_tid {
            let indeterminate_keys: Vec<(String, u64, String)> = self
                .poll_operations
                .iter()
                .filter(|(_, op)| {
                    op.thread_id == *tid
                        && matches!(op.phase, crate::gui::PollOpPhase::Indeterminate { .. })
                })
                .map(|(&req_id, op)| {
                    let err = match &op.phase {
                        crate::gui::PollOpPhase::Indeterminate { error } => error.clone(),
                        _ => String::new(),
                    };
                    (op.question.clone(), req_id, err)
                })
                .collect();
            for (q, req_id, err) in indeterminate_keys {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("⚠️ Bình chọn '{q}': chưa rõ kết quả ({err})"))
                            .size(11.0)
                            .color(Color32::from_rgb(220, 100, 0)),
                    );
                    if ui
                        .small_button("Hủy khóa")
                        .on_hover_text("Bỏ khóa bình chọn này để cho phép tạo lại")
                        .clicked()
                    {
                        self.poll_intent_lock.remove(&(tid.clone(), q.clone()));
                        if let Some(op) = self.poll_operations.get_mut(&req_id) {
                            op.phase = crate::gui::PollOpPhase::Rejected {
                                error: "Người dùng hủy khóa".into(),
                            };
                        }
                    }
                });
            }
        }

        if !self.tools.group_polls.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new("Các cuộc bình chọn hiện tại:").strong());
            for poll in &self.tools.group_polls {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&poll.question)
                                .strong()
                                .color(Color32::from_rgb(0, 120, 215)),
                        );
                        ui.label(
                            RichText::new(format!("Tổng số phiếu: {}", poll.total_votes))
                                .size(11.0),
                        );
                        for opt in &poll.options {
                            ui.horizontal(|ui| {
                                ui.label(format!("• {} ({} phiếu)", opt.text, opt.vote_count));
                            });
                        }
                    });
                });
            }
        }
    }

    fn render_tools_quick_messages(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("⚡ Tin nhắn nhanh & Phím tắt gõ nhanh (Zalo Parity B07)")
                .strong()
                .size(14.0),
        );
        ui.label(
            "Gõ phím tắt (bắt đầu bằng '/') trong khung chat để gợi ý và chèn nhanh nội dung.",
        );
        ui.add_space(4.0);

        // Server sync and local migration buttons
        ui.horizontal(|ui| {
            let loading = self.quick_message_in_flight_list_id.is_some();
            if ui
                .add_enabled(!loading, egui::Button::new("🔄 Tải lại từ máy chủ"))
                .clicked()
                && self.is_logged_in
            {
                self.trigger_quick_messages_fetch();
            }
            if loading {
                ui.label(RichText::new("Đang tải…").size(11.0));
            } else if let Some(err) = &self.quick_message_list_error {
                ui.label(
                    RichText::new(format!("Tải thất bại: {err}"))
                        .size(11.0)
                        .color(Color32::from_rgb(200, 90, 40)),
                );
            }
            let unmigrated_count = self.tools.config.quick_messages.unmigrated_items().len();
            if unmigrated_count > 0
                && self.is_logged_in
                && ui
                    .button(
                        RichText::new(format!("☁️ Đồng bộ {unmigrated_count} mục lên máy chủ"))
                            .strong()
                            .color(Color32::from_rgb(0, 120, 215)),
                    )
                    .clicked()
            {
                if let Some(tx) = WORKER_TX.get() {
                    for qm in self.tools.config.quick_messages.unmigrated_items() {
                        let _ = tx.send(WorkerCmd::CreateQuickMessage {
                            owner: self.user_uin.clone(),
                            runtime_id: self.account_runtime_id,
                            request_id: crate::search::generation(),
                            keyword: qm.shortcut.clone(),
                            message: qm.text.clone(),
                            media: qm.media.clone(),
                            msg_type: qm.msg_type,
                        });
                    }
                }
            }
        });
        ui.add_space(6.0);

        let mut remove_item = None;
        for qm in self.tools.config.quick_messages.items.iter() {
            let deleting = !qm.id.is_empty() && self.quick_message_delete_pending(&qm.id);
            let unknown = !qm.id.is_empty() && self.quick_message_delete_unknown.contains(&qm.id);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&qm.shortcut)
                            .strong()
                            .color(Color32::from_rgb(0, 120, 215)),
                    );
                    ui.label(format!("({})", qm.title));
                    if qm.id.is_empty() {
                        ui.label(
                            RichText::new("[Cục bộ]")
                                .size(10.5)
                                .color(Color32::from_rgb(180, 140, 40)),
                        );
                    }
                    if deleting {
                        ui.label(
                            RichText::new("Đang xóa trên máy chủ…")
                                .size(10.5)
                                .color(Color32::from_rgb(180, 140, 40)),
                        );
                    } else if unknown {
                        ui.label(
                            RichText::new("Chưa xác định kết quả xóa")
                                .size(10.5)
                                .color(Color32::from_rgb(200, 90, 40)),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if unknown { "Xoá lại" } else { "Xoá" };
                        if ui
                            .add_enabled(!deleting, egui::Button::new(label).small())
                            .clicked()
                        {
                            remove_item = Some(qm.clone());
                        }
                    });
                });
                ui.label(RichText::new(&qm.text).size(12.5));
            });
        }
        if let Some(item) = remove_item {
            self.request_quick_message_delete(&item);
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
            let sc = self.tools.new_quick_shortcut.trim().to_string();
            let title = self.tools.new_quick_title.trim().to_string();
            let text = self.tools.new_quick_text.trim().to_string();
            if self.add_quick_message_local(&sc, &title, &text) {
                self.tools.new_quick_shortcut.clear();
                self.tools.new_quick_title.clear();
                self.tools.new_quick_text.clear();
            }
        }
    }

    /// Local create/update of a quick message (also queues the server create when logged in).
    /// Returns false when the input is empty. Changes confirmed state, so the revision is bumped
    /// before the config is persisted.
    pub(super) fn add_quick_message_local(&mut self, sc: &str, title: &str, text: &str) -> bool {
        if sc.is_empty() || text.is_empty() {
            return false;
        }
        let shortcut = if sc.starts_with('/') {
            sc.to_string()
        } else {
            format!("/{sc}")
        };
        if self.is_logged_in {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::CreateQuickMessage {
                    owner: self.user_uin.clone(),
                    runtime_id: self.account_runtime_id,
                    request_id: crate::search::generation(),
                    keyword: shortcut.clone(),
                    message: text.to_string(),
                    media: None,
                    msg_type: 0,
                });
            }
        }
        self.tools
            .config
            .quick_messages
            .upsert(shortcut, title.into(), text.into());
        // A local edit changes confirmed Quick Messages state: older cache loads are stale.
        self.bump_quick_message_revision();
        self.tools_command(ToolsCommand::Save(self.tools.config.clone()));
        true
    }

    fn render_tools_business_lan(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("🏢 Danh mục doanh nghiệp")
                .strong()
                .size(14.0),
        );
        ui.label("Danh mục sản phẩm OA/Doanh nghiệp.");
        ui.label(
            RichText::new("ℹ️ Tính năng Danh mục sản phẩm yêu cầu tài khoản Official Account (OA) hoặc gói Business đã đăng ký với Zalo.")
                .size(11.5)
                .color(Color32::from_rgb(180, 180, 190)),
        );
        ui.add_space(6.0);

        ui.label("Danh mục chưa khả dụng: đang xác minh dịch vụ Zalo PC.");
        ui.add_enabled_ui(false, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📋 Tải Danh mục sản phẩm").clicked() {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::GetBusinessCatalog);
                    }
                }
            });
        });
        ui.separator();
        ui.label("Truyền tệp LAN chưa khả dụng: chưa có giao thức Zalo được xác minh.");

        ui.separator();
        ui.label(RichText::new("🔍 Trích xuất văn bản từ hình ảnh (Local OCR)").strong());
        let ocr_status = crate::ocr_windows::get_ocr_capability_status();
        let status_desc = match ocr_status {
            crate::ocr_windows::OcrCapabilityStatus::Ready => "Sẵn sàng (WinRT Media OCR)",
            crate::ocr_windows::OcrCapabilityStatus::ExperimentalPreprocessOnly => {
                "Tiền xử lý hình ảnh cục bộ (Giới hạn 1280px, chuẩn hóa tương phản)"
            }
        };
        ui.label(format!("Trạng thái OCR: {status_desc}"));
    }

    /// Dispatch a server media fetch request for ToolsPanel.
    ///
    /// Preserves the confirmed snapshot across in-flight reloads for the same category/thread,
    /// tracking pending request metadata independently.
    /// Returns `true` if the request was successfully enqueued to the worker.
    pub fn dispatch_tools_server_media_request(
        &mut self,
        thread_id: &str,
        media_type: u8,
        mode: ServerMediaFetchMode,
        sender: Option<&tokio::sync::mpsc::UnboundedSender<WorkerCmd>>,
    ) -> bool {
        if thread_id.is_empty() || self.tools.server_media_loading {
            return false;
        }

        let is_same_scope = self.tools.server_media_thread_id == thread_id
            && self.tools.server_media_type == media_type;

        // When switching thread or category on reload, reset the confirmed snapshot
        // so items from previous scope are not mislabeled.
        if mode == ServerMediaFetchMode::Reload && !is_same_scope {
            self.tools.server_media_items.clear();
            self.tools.server_media_page = 0;
            self.tools.server_media_has_more = false;
            self.tools.server_media_next_cursor = None;
            self.tools.server_media_thread_id = thread_id.to_string();
            self.tools.server_media_type = media_type;
        }

        let (page, cursor) = match mode {
            ServerMediaFetchMode::Reload => (1, None),
            ServerMediaFetchMode::Append => {
                if !self.tools.server_media_has_more
                    || self.tools.server_media_next_cursor.is_none()
                {
                    return false;
                }
                (
                    self.tools.server_media_page + 1,
                    self.tools.server_media_next_cursor.clone(),
                )
            }
        };

        let gen = self.tools.server_media_generation.wrapping_add(1);
        let cmd = WorkerCmd::FetchServerMedia {
            runtime_id: self.account_runtime_id,
            thread_id: thread_id.to_string(),
            media_type,
            page,
            count: 30,
            cursor: cursor.clone(),
            generation: gen,
            target: crate::server_media::ServerMediaTarget::ToolsPanel,
        };

        let sent = if let Some(tx) = sender {
            tx.send(cmd).is_ok()
        } else if let Some(tx) = WORKER_TX.get() {
            tx.send(cmd).is_ok()
        } else {
            false
        };

        if sent {
            self.tools.server_media_generation = gen;
            self.tools.server_media_loading = true;
            self.tools.server_media_pending = Some(PendingServerMediaRequest {
                runtime_id: self.account_runtime_id,
                thread_id: thread_id.to_string(),
                media_type,
                generation: gen,
                mode,
                requested_page: page,
                requested_cursor: cursor,
            });
            true
        } else {
            self.tools.server_media_loading = false;
            self.tools.server_media_pending = None;
            self.status_msg = "Không thể gửi yêu cầu tải media: worker chưa sẵn sàng".into();
            false
        }
    }

    fn render_tools_server_media_zcloud(&mut self, ui: &mut egui::Ui) {
        // Finding 4: Stale Server Media State Across Thread Switches
        // When active thread differs from self.tools.server_media_thread_id while non-empty,
        // automatically reset the server media view state for the new thread.
        let current_thread_id = self
            .threads
            .get(self.selected_thread_index)
            .map(|t| t.id.as_str())
            .unwrap_or("");
        if !self.tools.server_media_thread_id.is_empty()
            && self.tools.server_media_thread_id != current_thread_id
        {
            self.tools.server_media_thread_id = current_thread_id.to_string();
            self.tools.server_media_items.clear();
            self.tools.server_media_page = 0;
            self.tools.server_media_has_more = false;
            self.tools.server_media_next_cursor = None;
            self.tools.server_media_loading = false;
            self.tools.server_media_pending = None;
            self.tools.server_media_generation = self.tools.server_media_generation.wrapping_add(1);
        }

        ui.label(
            RichText::new("📦 Kho Lưu Trữ Media Server & zCloud (Zalo Parity A20 / B05)")
                .strong()
                .size(14.0),
        );
        ui.label("Truy xuất thư viện hình ảnh, video, tệp tin và liên kết từ máy chủ Zalo cùng dữ liệu lưu trữ đám mây zCloud.");
        ui.add_space(6.0);

        let is_loading = self.tools.server_media_loading;

        ui.horizontal(|ui| {
            if ui
                .add_enabled(!is_loading, egui::Button::new("🖼️ Tải Ảnh & Video"))
                .clicked()
            {
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    let tid = thread.id.clone();
                    self.dispatch_tools_server_media_request(
                        &tid,
                        1,
                        ServerMediaFetchMode::Reload,
                        None,
                    );
                }
            }
            if ui
                .add_enabled(!is_loading, egui::Button::new("📁 Tải Tệp tin"))
                .clicked()
            {
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    let tid = thread.id.clone();
                    self.dispatch_tools_server_media_request(
                        &tid,
                        3,
                        ServerMediaFetchMode::Reload,
                        None,
                    );
                }
            }
            if ui
                .add_enabled(!is_loading, egui::Button::new("🔗 Tải Liên kết"))
                .clicked()
            {
                if let Some(thread) = self.threads.get(self.selected_thread_index) {
                    let tid = thread.id.clone();
                    self.dispatch_tools_server_media_request(
                        &tid,
                        4,
                        ServerMediaFetchMode::Reload,
                        None,
                    );
                }
            }
            if is_loading {
                ui.spinner();
                ui.label(RichText::new("Đang tải dữ liệu…").size(12.0));
            }
        });

        ui.separator();
        ui.label(RichText::new("☁️ Khôi phục & Mở tệp zCloud (A20):").strong());
        ui.label(
            RichText::new("⚠️ Thử nghiệm (Experimental): Xác thực quyền/metadata từ server; chưa giải mã trực tiếp dữ liệu đám mây.")
                .size(11.0)
                .color(Color32::from_rgb(200, 150, 50)),
        );
        ui.label("Nhập mã file_id để kiểm tra trạng thái quyền xem trước:");
        ui.horizontal(|ui| {
            ui.label("File ID:");
            ui.text_edit_singleline(&mut self.tools.query);
            if ui.button("🔑 Lấy khoá zCloud").clicked() && !self.tools.query.trim().is_empty()
            {
                let fid = self.tools.query.trim().to_string();
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::GetCloudViewerKey { file_id: fid });
                }
            }
        });

        if !self.tools.cloud_viewer_keys.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Trạng thái quyền xem tệp zCloud đã xác thực:").strong());
            for (fid, k) in &self.tools.cloud_viewer_keys {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("Tệp: {fid}")).strong());
                        let exp_str = if k.expires_at > 0 {
                            crate::clock::local_datetime(k.expires_at)
                                .map(|dt| dt.format("%d/%m/%Y %H:%M").to_string())
                                .unwrap_or_else(|| k.expires_at.to_string())
                        } else {
                            "Không thời hạn".to_string()
                        };
                        let auth_status = if !k.viewer_key.is_empty() {
                            "Đã xác thực trên máy chủ (Khóa an toàn trong bộ nhớ worker)"
                        } else {
                            "Chưa có quyền"
                        };
                        ui.label(
                            RichText::new(format!("Quyền xem: {auth_status} • Hết hạn: {exp_str}"))
                                .size(11.0),
                        );
                    });
                });
            }
        }

        if !self.tools.server_media_items.is_empty() {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Thư viện media từ máy chủ (Trang {}, {} mục):",
                        self.tools.server_media_page,
                        self.tools.server_media_items.len()
                    ))
                    .strong(),
                );
            });
            for item in &self.tools.server_media_items {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let icon = match item.media_type {
                            crate::server_media::ServerMediaType::Photo => "🖼️",
                            crate::server_media::ServerMediaType::Video => "🎬",
                            crate::server_media::ServerMediaType::File => "📁",
                            crate::server_media::ServerMediaType::Link => "🔗",
                        };
                        ui.label(icon);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&item.file_name).strong());
                            ui.label(
                                RichText::new(format!(
                                    "Kích thước: {} bytes • Người gửi: {}",
                                    item.file_size, item.sender_id
                                ))
                                .size(11.0),
                            );
                            if !item.url.is_empty() {
                                ui.horizontal(|ui| {
                                    if ui.button("🌐 Mở liên kết").clicked() {
                                        crate::platform::open_url(&item.url);
                                    }
                                    if ui.button("📋 Sao chép URL").clicked() {
                                        ui.output_mut(|output| {
                                            output.copied_text = item.url.clone()
                                        });
                                    }
                                });
                            }
                        });
                    });
                });
            }

            if self.tools.server_media_has_more {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!is_loading, egui::Button::new("⬇️ Tải thêm"))
                        .clicked()
                    {
                        if let Some(thread) = self.threads.get(self.selected_thread_index) {
                            let tid = thread.id.clone();
                            let media_type = self.tools.server_media_type;
                            self.dispatch_tools_server_media_request(
                                &tid,
                                media_type,
                                ServerMediaFetchMode::Append,
                                None,
                            );
                        }
                    }
                    if is_loading {
                        ui.spinner();
                    }
                });
            }
        }
    }

    fn render_tools_devices(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("🔒 Thiết bị đăng nhập & Bảo mật")
                .strong()
                .size(14.0),
        );
        ui.label("Các thông tin bảo mật từ máy chủ chưa được xác minh.");
        ui.add_space(6.0);

        ui.label("Thiết bị và 2FA: chưa xác định. Các thao tác này chưa khả dụng.");
        ui.add_enabled_ui(false, |ui| {
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

            ui.label("Chi tiết thiết bị và trạng thái 2FA hiện chưa thể xác minh.");
        });
    }
}
/// Background body of "open diagnostics folder"; every path ends in an explicit outcome.
#[cfg(test)]
fn open_diagnostics_dir(
    dir: &std::path::Path,
    open: impl FnOnce(&std::path::Path) -> bool,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|_| "Không tạo được thư mục chẩn đoán".to_owned())?;
    if open(dir) {
        Ok(())
    } else {
        Err("Không mở được thư mục chẩn đoán".to_owned())
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

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ctdoz-diag-{tag}-{}",
            crate::param_cipher::random_hex(12)
        ))
    }

    #[test]
    fn diagnostics_open_creates_missing_dir_then_reports_success() {
        let dir = temp_dir("ok").join("nested thư mục");
        let mut opened = None;
        let result = open_diagnostics_dir(&dir, |path| {
            opened = Some(path.to_path_buf());
            true
        });
        assert_eq!(result, Ok(()));
        assert!(dir.is_dir());
        assert_eq!(opened.as_deref(), Some(dir.as_path()));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn diagnostics_open_reports_shell_failure_instead_of_silent_success() {
        let dir = temp_dir("shell");
        let result = open_diagnostics_dir(&dir, |_| false);
        assert_eq!(result, Err("Không mở được thư mục chẩn đoán".to_owned()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn diagnostics_open_reports_create_failure_without_calling_shell() {
        let file = temp_dir("file");
        std::fs::write(&file, b"x").unwrap();
        let mut called = false;
        let result = open_diagnostics_dir(&file.join("child"), |_| {
            called = true;
            true
        });
        assert_eq!(result, Err("Không tạo được thư mục chẩn đoán".to_owned()));
        assert!(!called);
        let _ = std::fs::remove_file(&file);
    }

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
        assert!(tools.new_poll_question.is_empty());
    }
}
