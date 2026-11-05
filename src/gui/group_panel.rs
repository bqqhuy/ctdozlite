//! Account-scoped group administration and shared-group dialogs.
use super::*;

#[derive(Default)]
pub(super) struct GroupPanel {
    pub open: bool,
    pub target: String,
    pub group: bool,
    pub create: bool,
    pub generation: u64,
    pub pending: bool,
    pub needs_refresh: bool,
    pub info: Option<serde_json::Value>,
    pub related: Vec<Thread>,
    pub selected: std::collections::BTreeSet<String>,
    pub name: String,
    pub query: String,
    pub member: String,
    pub status: String,
    pub confirm: Option<crate::groups::Action>,
}
impl ZaloGuiApp {
    pub(super) fn open_group_panel(&mut self, target: String, group: bool, create: bool) {
        self.groups_panel = GroupPanel {
            open: true,
            target,
            group,
            create,
            generation: crate::search::generation(),
            ..Default::default()
        };
        if !create {
            self.group_command(if group {
                crate::groups::Action::Inspect
            } else {
                crate::groups::Action::Related
            });
        }
    }
    fn group_command(&mut self, action: crate::groups::Action) {
        if self.groups_panel.pending {
            return;
        }
        if action.mutation() && self.groups_panel.needs_refresh {
            self.groups_panel.status = "Tải lại trạng thái nhóm trước khi thay đổi tiếp".into();
            return;
        }
        self.groups_panel.generation = crate::search::generation();
        self.groups_panel.pending = true;
        let sent = WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Group {
                owner: self.user_uin.clone(),
                target: self.groups_panel.target.clone(),
                generation: self.groups_panel.generation,
                action,
            })
            .is_ok()
        });
        if !sent {
            self.groups_panel.pending = false;
            self.groups_panel.status = "Worker chưa sẵn sàng".into();
        }
    }
    pub(super) fn render_group_panel(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.groups_panel = Default::default();
            return;
        }
        if !self.groups_panel.open {
            return;
        }
        let mut open = true;
        let mut intent = None;
        let mut selected_group = None;
        egui::Window::new(if self.groups_panel.create {
            "Tạo nhóm"
        } else if self.groups_panel.group {
            "Quản lý nhóm"
        } else {
            "Nhóm chung"
        })
        .id(Id::new("group-panel"))
        .open(&mut open)
        .default_width(480.0)
        .max_height((ctx.screen_rect().height() - 80.0).max(200.0))
        .show(ctx, |ui| {
            if self.groups_panel.pending {
                ui.spinner();
            }
            ui.label(&self.groups_panel.status);
            if !self.groups_panel.create
                && ui
                    .add_enabled(!self.groups_panel.pending, egui::Button::new("Tải lại"))
                    .clicked()
            {
                intent = Some(if self.groups_panel.group {
                    crate::groups::Action::Inspect
                } else {
                    crate::groups::Action::Related
                });
            }
            if !self.groups_panel.group && !self.groups_panel.create {
                for thread in &self.groups_panel.related {
                    if self
                        .conversation_preferences
                        .get(&thread.id)
                        .is_some_and(|prefs| prefs.hidden)
                    {
                        continue;
                    }
                    if ui.button(&thread.name).clicked() {
                        selected_group = Some(thread.clone());
                    }
                }
                return;
            }
            let state = self
                .groups_panel
                .info
                .clone()
                .map(|info| crate::group_state::Snapshot {
                    info,
                    removed: false,
                });
            let manager = state
                .as_ref()
                .is_some_and(|state| state.manager(&self.user_uin));
            let owner = state
                .as_ref()
                .is_some_and(|state| state.creator() == Some(&self.user_uin));
            ui.add_enabled_ui(
                !self.groups_panel.pending && !self.groups_panel.needs_refresh,
                |ui| {
                    if self.groups_panel.create {
                        ui.label("Tên nhóm");
                        ui.text_edit_singleline(&mut self.groups_panel.name);
                    }
                    if self.groups_panel.create || state.is_some() {
                        ui.label("Chọn liên hệ để mời");
                        ui.text_edit_singleline(&mut self.groups_panel.query);
                        let query = crate::search::fold(&self.groups_panel.query);
                        egui::ScrollArea::vertical()
                            .id_salt("group-contact-picker")
                            .max_height(180.0)
                            .show(ui, |ui| {
                                for thread in self
                                    .threads
                                    .iter()
                                    .filter(|t| {
                                        !t.is_group
                                            && t.id != self.user_uin
                                            && !self
                                                .conversation_preferences
                                                .get(&t.id)
                                                .is_some_and(|p| p.hidden)
                                            && (query.is_empty()
                                                || crate::search::fold(&t.name).contains(&query))
                                    })
                                    .take(500)
                                {
                                    let mut checked =
                                        self.groups_panel.selected.contains(&thread.id);
                                    if ui.checkbox(&mut checked, &thread.name).changed() {
                                        if checked {
                                            self.groups_panel.selected.insert(thread.id.clone());
                                        } else {
                                            self.groups_panel.selected.remove(&thread.id);
                                        }
                                    }
                                }
                            });
                        if ui
                            .button(if self.groups_panel.create {
                                "Tạo nhóm với người đã chọn"
                            } else {
                                "Mời người đã chọn"
                            })
                            .clicked()
                        {
                            let members = self.groups_panel.selected.iter().cloned().collect();
                            self.groups_panel.confirm = Some(if self.groups_panel.create {
                                crate::groups::Action::Create {
                                    name: self.groups_panel.name.clone(),
                                    members,
                                    client_id: crate::chat::next_client_message_id().to_string(),
                                }
                            } else {
                                crate::groups::Action::Invite { members }
                            });
                        }
                    }
                    if !self.groups_panel.create && state.is_some() {
                        ui.separator();
                        ui.label("Thành viên để quản trị");
                        let roster = self
                            .group_members
                            .get(&self.groups_panel.target)
                            .map(|(members, _)| members.clone())
                            .or_else(|| {
                                self.threads
                                    .iter()
                                    .find(|t| t.id == self.groups_panel.target)
                                    .map(|t| t.presentation.members.clone())
                            })
                            .unwrap_or_default();
                        egui::ComboBox::from_id_salt("admin-member-picker")
                            .selected_text(if self.groups_panel.member.is_empty() {
                                "Chọn thành viên"
                            } else {
                                &self.groups_panel.member
                            })
                            .show_ui(ui, |ui| {
                                for member in roster {
                                    ui.selectable_value(
                                        &mut self.groups_panel.member,
                                        member.id,
                                        member.name,
                                    );
                                }
                            });
                        ui.horizontal_wrapped(|ui| {
                            if manager && ui.button("Loại thành viên").clicked() {
                                self.groups_panel.confirm = Some(crate::groups::Action::Kick {
                                    member: self.groups_panel.member.clone(),
                                });
                            }
                            if owner {
                                if ui.button("Thêm phó nhóm").clicked() {
                                    self.groups_panel.confirm =
                                        Some(crate::groups::Action::Admin {
                                            member: self.groups_panel.member.clone(),
                                            add: true,
                                        });
                                }
                                if ui.button("Bỏ phó nhóm").clicked() {
                                    self.groups_panel.confirm =
                                        Some(crate::groups::Action::Admin {
                                            member: self.groups_panel.member.clone(),
                                            add: false,
                                        });
                                }
                                if ui.button("Chuyển trưởng nhóm").clicked() {
                                    self.groups_panel.confirm =
                                        Some(crate::groups::Action::Transfer {
                                            member: self.groups_panel.member.clone(),
                                        });
                                }
                                if ui.button("Giải tán nhóm").clicked() {
                                    self.groups_panel.confirm =
                                        Some(crate::groups::Action::Disband);
                                }
                            }
                            if ui.button("Rời nhóm").clicked() {
                                self.groups_panel.confirm = Some(crate::groups::Action::Leave);
                            }
                        });
                    }
                },
            );
            if let Some(action) = self.groups_panel.confirm.clone() {
                ui.separator();
                let text = match &action {
                    crate::groups::Action::Disband => "Giải tán nhóm cho tất cả thành viên?".into(),
                    crate::groups::Action::Leave => {
                        "Rời nhóm? Lịch sử cục bộ được giữ nguyên.".into()
                    }
                    crate::groups::Action::Transfer { member } => {
                        format!("Chuyển quyền trưởng nhóm cho UID {member}?")
                    }
                    crate::groups::Action::Kick { member } => {
                        format!("Loại UID {member} khỏi nhóm?")
                    }
                    crate::groups::Action::Admin { member, add } => format!(
                        "{} phó nhóm cho UID {member}?",
                        if *add { "Thêm" } else { "Bỏ" }
                    ),
                    crate::groups::Action::Create { name, members, .. } => {
                        format!("Tạo nhóm “{name}” với {} người đã chọn?", members.len())
                    }
                    crate::groups::Action::Invite { members } => format!(
                        "Gửi lời mời cho {} người? Có thể cần duyệt trước khi tham gia.",
                        members.len()
                    ),
                    _ => String::new(),
                };
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.groups_panel.pending, egui::Button::new("Xác nhận"))
                        .clicked()
                    {
                        intent = Some(action);
                        self.groups_panel.confirm = None;
                    }
                    if ui.button("Hủy").clicked() {
                        self.groups_panel.confirm = None;
                    }
                });
            }
        });
        self.groups_panel.open = open;
        if let Some(action) = intent {
            self.group_command(action);
        }
        if let Some(thread) = selected_group {
            let index = if let Some(index) = self.threads.iter().position(|t| {
                t.is_group
                    && crate::server_pins::identity(&t.id, true).ok()
                        == crate::server_pins::identity(&thread.id, true).ok()
            }) {
                index
            } else {
                self.threads.push(thread.clone());
                self.threads.len() - 1
            };
            self.open_thread(ctx, index, self.threads[index].id.clone(), true);
            self.groups_panel.open = false;
        }
    }
}
