//! Explicit server policy editor; changes require acknowledgement and a fresh snapshot.
use super::*;
#[derive(Default)]
pub(super) struct PolicyPanel {
    pub target: Option<(String, bool)>,
    pub generation: u64,
    pub pending: bool,
    pub status: String,
    pub ttl: i64,
    pub confirm: Option<crate::conversation_policy::Action>,
    pub snapshot: Option<crate::conversation_policy::Snapshot>,
}
impl ZaloGuiApp {
    pub(super) fn policy_command(&mut self, action: crate::conversation_policy::Action) {
        if self.policy_panel.pending {
            return;
        }
        self.policy_panel.pending = true;
        self.policy_panel.generation = crate::search::generation();
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::ConversationPolicy {
                owner: self.user_uin.clone(),
                generation: self.policy_panel.generation,
                action,
            })
            .is_ok()
        }) {
            self.policy_panel.pending = false;
            self.policy_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn content_protected(&self, thread: &str, group: bool) -> bool {
        self.policy_panel
            .snapshot
            .as_ref()
            .is_none_or(|snapshot| snapshot.protects(thread, group))
    }
    pub(super) fn render_policy_panel(&mut self, ctx: &egui::Context) {
        let Some((thread, group)) = self.policy_panel.target.clone() else {
            return;
        };
        let mut open = true;
        let mut action = None;
        egui::Window::new("Tin tự xóa & chặn chia sẻ").id(Id::new("conversation-policy")).open(&mut open).default_width(460.0).show(ctx,|ui|{
            ui.label(&self.policy_panel.status);
            if self.policy_panel.pending{ui.spinner();}
            ui.add_enabled_ui(!self.policy_panel.pending,|ui|{
                if ui.button("Đọc lại thiết lập trên Zalo").clicked(){action=Some(crate::conversation_policy::Action::Read);}
                let Some(snapshot)=self.policy_panel.snapshot.as_ref()else{ui.label("Chưa xác minh thiết lập server; chưa thể thay đổi.");return;};
                ui.label(format!("Thời hạn hiện tại: {} giờ",snapshot.ttl(&thread,group)/3600000));
                egui::ComboBox::from_id_salt("ttl-setting").selected_text(if self.policy_panel.ttl==0{"Tắt".into()}else{format!("{} giờ",self.policy_panel.ttl/3600000)}).show_ui(ui,|ui|{
                    for (ttl,label) in [(0,"Tắt"),(3600000,"1 giờ"),(86400000,"1 ngày"),(604800000,"7 ngày"),(1209600000,"14 ngày"),(2592000000,"30 ngày")]{ui.selectable_value(&mut self.policy_panel.ttl,ttl,label);}
                });
                if ui.button("Đổi thời hạn tự xóa…").clicked(){self.policy_panel.confirm=Some(crate::conversation_policy::Action::SetTtl{thread:thread.clone(),group,ttl:self.policy_panel.ttl});}
                let protected=snapshot.protects(&thread,group);
                ui.label(if protected{"Server đang bật chặn chia sẻ"}else{"Server chưa bật chặn chia sẻ"});
                if ui.button(if protected{"Tắt chặn chia sẻ…"}else{"Bật chặn chia sẻ…"}).clicked(){self.policy_panel.confirm=Some(crate::conversation_policy::Action::SetProtection{thread:thread.clone(),group,enabled:!protected});}
                ui.small("Server quyết định quyền và điều kiện tài khoản. Tắt tự xóa không khôi phục tin đã hết hạn; chặn chia sẻ không bảo đảm ngăn mọi cách chụp màn hình.");
                if self.policy_panel.confirm.is_some(){ui.separator();ui.label("Xác nhận thay đổi thiết lập cho hội thoại này trên Zalo?");ui.horizontal(|ui|{
                    if ui.button("Xác nhận").clicked(){action=self.policy_panel.confirm.take();}
                    if ui.button("Hủy").clicked(){self.policy_panel.confirm=None;}
                });}
            });
        });
        if !open {
            self.policy_panel.target = None;
            self.policy_panel.confirm = None;
        }
        if let Some(action) = action {
            self.policy_command(action);
        }
    }
}
