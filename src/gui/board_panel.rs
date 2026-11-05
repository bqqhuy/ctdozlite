//! Server reminder editor. Acknowledged mutations are followed by an explicit list refresh.
use super::*;
#[derive(Default)]
pub(super) struct BoardPanel {
    pub open: bool,
    pub thread: String,
    pub group: bool,
    pub generation: u64,
    pub pending: bool,
    pub needs_refresh: bool,
    pub page: crate::board::Page,
    pub page_number: u32,
    pub pins: bool,
    pub status: String,
    pub title: String,
    pub when: String,
    pub editing: Option<crate::board::Item>,
    pub confirm_remove: Option<crate::board::Item>,
}
impl ZaloGuiApp {
    pub(super) fn open_server_board(&mut self, thread: String, group: bool, title: String) {
        let generation = crate::search::generation();
        self.server_board = BoardPanel {
            open: true,
            thread,
            group,
            generation,
            title,
            ..Default::default()
        };
        self.board_command(crate::board::Action::List {
            page: 0,
            last: String::new(),
            pins: false,
        });
    }
    pub(super) fn pin_server_message(&mut self, thread: String, group: bool, message: ChatMessage) {
        self.server_board = BoardPanel {
            open: true,
            thread,
            group,
            pins: true,
            generation: crate::search::generation(),
            ..Default::default()
        };
        self.board_command(crate::board::Action::PinMessage {
            message: Box::new(message),
        });
    }
    fn board_command(&mut self, action: crate::board::Action) {
        if self.server_board.pending {
            return;
        }
        if self.server_board.needs_refresh
            && matches!(
                action,
                crate::board::Action::Save { .. }
                    | crate::board::Action::Remove { .. }
                    | crate::board::Action::PinMessage { .. }
                    | crate::board::Action::Unpin { .. }
            )
        {
            self.server_board.status =
                "Tải lại danh sách trước khi thay đổi tiếp để tránh lặp thao tác".into();
            return;
        }
        self.server_board.pending = true;
        self.server_board.generation = crate::search::generation();
        if let Some(tx) = WORKER_TX.get() {
            if tx
                .send(WorkerCmd::Board {
                    owner: self.user_uin.clone(),
                    thread: self.server_board.thread.clone(),
                    group: self.server_board.group,
                    generation: self.server_board.generation,
                    action,
                })
                .is_err()
            {
                self.server_board.pending = false;
                self.server_board.status = "Worker đã đóng".into();
            }
        } else {
            self.server_board.pending = false;
            self.server_board.status = "Chưa có kết nối".into();
        }
    }
    pub(super) fn render_server_board(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.server_board = Default::default();
            return;
        }
        if !self.server_board.open {
            return;
        }
        let mut open = true;
        let mut action = None;
        egui::Window::new(crate::i18n::tr("Nhắc hẹn & tin ghim trên Zalo")).id(Id::new("server-board")).open(&mut open).default_width(620.0).max_width((ctx.screen_rect().width()-32.0).max(240.0)).max_height((ctx.screen_rect().height()-60.0).max(200.0)).show(ctx,|ui|{
            ui.label(crate::i18n::tr("Nhắc hẹn do server Zalo quản lý, khác với Hẹn giờ gửi cục bộ."));
            ui.add_enabled_ui(!self.server_board.pending,|ui|{
                ui.horizontal(|ui|{
                    if ui.selectable_label(!self.server_board.pins,crate::i18n::tr("Nhắc hẹn")).clicked(){self.server_board.pins=false;self.server_board.page_number=0;action=Some(crate::board::Action::List{page:0,last:String::new(),pins:false});}
                    if self.server_board.group && ui.selectable_label(self.server_board.pins,crate::i18n::tr("Tin ghim server")).clicked(){self.server_board.pins=true;action=Some(crate::board::Action::List{page:0,last:String::new(),pins:true});}
                    if ui.button(crate::i18n::tr("Tải lại")).clicked(){self.server_board.page_number=0;action=Some(crate::board::Action::List{page:0,last:String::new(),pins:self.server_board.pins});}
                });
                if !self.server_board.pins {
                    ui.separator();ui.label(crate::i18n::tr(if self.server_board.editing.is_some(){"Sửa nhắc hẹn một lần"}else{"Tạo nhắc hẹn một lần"}));
                    ui.add(egui::TextEdit::multiline(&mut self.server_board.title).char_limit(4000).desired_rows(2));
                    ui.horizontal_wrapped(|ui|{
                        ui.label(crate::i18n::tr("Giờ máy YYYY-MM-DD HH:MM"));ui.text_edit_singleline(&mut self.server_board.when);
                        if ui.add_enabled(!self.server_board.needs_refresh,egui::Button::new(crate::i18n::tr("Lưu lên Zalo"))).clicked(){
                            use chrono::{NaiveDateTime,Local,TimeZone};
                            match NaiveDateTime::parse_from_str(self.server_board.when.trim(),"%Y-%m-%d %H:%M").ok().and_then(|t|Local.from_local_datetime(&t).single()){
                                Some(time)=>action=Some(crate::board::Action::Save{id:self.server_board.editing.as_ref().map(|i|i.id.clone()),title:self.server_board.title.clone(),start:time.timestamp_millis(),original:self.server_board.editing.as_ref().map(|i|i.raw.clone())}),
                                None=>self.server_board.status="Thời gian không hợp lệ hoặc không xác định duy nhất tại múi giờ máy".into(),
                            }
                        }
                        if self.server_board.editing.is_some()&&ui.button(crate::i18n::tr("Hủy sửa")).clicked(){self.server_board.editing=None;self.server_board.title.clear();self.server_board.when.clear();}
                    });
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height((ctx.screen_rect().height()-320.0).max(100.0)).show(ui,|ui|{
                    let mut jump=None;
                    for item in &self.server_board.page.items {
                        ui.push_id(&item.id,|ui|{ui.group(|ui|{
                            ui.strong(&item.title);if item.start>0 {ui.small(crate::clock::message_time(item.start));}
                            ui.horizontal(|ui|{
                                if !self.server_board.pins {
                                    if ui.button(crate::i18n::tr("Chi tiết")).clicked(){action=Some(crate::board::Action::Detail{event:item.id.clone()});}
                                    if item.creator==self.user_uin {
                                        if !item.repeating && ui.button(crate::i18n::tr("Sửa")).clicked(){
                                            self.server_board.editing=Some(item.clone());self.server_board.title=item.title.clone();
                                            self.server_board.when=chrono::DateTime::from_timestamp_millis(item.start).map(|t|t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
                                        }
                                        if ui.button(crate::i18n::tr("Xóa…")).clicked(){self.server_board.confirm_remove=Some(item.clone());}
                                    }
                                }else {
                                    if !item.message_id.is_empty()&&ui.button(crate::i18n::tr("Đi tới tin gốc")).clicked(){jump=Some(item.message_id.clone());}
                                    if let Some(version)=self.server_board.page.version {if ui.button(crate::i18n::tr("Bỏ ghim trên mọi thiết bị")).clicked(){action=Some(crate::board::Action::Unpin{item:item.raw.clone(),version});}}
                                }
                            });
                        });});
                    }
                    if let Some(id)=jump{self.tools_command(super::tools_panel::ToolsCommand::OpenHit{thread:self.server_board.thread.clone(),id});}
                });
                if let Some(item)=&self.server_board.confirm_remove {
                    ui.label(format!("Xóa nhắc hẹn ‘{}’ trên Zalo?",item.title));
                    if ui.button(crate::i18n::tr("Xác nhận xóa")).clicked(){action=Some(crate::board::Action::Remove{id:item.id.clone(),original:item.raw.clone()});self.server_board.confirm_remove=None;}
                    if ui.button(crate::i18n::tr("Giữ lại")).clicked(){self.server_board.confirm_remove=None;}
                }
                if let Some(last)=&self.server_board.page.next {
                    if ui.button(crate::i18n::tr("Trang tiếp theo")).clicked(){self.server_board.page_number+=1;action=Some(crate::board::Action::List{page:self.server_board.page_number,last:last.clone(),pins:self.server_board.pins});}
                }
            });
            if self.server_board.pending{ui.spinner();}
            ui.label(&self.server_board.status);
        });
        self.server_board.open = open;
        if !open {
            self.server_board.generation = crate::search::generation();
        }
        if let Some(action) = action {
            self.board_command(action);
        }
    }
}
