//! Compact scheduling dialog backed by the existing durable, account-owned scheduler.
use super::*;
use chrono::{TimeZone, Timelike};

pub(super) struct Draft {
    owner: String,
    thread: String,
    group: bool,
    name: String,
    text: String,
    minutes: i64,
    when: String,
    error: String,
    pending_at: Option<i64>,
    request: u64,
}
impl Draft {
    pub(super) fn matches_saved(
        &self,
        request: Option<u64>,
        thread: &str,
        text: &str,
        at: i64,
    ) -> bool {
        request == Some(self.request)
            && self.pending_at == Some(at)
            && self.thread == thread
            && self.text == text
    }
    pub(super) fn failed(&mut self, error: String) {
        if self.pending_at.take().is_some() {
            self.error = error;
        }
    }

    fn command(&self, owner: &str, now: i64) -> Option<tools_panel::ToolsCommand> {
        if owner.is_empty()
            || owner != self.owner
            || self.thread.is_empty()
            || self.text.trim().is_empty()
        {
            return None;
        }
        let at = if self.when.trim().is_empty() {
            now.checked_add(self.minutes.checked_mul(60)?)?
        } else {
            chrono::NaiveDateTime::parse_from_str(self.when.trim(), "%Y-%m-%d %H:%M")
                .ok()
                .and_then(|d| chrono::Local.from_local_datetime(&d).single())?
                .timestamp()
        };
        if at <= now || at > now.checked_add(366 * 86400)? {
            return None;
        }
        Some(tools_panel::ToolsCommand::Schedule {
            request: Some(self.request),
            thread: self.thread.clone(),
            group: self.group,
            text: self.text.clone(),
            at,
        })
    }
}
impl ZaloGuiApp {
    pub(super) fn open_quick_schedule(&mut self) {
        self.composer_mentions.edit(&self.input_message);
        if !self.composer_mentions.mentions.is_empty() {
            self.status_msg = "Chưa hỗ trợ hẹn giờ tin có mention; hãy gửi trực tiếp".into();
            return;
        }
        let Some(thread) = self.threads.get(self.selected_thread_index) else {
            return;
        };
        self.quick_schedule = Some(Draft {
            owner: self.user_uin.clone(),
            thread: thread.id.clone(),
            group: thread.is_group,
            name: thread.name.clone(),
            text: self.input_message.clone(),
            minutes: 15,
            when: String::new(),
            error: String::new(),
            pending_at: None,
            request: crate::chat::next_client_message_id(),
        });
    }
    pub(super) fn render_quick_schedule(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let Some(mut draft) = self.quick_schedule.take() else {
            return;
        };
        if !self.is_logged_in || draft.owner != self.user_uin {
            return;
        }
        let screen = ctx.screen_rect();
        egui::Area::new(Id::new("quick_schedule_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(145));
            });
        let mut open = true;
        let mut submit = false;
        let mut close = false;
        let now = chrono::Local::now();
        let response=egui::Window::new("Hẹn giờ gửi tin nhắn").id(Id::new("quick_schedule"))
            .order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO)
            .open(&mut open).collapsible(false).resizable(false)
            .default_width((screen.width()-48.0).clamp(220.0,460.0))
            .max_height((screen.height()-48.0).max(100.0)).show(ctx,|ui|{
                egui::ScrollArea::vertical().max_height((screen.height()-112.0).max(80.0)).show(ui,|ui|{
                    if draft.pending_at.is_some(){ui.disable();}
                    ui.label(RichText::new(format!("Gửi tới: {}",draft.name)).strong());
                    ui.add(egui::TextEdit::multiline(&mut draft.text).desired_rows(3).desired_width(f32::INFINITY).hint_text("Nội dung tin nhắn"));
                    ui.add_space(8.0);
                    let width=((ui.available_width()-8.0)/2.0).max(80.0);
                    egui::Grid::new("quick_schedule_presets").spacing(Vec2::splat(8.0)).show(ui,|ui|{
                        for (index,(minutes,label)) in [(15,"Sau 15 phút"),(60,"Sau 1 giờ"),(0,"18:00 hôm nay"),(0,"08:00 ngày mai")].into_iter().enumerate(){
                            let enabled=index!=2 || now.hour()<18;
                            if ui.add_enabled(enabled,egui::Button::new(label).min_size(Vec2::new(width,38.0))).clicked(){
                                draft.minutes=minutes;
                                draft.when=match index {
                                    2=>now.date_naive().and_hms_opt(18,0,0).map(|d|d.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default(),
                                    3=>now.date_naive().succ_opt().and_then(|d|d.and_hms_opt(8,0,0)).map(|d|d.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default(),
                                    _=>String::new(),
                                };
                            }
                            if index%2==1 {ui.end_row();}
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui|{ui.label("Sau (phút):");ui.add(egui::DragValue::new(&mut draft.minutes).range(1..=527040));});
                    ui.label("Hoặc giờ cụ thể (YYYY-MM-DD HH:MM):");
                    ui.text_edit_singleline(&mut draft.when);
                    if let Some(tools_panel::ToolsCommand::Schedule{at,..})=draft.command(&self.user_uin,now.timestamp()) {
                        if let Some(time)=chrono::Local.timestamp_opt(at,0).single(){ui.label(format!("Giờ gửi: {}",time.format("%d/%m/%Y %H:%M")));}
                    }
                    egui::Frame::none().fill(pal.bg_elevated).stroke(Stroke::new(1.0_f32,pal.primary)).rounding(Rounding::same(8.0)).inner_margin(Margin::same(10.0)).show(ui,|ui|{
                        ui.label(RichText::new("Lịch được lưu trên máy này. App cần chạy và tài khoản đăng nhập để gửi; khi mở lại có thể gửi trễ. Đây không phải lịch gửi trên máy chủ Zalo.").color(pal.text_primary).size(12.0));
                    });
                    if !draft.error.is_empty(){ui.colored_label(Color32::from_rgb(225,80,80),&draft.error);}
                    ui.horizontal(|ui|{
                        close=ui.button("Hủy").clicked();
                        submit=ui.add_enabled(!draft.text.trim().is_empty(),egui::Button::new("Hẹn giờ gửi").fill(pal.primary)).clicked();
                    });
                });
            });
        if let Some(window) = response {
            ctx.move_to_top(window.response.layer_id);
        }
        if draft.pending_at.is_none() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            open = false;
        }
        if submit {
            if let Some(command) = draft.command(&self.user_uin, crate::login::unix_now()) {
                let at = match &command {
                    tools_panel::ToolsCommand::Schedule { at, .. } => *at,
                    _ => return,
                };
                if WORKER_TX.get().is_some_and(|tx| {
                    tx.send(WorkerCmd::Tools {
                        owner: self.user_uin.clone(),
                        command,
                    })
                    .is_ok()
                }) {
                    draft.pending_at = Some(at);
                    draft.error.clear();
                } else {
                    draft.error = "Bộ xử lý chưa sẵn sàng; nội dung được giữ để thử lại".into();
                }
            } else {
                draft.error =
                    "Nội dung/giờ hẹn không hợp lệ, đã qua hoặc không xác định được múi giờ".into();
            }
        }
        if draft.pending_at.is_some() {
            open = true;
            close = false;
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if open && !close {
            self.quick_schedule = Some(draft);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Draft {
        Draft {
            owner: "owner-a".into(),
            thread: "thread-a".into(),
            group: true,
            name: "Trùng tên".into(),
            text: "Tiếng Việt: 2*3*4".into(),
            minutes: 15,
            when: String::new(),
            error: String::new(),
            pending_at: None,
            request: crate::chat::next_client_message_id(),
        }
    }
    #[test]
    fn schedule_draft_keeps_original_recipient_and_literal_text() {
        let draft = sample();
        let Some(tools_panel::ToolsCommand::Schedule {
            thread,
            group,
            text,
            at,
            ..
        }) = draft.command("owner-a", 1_000_000)
        else {
            panic!("schedule command");
        };
        assert_eq!(thread, "thread-a");
        assert!(group);
        assert_eq!(text, "Tiếng Việt: 2*3*4");
        assert_eq!(at, 1_000_900);
        assert!(draft.command("owner-b", 1_000_000).is_none());
    }
    #[test]
    fn invalid_empty_past_or_excessive_schedule_is_rejected() {
        let mut draft = sample();
        draft.minutes = -1;
        assert!(draft.command("owner-a", 1_000_000).is_none());
        draft.minutes = i64::MAX;
        assert!(draft.command("owner-a", 1_000_000).is_none());
        draft.minutes = 15;
        draft.when = "not a date".into();
        assert!(draft.command("owner-a", 1_000_000).is_none());
        draft.when = "2000-01-01 10:00".into();
        assert!(draft.command("owner-a", crate::login::unix_now()).is_none());
        draft.when.clear();
        draft.text.clear();
        assert!(draft.command("owner-a", 1_000_000).is_none());
    }
    fn app_with_pending_draft(ctx: &egui::Context) -> ZaloGuiApp {
        let mut session = crate::session::Session::new(false, false, false).unwrap();
        session.login = Some(crate::session::LoginInfo {
            uin: "owner-a".into(),
            ..Default::default()
        });
        let mut app = ZaloGuiApp::from_context(
            ctx,
            session,
            false,
            vec![Thread {
                id: "thread-a".into(),
                name: "Recipient".into(),
                is_group: true,
                ..Default::default()
            }],
            false,
        );
        app.input_message = "Keep literal 2*3*4".into();
        app.open_quick_schedule();
        app.quick_schedule.as_mut().unwrap().pending_at = Some(1_000_900);
        app
    }
    fn ack(app: &ZaloGuiApp, request: u64, result: Result<(), String>) -> GuiEvent {
        GuiEvent::Tools {
            owner: "owner-a".into(),
            result: Ok(tools_panel::ToolsResult::ScheduleSaved {
                request: Some(request),
                thread: "thread-a".into(),
                text: app.input_message.clone(),
                at: 1_000_900,
                result,
            }),
        }
    }
    #[test]
    fn schedule_write_failure_keeps_draft_and_composer_for_retry() {
        let ctx = egui::Context::default();
        let mut app = app_with_pending_draft(&ctx);
        let request = app.quick_schedule.as_ref().unwrap().request;
        app.event_tx
            .send(ack(&app, request, Err("Disk full".into())))
            .unwrap();
        app.check_events(&ctx);
        let draft = app.quick_schedule.as_ref().unwrap();
        assert_eq!(draft.error, "Disk full");
        assert!(draft.pending_at.is_none());
        assert_eq!(app.input_message, "Keep literal 2*3*4");
    }
    #[test]
    fn only_the_matching_schedule_ack_clears_the_original_composer() {
        let ctx = egui::Context::default();
        let mut app = app_with_pending_draft(&ctx);
        let request = app.quick_schedule.as_ref().unwrap().request;
        app.event_tx.send(ack(&app, request + 1, Ok(()))).unwrap();
        app.check_events(&ctx);
        assert!(app.quick_schedule.is_some());
        assert!(!app.input_message.is_empty());
        app.event_tx.send(ack(&app, request, Ok(()))).unwrap();
        app.check_events(&ctx);
        assert!(app.quick_schedule.is_none());
        assert!(app.input_message.is_empty());
    }
}
