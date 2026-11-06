//! Native picker and prepared avatar preview, with an explicit upload confirmation.
use super::*;
#[derive(Default)]
pub(super) struct AvatarPanel {
    pub open: bool,
    pub pending: bool,
    pub uncertain: bool,
    pub generation: u64,
    pub target: Option<crate::avatar::Target>,
    pub draft: Option<std::sync::Arc<crate::avatar::Draft>>,
    pub texture: Option<egui::TextureHandle>,
    pub status: String,
    pub history: Option<crate::avatar_history::Page>,
    pub history_uncertain: bool,
    pub history_previews_requested: std::collections::HashSet<String>,
    pub history_confirm: Option<crate::avatar_history::Action>,
}
impl ZaloGuiApp {
    pub(super) fn open_avatar(&mut self, target: crate::avatar::Target) {
        if matches!(target, crate::avatar::Target::Own) && self.profile_panel.pending {
            self.show_toast("Đang xử lý hồ sơ; chờ kết quả rồi thay ảnh đại diện");
            return;
        }
        if self.avatar_panel.pending {
            self.show_toast("Đang xử lý ảnh đại diện trước; chờ kết quả rồi thử lại");
            return;
        }
        self.avatar_panel = AvatarPanel {
            open: true,
            target: Some(target),
            generation: crate::search::generation(),
            ..Default::default()
        };
    }
    pub(super) fn render_avatar_panel(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.avatar_panel = Default::default();
            return;
        }
        if !self.avatar_panel.open {
            return;
        }
        let mut open = true;
        let mut pick = false;
        let mut upload = false;
        let mut history_action = None;
        egui::Window::new("Thay ảnh đại diện").id(Id::new("avatar-review")).open(&mut open)
            .vscroll(true)
            .min_width(120.0)
            .max_width((ctx.screen_rect().width()-32.0).max(180.0))
            .max_height((ctx.screen_rect().height()-64.0).max(100.0)).default_width(340.0_f32.min((ctx.screen_rect().width()-48.0).max(160.0))).show(ctx,|ui| {
                ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
            ui.label("Ảnh sẽ cắt vuông ở giữa, thu về 512 × 512 và bỏ nền trong suốt. Kiểm tra bản xem trước trước khi tải lên.");
            ui.label(&self.avatar_panel.status);if self.avatar_panel.pending {ui.spinner();}
            if ui.add_enabled(!self.avatar_panel.pending,egui::Button::new("Chọn ảnh…")).clicked(){pick=true;}
            if let Some(texture)=&self.avatar_panel.texture {ui.image((texture.id(),Vec2::splat(ui.available_width().min(240.0))));}
            if ui.add_enabled(self.avatar_panel.draft.is_some() && !self.avatar_panel.pending && !self.avatar_panel.uncertain && !(self.profile_panel.pending && matches!(self.avatar_panel.target,Some(crate::avatar::Target::Own))),egui::Button::new("Xác nhận tải ảnh đại diện lên máy chủ")).clicked(){upload=true;}
            if matches!(self.avatar_panel.target,Some(crate::avatar::Target::Own)) {
                ui.separator();
                ui.label("Nếu ảnh xem trước không tải được, tải lại lịch sử để thử lại. Chỉ thao tác sau khi nhìn thấy đúng ảnh.");
                if ui.add_enabled(!self.avatar_panel.pending,egui::Button::new("Tải lịch sử ảnh đại diện")).clicked(){history_action=Some(crate::avatar_history::Action::Read {page:1,photo:String::new(),album:"0".into()});}
                if let Some(page)=&self.avatar_panel.history {
                    ui.label(format!("Trang {} • {} ảnh",page.page,page.photos.len()));
                    for photo in &page.photos {
                        ui.horizontal_wrapped(|ui| {
                            let key=format!("avatar-history:{}:{}:thumb",self.user_uin,photo.id.text);
                            let (rect,_)=ui.allocate_exact_size(Vec2::splat(64.0),Sense::hover());
                            if ui.is_rect_visible(rect) && !self.sticker_textures.contains_key(&key) && !self.loading_stickers.contains(&key) && !self.avatar_panel.history_previews_requested.contains(&key) {
                                if let (Some(url),Some(tx))=(photo.thumb.as_ref().or(photo.original.as_ref()),WORKER_TX.get()) {
                                    self.avatar_panel.history_previews_requested.insert(key.clone());
                                    self.loading_stickers.insert(key.clone());
                                    if tx.send(WorkerCmd::FetchStickerImage {key:key.clone(),url:url.clone()}).is_err(){self.loading_stickers.remove(&key);}
                                }
                            }
                            let preview=self.sticker_textures.get(&key);
                            if let Some(texture)=preview {ui.painter().image(texture.id(),rect,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),Color32::WHITE);}
                            else {ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,"Ảnh",egui::FontId::proportional(12.0),ui.visuals().text_color());}
                            ui.label(format!("Ảnh {}",photo.id.text));
                            let enabled=preview.is_some() && !self.avatar_panel.pending && !self.avatar_panel.uncertain && !self.avatar_panel.history_uncertain;
                            if ui.add_enabled(enabled,egui::Button::new("Dùng lại")).clicked(){self.avatar_panel.history_confirm=Some(crate::avatar_history::Action::Reuse {id:photo.id.clone()});}
                            if ui.add_enabled(enabled,egui::Button::new("Xóa")).clicked(){self.avatar_panel.history_confirm=Some(crate::avatar_history::Action::Delete {id:photo.id.clone()});}
                        });
                    }
                    if page.more && ui.add_enabled(!self.avatar_panel.pending,egui::Button::new("Trang tiếp")).clicked(){history_action=Some(crate::avatar_history::Action::Read {page:page.page+1,photo:page.next.clone(),album:page.album.clone()});}
                }
                if let Some(action)=self.avatar_panel.history_confirm.clone() {
                    let (verb,id)=match &action {crate::avatar_history::Action::Reuse {id}=>("dùng lại",&id.text),crate::avatar_history::Action::Delete {id}=>("xóa khỏi lịch sử",&id.text),_=>unreachable!()};
                    ui.label(format!("Xác nhận {verb} ảnh {id}? Dùng lại không đăng lên nhật ký."));
                    if ui.add_enabled(!self.avatar_panel.pending && !self.avatar_panel.uncertain && !self.avatar_panel.history_uncertain,egui::Button::new("Xác nhận thao tác ảnh")).clicked(){history_action=Some(action);self.avatar_panel.history_confirm=None;}
                    if ui.button("Hủy thao tác ảnh").clicked(){self.avatar_panel.history_confirm=None;}
                }
            }
            if self.avatar_panel.uncertain {ui.label("Chưa rõ kết quả tải lên. Kiểm tra ảnh trên Zalo chính thức trước khi chọn một thay đổi mới.");}
        });
        self.avatar_panel.open = open;
        if let Some(action) = history_action {
            if self.profile_panel.pending {
                self.avatar_panel.status = "Đang xử lý hồ sơ; chờ kết quả trước".into();
            } else if let Some(tx) = WORKER_TX.get() {
                self.avatar_panel.generation = crate::search::generation();
                self.avatar_panel.pending = true;
                if tx
                    .send(WorkerCmd::AvatarHistory {
                        owner: self.user_uin.clone(),
                        generation: self.avatar_panel.generation,
                        action,
                    })
                    .is_err()
                {
                    self.avatar_panel.pending = false;
                    self.avatar_panel.status = "Worker chưa sẵn sàng".into();
                }
            }
        }
        if pick {
            self.avatar_panel.generation = crate::search::generation();
            self.avatar_panel.pending = true;
            self.avatar_panel.uncertain = false;
            let tx = self.event_tx.clone();
            let owner = self.user_uin.clone();
            let generation = self.avatar_panel.generation;
            let context = ctx.clone();
            std::thread::spawn(move || {
                let path =
                    pick_native_files_dialog(true).and_then(|paths| paths.into_iter().next());
                let _ = tx.send(GuiEvent::AvatarPicked {
                    owner,
                    generation,
                    path,
                });
                context.request_repaint();
            });
        }
        if upload {
            if self.profile_panel.pending
                && matches!(self.avatar_panel.target, Some(crate::avatar::Target::Own))
            {
                self.avatar_panel.status = "Đang xử lý hồ sơ; chờ kết quả rồi tải ảnh lên".into();
                return;
            }
            if let (Some(target), Some(draft), Some(tx)) = (
                self.avatar_panel.target.clone(),
                self.avatar_panel.draft.clone(),
                WORKER_TX.get(),
            ) {
                self.avatar_panel.generation = crate::search::generation();
                self.avatar_panel.pending = true;
                let client = format!("{}{}", self.user_uin, crate::chat::next_client_message_id());
                if tx
                    .send(WorkerCmd::UploadAvatar {
                        owner: self.user_uin.clone(),
                        generation: self.avatar_panel.generation,
                        target,
                        draft,
                        client,
                    })
                    .is_err()
                {
                    self.avatar_panel.pending = false;
                    self.avatar_panel.status = "Worker chưa sẵn sàng".into();
                }
            }
        }
    }
}
