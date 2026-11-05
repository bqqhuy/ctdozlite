//! Separate group notes and poll UI, using server identities and explicit acknowledgements.
use super::*;
#[derive(Default)]
pub(super) struct ContentPanel {
    pub open: bool,
    pub thread: String,
    pub generation: u64,
    pub pending: bool,
    pub needs_refresh: bool,
    pub polls_mode: bool,
    pub notes: Vec<crate::board::Item>,
    pub polls: Vec<crate::group_content::Poll>,
    pub detail: Option<crate::group_content::Poll>,
    pub selected: std::collections::BTreeSet<i64>,
    pub text: String,
    pub options: String,
    pub multi: bool,
    pub anonymous: bool,
    pub allow_add: bool,
    pub option: String,
    pub editing: Option<crate::board::Item>,
    pub confirm: Option<crate::group_content::Action>,
    pub status: String,
    pub next: Option<(u32, String, i64)>,
}
impl ZaloGuiApp {
    pub(super) fn open_group_content(&mut self, thread: String) {
        self.group_content = ContentPanel {
            open: true,
            thread,
            generation: crate::search::generation(),
            ..Default::default()
        };
        self.content_command(crate::group_content::Action::ListNotes {
            page: 1,
            last: String::new(),
            last_type: 0,
        });
    }
    fn content_command(&mut self, action: crate::group_content::Action) {
        if self.group_content.pending {
            return;
        }
        if self.group_content.needs_refresh && action.mutation() {
            self.group_content.status = "Tải lại dữ liệu trước khi thay đổi tiếp".into();
            return;
        }
        self.group_content.pending = true;
        self.group_content.generation = crate::search::generation();
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::GroupContent {
                owner: self.user_uin.clone(),
                thread: self.group_content.thread.clone(),
                generation: self.group_content.generation,
                action,
            })
            .is_ok()
        }) {
            self.group_content.pending = false;
            self.group_content.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn render_group_content(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.group_content = Default::default();
            return;
        }
        if !self.group_content.open {
            return;
        }
        let mut open = true;
        let mut action = None;
        egui::Window::new("Ghi chú & bình chọn nhóm").id(Id::new("group-content")).open(&mut open).default_width(580.0).max_height((ctx.screen_rect().height()-80.0).max(220.0)).show(ctx,|ui|{
            ui.label(&self.group_content.status);if self.group_content.pending{ui.spinner();}
            ui.add_enabled_ui(!self.group_content.pending,|ui|{
                ui.horizontal(|ui|{
                    for (polls,label) in [(false,"Ghi chú"),(true,"Bình chọn")]{if ui.selectable_label(self.group_content.polls_mode==polls,label).clicked(){self.group_content.polls_mode=polls;self.group_content.detail=None;self.group_content.next=None;action=Some(if polls{crate::group_content::Action::ListPolls{page:1,last:String::new(),last_type:0}}else{crate::group_content::Action::ListNotes{page:1,last:String::new(),last_type:0}});}}
                    if ui.button("Tải lại").clicked(){action=Some(if self.group_content.polls_mode{crate::group_content::Action::ListPolls{page:1,last:String::new(),last_type:0}}else{crate::group_content::Action::ListNotes{page:1,last:String::new(),last_type:0}});}
                });
                egui::ScrollArea::vertical().id_salt("group-content-list").max_height(220.0).show(ui,|ui|{
                    if self.group_content.polls_mode{
                        for poll in &self.group_content.polls{if ui.button(&poll.question).clicked(){action=Some(crate::group_content::Action::PollDetail{id:poll.id.clone()});}}
                    }else{
                        for note in &self.group_content.notes{
                            ui.group(|ui|{ui.label(&note.title);ui.horizontal(|ui|{
                                if ui.button("Sửa").clicked(){self.group_content.text=note.title.clone();self.group_content.editing=Some(note.clone());}
                                if ui.button("Xóa…").clicked(){self.group_content.confirm=Some(crate::group_content::Action::RemoveNote{id:note.id.clone(),creator:note.creator.clone()});}
                            });});
                        }
                    }
                });
                if let Some((page,last,last_type))=&self.group_content.next{if ui.button("Tải trang tiếp theo").clicked(){action=Some(if self.group_content.polls_mode{crate::group_content::Action::ListPolls{page:*page,last:last.clone(),last_type:*last_type}}else{crate::group_content::Action::ListNotes{page:*page,last:last.clone(),last_type:*last_type}});}}
                if let Some(poll)=&self.group_content.detail{
                    ui.separator();ui.strong(&poll.question);ui.label(if poll.closed{"Đã kết thúc"}else if poll.anonymous{"Bình chọn ẩn danh"}else{"Đang bình chọn"});
                    for option in &poll.options{
                        let mut selected=self.group_content.selected.contains(&option.id);
                        let label=if let Some(votes)=option.votes{format!("{} ({votes})",option.text)}else{option.text.clone()};
                        if ui.add_enabled(!poll.closed,egui::Checkbox::new(&mut selected,label)).changed(){
                            if !poll.multi{self.group_content.selected.clear();}
                            if selected{self.group_content.selected.insert(option.id);}else{self.group_content.selected.remove(&option.id);}
                        }
                    }
                    if ui.add_enabled(!poll.closed&&!self.group_content.needs_refresh,egui::Button::new("Gửi lựa chọn")).clicked(){action=Some(crate::group_content::Action::Vote{id:poll.id.clone(),options:self.group_content.selected.iter().copied().collect()});}
                    if poll.add_options&&!poll.closed{ui.text_edit_singleline(&mut self.group_content.option);if ui.button("Thêm lựa chọn").clicked(){action=Some(crate::group_content::Action::AddOption{id:poll.id.clone(),text:self.group_content.option.clone()});}}
                    if ui.button("Tải lại bình chọn").clicked(){action=Some(crate::group_content::Action::PollDetail{id:poll.id.clone()});}
                    if !poll.closed&&ui.button("Kết thúc bình chọn…").clicked(){self.group_content.confirm=Some(crate::group_content::Action::EndPoll{id:poll.id.clone()});}
                }
                ui.separator();
                ui.add_enabled_ui(!self.group_content.needs_refresh,|ui|{
                    ui.label(if self.group_content.polls_mode{"Câu hỏi mới"}else{"Nội dung ghi chú"});ui.text_edit_multiline(&mut self.group_content.text);
                    if self.group_content.polls_mode{
                        ui.label("Các lựa chọn, mỗi dòng một lựa chọn");ui.text_edit_multiline(&mut self.group_content.options);
                        ui.checkbox(&mut self.group_content.multi,"Cho phép nhiều lựa chọn");ui.checkbox(&mut self.group_content.anonymous,"Ẩn danh");ui.checkbox(&mut self.group_content.allow_add,"Cho phép thêm lựa chọn");
                        if ui.button("Tạo bình chọn…").clicked(){self.group_content.confirm=Some(crate::group_content::Action::CreatePoll{question:self.group_content.text.clone(),options:self.group_content.options.lines().map(str::trim).filter(|line|!line.is_empty()).map(str::to_owned).collect(),multi:self.group_content.multi,anonymous:self.group_content.anonymous,allow_add:self.group_content.allow_add});}
                    }else{
                        if ui.button(if self.group_content.editing.is_some(){"Lưu ghi chú…"}else{"Tạo ghi chú…"}).clicked(){self.group_content.confirm=Some(crate::group_content::Action::SaveNote{id:self.group_content.editing.as_ref().map(|note|note.id.clone()),text:self.group_content.text.clone(),creator:self.group_content.editing.as_ref().map(|note|note.creator.clone()).unwrap_or_default()});}
                        if self.group_content.editing.is_some()&&ui.button("Hủy sửa").clicked(){self.group_content.editing=None;self.group_content.text.clear();}
                    }
                });
                if self.group_content.confirm.is_some(){ui.separator();ui.label("Xác nhận thao tác trên server Zalo? Thao tác không tự thử lại khi mất xác nhận.");ui.horizontal(|ui|{
                    if ui.button("Xác nhận").clicked(){action=self.group_content.confirm.take();}
                    if ui.button("Hủy").clicked(){self.group_content.confirm=None;}
                });}
            });
        });
        self.group_content.open = open;
        if let Some(action) = action {
            self.content_command(action);
        }
    }
}
