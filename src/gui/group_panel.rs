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
    pub link: Option<serde_json::Value>,
    pub link_qr: Option<egui::TextureHandle>,
    pub pending_members: Vec<(String, String)>,
    pub blocked_members: Vec<String>,
    pub blocked_page: u32,
    pub blocked_more: bool,
    pub community: Option<crate::groups::CommunityQuota>,
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
            .vscroll(true)
            .min_width(120.0)
            .max_width((ctx.screen_rect().width()-32.0).max(180.0))
            .max_height((ctx.screen_rect().height()-64.0).max(100.0))
        .default_width(480.0_f32.min((ctx.screen_rect().width()-48.0).max(160.0)))
        .show(ctx, |ui| {
                ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
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
            if !self.groups_panel.create && ui.button("Báo cáo nhóm…").clicked(){self.open_group_report(self.groups_panel.target.clone());}
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
                .is_some_and(|state| state.creator_is(&self.user_uin));
            ui.add_enabled_ui(
                !self.groups_panel.pending && !self.groups_panel.needs_refresh,
                |ui| {
                    if self.groups_panel.create {
                        ui.label("Tên nhóm");
                        ui.add(egui::TextEdit::singleline(&mut self.groups_panel.name).desired_width(ui.available_width()));
                    }
                    if !self.groups_panel.create && state.is_some() {
                        let can_avatar = manager
                            || state.as_ref().is_some_and(|snapshot| {
                                snapshot
                                    .info
                                    .pointer("/setting/blockName")
                                    .and_then(crate::api_response::integer)
                                    == Some(0)
                            });
                        if ui
                            .add_enabled(can_avatar, egui::Button::new("Thay ảnh đại diện nhóm…"))
                            .clicked()
                        {
                            self.open_avatar(crate::avatar::Target::Group(
                                self.groups_panel.target.clone(),
                            ));
                        }
                        ui.collapsing("Thông tin và thiết lập nhóm", |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.groups_panel.name).desired_width((ui.available_width()-120.0).max(64.0)));
                                if ui.button("Đổi tên nhóm").clicked() {
                                    self.groups_panel.confirm =
                                        Some(crate::groups::Action::Rename {
                                            name: self.groups_panel.name.clone(),
                                        });
                                }
                            });
                            if manager {
                                for (setting, label) in [
                                    (
                                        crate::groups::Setting::LockName,
                                        "Chỉ quản trị viên đổi tên",
                                    ),
                                    (
                                        crate::groups::Setting::LockPoll,
                                        "Chỉ quản trị viên tạo bình chọn",
                                    ),
                                    (
                                        crate::groups::Setting::LockPost,
                                        "Chỉ quản trị viên tạo ghi chú",
                                    ),
                                    (
                                        crate::groups::Setting::LockSend,
                                        "Chỉ quản trị viên gửi tin",
                                    ),
                                    (
                                        crate::groups::Setting::ApproveMembers,
                                        "Duyệt thành viên mới",
                                    ),
                                ] {
                                    if let Some(value) = state
                                        .as_ref()
                                        .and_then(|snapshot| snapshot.info.get("setting"))
                                        .and_then(|settings| settings.get(setting.key()))
                                        .and_then(serde_json::Value::as_i64)
                                        .filter(|value| matches!(value, 0 | 1))
                                    {
                                        let mut enabled = value == 1;
                                        if ui.checkbox(&mut enabled, label).changed() {
                                            self.groups_panel.confirm =
                                                Some(crate::groups::Action::Setting {
                                                    setting,
                                                    enabled,
                                                });
                                        }
                                    }
                                }
                            }
                        });
                        ui.collapsing("Cộng đồng",|ui| {
                            if ui.button("Kiểm tra điều kiện nâng cấp").clicked(){intent=Some(crate::groups::Action::CommunityQuota);}
                            if let Some(quota)=&self.groups_panel.community {
                                ui.label(if quota.can_upgrade {"Máy chủ cho phép nâng cấp"}else{"Máy chủ chưa cho phép nâng cấp"});
                                if let Some(message)=&quota.message {ui.label(message);}
                                if owner && quota.can_upgrade && state.as_ref().and_then(|state|state.info.get("type")).and_then(crate::api_response::integer)==Some(1) && ui.button("Nâng cấp nhóm thành cộng đồng…").clicked(){self.groups_panel.confirm=Some(crate::groups::Action::UpgradeCommunity);}
                            }
                        });
                        ui.collapsing("Link mời và duyệt thành viên", |ui| {
                            if ui.button("Xem link mời").clicked() {
                                intent = Some(crate::groups::Action::Link);
                            }
                            if let Some(link) = &self.groups_panel.link {
                                match crate::groups::shareable_link(link,crate::login::unix_now()*1000) {
                                    Ok(url)=>{
                                        ui.label(url);
                                        if ui.button("Sao chép link").clicked(){ui.output_mut(|output|output.copied_text=url.to_owned());}
                                        if ui.button("Hiện mã QR mời nhóm").clicked(){
                                            match group_link_qr(ui.ctx(),url) {Ok(texture)=>self.groups_panel.link_qr=Some(texture),Err(error)=>self.groups_panel.status=error.to_string()}
                                        }
                                        if let Some(qr)=&self.groups_panel.link_qr {ui.image((qr.id(),Vec2::splat(ui.available_width().min(240.0))));}
                                    }
                                    Err(error)=>{ui.label(error.to_string());self.groups_panel.link_qr=None;}
                                }
                            }
                            if manager {
                                if ui.button("Danh sách chặn vào nhóm").clicked() {
                                    intent = Some(crate::groups::Action::Blocked { page: 1 });
                                }
                                for uid in &self.groups_panel.blocked_members {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(uid);
                                        if ui.button("Bỏ chặn vào nhóm").clicked() {
                                            self.groups_panel.confirm =
                                                Some(crate::groups::Action::Ban {
                                                    members: vec![uid.clone()],
                                                    blocked: false,
                                                });
                                        }
                                    });
                                }
                                if self.groups_panel.blocked_more
                                    && self.groups_panel.blocked_page < 100
                                    && ui.button("Trang chặn tiếp").clicked()
                                {
                                    intent = Some(crate::groups::Action::Blocked {
                                        page: self.groups_panel.blocked_page + 1,
                                    });
                                }
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("Tạo link mới").clicked() {
                                        self.groups_panel.confirm =
                                            Some(crate::groups::Action::RenewLink);
                                    }
                                    if ui.button("Tắt link mời").clicked() {
                                        self.groups_panel.confirm =
                                            Some(crate::groups::Action::DisableLink);
                                    }
                                    if ui.button("Tải danh sách chờ duyệt").clicked() {
                                        intent = Some(crate::groups::Action::Pending);
                                    }
                                });
                                egui::ScrollArea::vertical().id_salt("group-pending-list").max_height(180.0).show_rows(ui,24.0,self.groups_panel.pending_members.len(),|ui,range| {
                                    for index in range {
                                        let (uid,name)=&self.groups_panel.pending_members[index];
                                        ui.horizontal(|ui| {
                                            let width=(ui.available_width()-115.0).max(20.0);
                                            ui.add_sized([width,24.0],egui::Label::new(name).truncate()).on_hover_text(format!("{name} • {uid}"));
                                            for (label,approve) in [("Duyệt",true),("Từ chối",false)] {
                                                if ui.button(label).clicked(){self.groups_panel.confirm=Some(crate::groups::Action::Review {members:vec![uid.clone()],approve});}
                                            }
                                        });
                                    }
                                });
                            }
                        });
                    }
                    if self.groups_panel.create || state.is_some() {
                        ui.label("Chọn liên hệ để mời");
                        ui.add(egui::TextEdit::singleline(&mut self.groups_panel.query).desired_width(ui.available_width()));
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
                            if manager
                                && !self.groups_panel.member.is_empty()
                                && ui.button("Chặn thành viên vào lại nhóm").clicked()
                            {
                                self.groups_panel.confirm = Some(crate::groups::Action::Ban {
                                    members: vec![self.groups_panel.member.clone()],
                                    blocked: true,
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
                    crate::groups::Action::UpgradeCommunity=>"Nâng cấp nhóm thành cộng đồng? Fork không có thao tác hoàn tác về nhóm thường.".into(),
                    crate::groups::Action::Rename { name } => {
                        format!("Đổi tên nhóm thành “{name}”?")
                    }
                    crate::groups::Action::Setting { .. } => {
                        "Thay đổi thiết lập nhóm cho mọi thành viên?".into()
                    }
                    crate::groups::Action::RenewLink => {
                        "Tạo link mời mới? Link cũ sẽ hết hiệu lực.".into()
                    }
                    crate::groups::Action::DisableLink => "Tắt link mời nhóm?".into(),
                    crate::groups::Action::Review { members, approve } => format!(
                        "{} {} người đang chờ?",
                        if *approve { "Duyệt" } else { "Từ chối" },
                        members.len()
                    ),
                    crate::groups::Action::Ban { members, blocked } => format!(
                        "{} vào lại nhóm đối với {} người?",
                        if *blocked { "Chặn" } else { "Bỏ chặn" },
                        members.len()
                    ),
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
                ui.horizontal_wrapped(|ui| {
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

fn group_link_qr(ctx: &egui::Context, url: &str) -> anyhow::Result<egui::TextureHandle> {
    let code = qrcode::QrCode::new(url.as_bytes())?;
    let width = code.width();
    let scale = if width > 100 { 4 } else { 6 };
    let side = (width + 8) * scale;
    anyhow::ensure!(side <= 1024, "Link quá dài để hiển thị QR");
    let mut pixels = vec![Color32::WHITE; side * side];
    for y in 0..width {
        for x in 0..width {
            if code[(x, y)] == qrcode::Color::Dark {
                for dy in 0..scale {
                    for dx in 0..scale {
                        pixels[((y + 4) * scale + dy) * side + (x + 4) * scale + dx] =
                            Color32::BLACK;
                    }
                }
            }
        }
    }
    Ok(ctx.load_texture(
        "group-invite-qr",
        egui::ColorImage {
            size: [side, side],
            pixels,
        },
        egui::TextureOptions::NEAREST,
    ))
}
