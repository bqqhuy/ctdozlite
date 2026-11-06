//! Invitation box and join-by-link preview; acknowledgement is not established membership.
use super::*;
#[derive(Default)]
pub(super) struct GroupEntryPanel {
    pub open: bool,
    pub pending: bool,
    pub uncertain: bool,
    pub generation: u64,
    pub status: String,
    pub page: Option<crate::group_entry::Page>,
    pub link: String,
    pub preview: Option<crate::group_entry::Preview>,
    pub preview_link: String,
    pub block_group: bool,
    pub confirm: Option<crate::group_entry::Action>,
}
impl ZaloGuiApp {
    pub(super) fn open_group_entry(&mut self) {
        self.group_entry_panel.open = true;
        if !self.group_entry_panel.pending {
            self.group_entry_command(crate::group_entry::Action::Invitations {
                page: 0,
                last: None,
            });
        }
    }
    fn group_entry_command(&mut self, action: crate::group_entry::Action) {
        if self.group_entry_panel.pending {
            return;
        }
        self.group_entry_panel.pending = true;
        self.group_entry_panel.generation = crate::search::generation();
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::GroupEntry {
                owner: self.user_uin.clone(),
                generation: self.group_entry_panel.generation,
                action,
            })
            .is_ok()
        }) {
            self.group_entry_panel.pending = false;
            self.group_entry_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn render_group_entry(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.group_entry_panel = Default::default();
            return;
        }
        if !self.group_entry_panel.open {
            return;
        }
        let mut open = true;
        let mut intent = None;
        egui::Window::new("Tham gia nhóm").id(Id::new("group-entry")).open(&mut open)
            .vscroll(true)
            .min_width(120.0)
            .max_width((ctx.screen_rect().width()-32.0).max(180.0))
            .max_height((ctx.screen_rect().height()-64.0).max(100.0)).default_width(500.0_f32.min((ctx.screen_rect().width()-48.0).max(160.0))).show(ctx,|ui| {
                ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
            ui.label(&self.group_entry_panel.status);if self.group_entry_panel.pending{ui.spinner();}
            ui.add_enabled_ui(!self.group_entry_panel.pending,|ui| {
                if ui.button("Tải lại lời mời").clicked(){intent=Some(crate::group_entry::Action::Invitations {page:0,last:None});}
                if let Some(page)=&self.group_entry_panel.page {
                    ui.label(format!("Trang {} • {} lời mời trên máy chủ",page.page+1,page.total));
                    egui::ScrollArea::vertical().max_height(230.0).show(ui,|ui| {
                        for invitation in &page.invitations {
                            ui.push_id(&invitation.group,|ui| {
                                ui.label(format!("{} • {}",invitation.name,invitation.group));
                                let expired=invitation.expires<=crate::login::unix_now()*1000;
                                if invitation.pending {ui.label("Đang chờ duyệt");}else if expired {ui.label("Lời mời đã hết hạn");}
                                ui.add_enabled_ui(!expired && !invitation.pending && !self.group_entry_panel.uncertain,|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Tham gia").clicked(){self.group_entry_panel.confirm=Some(crate::group_entry::Action::JoinInvitation {group:invitation.group.clone()});}
                                        if ui.button("Từ chối lời mời").clicked(){self.group_entry_panel.confirm=Some(crate::group_entry::Action::RejectInvitation {group:invitation.group.clone(),block_group:self.group_entry_panel.block_group});}
                                    });
                                });ui.separator();
                            });
                        }
                    });
                    ui.checkbox(&mut self.group_entry_panel.block_group,"Kèm chặn lời mời từ nhóm khi từ chối (không chặn người mời)");
                    if page.more && ui.button("Trang lời mời tiếp").clicked(){intent=Some(crate::group_entry::Action::Invitations {page:page.page+1,last:page.last.clone()});}
                }
                ui.separator();ui.label("Link mời nhóm");
                if ui.add(egui::TextEdit::singleline(&mut self.group_entry_panel.link).desired_width(ui.available_width())).changed(){self.group_entry_panel.preview=None;self.group_entry_panel.preview_link.clear();self.group_entry_panel.confirm=None;}
                if ui.button("Xem thông tin nhóm từ link").clicked(){self.group_entry_panel.preview_link=self.group_entry_panel.link.clone();intent=Some(crate::group_entry::Action::LinkInfo {link:self.group_entry_panel.link.clone()});}
                if let Some(preview)=&self.group_entry_panel.preview {
                    ui.label(format!("{} • {}",preview.name,preview.group));if let Some(count)=preview.members{ui.label(format!("{count} thành viên"));}
                    if !self.group_entry_panel.uncertain && self.group_entry_panel.link==self.group_entry_panel.preview_link && ui.button("Yêu cầu tham gia nhóm qua link").clicked(){self.group_entry_panel.confirm=Some(crate::group_entry::Action::JoinLink {link:self.group_entry_panel.link.clone()});}
                }
            });
            if let Some(action)=self.group_entry_panel.confirm.clone() {
                ui.separator();ui.label(match &action {crate::group_entry::Action::RejectInvitation {block_group:true,..}=>"Từ chối và chặn lời mời từ nhóm này? Người mời không bị chặn.",crate::group_entry::Action::RejectInvitation {..}=>"Từ chối lời mời nhóm này?",_=>"Gửi yêu cầu tham gia nhóm? Có thể cần quản trị viên duyệt."});
                if ui.add_enabled(!self.group_entry_panel.pending,egui::Button::new("Xác nhận")).clicked(){intent=Some(action);self.group_entry_panel.confirm=None;}
                if ui.button("Hủy").clicked(){self.group_entry_panel.confirm=None;}
            }
            if self.group_entry_panel.uncertain{ui.label("Chưa rõ kết quả yêu cầu. Tải lại lời mời hoặc kiểm tra trên Zalo chính thức trước khi thử tiếp.");}
        });
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            open = false;
        }
        self.group_entry_panel.open = open;
        if let Some(action) = intent {
            self.group_entry_command(action);
        }
    }
}
