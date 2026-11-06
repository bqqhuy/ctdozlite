//! Account-scoped contact operations with explicit confirmation and uncertain-ACK handling.
use super::*;
#[derive(Default)]
pub(super) struct ContactPanel {
    pub open: bool,
    pub group_report: bool,
    pub lookup_mode: bool,
    pub username: String,
    pub lookup: Option<crate::contact_actions::Lookup>,
    pub close_friends: Vec<crate::contact_actions::Lookup>,
    pub target: String,
    pub generation: u64,
    pub pending: bool,
    pub uncertain: bool,
    pub alias: String,
    pub report: String,
    pub qr: Option<String>,
    pub qr_requested: bool,
    pub presence_checked: Option<std::time::Instant>,
    pub status: String,
    pub presence: Option<crate::last_activity::Status>,
    pub confirm: Option<crate::contact_actions::Action>,
    pub blocked: Vec<(String, String)>,
    pub blocked_page: u32,
    pub blocked_more: bool,
}
impl ZaloGuiApp {
    pub(super) fn open_username_lookup(&mut self) {
        if self.contact_panel.pending {
            self.show_toast("Chờ thao tác liên hệ trước hoàn tất");
            return;
        }
        self.contact_panel = ContactPanel {
            open: true,
            lookup_mode: true,
            ..Default::default()
        };
    }
    pub(super) fn open_group_report(&mut self, target: String) {
        if self.contact_panel.pending {
            self.show_toast("Chờ thao tác liên hệ trước hoàn tất");
            return;
        }
        self.contact_panel = ContactPanel {
            open: true,
            target,
            group_report: true,
            ..Default::default()
        };
    }
    pub(super) fn open_contact_panel(&mut self, target: String) {
        if self.contact_panel.pending {
            self.show_toast("Đang xử lý liên hệ; chờ kết quả trước khi chuyển");
            return;
        }
        self.contact_panel = ContactPanel {
            open: true,
            target,
            ..Default::default()
        };
        self.contact_command(crate::contact_actions::Action::Inspect);
    }
    pub(super) fn open_blocklist(&mut self) {
        if self.contact_panel.pending {
            self.show_toast("Đang xử lý liên hệ; chờ kết quả trước khi mở danh sách chặn");
            return;
        }
        self.contact_panel = ContactPanel {
            open: true,
            ..Default::default()
        };
        self.contact_command(crate::contact_actions::Action::Blocked { page: 1 });
    }
    fn contact_command(&mut self, action: crate::contact_actions::Action) {
        if self.contact_panel.pending {
            return;
        }
        self.contact_panel.generation = crate::search::generation();
        self.contact_panel.pending = true;
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Contact {
                owner: self.user_uin.clone(),
                target: self.contact_panel.target.clone(),
                generation: self.contact_panel.generation,
                action,
            })
            .is_ok()
        }) {
            self.contact_panel.pending = false;
            self.contact_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn render_contact_panel(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.contact_panel = Default::default();
            return;
        }
        if !self.contact_panel.open {
            return;
        }
        let mut open = true;
        let mut intent = None;
        let mut selected_contact = None;
        if !self.contact_panel.target.is_empty()
            && !self.contact_panel.pending
            && self
                .contact_panel
                .presence_checked
                .is_some_and(|at| at.elapsed() > Duration::from_secs(120))
        {
            if let Some(official) = self
                .threads
                .iter()
                .find(|thread| thread.id == self.contact_panel.target && !thread.is_group)
                .and_then(|thread| thread.presentation.inbox.official_account)
            {
                intent = Some(crate::contact_actions::Action::Presence { official });
                self.contact_panel.presence_checked = Some(std::time::Instant::now());
            }
        }
        egui::Window::new("Liên hệ và quyền riêng tư").id(Id::new("contact-actions")).open(&mut open)
            .vscroll(true)
            .min_width(120.0)
            .max_width((ctx.screen_rect().width()-32.0).max(180.0))
            .max_height((ctx.screen_rect().height()-64.0).max(100.0)).default_width(420.0_f32.min((ctx.screen_rect().width()-48.0).max(160.0))).show(ctx,|ui| {
                ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
            if self.contact_panel.lookup_mode {
                ui.label("Tra cứu tên đăng nhập Zalo (username), khác tên hiển thị và số điện thoại.");
                ui.add(egui::TextEdit::singleline(&mut self.contact_panel.username).char_limit(128).desired_width(ui.available_width()));
                if ui.add_enabled(!self.contact_panel.pending,egui::Button::new("Tra cứu username")).clicked(){intent=Some(crate::contact_actions::Action::LookupUsername {username:self.contact_panel.username.clone()});}
                if ui.add_enabled(!self.contact_panel.pending,egui::Button::new("Tải danh sách bạn thân từ máy chủ")).clicked(){intent=Some(crate::contact_actions::Action::CloseFriends);}
                ui.label(&self.contact_panel.status);
                if let Some(profile)=&self.contact_panel.lookup {ui.label(format!("{} • UID {}",profile.name,profile.uid));if ui.button("Mở thông tin liên hệ này").clicked(){selected_contact=Some(profile.uid.clone());}}
                egui::ScrollArea::vertical().id_salt("close-friends-read").max_height(220.0).show_rows(ui,24.0,self.contact_panel.close_friends.len(),|ui,range| {for index in range {let profile=&self.contact_panel.close_friends[index];if ui.add_sized([ui.available_width(),24.0],egui::Button::new(egui::RichText::new(&profile.name))).on_hover_text(format!("UID {}",profile.uid)).clicked(){selected_contact=Some(profile.uid.clone());}}});
                if self.contact_panel.pending {ui.spinner();}return;
            }
            ui.label(format!("UID: {}",self.contact_panel.target));
            ui.label(&self.contact_panel.status);
            if self.contact_panel.pending { ui.spinner(); }
            if self.contact_panel.target.is_empty() {
                ui.add_enabled_ui(!self.contact_panel.pending,|ui| {
                    if ui.button("Tải lại danh sách chặn").clicked(){intent=Some(crate::contact_actions::Action::Blocked {page:1});}
                    egui::ScrollArea::vertical().max_height(350.0).show(ui,|ui| {for (uid,name) in &self.contact_panel.blocked {if ui.button(format!("{name} • {uid}")).clicked(){selected_contact=Some(uid.clone());}}});
                    if self.contact_panel.blocked_more && self.contact_panel.blocked_page<400 && ui.button("Trang chặn tiếp").clicked(){intent=Some(crate::contact_actions::Action::Blocked {page:self.contact_panel.blocked_page+1});}
                });
                return;
            }
            if ui.add_enabled(!self.contact_panel.pending,egui::Button::new("Tải lại trạng thái kết bạn")).clicked() { intent=Some(crate::contact_actions::Action::Inspect); }
            if let Some(official)=self.threads.iter().find(|thread|thread.id==self.contact_panel.target && !thread.is_group).and_then(|thread|thread.presentation.inbox.official_account) {
                if ui.add_enabled(!self.contact_panel.pending,egui::Button::new("Kiểm tra hoạt động gần nhất")).clicked(){intent=Some(crate::contact_actions::Action::Presence {official});}
            }
            if !self.contact_panel.group_report && ui.add_enabled(!self.contact_panel.pending,egui::Button::new("Tải mã QR liên hệ")).clicked(){intent=Some(crate::contact_actions::Action::Qr);}
            if let Some(url)=&self.contact_panel.qr {
                let key=format!("contact-qr:{}:{}",self.user_uin,self.contact_panel.target);
                if let Some(texture)=self.sticker_textures.get(&key){ui.image((texture.id(),Vec2::splat(ui.available_width().min(200.0))));}
                else if !self.contact_panel.qr_requested && !self.loading_stickers.contains(&key) {if let Some(tx)=WORKER_TX.get(){self.contact_panel.qr_requested=true;self.loading_stickers.insert(key.clone());let _=tx.send(WorkerCmd::FetchStickerImage {key,url:url.clone()});}}
            }
            if let Some(status)=&self.contact_panel.presence {
                ui.label(match status {
                    crate::last_activity::Status::PrivacyUnavailable=>"Không truy vấn khi quyền hiển thị hoạt động đang tắt hoặc chưa xác định".to_owned(),
                    crate::last_activity::Status::Unavailable=>"Máy chủ chưa cung cấp thời gian hoạt động".to_owned(),
                    crate::last_activity::Status::LastActivity(timestamp)=>chrono::DateTime::from_timestamp_millis(*timestamp).map(|time|format!("Hoạt động gần nhất: {}",time.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M"))).unwrap_or_else(||"Thời gian hoạt động chưa xác định".into()),
                });
            }
            ui.add_enabled_ui(!self.contact_panel.pending && !self.contact_panel.uncertain,|ui| {
                if !self.contact_panel.group_report {ui.label("Biệt danh đồng bộ máy chủ (để trống để xóa)");
                ui.add(egui::TextEdit::singleline(&mut self.contact_panel.alias).desired_width(ui.available_width()));
                if ui.button("Lưu biệt danh").clicked() { self.contact_panel.confirm=Some(crate::contact_actions::Action::Alias(self.contact_panel.alias.clone())); }}
                ui.collapsing("Báo cáo tới Zalo",|ui| {
                    ui.label("Lý do khác: mô tả nội dung báo cáo (tối đa 1000 ký tự)");
                    ui.add(egui::TextEdit::multiline(&mut self.contact_panel.report).desired_width(ui.available_width()));
                    if ui.button("Xem lại báo cáo").clicked(){self.contact_panel.confirm=Some(if self.contact_panel.group_report {crate::contact_actions::Action::ReportGroupOther {description:self.contact_panel.report.clone()}}else{crate::contact_actions::Action::ReportOther {description:self.contact_panel.report.clone()}});}
                });
                if !self.contact_panel.group_report {ui.horizontal_wrapped(|ui| {
                    for (label,block) in [("Chặn tin nhắn",true),("Bỏ chặn tin nhắn",false)] {
                        if ui.button(label).clicked() { self.contact_panel.confirm=Some(crate::contact_actions::Action::Block(block)); }
                    }
                    if ui.add_enabled(self.friend_status_map.get(&self.contact_panel.target)==Some(&crate::chat::FriendshipStatus::Friend),egui::Button::new("Xóa khỏi bạn bè")).clicked() { self.contact_panel.confirm=Some(crate::contact_actions::Action::Remove); }
                });}
            });
            if let Some(action)=self.contact_panel.confirm.clone() {
                ui.separator();
                ui.label(match &action {
                    crate::contact_actions::Action::Block(true)=>"Chặn tin nhắn từ liên hệ này?",
                    crate::contact_actions::Action::Block(false)=>"Bỏ chặn tin nhắn từ liên hệ này?",
                    crate::contact_actions::Action::ReportGroupOther {..}=>"Gửi báo cáo này tới Zalo về nhóm đang chọn? Không tự rời nhóm hay xóa lịch sử.",
                    crate::contact_actions::Action::ReportOther {..}=>"Gửi nội dung báo cáo này tới Zalo về tài khoản đang chọn?",
                    crate::contact_actions::Action::Remove=>"Xóa khỏi danh sách bạn bè? Lịch sử cục bộ được giữ nguyên.",
                    _=>"Lưu thay đổi biệt danh trên máy chủ?",
                });
                if let crate::contact_actions::Action::ReportOther {description}|crate::contact_actions::Action::ReportGroupOther {description}=&action{ui.label(description);}
                if ui.add_enabled(!self.contact_panel.pending,egui::Button::new("Xác nhận")).clicked() { intent=Some(action);self.contact_panel.confirm=None; }
                if ui.button("Hủy").clicked() { self.contact_panel.confirm=None; }
            }
            if self.contact_panel.uncertain { ui.label("Chưa rõ máy chủ đã áp dụng hay chưa. Kiểm tra trên Zalo chính thức trước khi thử lại; mở lại bảng này sau khi đã kiểm tra."); }
        });
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) && !self.contact_panel.pending {
            open = false;
        }
        self.contact_panel.open = open;
        if let Some(action) = intent {
            self.contact_command(action);
        }
        if let Some(target) = selected_contact {
            self.open_contact_panel(target);
        }
    }
}

impl ZaloGuiApp {
    pub(super) fn archive_command(&mut self, target: String, group: bool, archived: Option<bool>) {
        if self.archive_pending || (archived.is_some() && self.archive_uncertain) {
            return;
        }
        self.archive_generation = crate::search::generation();
        self.archive_pending = true;
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Archive {
                owner: self.user_uin.clone(),
                generation: self.archive_generation,
                target,
                group,
                archived,
            })
            .is_ok()
        }) {
            self.archive_pending = false;
            self.show_toast("Worker chưa sẵn sàng");
        }
    }
}

impl ZaloGuiApp {
    pub(super) fn manual_mark_command(
        &mut self,
        target: String,
        group: bool,
        action: crate::unread::MarkAction,
    ) {
        if self.manual_mark_pending
            || (self.manual_mark_uncertain && !matches!(action, crate::unread::MarkAction::Refresh))
        {
            return;
        }
        self.manual_mark_generation = crate::search::generation();
        self.manual_mark_pending = true;
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::ManualMark {
                owner: self.user_uin.clone(),
                generation: self.manual_mark_generation,
                target,
                group,
                action,
            })
            .is_ok()
        }) {
            self.manual_mark_pending = false;
            self.show_toast("Worker chưa sẵn sàng");
        }
    }
}
