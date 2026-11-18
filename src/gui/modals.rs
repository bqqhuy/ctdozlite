use super::*;

fn mark_preview_attempt(ctx: &egui::Context, key: Id, url: &str, edit: std::time::Instant) -> bool {
    ctx.data_mut(|data| {
        if data
            .get_temp::<(String, std::time::Instant)>(key)
            .is_some_and(|(previous, at)| previous == url && at == edit)
        {
            return false;
        }
        data.insert_temp(key, (url.to_owned(), edit));
        true
    })
}

#[cfg(test)]
mod preview_attempt_tests {
    use super::*;

    #[test]
    fn unchanged_failed_draft_only_attempts_once_until_edited_or_url_changes() {
        let ctx = egui::Context::default();
        let key = Id::new("fixture-preview");
        let edit = std::time::Instant::now();
        assert!(mark_preview_attempt(
            &ctx,
            key,
            "https://example.invalid/one",
            edit
        ));
        // A failed worker response clears the loading set, but not this draft attempt.
        for _ in 0..20 {
            assert!(!mark_preview_attempt(
                &ctx,
                key,
                "https://example.invalid/one",
                edit
            ));
        }
        assert!(mark_preview_attempt(
            &ctx,
            key,
            "https://example.invalid/one",
            edit + Duration::from_millis(1)
        ));
        assert!(mark_preview_attempt(
            &ctx,
            key,
            "https://example.invalid/two",
            edit + Duration::from_millis(1)
        ));
    }
}

#[derive(Default)]
pub(super) struct SettingsDevices {
    pub generation: u64,
    pub pending: bool,
    pub devices: Option<crate::media::CallDeviceLists>,
    pub error: String,
}

impl ZaloGuiApp {
    fn request_settings_devices(&mut self, refresh: bool) {
        if self.settings_devices.pending {
            return;
        }
        self.settings_devices.generation = crate::search::generation();
        self.settings_devices.pending = true;
        self.settings_devices.error.clear();
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::ListSettingsDevices {
                generation: self.settings_devices.generation,
                refresh,
            })
            .is_ok()
        }) {
            self.settings_devices.pending = false;
            self.settings_devices.error = "Worker chưa sẵn sàng".into();
        }
    }
}

/// Paint a standard dimmed backdrop behind centered modals.
pub fn render_modal_backdrop(ctx: &egui::Context, id: &str) {
    let screen = ctx.screen_rect();
    egui::Area::new(Id::new(id))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter()
                .rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(110));
        });
}

impl ZaloGuiApp {
    pub(super) fn render_profile_popover(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if !self.profile_menu_open {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.profile_menu_open = false;
            return;
        }

        let mut close_menu = false;
        let mut do_logout = false;
        let area_resp = egui::Area::new(Id::new("left_rail_profile_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(68.0, 44.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(12.0))
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(220.0);
                        let display_user = if self.user_name.is_empty() {
                            "Người dùng Zalo"
                        } else {
                            &self.user_name
                        };
                        ui.label(
                            RichText::new(display_user)
                                .size(14.0)
                                .color(pal.text_primary)
                                .strong(),
                        );
                        ui.horizontal(|ui| {
                            let (dr, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                            ui.painter()
                                .circle_filled(dr.center(), 3.8, pal.accent_online);
                            ui.label(
                                RichText::new(crate::i18n::tr("Đang hoạt động"))
                                    .size(11.5)
                                    .color(pal.text_secondary),
                            );
                        });
                        if !self.user_uin.is_empty() {
                            ui.label(
                                RichText::new(format!("UIN: {}", self.user_uin))
                                    .size(10.5)
                                    .color(pal.text_muted),
                            );
                        }

                        if ui.button("Hồ sơ của tôi…").clicked() {
                            self.open_own_profile();
                            close_menu = true;
                        }

                        // Keep account selection in this single foreground menu surface.
                        if self.profiles.len() > 1 {
                            let profiles_list: Vec<(&str, &crate::profiles::Profile)> = self
                                .profiles
                                .iter()
                                .map(|profile| (profile.display_name.as_str(), profile))
                                .collect();
                            let active = profiles_list
                                .iter()
                                .find(|(_, profile)| profile.uid == self.user_uin)
                                .map(|(_, profile)| *profile);
                            if let Some(target_uid) =
                                self.account_switcher.show(ui, active, &profiles_list)
                            {
                                if target_uid != self.user_uin {
                                    if let Some(tx) = WORKER_TX.get() {
                                        if tx
                                            .send(WorkerCmd::SwitchProfile {
                                                owner: self.user_uin.clone(),
                                                target_uid,
                                            })
                                            .is_err()
                                        {
                                            self.show_toast(
                                                "Không kết nối được bộ xử lý tài khoản",
                                            );
                                        }
                                    }
                                }
                                close_menu = true;
                            }
                        }
                        if ui.button("Thêm tài khoản").clicked() {
                            if let Some(tx) = WORKER_TX.get() {
                                if tx.send(WorkerCmd::AddAccount).is_err() {
                                    self.show_toast("Không kết nối được bộ xử lý tài khoản");
                                }
                            }
                            close_menu = true;
                        }
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        if ui
                            .add_sized(
                                [200.0, 28.0],
                                egui::Button::new(
                                    RichText::new(format!("{}  Đã đồng bộ đám mây", lucide::CHECK))
                                        .size(12.2)
                                        .color(pal.text_primary),
                                )
                                .fill(Color32::TRANSPARENT),
                            )
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::RefreshThreads);
                            }
                            self.show_toast("Đang cập nhật hội thoại và tin gần đây…");
                            close_menu = true;
                        }

                        let theme_lbl = if self.dark_mode {
                            format!("{}  Chuyển giao diện Sáng", lucide::SUN)
                        } else {
                            format!("{}  Chuyển giao diện Tối", lucide::MOON)
                        };
                        if ui
                            .add_sized(
                                [200.0, 28.0],
                                egui::Button::new(
                                    RichText::new(theme_lbl).size(12.2).color(pal.text_primary),
                                )
                                .fill(Color32::TRANSPARENT),
                            )
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.dark_mode = !self.dark_mode;
                            self.theme_dirty = true;
                            close_menu = true;
                        }

                        if self.dev_mode {
                            let dev_lbl = if self.dev_panel_open {
                                "Đóng Dev Inspector"
                            } else {
                                "Mở Dev Inspector"
                            };
                            if ui
                                .add_sized(
                                    [200.0, 28.0],
                                    egui::Button::new(
                                        RichText::new(dev_lbl).size(12.2).color(pal.warning),
                                    )
                                    .fill(Color32::TRANSPARENT),
                                )
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.dev_panel_open = !self.dev_panel_open;
                                if self.dev_panel_open {
                                    self.scanner_findings = logger::scan_logs();
                                }
                                close_menu = true;
                            }
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        if ui
                            .add_sized(
                                [200.0, 30.0],
                                egui::Button::new(
                                    RichText::new(crate::i18n::tr("Đăng xuất tài khoản"))
                                        .size(12.5)
                                        .color(pal.danger)
                                        .strong(),
                                )
                                .fill(pal.danger.linear_multiply(0.12))
                                .rounding(Rounding::same(7.0)),
                            )
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            do_logout = true;
                            close_menu = true;
                        }
                        ui.add_space(4.0);
                        if ui
                            .add_sized(
                                [200.0, 28.0],
                                egui::Button::new(
                                    RichText::new(format!(
                                        "{}  Thoát hoàn toàn (Ctrl+Q)",
                                        lucide::X
                                    ))
                                    .size(12.0)
                                    .color(pal.text_secondary),
                                )
                                .fill(pal.bg_input)
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .rounding(Rounding::same(7.0)),
                            )
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.allow_quit = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
            });

        if ctx.input(|i| i.pointer.any_click())
            && !area_resp.response.hovered()
            && ctx
                .input(|i| i.pointer.interact_pos())
                .map(|p| p.x > 64.0 || p.y > 60.0)
                .unwrap_or(false)
        {
            close_menu = true;
        }

        if do_logout {
            self.flush_composer_before_logout();
            self.cancel_voice_recording();
            // L-03: Two-phase logout. Do not switch UI or clear session until worker confirms LoggedOut.
            self.status_msg = "Đang đăng xuất…".into();
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::Logout);
            }
        }
        if close_menu {
            self.profile_menu_open = false;
        }
    }

    pub(super) fn render_toast_overlay(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let Some((ref text, ts)) = self.toast_notice else {
            return;
        };
        if ts.elapsed() > Duration::from_millis(2500) {
            self.toast_notice = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(200));
        let screen = ctx.screen_rect();
        egui::Area::new(Id::new("floating_toast_notice"))
            .order(egui::Order::Tooltip)
            .fixed_pos(Pos2::new(
                (screen.right() - 320.0).max(80.0),
                screen.bottom() - 54.0,
            ))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(Margin::symmetric(14.0, 9.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (dr, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                            ui.painter()
                                .circle_filled(dr.center(), 4.0, pal.accent_online);
                            ui.label(
                                RichText::new(text)
                                    .size(12.0)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                        });
                    });
            });
    }

    pub(super) fn render_phone_lookup_modal(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if !self.phone_modal_open {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.phone_modal_open = false;
            return;
        }

        render_modal_backdrop(ctx, "phone_modal_backdrop");

        let mut open = true;
        let mut trigger_lookup: Option<String> = None;
        let mut open_chat_user: Option<crate::chat::PhoneLookupResult> = None;
        let mut add_friend_user: Option<(String, String)> = None;

        egui::Window::new(crate::i18n::tr("Tìm kiếm Số điện thoại & Kết bạn Zalo"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width((ctx.screen_rect().width() * 0.78).clamp(280.0, 390.0))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(pal.bg_elevated)
                    .rounding(Rounding::same(14.0))
                    .stroke(Stroke::new(1.2_f32, pal.primary))
                    .inner_margin(Margin::symmetric(20.0, 16.0)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(crate::i18n::tr(
                        "Nhập số điện thoại để tìm tài khoản Zalo (VD: 0912345678 hoặc +84...):",
                    ))
                    .size(12.2)
                    .color(pal.text_secondary),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    let te = egui::TextEdit::singleline(&mut self.phone_modal_input)
                        .hint_text(crate::i18n::tr("Số điện thoại Zalo..."))
                        .margin(Margin::symmetric(10.0, 7.0))
                        .desired_width(250.0);
                    let resp = ui.add(te);
                    let enter = (resp.lost_focus() || resp.has_focus())
                        && ui.input(|i| i.key_pressed(Key::Enter));

                    let search_btn = egui::Button::new(
                        RichText::new(crate::i18n::tr("Tìm kiếm"))
                            .size(12.8)
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(pal.primary)
                    .rounding(Rounding::same(8.0))
                    .min_size(Vec2::new(88.0, 32.0));

                    if (ui
                        .add(search_btn)
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                        || enter)
                        && !self.phone_modal_input.trim().is_empty()
                    {
                        trigger_lookup = Some(self.phone_modal_input.trim().to_string());
                    }
                });

                ui.add_space(12.0);

                if self.phone_lookup_loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(16.0);
                        ui.spinner();
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(crate::i18n::tr(
                                "Đang tra cứu hồ sơ Zalo theo số điện thoại…",
                            ))
                            .size(12.5)
                            .color(pal.text_secondary),
                        );
                        ui.add_space(12.0);
                    });
                } else if let Some(ref err) = self.phone_lookup_error {
                    egui::Frame::none()
                        .fill(pal.danger.linear_multiply(0.14))
                        .stroke(Stroke::new(1.0_f32, pal.danger))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(Margin::symmetric(12.0, 10.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("Không tìm thấy: {err}"))
                                    .size(12.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                        });
                } else if let Some(ref prof) = self.phone_lookup_result {
                    egui::Frame::none()
                        .fill(pal.bg_input)
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .rounding(Rounding::same(12.0))
                        .inner_margin(Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            let resolved_phone_name = resolve_contact_name_from(
                                &self.server_aliases,
                                &prof.user_id,
                                &prof.display_name,
                            );
                            ui.horizontal(|ui| {
                                let (av_rect, _) =
                                    ui.allocate_exact_size(Vec2::splat(54.0), Sense::hover());
                                paint_user_avatar(
                                    ui.painter(),
                                    av_rect.center(),
                                    27.0,
                                    resolved_phone_name,
                                    false,
                                    Some(prof.avatar.as_str()),
                                    &self.sticker_textures,
                                    &mut self.loading_stickers,
                                );

                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(resolved_phone_name)
                                                .size(15.0)
                                                .color(pal.text_primary)
                                                .strong(),
                                        );
                                        let (f_lbl, f_col) = match prof.friendship {
                                            crate::chat::FriendshipStatus::Friend => {
                                                ("✓ Bạn bè", pal.accent_online)
                                            }
                                            crate::chat::FriendshipStatus::IncomingPending => {
                                                ("Chờ bạn đồng ý", pal.primary_hover)
                                            }
                                            crate::chat::FriendshipStatus::OutgoingPending => {
                                                ("Đã gửi lời mời", pal.warning)
                                            }
                                            crate::chat::FriendshipStatus::Stranger => {
                                                ("Chưa kết bạn", Color32::from_rgb(249, 115, 22))
                                            }
                                            crate::chat::FriendshipStatus::Unknown => {
                                                ("Đang kiểm tra…", pal.text_muted)
                                            }
                                        };
                                        egui::Frame::none()
                                            .fill(f_col.linear_multiply(0.15))
                                            .rounding(Rounding::same(5.0))
                                            .inner_margin(Margin::symmetric(6.0, 2.0))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(f_lbl)
                                                        .size(10.5)
                                                        .color(f_col)
                                                        .strong(),
                                                );
                                            });
                                    });
                                    let zalo_sub = (!prof.zalo_name.trim().is_empty()
                                        && prof.zalo_name.trim() != resolved_phone_name)
                                        .then_some(prof.zalo_name.trim())
                                        .or_else(|| {
                                            (!prof.display_name.trim().is_empty()
                                                && prof.display_name.trim() != resolved_phone_name)
                                                .then_some(prof.display_name.trim())
                                        });
                                    if let Some(zalo_name) = zalo_sub {
                                        ui.label(
                                            RichText::new(format!("Tên Zalo: {zalo_name}"))
                                                .size(11.5)
                                                .color(pal.text_muted),
                                        );
                                    }
                                    ui.label(
                                        RichText::new(format!(
                                            "SĐT: {} · UID: {}",
                                            prof.phone, prof.user_id
                                        ))
                                        .size(11.5)
                                        .color(pal.text_secondary),
                                    );
                                    if !prof.status.is_empty() {
                                        ui.label(
                                            RichText::new(&prof.status)
                                                .size(11.5)
                                                .color(pal.text_muted),
                                        );
                                    }
                                });
                            });

                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                let chat_btn = egui::Button::new(
                                    RichText::new(crate::i18n::tr("Nhắn tin ngay"))
                                        .size(12.5)
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(pal.primary)
                                .rounding(Rounding::same(8.0))
                                .min_size(Vec2::new(155.0, 32.0));
                                if ui
                                    .add(chat_btn)
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    open_chat_user = Some(prof.clone());
                                }

                                match prof.friendship {
                                    crate::chat::FriendshipStatus::Friend => {
                                        let badge_btn = egui::Button::new(
                                            RichText::new(format!(
                                                "{} Đã là bạn bè",
                                                lucide::CHECK
                                            ))
                                            .size(12.2)
                                            .color(pal.accent_online)
                                            .strong(),
                                        )
                                        .fill(pal.accent_online.linear_multiply(0.14))
                                        .stroke(Stroke::new(1.0_f32, pal.accent_online))
                                        .rounding(Rounding::same(8.0))
                                        .min_size(Vec2::new(155.0, 32.0));
                                        ui.add_enabled(false, badge_btn);
                                    }
                                    crate::chat::FriendshipStatus::OutgoingPending => {
                                        let pending_btn = egui::Button::new(
                                            RichText::new(crate::i18n::tr("Đã gửi lời mời"))
                                                .size(12.2)
                                                .color(pal.warning)
                                                .strong(),
                                        )
                                        .fill(pal.warning.linear_multiply(0.14))
                                        .stroke(Stroke::new(1.0_f32, pal.warning))
                                        .rounding(Rounding::same(8.0))
                                        .min_size(Vec2::new(155.0, 32.0));
                                        ui.add_enabled(false, pending_btn);
                                    }
                                    crate::chat::FriendshipStatus::IncomingPending => {
                                        let accept_btn = egui::Button::new(
                                            RichText::new(format!(
                                                "{} Đồng ý kết bạn",
                                                lucide::CHECK
                                            ))
                                            .size(12.5)
                                            .color(Color32::WHITE)
                                            .strong(),
                                        )
                                        .fill(pal.accent_online)
                                        .rounding(Rounding::same(8.0))
                                        .min_size(Vec2::new(155.0, 32.0));
                                        if ui
                                            .add(accept_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            add_friend_user = Some((
                                                prof.user_id.clone(),
                                                prof.display_name.clone(),
                                            ));
                                        }
                                    }
                                    _ => {
                                        let friend_btn = egui::Button::new(
                                            RichText::new(format!(
                                                "{} Kết bạn Zalo",
                                                lucide::USER_PLUS
                                            ))
                                            .size(12.5)
                                            .color(pal.accent_online)
                                            .strong(),
                                        )
                                        .fill(pal.accent_online.linear_multiply(0.15))
                                        .stroke(Stroke::new(1.0_f32, pal.accent_online))
                                        .rounding(Rounding::same(8.0))
                                        .min_size(Vec2::new(155.0, 32.0));
                                        if ui
                                            .add(friend_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            add_friend_user = Some((
                                                prof.user_id.clone(),
                                                prof.display_name.clone(),
                                            ));
                                        }
                                    }
                                }
                            });
                        });
                }
            });

        if let Some(phone_q) = trigger_lookup {
            self.phone_lookup_loading = true;
            self.phone_lookup_error = None;
            self.phone_lookup_result = None;
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::LookupPhone { phone: phone_q });
            }
        }

        if let Some(prof) = open_chat_user {
            let opened_name = self
                .resolve_contact_name(&prof.user_id, &prof.display_name)
                .to_owned();
            if let Some(idx) = self.threads.iter().position(|t| t.id == prof.user_id) {
                self.selected_thread_index = idx;
            } else {
                self.threads.insert(
                    0,
                    Thread {
                        presentation: Default::default(),
                        unread: Default::default(),
                        id: prof.user_id.clone(),
                        name: prof.display_name.clone(),
                        preview: format!("SĐT: {}", prof.phone),
                        time: crate::ui::clock(),
                        avatar: if prof.avatar.is_empty() {
                            None
                        } else {
                            Some(prof.avatar)
                        },
                        is_group: false,
                        messages: Vec::new(),
                    },
                );
                self.apply_server_aliases();
                self.selected_thread_index = self
                    .threads
                    .iter()
                    .position(|t| t.id == prof.user_id)
                    .unwrap_or(0);
                self.persist_threads_async();
            }
            self.search_query.clear();
            self.phone_modal_open = false;
            self.show_toast(format!("Đã mở cuộc trò chuyện với {opened_name}"));
        }

        if let Some((uid, uname)) = add_friend_user {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::AddFriend {
                    user_id: uid,
                    user_name: uname.clone(),
                });
            }
            self.show_toast(format!("Đang gửi lời mời kết bạn tới {uname}…"));
        }

        if !open {
            self.phone_modal_open = false;
        }
    }
    pub(super) fn render_settings_modal(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if !self.settings_modal_open {
            return;
        }
        if !self.pin_setup_modal_open && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.settings_modal_open = false;
            self.save_settings();
            return;
        }

        render_modal_backdrop(ctx, "settings_modal_backdrop");

        let previous_remember = self.settings.remember_login;
        let mut open = true;
        let screen = ctx.screen_rect();
        let win_w = (screen.width() - 16.0).clamp(160.0, 800.0);
        let win_h = (screen.height() - 16.0).clamp(160.0, 560.0);
        let compact = win_w < 700.0;
        let sidebar_w = if compact { 48.0 } else { 200.0 };

        let mut theme_switched = false;
        let mut trigger_sync = false;
        let mut pick_dl_dir = false;
        let mut open_dl_dir = false;

        let response = egui::Window::new("settings_window_modal")
            .title_bar(false)
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(win_w, win_h))
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_chat_canvas)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(12.0))
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    if !compact {
                    // Left Tabs Navigation Sidebar (200px)
                    egui::Frame::none()
                        .fill(pal.bg_sidebar)
                        .inner_margin(Margin::symmetric(10.0, 16.0))
                        .show(ui, |ui| {
                            ui.set_width(sidebar_w);
                            ui.set_height(win_h - 32.0);
                            ui.vertical(|ui| {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(crate::i18n::tr("Cài đặt"))
                                        .size(17.5)
                                        .color(pal.text_primary)
                                        .strong(),
                                );
                                ui.add_space(14.0);

                                let tabs = [
                                    (SettingsTab::General, lucide::SETTINGS, "Cài đặt chung"),
                                    (SettingsTab::AccountSecurity, lucide::SHIELD_CHECK, "Tài khoản & bảo mật"),
                                    (SettingsTab::Privacy, lucide::LOCK, "Quyền riêng tư"),
                                    (SettingsTab::Sync, lucide::REFRESH, "Đồng bộ tin nhắn"),
                                    (SettingsTab::DataManagement, lucide::FILE_TEXT, "Quản lý dữ liệu"),
                                    (SettingsTab::Appearance, lucide::PAINTBRUSH, "Giao diện"),
                                    (SettingsTab::Notifications, lucide::BELL, "Thông báo"),
                                    (SettingsTab::Messages, lucide::MESSAGE_SQUARE, "Tin nhắn"),
                                    (SettingsTab::Calls, lucide::PHONE_CALL, "Cuộc gọi"),
                                    (SettingsTab::Utilities, lucide::SLIDERS, "Tiện ích"),
                                ];

                                for (tab, icon, title) in tabs {
                                    let is_active = self.settings_tab == tab;
                                    let text_col = if is_active {
                                        pal.primary_hover
                                    } else {
                                        pal.text_secondary
                                    };

                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(sidebar_w - 4.0, 36.0),
                                        Sense::click(),
                                    );
                                    if is_active || resp.hovered() {
                                        ui.painter().rect_filled(
                                            rect,
                                            Rounding::same(8.0),
                                            if is_active {
                                                pal.bg_selected
                                            } else {
                                                pal.bg_hover
                                            },
                                        );
                                    }
                                    let icon_pt = Pos2::new(rect.left() + 18.0, rect.center().y);
                                    draw_lucide_icon(
                                        ui.painter(),
                                        icon_pt,
                                        icon,
                                        14.5,
                                        text_col,
                                    );
                                    if !compact {
                                    let text_pt = Pos2::new(rect.left() + 38.0, rect.center().y);
                                    ui.painter().text(
                                        text_pt,
                                        egui::Align2::LEFT_CENTER,
                                        title,
                                        FontId::proportional(13.2),
                                        if is_active {
                                            pal.text_primary
                                        } else {
                                            text_col
                                        },
                                    );

                                    }

                                    if resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        self.settings_tab = tab;
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                        });

                    }
                    // Right Content Panel
                    let content_w = (win_w - if compact { 0.0 } else { sidebar_w + 20.0 } - 48.0).max(64.0);
                    egui::Frame::none()
                        .fill(pal.bg_chat_canvas)
                        .inner_margin(Margin::symmetric(24.0, 16.0))
                        .show(ui, |ui| {
                            // The surrounding row is horizontal. Explicitly stack
                            // the page header and scroll viewport vertically.
                            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            ui.set_width(content_w);
                            ui.set_height(win_h - 32.0);

                            ui.horizontal(|ui| {
                                let tab_title = match self.settings_tab {
                                    SettingsTab::General => "Cài đặt chung",
                                    SettingsTab::AccountSecurity => "Tài khoản & bảo mật",
                                    SettingsTab::Privacy => "Quyền riêng tư",
                                    SettingsTab::Sync => "Đồng bộ tin nhắn",
                                    SettingsTab::DataManagement => "Quản lý dữ liệu",
                                    SettingsTab::Appearance => "Cài đặt giao diện",
                                    SettingsTab::Notifications => "Cài đặt thông báo",
                                    SettingsTab::Messages => "Cài đặt tin nhắn",
                                    SettingsTab::Calls => "Cài đặt cuộc gọi",
                                    SettingsTab::Utilities => "Tiện ích & Gợi ý",
                                };
                                if compact {
                                    egui::ComboBox::from_id_salt("settings_compact_tab")
                                        .width((content_w - 70.0).max(40.0))
                                        .selected_text(tab_title)
                                        .show_ui(ui, |ui| {
                                            for (tab, title) in [
                                                (SettingsTab::General,"Cài đặt chung"),
                                                (SettingsTab::AccountSecurity,"Tài khoản & bảo mật"),
                                                (SettingsTab::Privacy,"Quyền riêng tư"),
                                                (SettingsTab::Sync,"Lịch sử tin nhắn"),
                                                (SettingsTab::DataManagement,"Quản lý dữ liệu"),
                                                (SettingsTab::Appearance,"Giao diện"),
                                                (SettingsTab::Notifications,"Thông báo"),
                                                (SettingsTab::Messages,"Tin nhắn"),
                                                (SettingsTab::Calls,"Cuộc gọi"),
                                                (SettingsTab::Utilities,"Tiện ích"),
                                            ] { ui.selectable_value(&mut self.settings_tab,tab,crate::i18n::tr(title)); }
                                        });
                                } else {
                                ui.label(
                                    RichText::new(tab_title)
                                        .size(15.5)
                                        .color(pal.text_primary)
                                        .strong(),
                                );
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let (cr, c_resp) = ui.allocate_exact_size(
                                        Vec2::splat(26.0),
                                        Sense::click(),
                                    );
                                    if c_resp.hovered() {
                                        ui.painter().rect_filled(
                                            cr,
                                            Rounding::same(6.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    draw_lucide_icon(
                                        ui.painter(),
                                        cr.center(),
                                        lucide::X,
                                        14.0,
                                        if c_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if c_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        open = false;
                                    }
                                });
                            });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            egui::ScrollArea::both()
                                .max_width(content_w)
                                .min_scrolled_width(0.0)
                                .max_height((win_h - 100.0).max(32.0))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    match self.settings_tab {
                                        SettingsTab::General => {
                                            ui.label(RichText::new(crate::i18n::tr("Danh bạ")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(2.0);
                                            ui.label(RichText::new(crate::i18n::tr("Chọn hiển thị bạn bè Zalo hoặc cả những người đã có hội thoại trên thiết bị này")).color(pal.text_secondary).size(12.0));
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.set_min_width(ui.available_width());
                                                    ui.radio_value(&mut self.settings.show_all_contacts, true, "Bạn bè và người đã liên hệ");
                                                        ui.add_space(4.0);
                                                        ui.radio_value(&mut self.settings.show_all_contacts, false, "Chỉ hiển thị bạn bè đang sử dụng Zalo");
                                                    ui.label("Danh bạ điện thoại chưa dùng Zalo chưa được nhập vào ứng dụng.");
                                                });
                                            ui.add_space(14.0);

                                            ui.label(RichText::new(crate::i18n::tr("Ngôn ngữ")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Thay đổi ngôn ngữ")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            egui::ComboBox::from_id_salt("setting_lang_combo")
                                                                .selected_text(&self.settings.language)
                                                                .width(130.0)
                                                                .show_ui(ui, |ui| {
                                                                    ui.selectable_value(&mut self.settings.language, "Tiếng Việt".into(), "Tiếng Việt");
                                                                    ui.selectable_value(&mut self.settings.language, "English".into(), "English");
                                                                });
                                                        });
                                                    });
                                                });
                                            ui.add_space(14.0);

                                            ui.label(RichText::new(crate::i18n::tr("Khởi động & Lưu File")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Khởi động Zalo khi mở máy")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            ui.add_enabled_ui(cfg!(windows), |ui| { render_toggle_switch(ui, &mut self.settings.autostart_on_boot, pal); });
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Ghi nhớ tài khoản đăng nhập lần trước")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.remember_login, pal);
                                                        });
                                                    });
                                                    ui.add_space(10.0);
                                                    ui.separator();
                                                    ui.add_space(6.0);
                                                    ui.label(RichText::new(crate::i18n::tr("File được lưu tại:")).color(pal.text_secondary).size(12.2));
                                                    ui.add_space(4.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            RichText::new(truncate_chars(&self.settings.download_dir, 36))
                                                                .size(12.0)
                                                                .color(pal.text_muted),
                                                        ).on_hover_text(&self.settings.download_dir);
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            if ui.add_enabled(!self.download_picker_pending,egui::Button::new(crate::i18n::tr("Thay đổi"))).clicked() {
                                                                pick_dl_dir = true;
                                                            }
                                                            if ui.button(crate::i18n::tr("Mở thư mục")).clicked() {
                                                                open_dl_dir = true;
                                                            }
                                                        });
                                                    });
                                                });
                                            ui.add_space(14.0);

                                            ui.label(RichText::new(crate::i18n::tr("Thiết bị Âm thanh & Video (Cuộc gọi)")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    if self.smoke.is_none() && self.settings_devices.devices.is_none()
                                                        && !self.settings_devices.pending && self.settings_devices.error.is_empty() {
                                                        self.request_settings_devices(false);
                                                    }
                                                    let (mics, _speakers, cams) = if self.smoke.is_some() {
                                                        (vec!["Fixture microphone".into()], vec!["Fixture speaker".into()], vec!["Fixture camera".into()])
                                                    } else { self.settings_devices.devices.clone().unwrap_or_default() };
                                                    if self.settings_devices.pending { ui.horizontal(|ui| { ui.spinner(); ui.label("Đang quét thiết bị…"); }); }
                                                    if !self.settings_devices.error.is_empty() { ui.label(&self.settings_devices.error); }
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(format!("{} Đầu vào Mic:", lucide::MIC)).color(pal.text_primary).size(13.0));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            let cur_mic_lbl = if self.settings.selected_mic_device.is_empty() {
                                                                "Mặc định hệ thống"
                                                            } else {
                                                                &self.settings.selected_mic_device
                                                            };
                                                            egui::ComboBox::from_id_salt("setting_mic_device_combo")
                                                                .selected_text(truncate_chars(cur_mic_lbl, 30))
                                                                .width(230.0)
                                                                .show_ui(ui, |ui| {
                                                                    if ui.selectable_label(self.settings.selected_mic_device.is_empty(), crate::i18n::tr("Mặc định hệ thống")).clicked() {
                                                                        self.settings.selected_mic_device.clear();
                                                                        crate::media::set_selected_mic_device("");
                                                                        self.save_settings();
                                                                    }
                                                                    for m in &mics {
                                                                        if ui.selectable_label(self.settings.selected_mic_device == *m, m).clicked() {
                                                                            self.settings.selected_mic_device = m.clone();
                                                                            crate::media::set_selected_mic_device(m);
                                                                            self.save_settings();
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(format!("{} Máy ảnh (Camera):", lucide::VIDEO)).color(pal.text_primary).size(13.0));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            let cur_cam_lbl = if self.settings.selected_camera_device.is_empty() {
                                                                cams.first().map(String::as_str).unwrap_or("Mặc định hệ thống")
                                                            } else {
                                                                &self.settings.selected_camera_device
                                                            };
                                                            egui::ComboBox::from_id_salt("setting_cam_device_combo")
                                                                .selected_text(truncate_chars(cur_cam_lbl, 30))
                                                                .width(230.0)
                                                                .show_ui(ui, |ui| {
                                                                    if ui.selectable_label(self.settings.selected_camera_device.is_empty(), crate::i18n::tr("Tự động (Camera đầu tiên)")).clicked() {
                                                                        self.settings.selected_camera_device.clear();
                                                                        crate::media::set_selected_camera_device("");
                                                                        self.save_settings();
                                                                    }
                                                                    for c in &cams {
                                                                        if ui.selectable_label(self.settings.selected_camera_device == *c, c).clicked() {
                                                                            self.settings.selected_camera_device = c.clone();
                                                                            crate::media::set_selected_camera_device(c);
                                                                            self.save_settings();
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                    });
                                                    ui.add_space(6.0);
                                                    ui.horizontal(|ui| {
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            if ui.add_enabled(!self.settings_devices.pending, egui::Button::new(format!("{} Quét lại thiết bị", lucide::REFRESH))).clicked() {
                                                                self.request_settings_devices(true);
                                                            }
                                                        });
                                                    });
                                                });
                                        }
                                        SettingsTab::AccountSecurity => {
                                            ui.label(
                                                RichText::new(crate::i18n::tr("Khóa ứng dụng Zalo"))
                                                    .color(pal.text_primary)
                                                    .size(13.5)
                                                    .strong(),
                                            );
                                            ui.add_space(2.0);
                                            ui.label(
                                                RichText::new(crate::i18n::tr(
                                                    "Bảo vệ ứng dụng bằng mã PIN 4 số hoặc Windows Hello khi khởi động và khi khóa nhanh (Ctrl+L)",
                                                ))
                                                .color(pal.text_secondary)
                                                .size(12.0),
                                            );
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 12.0))
                                                .show(ui, |ui| {
                                                    ui.set_min_width(ui.available_width());

                                                    // Row 1: Bật/Tắt Khóa ứng dụng Zalo
                                                    ui.horizontal(|ui| {
                                                        ui.vertical(|ui| {
                                                            ui.label(
                                                                RichText::new(crate::i18n::tr("Khóa ứng dụng Zalo"))
                                                                    .color(pal.text_primary)
                                                                    .size(13.2)
                                                                    .strong(),
                                                            );
                                                            ui.label(
                                                                RichText::new(crate::i18n::tr(
                                                                    "Yêu cầu xác thực khi mở ứng dụng hoặc sau 5 phút không hoạt động",
                                                                ))
                                                                .color(pal.text_secondary)
                                                                .size(11.8),
                                                            );
                                                        });
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            let mut lock_toggle = self.settings.enable_app_lock;
                                                            if render_toggle_switch(ui, &mut lock_toggle, pal).changed() {
                                                                if lock_toggle {
                                                                    if self.settings.has_pin() {
                                                                        self.settings.enable_app_lock = true;
                                                                        self.save_settings();
                                                                    } else {
                                                                        self.pin_setup_modal_open = true;
                                                                        self.pin_setup_enable_after = true;
                                                                        self.pin_setup_first.clear();
                                                                        self.pin_setup_confirm.clear();
                                                                        self.pin_setup_error = None;
                                                                    }
                                                                } else {
                                                                    self.settings.enable_app_lock = false;
                                                                    self.save_settings();
                                                                }
                                                            }
                                                        });
                                                    });

                                                    ui.add_space(10.0);
                                                    ui.separator();
                                                    ui.add_space(10.0);

                                                    // Row 2: Mã PIN 4 số
                                                    ui.horizontal(|ui| {
                                                        ui.vertical(|ui| {
                                                            ui.label(
                                                                RichText::new(crate::i18n::tr("Mã PIN 4 số"))
                                                                    .color(pal.text_primary)
                                                                    .size(13.2)
                                                                    .strong(),
                                                            );
                                                            let pin_status = if self.settings.has_pin() {
                                                                "Đã thiết lập mã PIN 4 số"
                                                            } else {
                                                                "Chưa thiết lập mã PIN"
                                                            };
                                                            ui.label(
                                                                RichText::new(crate::i18n::tr(pin_status))
                                                                    .color(pal.text_secondary)
                                                                    .size(11.8),
                                                            );
                                                        });
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            let btn_label = if self.settings.has_pin() {
                                                                "Đổi mã PIN"
                                                            } else {
                                                                "Tạo mã PIN"
                                                            };
                                                            if ui.button(crate::i18n::tr(btn_label)).clicked() {
                                                                self.pin_setup_modal_open = true;
                                                                self.pin_setup_enable_after = !self.settings.has_pin();
                                                                self.pin_setup_first.clear();
                                                                self.pin_setup_confirm.clear();
                                                                self.pin_setup_error = None;
                                                            }
                                                            if self.can_lock_app()
                                                                && ui.button(crate::i18n::tr("Khóa ngay (Ctrl+L)")).clicked()
                                                            {
                                                                open = false;
                                                                self.lock_app();
                                                            }
                                                        });
                                                    });

                                                    ui.add_space(10.0);
                                                    ui.separator();
                                                    ui.add_space(10.0);

                                                    // Row 3: Mở khóa bằng Windows Hello
                                                    let bio_avail = self.lock_guard.is_biometric_available();
                                                    let hello_tooltip = crate::i18n::tr(
                                                        "Thiết bị không hỗ trợ Windows Hello hoặc chưa được thiết lập",
                                                    );
                                                    ui.horizontal(|ui| {
                                                        let label_col = if bio_avail {
                                                            pal.text_primary
                                                        } else {
                                                            pal.text_muted
                                                        };
                                                        let sub_col = if bio_avail {
                                                            pal.text_secondary
                                                        } else {
                                                            pal.text_muted
                                                        };
                                                        let info_resp = ui
                                                            .vertical(|ui| {
                                                                ui.label(
                                                                    RichText::new(crate::i18n::tr("Mở khóa bằng Windows Hello"))
                                                                        .color(label_col)
                                                                        .size(13.2)
                                                                        .strong(),
                                                                );
                                                                ui.label(
                                                                    RichText::new(crate::i18n::tr(
                                                                        "Dùng vân tay, khuôn mặt hoặc Windows Hello thay cho mã PIN",
                                                                    ))
                                                                    .color(sub_col)
                                                                    .size(11.8),
                                                                );
                                                            })
                                                            .response;
                                                        if !bio_avail {
                                                            info_resp.on_hover_text(hello_tooltip);
                                                        }
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            let mut hello_toggle = self.settings.use_windows_hello;
                                                            let toggle_resp = ui
                                                                .add_enabled_ui(bio_avail, |ui| {
                                                                    render_toggle_switch(ui, &mut hello_toggle, pal)
                                                                })
                                                                .inner;
                                                            if bio_avail {
                                                                if toggle_resp.changed() {
                                                                    self.settings.use_windows_hello = hello_toggle;
                                                                    self.save_settings();
                                                                }
                                                            } else {
                                                                toggle_resp
                                                                    .on_hover_text(hello_tooltip)
                                                                    .on_disabled_hover_text(hello_tooltip);
                                                            }
                                                        });
                                                    });
                                                });
                                        }
                                        SettingsTab::Privacy => {
                                            self.render_server_privacy(ui);
                                        }
                                        SettingsTab::Sync => {
                                            if ui.button("Nhập backup Zalo Desktop V2…").clicked(){self.backup_panel.open=true;open=false;}
                                            if ui.button(crate::i18n::tr("Nhập / Xuất lịch sử…")).clicked(){self.tools.open=true;self.tools.tab=1;open=false;}

                                            ui.label(RichText::new(crate::i18n::tr("Đồng bộ tin nhắn")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(10.0))
                                                .inner_margin(Margin::same(14.0))
                                                .show(ui, |ui| {
                                                    ui.vertical(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Đồng bộ từ điện thoại yêu cầu xác nhận trong Zalo di động. Lịch sử được lưu vào SQLite theo tài khoản; chỉ khôi phục dữ liệu điện thoại còn giữ, không bảo đảm tệp đính kèm cũ còn hạn.")).color(pal.text_primary).size(13.0));
                                                        ui.add_space(4.0);
                                                        let total_msgs: usize = self.threads.iter().map(|t| t.messages.len()).sum();
                                                        ui.label(RichText::new(format!("Hiện có {} hội thoại ({total_msgs} tin nhắn) trong bộ nhớ.", self.threads.len())).color(pal.text_secondary).size(12.2));
                                                        ui.add_space(10.0);
                                                        if ui.button(RichText::new(format!("{} Cập nhật hội thoại", lucide::REFRESH)).color(pal.text_primary).size(13.0).strong()).clicked() {
                                                            trigger_sync = true;
                                                        }
                                                        ui.add_space(8.0);
                                                        ui.label(crate::i18n::tr("Tải dữ liệu máy chủ còn cung cấp: lịch sử nhóm và Cloud của tôi (tối đa 200 trang/hội thoại mỗi lượt). Máy chủ thường chỉ trả tin từ sau lần đăng nhập đầu tiên trên thiết bị này; tin cũ và chat cá nhân cần Đồng bộ từ điện thoại."));
                                                        let mut history_command = None;
                                                        ui.horizontal_wrapped(|ui| {
                                                            if ui.button(if self.history_sync_running { "Dừng tải" } else { "Tải dữ liệu máy chủ còn cung cấp" }).clicked() {
                                                                history_command = Some(if self.history_sync_running { WorkerCmd::CancelHistorySync }
                                                                    else { WorkerCmd::SyncHistory { threads: self.threads.clone(), retry_filtered: false } });
                                                            }
                                                            if !self.history_sync_running && ui.button(crate::i18n::tr("Thử lại hội thoại bị giới hạn")).on_hover_text(crate::i18n::tr("Hỏi lại máy chủ cả những hội thoại vừa bị giới hạn trong 24 giờ qua")).clicked() {
                                                                history_command = Some(WorkerCmd::SyncHistory { threads: self.threads.clone(), retry_filtered: true });
                                                            }
                                                        });
                                                        if let (Some(command), Some(tx)) = (history_command, WORKER_TX.get()) {
                                                            let cancel = matches!(command, WorkerCmd::CancelHistorySync);
                                                            if tx.send(command).is_ok() {
                                                                if cancel { self.sync_cancel_requested=true; }
                                                                else { self.history_sync_running=true; self.history_sync_status="Đang bắt đầu tải dữ liệu máy chủ…".into(); self.sync_notice_dismissed=false; }
                                                            }
                                                        }
                                                        ui.label(&self.history_sync_status);
                                                        ui.separator();
                                                        ui.horizontal(|ui| {
                                                            ui.label(crate::i18n::tr("Khoảng lịch sử:"));
                                                            let label = |days: u32| match days { 0 => "Toàn bộ", 365 => "1 năm", 90 => "90 ngày", _ => "30 ngày" };
                                                            egui::ComboBox::from_id_salt("phone_sync_range")
                                                                .selected_text(label(self.settings.phone_sync_range_days))
                                                                .show_ui(ui, |ui| {
                                                                    for days in [0u32, 365, 90, 30] {
                                                                        ui.selectable_value(&mut self.settings.phone_sync_range_days, days, label(days));
                                                                    }
                                                                });
                                                        });
                                                        if ui.button(if self.phone_sync_running {"Dừng đồng bộ điện thoại"}else{"Đồng bộ từ điện thoại"}).clicked() {
                                                            if let Some(tx)=WORKER_TX.get() {
                                                                if tx.send(if self.phone_sync_running {WorkerCmd::CancelPhoneHistory}else{WorkerCmd::SyncPhoneHistory{range_days:self.settings.phone_sync_range_days}}).is_ok() {
                                                                    if self.phone_sync_running {self.sync_cancel_requested=true;}
                                                                    else {self.phone_sync_running=true;self.phone_sync_status="Đang chuẩn bị đồng bộ điện thoại…".into();self.sync_notice_dismissed=false;}
                                                                }
                                                            }
                                                        }
                                                        if ui.add_enabled(!self.phone_sync_running,egui::Button::new(crate::i18n::tr("Chỉ cập nhật tin gần đây"))).on_hover_text(crate::i18n::tr("Lấy phần mới từ checkpoint; nếu chưa có mốc thì lấy 7 ngày gần nhất, chưa tải lịch sử cũ hơn")).clicked() {
                                                            if let Some(tx)=WORKER_TX.get() {if tx.send(WorkerCmd::SyncPhoneRecent).is_ok() {self.phone_sync_running=true;self.phone_sync_status="Đang chuẩn bị lấy tin gần đây…".into();self.sync_notice_dismissed=false;}}
                                                        }
                                                        if ui.add_enabled(!self.phone_sync_running,egui::Button::new(crate::i18n::tr("Quét lại toàn bộ lịch sử điện thoại"))).on_hover_text(crate::i18n::tr("Bỏ mốc tiến độ để kiểm tra lại lịch sử; không xóa tin đã lưu")).clicked() {
                                                            if let Some(tx)=WORKER_TX.get() {if tx.send(WorkerCmd::RescanPhoneHistory{range_days:self.settings.phone_sync_range_days}).is_ok() {self.phone_sync_running=true;self.phone_sync_status="Đang chuẩn bị quét lại lịch sử điện thoại…".into();self.sync_notice_dismissed=false;}}
                                                        }
                                                        if ui.add_enabled(!self.phone_sync_running,egui::Button::new("V1 dự phòng (RSA/ZDB4)")).on_hover_text("Luồng thử nghiệm riêng: điện thoại phải hỗ trợ sao lưu V1. Không tự chạy khi Sync2 lỗi; không thay checkpoint Sync2.").clicked() {
                                                            if let Some(tx)=WORKER_TX.get(){if tx.send(WorkerCmd::SyncPhoneLegacy).is_ok(){self.phone_sync_running=true;self.phone_sync_status="Đang chuẩn bị V1 dự phòng…".into();self.sync_notice_dismissed=false;}}
                                                        }
                                                        ui.label(crate::i18n::tr("Ưu tiên hội thoại hoạt động gần đây; lấy khoảng tin mới trước rồi mới tải lịch sử cũ. Lần sau tiếp tục từ mốc đã hoàn tất."));
                                                        if self.phone_sync_running && ui.button(crate::i18n::tr("Đánh thức điện thoại")).on_hover_text(crate::i18n::tr("Gửi lại thông báo tới Zalo trên điện thoại khi việc gửi lịch sử bị đứng")).clicked() {
                                                            crate::phone_sync::request_wake();
                                                        }
                                                        ui.label(&self.phone_sync_status);
                                                        ui.label(crate::i18n::tr("Mở Zalo trên điện thoại, chọn Đồng bộ ngay khi được hỏi. Có thể dừng và chạy lại; các tin đã lưu không bị mất."));

                                                    });
                                                });
                                        }
                                        SettingsTab::DataManagement => {
                                            ui.label(RichText::new(crate::i18n::tr("Quản lý dữ liệu")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            ui.label(crate::i18n::tr("Lịch sử trò chuyện và tệp tải xuống được lưu theo từng tài khoản trên thiết bị này."));
                                            ui.add_space(8.0);
                                            if ui.button(crate::i18n::tr("Mở công cụ nhập / xuất dữ liệu")).clicked() {
                                                self.tools.open = true;
                                                self.tools.tab = 1;
                                                open = false;
                                            }
                                            if ui.button(crate::i18n::tr("Mở kho Media và zCloud")).clicked() {
                                                self.tools.open = true;
                                                self.tools.tab = 8;
                                                open = false;
                                            }
                                            ui.label(RichText::new(crate::i18n::tr("Xóa dữ liệu cục bộ chỉ thực hiện trong luồng quản lý tài khoản và phải giữ nguyên phạm vi tài khoản hiện tại.")).color(pal.text_secondary).size(12.0));
                                        }
                                        SettingsTab::Appearance => {
                                            self.appearance_controls(ui);
                                            ui.label(RichText::new(crate::i18n::tr("Cài đặt giao diện")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(8.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(10.0))
                                                .inner_margin(Margin::symmetric(18.0, 16.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.spacing_mut().item_spacing.x = 8.0;
                                                        for (mode_key, label, is_dark_val) in [
                                                            ("light", "Sáng", false),
                                                            ("dark", "Tối", true),
                                                            ("system", "Hệ Thống", true),
                                                        ] {
                                                            let selected = self.settings.theme_mode == mode_key;
                                                            ui.vertical(|ui| {
                                                                let (card_r, c_resp) = ui.allocate_exact_size(Vec2::new(((content_w - 52.0) / 3.0).clamp(60.0, 132.0), 84.0), Sense::click());
                                                                let stroke_col = if selected {
                                                                    pal.primary_hover
                                                                } else if c_resp.hovered() {
                                                                    pal.border
                                                                } else {
                                                                    pal.border.linear_multiply(0.6)
                                                                };
                                                                if mode_key == "system" {
                                                                    let left_half = Rect::from_min_max(card_r.min, Pos2::new(card_r.center().x, card_r.max.y));
                                                                    let right_half = Rect::from_min_max(Pos2::new(card_r.center().x, card_r.min.y), card_r.max);
                                                                    ui.painter().rect_filled(left_half, Rounding { nw: 8.0, sw: 8.0, ne: 0.0, se: 0.0 }, Color32::from_rgb(235, 242, 252));
                                                                    ui.painter().rect_filled(right_half, Rounding { nw: 0.0, sw: 0.0, ne: 8.0, se: 8.0 }, Color32::from_rgb(24, 26, 31));
                                                                } else {
                                                                    ui.painter().rect_filled(
                                                                        card_r,
                                                                        Rounding::same(8.0),
                                                                        if is_dark_val {
                                                                            Color32::from_rgb(24, 26, 31)
                                                                        } else {
                                                                            Color32::from_rgb(235, 242, 252)
                                                                        },
                                                                    );
                                                                }
                                                                ui.painter().rect_stroke(card_r, Rounding::same(8.0), Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, stroke_col));
                                                                ui.painter().circle_filled(Pos2::new(card_r.left() + 18.0, card_r.top() + 22.0), 5.5, Color32::from_rgb(82, 139, 255));
                                                                let mock_in = Rect::from_min_size(Pos2::new(card_r.left() + 30.0, card_r.top() + 15.0), Vec2::new(62.0, 22.0));
                                                                ui.painter().rect_filled(mock_in, Rounding::same(4.0), if is_dark_val && mode_key != "light" { Color32::from_rgb(76, 82, 94) } else { Color32::WHITE });
                                                                let mock_out = Rect::from_min_size(Pos2::new(card_r.right() - 56.0, card_r.bottom() - 28.0), Vec2::new(44.0, 18.0));
                                                                ui.painter().rect_filled(mock_out, Rounding::same(4.0), Color32::from_rgb(82, 139, 255));

                                                                ui.add_space(6.0);
                                                                let r_clicked = ui.radio(selected, label).clicked();
                                                                if c_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() || r_clicked {
                                                                    self.settings.theme_mode = mode_key.into();
                                                                    self.dark_mode = if mode_key == "system" { ctx.system_theme() != Some(egui::Theme::Light) } else { is_dark_val };
                                                                    theme_switched = true;
                                                                }
                                                            });
                                                        }
                                                    });
                                                });
                                            ui.add_space(16.0);

                                            ui.label(RichText::new(crate::i18n::tr("Hình nền chat")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Sử dụng Avatar làm hình nền")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.use_avatar_as_wallpaper, pal);
                                                        });
                                                    });
                                                });
                                        }
                                        SettingsTab::Notifications => {
                                            ui.label(RichText::new(crate::i18n::tr("Cài đặt thông báo")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(2.0);
                                            ui.label(RichText::new(crate::i18n::tr("Nhận được thông báo mỗi khi có tin nhắn mới")).color(pal.text_secondary).size(12.2));
                                            ui.add_space(8.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(10.0))
                                                .inner_margin(Margin::symmetric(24.0, 16.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.spacing_mut().item_spacing.x = 28.0;
                                                        for (en_val, label) in [(true, "Bật"), (false, "Tắt")] {
                                                            let selected = self.settings.enable_notifications == en_val;
                                                            ui.vertical_centered(|ui| {
                                                                let (lap_r, l_resp) = ui.allocate_exact_size(Vec2::new(148.0, 92.0), Sense::click());
                                                                let stroke_c = if selected {
                                                                    pal.primary_hover
                                                                } else {
                                                                    pal.border
                                                                };
                                                                let scr_r = Rect::from_min_max(Pos2::new(lap_r.left() + 14.0, lap_r.top() + 6.0), Pos2::new(lap_r.right() - 14.0, lap_r.bottom() - 16.0));
                                                                ui.painter().rect_stroke(scr_r, Rounding::same(4.0), Stroke::new(1.8_f32, stroke_c));
                                                                let base_r = Rect::from_min_max(Pos2::new(lap_r.left() + 6.0, lap_r.bottom() - 16.0), Pos2::new(lap_r.right() - 6.0, lap_r.bottom() - 6.0));
                                                                ui.painter().rect_stroke(base_r, Rounding::same(4.0), Stroke::new(1.8_f32, stroke_c));
                                                                if en_val {
                                                                    let toast_r = Rect::from_min_size(Pos2::new(scr_r.right() - 62.0, scr_r.top() + 8.0), Vec2::new(54.0, 22.0));
                                                                    ui.painter().rect_filled(toast_r, Rounding::same(3.0), pal.bg_selected);
                                                                }
                                                                ui.add_space(6.0);
                                                                let r_click = ui.radio(selected, label).clicked();
                                                                if l_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() || r_click {
                                                                    self.settings.enable_notifications = en_val;
                                                                }
                                                            });
                                                        }
                                                    });
                                                });
                                            ui.add_space(16.0);

                                            ui.label(RichText::new(crate::i18n::tr("Âm thanh thông báo")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    let avail = ui.available_width();
                                                    if avail < 520.0 {
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(crate::i18n::tr("Phát âm thanh khi có tin nhắn & thông báo mới")).color(pal.text_primary).size(13.2));
                                                            ui.add_space(4.0);
                                                            ui.horizontal(|ui| {
                                                                render_toggle_switch(ui, &mut self.settings.play_notification_sound, pal);
                                                                if ui.add_enabled(!self.sound_preparing,egui::Button::new(crate::i18n::tr("Thử âm báo"))).clicked(){
                                                                    if let Some(tx)=WORKER_TX.get(){self.sound_preparing=tx.send(WorkerCmd::PrepareNotificationSound{play:true}).is_ok();self.sound_attempts=0;}
                                                                }
                                                            });
                                                        });
                                                    } else {
                                                        ui.horizontal(|ui| {
                                                            ui.label(RichText::new(crate::i18n::tr("Phát âm thanh khi có tin nhắn & thông báo mới")).color(pal.text_primary).size(13.2));
                                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                                render_toggle_switch(ui, &mut self.settings.play_notification_sound, pal);
                                                                if ui.add_enabled(!self.sound_preparing,egui::Button::new(crate::i18n::tr("Thử âm báo"))).clicked(){
                                                                    if let Some(tx)=WORKER_TX.get(){self.sound_preparing=tx.send(WorkerCmd::PrepareNotificationSound{play:true}).is_ok();self.sound_attempts=0;}
                                                                }
                                                            });
                                                        });
                                                    }
                                                });
                                        }
                                        SettingsTab::Messages => {
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Tin nhắn nhanh")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.quick_messages_enabled, pal);
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Hiện khi người khác đang soạn tin")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.show_typing, pal);
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Cho người khác biết khi tôi đang soạn tin")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.send_typing, pal);
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Gửi ảnh ở chất lượng gốc")).color(pal.text_primary).size(13.2))
                                                            .on_hover_text(crate::i18n::tr("Tắt để gửi bản HD (cạnh dài tối đa 2560 px), nhẹ và nhanh hơn"));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.send_photo_original, pal);
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Hiển thị thumbnail tệp và media")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.file_thumbnails_enabled, pal);
                                                        });
                                                    });
                                                });
                                            ui.add_space(14.0);

                                            ui.label(RichText::new(crate::i18n::tr("Thiết lập ẩn trò chuyện")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(2.0);
                                            ui.label(RichText::new(crate::i18n::tr("Đặt mã PIN cho các cuộc trò chuyện riêng tư để tránh người khác nhìn thấy trên máy của bạn")).color(pal.text_secondary).size(12.0));
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Mã PIN")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            if ui.button("Quản lý PIN máy chủ…").clicked(){self.hidden_panel.open=true;}
                                                        });
                                                    });
                                                });
                                            ui.add_space(14.0);

                                            ui.label(RichText::new(crate::i18n::tr("Chia mục Ưu tiên và Khác")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(2.0);
                                            ui.label(RichText::new(crate::i18n::tr("Tách riêng trò chuyện không ưu tiên và chuyển sang mục Khác để tập trung hơn")).color(pal.text_secondary).size(12.0));
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Chia mục Ưu tiên theo hội thoại đã ghim")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.split_priority_inbox, pal);
                                                        });
                                                    });
                                                });
                                            ui.add_space(14.0);

                                            ui.label(RichText::new(crate::i18n::tr("Hiển thị nội dung file Excel")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(2.0);
                                            ui.label(RichText::new(crate::i18n::tr("Định dạng khi sao chép file Excel sau đó dán vào Zalo")).color(pal.text_secondary).size(12.0));
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    for (val, lbl) in [("image", "Dạng hình"), ("text", "Dạng chữ"), ("ask", "Luôn hỏi")] {
                                                        ui.horizontal(|ui| {
                                                            ui.label(RichText::new(lbl).color(pal.text_primary).size(13.0));
                                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                                ui.radio_value(&mut self.settings.excel_paste_mode, val.into(), "");
                                                            });
                                                        });
                                                        ui.add_space(4.0);
                                                    }
                                                });
                                            ui.add_space(14.0);

                                            ui.label(RichText::new(crate::i18n::tr("Cài đặt khác")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Bấm đúp vào vùng cạnh tin nhắn để trả lời")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.double_click_to_reply, pal);
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Bạn sẽ thấy bạn bè bạn đang soạn tin nhắn")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.show_typing_indicator, pal);
                                                        });
                                                    });
                                                });
                                        }
                                        SettingsTab::Calls => {
                                            ui.label(RichText::new(crate::i18n::tr("Cuộc gọi")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            ui.label(RichText::new(crate::i18n::tr("Thiết bị được áp dụng khi bắt đầu cuộc gọi tiếp theo.")).color(pal.text_secondary).size(12.0));
                                            ui.add_space(8.0);
                                            if self.smoke.is_none() && self.settings_devices.devices.is_none()
                                                && !self.settings_devices.pending && self.settings_devices.error.is_empty() {
                                                self.request_settings_devices(false);
                                            }
                                            let (mics, speakers, cams) = if self.smoke.is_some() {
                                                (vec!["Fixture microphone".into()], vec!["Fixture speaker".into()], vec!["Fixture camera".into()])
                                            } else { self.settings_devices.devices.clone().unwrap_or_default() };
                                            if self.settings_devices.pending { ui.horizontal(|ui| { ui.spinner(); ui.label(crate::i18n::tr("Đang quét thiết bị…")); }); }
                                            if !self.settings_devices.error.is_empty() { ui.label(&self.settings_devices.error); }
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(format!("{} Microphone", lucide::MIC)).color(pal.text_primary).size(13.0));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            let selected = if self.settings.selected_mic_device.is_empty() { crate::i18n::tr("Mặc định hệ thống") } else { &self.settings.selected_mic_device };
                                                            egui::ComboBox::from_id_salt("call_settings_mic")
                                                                .selected_text(truncate_chars(selected, 30)).width(250.0)
                                                                .show_ui(ui, |ui| {
                                                                    if ui.selectable_label(self.settings.selected_mic_device.is_empty(), crate::i18n::tr("Mặc định hệ thống")).clicked() {
                                                                        self.settings.selected_mic_device.clear();
                                                                        crate::media::set_selected_mic_device("");
                                                                        self.save_settings();
                                                                    }
                                                                    for device in &mics {
                                                                        if ui.selectable_label(self.settings.selected_mic_device == *device, device).clicked() {
                                                                            self.settings.selected_mic_device = device.clone();
                                                                            crate::media::set_selected_mic_device(device);
                                                                            self.save_settings();
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Loa")).color(pal.text_primary).size(13.0));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            let selected = if self.settings.selected_speaker_device.is_empty() { crate::i18n::tr("Mặc định hệ thống") } else { &self.settings.selected_speaker_device };
                                                            egui::ComboBox::from_id_salt("call_settings_speaker")
                                                                .selected_text(truncate_chars(selected, 30)).width(250.0)
                                                                .show_ui(ui, |ui| {
                                                                    if ui.selectable_label(self.settings.selected_speaker_device.is_empty(), crate::i18n::tr("Mặc định hệ thống")).clicked() {
                                                                        self.settings.selected_speaker_device.clear();
                                                                        crate::media::set_selected_speaker_device("");
                                                                        self.save_settings();
                                                                    }
                                                                    for device in &speakers {
                                                                        if ui.selectable_label(self.settings.selected_speaker_device == *device, device).clicked() {
                                                                            self.settings.selected_speaker_device = device.clone();
                                                                            crate::media::set_selected_speaker_device(device);
                                                                            self.save_settings();
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                    });
                                                    ui.add_space(8.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(format!("{} Camera", lucide::VIDEO)).color(pal.text_primary).size(13.0));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            let selected = if self.settings.selected_camera_device.is_empty() { crate::i18n::tr("Tự động") } else { &self.settings.selected_camera_device };
                                                            egui::ComboBox::from_id_salt("call_settings_camera")
                                                                .selected_text(truncate_chars(selected, 30)).width(250.0)
                                                                .show_ui(ui, |ui| {
                                                                    if ui.selectable_label(self.settings.selected_camera_device.is_empty(), crate::i18n::tr("Tự động (Camera đầu tiên)")).clicked() {
                                                                        self.settings.selected_camera_device.clear();
                                                                        crate::media::set_selected_camera_device("");
                                                                        self.save_settings();
                                                                    }
                                                                    for device in &cams {
                                                                        if ui.selectable_label(self.settings.selected_camera_device == *device, device).clicked() {
                                                                            self.settings.selected_camera_device = device.clone();
                                                                            crate::media::set_selected_camera_device(device);
                                                                            self.save_settings();
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                    });
                                                    ui.add_space(6.0);
                                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                        if ui.add_enabled(!self.settings_devices.pending, egui::Button::new(format!("{} Quét lại thiết bị", lucide::REFRESH))).clicked() {
                                                            self.request_settings_devices(true);
                                                        }
                                                    });
                                                });
                                            ui.add_space(14.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(crate::i18n::tr("Thông báo cuộc gọi đến")).color(pal.text_primary).size(13.2));
                                                            ui.label(RichText::new(crate::i18n::tr("Hiển thị chú ý trên cửa sổ khi có cuộc gọi đến")).color(pal.text_secondary).size(12.0));
                                                        });
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.call_notifications_enabled, pal);
                                                        });
                                                    });
                                                });
                                        }
                                        SettingsTab::Utilities => {
                                            ui.label(format!("Phiên bản {} • Windows ARM64",crate::PRODUCT_VERSION));
                                            if ui.add_enabled(!self.update_busy,egui::Button::new(crate::i18n::tr("Kiểm tra cập nhật"))).clicked(){if let Some(tx)=WORKER_TX.get(){if tx.send(WorkerCmd::CheckUpdate).is_ok(){self.update_busy=true;self.update_status="Đang kiểm tra bản phát hành của fork…".into();}}}
                                            if let Some(release)=self.update_offer.clone(){if ui.add_enabled(!self.update_busy,egui::Button::new(format!("Tải bộ cài {}",release.tag))).clicked(){if let Some(tx)=WORKER_TX.get(){if tx.send(WorkerCmd::PrepareUpdate(release)).is_ok(){self.update_busy=true;self.update_status="Đang tải và kiểm tra bản cập nhật…".into();}}}}
                                            if let Some(ready)=self.update_ready.clone(){if ui.add_enabled(!self.update_busy,egui::Button::new(crate::i18n::tr("Mở bộ cài và đóng ứng dụng"))).clicked(){if let Some(tx)=WORKER_TX.get(){if tx.send(WorkerCmd::LaunchUpdate(ready)).is_ok(){self.update_busy=true;}}}}
                                            ui.label(&self.update_status);ui.separator();

                                            if ui.button(crate::i18n::tr("Tìm lịch sử, nhập dữ liệu, theme/sticker, hẹn giờ & tự trả lời")).clicked() { self.tools.open=true; }
                                            ui.label(RichText::new(crate::i18n::tr("Gợi ý Sticker")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Hiện gợi ý Sticker phù hợp với nội dung tin nhắn đang soạn")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.suggest_stickers_while_typing, pal);
                                                        });
                                                    });
                                                    ui.separator();
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(crate::i18n::tr("Hiệu ứng nhãn dán động (Sprite Sheet Zalo)")).color(pal.text_primary).size(13.2));
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.enable_animated_stickers, pal);
                                                        });
                                                    });
                                                });
                                            ui.add_space(16.0);

                                            ui.label(RichText::new(crate::i18n::tr("Gợi ý @")).color(pal.text_primary).size(13.5).strong());
                                            ui.add_space(6.0);
                                            egui::Frame::none()
                                                .fill(pal.bg_elevated)
                                                .stroke(Stroke::new(1.0_f32, pal.border))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(14.0, 12.0))
                                                .show(ui, |ui| {
                                                    // Row 1: Gợi ý nhắc tên
                                                    ui.horizontal(|ui| {
                                                        let (b_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                                                        ui.painter().circle_filled(b_rect.center(), 15.0, Color32::from_rgb(224, 108, 117));
                                                        draw_lucide_icon(ui.painter(), b_rect.center(), lucide::USER, 15.0, Color32::WHITE);
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(crate::i18n::tr("Gợi ý nhắc tên")).color(pal.text_primary).size(13.2).strong());
                                                            ui.label(RichText::new(crate::i18n::tr("Gợi ý nhắc tên theo nội dung đang soạn")).color(pal.text_secondary).size(11.8));
                                                        });
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.suggest_mentions, pal);
                                                        });
                                                    });
                                                    ui.add_space(10.0);
                                                    // Row 2: Tìm kiếm GIF
                                                    ui.horizontal(|ui| {
                                                        let (b_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                                                        ui.painter().circle_filled(b_rect.center(), 15.0, Color32::from_rgb(82, 139, 255));
                                                        ui.painter().text(b_rect.center(), egui::Align2::CENTER_CENTER, "GIF", FontId::proportional(10.5), Color32::WHITE);
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(crate::i18n::tr("Tìm kiếm GIF")).color(pal.text_primary).size(13.2).strong());
                                                            ui.label(RichText::new(crate::i18n::tr("Gõ từ khóa để tìm GIF")).color(pal.text_secondary).size(11.8));
                                                        });
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.enable_gif_search, pal);
                                                        });
                                                    });
                                                    ui.add_space(10.0);
                                                    // Row 3: Tìm sticker
                                                    ui.horizontal(|ui| {
                                                        let (b_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                                                        ui.painter().circle_filled(b_rect.center(), 15.0, Color32::from_rgb(152, 88, 178));
                                                        draw_lucide_icon(ui.painter(), b_rect.center(), lucide::SMILE, 15.5, Color32::WHITE);
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(crate::i18n::tr("Tìm sticker")).color(pal.text_primary).size(13.2).strong());
                                                            ui.label(RichText::new(crate::i18n::tr("Gõ từ khóa để tìm Sticker")).color(pal.text_secondary).size(11.8));
                                                        });
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.enable_sticker_keyword_search, pal);
                                                        });
                                                    });
                                                    ui.add_space(10.0);
                                                    ui.horizontal(|ui| {
                                                        let (b_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                                                        ui.painter().circle_filled(b_rect.center(), 15.0, Color32::from_rgb(42, 156, 143));
                                                        draw_lucide_icon(ui.painter(), b_rect.center(), lucide::LINK, 15.0, Color32::WHITE);
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(crate::i18n::tr("Gợi ý liên kết")).color(pal.text_primary).size(13.2).strong());
                                                            ui.label(RichText::new(crate::i18n::tr("Tải tiêu đề và ảnh xem trước của liên kết khi cần")).color(pal.text_secondary).size(11.8));
                                                        });
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.link_suggestions_enabled, pal);
                                                        });
                                                    });
                                                    ui.add_space(10.0);
                                                    // Row 4: Dịch tin nhắn AI (Thử nghiệm)
                                                    ui.horizontal(|ui| {
                                                        let (b_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                                                        ui.painter().circle_filled(b_rect.center(), 15.0, Color32::from_rgb(46, 139, 87));
                                                        ui.painter().text(b_rect.center(), egui::Align2::CENTER_CENTER, "AI", FontId::proportional(11.0), Color32::WHITE);
                                                        ui.vertical(|ui| {
                                                            ui.label(RichText::new(crate::i18n::tr("Đồng ý dịch tin nhắn bằng AI")).color(pal.text_primary).size(13.2).strong());
                                                            ui.label(RichText::new(crate::i18n::tr("Cho phép gửi văn bản cần dịch lên máy chủ AI Zalo (không áp dụng cho tin nhắn mã hóa)")).color(pal.text_secondary).size(11.8));
                                                        });
                                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                            render_toggle_switch(ui, &mut self.settings.ai_translation_consent, pal);
                                                        });
                                                    });
                                                });
                                        }
                                    }
                                });
                            });
                        });
                });
            });

        if self.smoke.as_ref().is_some_and(|smoke| smoke.frames >= 3) {
            if let Some(response) = response {
                let rect = response.response.rect;
                assert!(
                    screen.expand(1.0).contains_rect(rect),
                    "Settings outside screen: {rect:?}, screen={screen:?}"
                );
            }
        }

        if self.settings.remember_login != previous_remember {
            self.save_settings();
        }
        if theme_switched {
            self.theme_dirty = true;
            self.save_settings();
            self.show_toast(if self.dark_mode {
                "Đã chuyển sang giao diện Tối (Atom One Dark)"
            } else {
                "Đã chuyển sang giao diện Sáng (Zalo Light)"
            });
        }
        if trigger_sync {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::RefreshThreads);
            }
            self.show_toast("Đang cập nhật hội thoại và tin gần đây…");
        }
        if pick_dl_dir && !self.download_picker_pending {
            self.download_picker_pending = true;
            let tx = self.event_tx.clone();
            let context = ctx.clone();
            std::thread::spawn(move || {
                let result = pick_native_folder_dialog().map(|path| {
                    validate_download_folder(&path)
                        .map(|()| path)
                        .map_err(|error| format!("Không dùng được thư mục tải về: {error}"))
                });
                let _ = tx.send(GuiEvent::DownloadFolderPicked(result));
                context.request_repaint();
            });
        }
        if open_dl_dir {
            let p = std::path::PathBuf::from(&self.settings.download_dir);
            let tx = self.event_tx.clone();
            let context = ctx.clone();
            std::thread::spawn(move || {
                let result = std::fs::create_dir_all(&p)
                    .map_err(|_| "Không tạo được thư mục tải về")
                    .and_then(|()| {
                        if crate::platform::open_folder(&p) {
                            Ok(())
                        } else {
                            Err("Không mở được thư mục tải về")
                        }
                    });
                if let Err(message) = result {
                    let _ = tx.send(GuiEvent::DownloadFolderFailed(message.into()));
                    context.request_repaint();
                }
            });
        }

        if !open {
            self.settings_modal_open = false;
            self.save_settings();
        }
    }

    pub(super) fn render_pin_setup_modal(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if !self.pin_setup_modal_open {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.pin_setup_modal_open = false;
            self.pin_setup_enable_after = false;
            self.pin_setup_first.clear();
            self.pin_setup_confirm.clear();
            self.pin_setup_error = None;
            return;
        }

        render_modal_backdrop(ctx, "pin_setup_modal_backdrop");

        let mut open = true;
        let mut submit = false;
        let mut cancel = false;
        let title = if self.settings.has_pin() {
            "Đổi mã PIN 4 số"
        } else {
            "Thiết lập mã PIN 4 số"
        };

        egui::Window::new(crate::i18n::tr(title))
            .id(Id::new("pin_setup_modal_window"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(340.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(pal.bg_elevated)
                    .rounding(Rounding::same(12.0))
                    .stroke(Stroke::new(1.2_f32, pal.primary))
                    .inner_margin(Margin::symmetric(20.0, 16.0)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(crate::i18n::tr(
                        "Nhập mã PIN gồm 4 chữ số (0-9) để khóa và mở khóa ứng dụng Zalo.",
                    ))
                    .size(12.2)
                    .color(pal.text_secondary),
                );
                ui.add_space(10.0);

                ui.label(
                    RichText::new(crate::i18n::tr("Mã PIN mới (4 số):"))
                        .size(12.5)
                        .color(pal.text_primary),
                );
                ui.add_space(4.0);
                let first_resp = ui.add(
                    egui::TextEdit::singleline(&mut *self.pin_setup_first)
                        .password(true)
                        .char_limit(4)
                        .hint_text("••••")
                        .margin(Margin::symmetric(10.0, 7.0))
                        .desired_width(ui.available_width()),
                );
                self.pin_setup_first.retain(|c| c.is_ascii_digit());
                if first_resp.changed() {
                    self.pin_setup_error = None;
                }

                ui.add_space(10.0);
                ui.label(
                    RichText::new(crate::i18n::tr("Xác nhận mã PIN:"))
                        .size(12.5)
                        .color(pal.text_primary),
                );
                ui.add_space(4.0);
                let confirm_resp = ui.add(
                    egui::TextEdit::singleline(&mut *self.pin_setup_confirm)
                        .password(true)
                        .char_limit(4)
                        .hint_text("••••")
                        .margin(Margin::symmetric(10.0, 7.0))
                        .desired_width(ui.available_width()),
                );
                self.pin_setup_confirm.retain(|c| c.is_ascii_digit());
                if confirm_resp.changed() {
                    self.pin_setup_error = None;
                }

                if (first_resp.lost_focus()
                    || confirm_resp.lost_focus()
                    || confirm_resp.has_focus())
                    && ui.input(|i| i.key_pressed(Key::Enter))
                {
                    submit = true;
                }

                if let Some(ref err) = self.pin_setup_error {
                    ui.add_space(8.0);
                    ui.label(RichText::new(err).size(12.0).color(pal.danger));
                }

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let save_btn = egui::Button::new(
                            RichText::new(crate::i18n::tr("Lưu mã PIN"))
                                .color(Color32::WHITE)
                                .size(12.8)
                                .strong(),
                        )
                        .fill(pal.primary)
                        .rounding(Rounding::same(8.0))
                        .min_size(Vec2::new(96.0, 32.0));
                        if ui
                            .add(save_btn)
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            submit = true;
                        }
                        if ui.button(crate::i18n::tr("Hủy")).clicked() {
                            cancel = true;
                        }
                    });
                });
            });

        if submit {
            if self.pin_setup_first.len() != 4
                || !self.pin_setup_first.bytes().all(|b| b.is_ascii_digit())
            {
                self.pin_setup_error = Some("Mã PIN phải gồm đúng 4 chữ số (0-9).".into());
            } else if *self.pin_setup_first != *self.pin_setup_confirm {
                self.pin_setup_error = Some("Mã PIN xác nhận không khớp.".into());
            } else {
                let new_pin = self.pin_setup_first.clone();
                match self.settings.set_pin(&new_pin) {
                    Ok(()) => {
                        if self.pin_setup_enable_after {
                            self.settings.enable_app_lock = true;
                        }
                        self.pin_setup_modal_open = false;
                        self.pin_setup_enable_after = false;
                        self.pin_setup_first.clear();
                        self.pin_setup_confirm.clear();
                        self.pin_setup_error = None;
                        self.save_settings();
                        self.show_toast("Đã lưu mã PIN khóa ứng dụng");
                    }
                    Err(msg) => {
                        self.pin_setup_error = Some(msg.into());
                    }
                }
            }
        } else if cancel || !open {
            self.pin_setup_modal_open = false;
            self.pin_setup_enable_after = false;
            self.pin_setup_first.clear();
            self.pin_setup_confirm.clear();
            self.pin_setup_error = None;
        }
    }

    pub(super) fn render_call_overlay(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let Some(mut call) = self.active_call.clone() else {
            return;
        };
        // Keep timer and video textures updating smoothly while call window is open
        ctx.request_repaint_after(Duration::from_millis(66));

        let elapsed_f = call.started_at.elapsed().as_secs_f32();
        let elapsed = elapsed_f as u64;
        let mins = elapsed / 60;
        let secs = elapsed % 60;
        let timer_str = format!("{mins:02}:{secs:02}");

        let mut hangup_clicked = false;
        let mut retry_clicked = false;

        let has_err = call.error_reason.is_some();
        let is_video_stage = (call.is_video
            || self.sticker_textures.contains_key("call_remote_video")
            || self.sticker_textures.contains_key("call_remote_video_1"))
            && !has_err;
        let default_w = if is_video_stage { 860.0 } else { 440.0 };
        let default_h = if is_video_stage { 660.0 } else { 470.0 };
        let win_title = if call.is_video {
            format!("Zalo Video Call — {}", call.partner_name)
        } else {
            format!("Zalo Voice Call — {}", call.partner_name)
        };

        let call_ava = self
            .threads
            .iter()
            .find(|t| t.id == call.partner_id || t.name == call.partner_name)
            .and_then(|t| t.avatar.clone());

        let viewport_id = egui::ViewportId::from_hash_of("zalo_dedicated_call_window");
        let builder = egui::ViewportBuilder::default()
            .with_title(win_title)
            .with_inner_size([default_w, default_h])
            .with_min_inner_size([380.0, 420.0]);

        ctx.show_viewport_immediate(viewport_id, builder, |vp_ctx, _class| {
            if vp_ctx.input(|i| i.viewport().close_requested()) {
                hangup_clicked = true;
            }
            if vp_ctx.input(|i| i.key_pressed(Key::Escape)) && call.error_reason.is_some() {
                hangup_clicked = true;
            }

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(pal.bg_sidebar)
                        .inner_margin(Margin::symmetric(20.0, 16.0)),
                )
                .show(vp_ctx, |ui| {
                    let avail_w = ui.available_width();
                    let avail_h = ui.available_height();

                    // 1. Symmetrically Centered Top Toolbar: Badge · Mic · Camera · Refresh
                    ui.vertical_centered(|ui| {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .rounding(Rounding::same(14.0))
                            .inner_margin(Margin::symmetric(12.0, 6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    let pill_col = if has_err {
                                        pal.danger.linear_multiply(0.16)
                                    } else {
                                        pal.accent_online.linear_multiply(0.15)
                                    };
                                    let pill_fg = if has_err {
                                        pal.danger
                                    } else {
                                        pal.accent_online
                                    };
                                    let badge_txt = if call.is_video {
                                        format!("{} Zalo Video · ZRTP", lucide::VIDEO)
                                    } else {
                                        format!("{} Zalo Voice · ZRTP", lucide::SHIELD_CHECK)
                                    };
                                    egui::Frame::none()
                                        .fill(pill_col)
                                        .rounding(Rounding::same(10.0))
                                        .inner_margin(Margin::symmetric(10.0, 4.0))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(badge_txt)
                                                    .size(11.5)
                                                    .color(pill_fg)
                                                    .strong(),
                                            );
                                        });

                                    let (mics, cams) = crate::media::list_media_devices(false);
                                    let cur_mic_lbl =
                                        if self.settings.selected_mic_device.is_empty() {
                                            mics.first()
                                                .map(String::as_str)
                                                .unwrap_or("Mic mặc định")
                                        } else {
                                            &self.settings.selected_mic_device
                                        };
                                    egui::ComboBox::from_id_salt("call_vp_mic_combo")
                                        .selected_text(format!(
                                            "{} {}",
                                            lucide::MIC,
                                            truncate_chars(cur_mic_lbl, 22)
                                        ))
                                        .width(190.0)
                                        .show_ui(ui, |ui| {
                                            if ui
                                                .selectable_label(
                                                    self.settings.selected_mic_device.is_empty(),
                                                    crate::i18n::tr("Mặc định hệ thống"),
                                                )
                                                .clicked()
                                            {
                                                self.settings.selected_mic_device.clear();
                                                crate::media::set_selected_mic_device("");
                                                self.save_settings();
                                            }
                                            for m in &mics {
                                                if ui
                                                    .selectable_label(
                                                        self.settings.selected_mic_device == *m,
                                                        m,
                                                    )
                                                    .clicked()
                                                {
                                                    self.settings.selected_mic_device = m.clone();
                                                    crate::media::set_selected_mic_device(m);
                                                    self.save_settings();
                                                }
                                            }
                                        });

                                    let cur_cam_lbl =
                                        if self.settings.selected_camera_device.is_empty() {
                                            cams.first()
                                                .map(String::as_str)
                                                .unwrap_or("Camera mặc định")
                                        } else {
                                            &self.settings.selected_camera_device
                                        };
                                    egui::ComboBox::from_id_salt("call_vp_cam_combo")
                                        .selected_text(format!(
                                            "{} {}",
                                            lucide::VIDEO,
                                            truncate_chars(cur_cam_lbl, 22)
                                        ))
                                        .width(190.0)
                                        .show_ui(ui, |ui| {
                                            if ui
                                                .selectable_label(
                                                    self.settings.selected_camera_device.is_empty(),
                                                    crate::i18n::tr("Tự động (Camera đầu tiên)"),
                                                )
                                                .clicked()
                                            {
                                                self.settings.selected_camera_device.clear();
                                                crate::media::set_selected_camera_device("");
                                                self.save_settings();
                                            }
                                            for c in &cams {
                                                if ui
                                                    .selectable_label(
                                                        self.settings.selected_camera_device == *c,
                                                        c,
                                                    )
                                                    .clicked()
                                                {
                                                    self.settings.selected_camera_device =
                                                        c.clone();
                                                    crate::media::set_selected_camera_device(c);
                                                    self.save_settings();
                                                }
                                            }
                                        });

                                    if ui
                                        .small_button(lucide::REFRESH)
                                        .on_hover_text(crate::i18n::tr(
                                            "Quét lại danh sách Mic & Camera",
                                        ))
                                        .clicked()
                                    {
                                        let _ = crate::media::list_media_devices(true);
                                    }
                                });
                            });
                    });

                    ui.add_space(12.0);

                    // 2. Main Stage — Strictly Centered
                    ui.vertical_centered(|ui| {
                        if is_video_stage {
                            let stage_w = avail_w.max(320.0);
                            let stage_h = (avail_h - 156.0).max(240.0);
                            let (stage_rect, _) =
                                ui.allocate_exact_size(Vec2::new(stage_w, stage_h), Sense::hover());
                            let painter = ui.painter();
                            painter.rect_filled(
                                stage_rect,
                                Rounding::same(16.0),
                                Color32::from_rgb(15, 20, 30),
                            );
                            painter.rect_stroke(
                                stage_rect,
                                Rounding::same(16.0),
                                Stroke::new(1.0_f32, pal.border),
                            );

                            // Remote video frame card: fitted tightly to the true video aspect ratio (no black bars!)
                            if let Some(vtex) = self.sticker_textures.get("call_remote_video") {
                                let inner = stage_rect.shrink(10.0);
                                let tex_size = vtex.size_vec2();
                                let tex_aspect = if tex_size.y > 0.0 {
                                    tex_size.x / tex_size.y
                                } else {
                                    4.0 / 3.0
                                };
                                let box_aspect = inner.width() / inner.height().max(1.0);
                                let draw_size = if box_aspect > tex_aspect {
                                    Vec2::new(inner.height() * tex_aspect, inner.height())
                                } else {
                                    Vec2::new(inner.width(), inner.width() / tex_aspect)
                                };
                                let draw_rect = Rect::from_center_size(inner.center(), draw_size);
                                painter.rect_filled(
                                    draw_rect.expand(2.0),
                                    Rounding::same(12.0),
                                    Color32::from_rgb(8, 11, 18),
                                );
                                painter.image(
                                    vtex.id(),
                                    draw_rect,
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                                painter.rect_stroke(
                                    draw_rect.expand(1.0),
                                    Rounding::same(12.0),
                                    Stroke::new(
                                        1.2_f32,
                                        Color32::from_rgba_unmultiplied(148, 163, 184, 70),
                                    ),
                                );
                            } else {
                                let center =
                                    Pos2::new(stage_rect.center().x, stage_rect.center().y - 16.0);
                                let pulse = ((elapsed_f * 2.6).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                                painter.circle_filled(
                                    center,
                                    46.0 + pulse * 6.0,
                                    pal.primary.linear_multiply(0.12),
                                );
                                painter.circle_stroke(
                                    center,
                                    43.0,
                                    Stroke::new(1.8_f32, pal.primary_hover.linear_multiply(0.8)),
                                );
                                paint_user_avatar(
                                    painter,
                                    center,
                                    40.0,
                                    &call.partner_name,
                                    call.is_group,
                                    call_ava.as_deref(),
                                    &self.sticker_textures,
                                    &mut self.loading_stickers,
                                );
                                painter.text(
                                    Pos2::new(stage_rect.center().x, center.y + 56.0),
                                    egui::Align2::CENTER_CENTER,
                                    &call.partner_name,
                                    FontId::proportional(17.5),
                                    pal.text_primary,
                                );
                            }

                            // Local Camera Picture-in-Picture (PIP) preview in bottom-right corner
                            let pip_w = 184.0_f32;
                            let pip_h = 138.0_f32;
                            let pip_rect = Rect::from_min_size(
                                Pos2::new(
                                    stage_rect.right() - pip_w - 16.0,
                                    stage_rect.bottom() - pip_h - 16.0,
                                ),
                                Vec2::new(pip_w, pip_h),
                            );
                            painter.rect_filled(pip_rect, Rounding::same(12.0), pal.bg_elevated);
                            if call.is_group {
                                if let Some(texture) =
                                    self.sticker_textures.get("call_remote_video_1")
                                {
                                    let width = (stage_rect.width() * 0.32).clamp(80.0, 220.0);
                                    let height = width * texture.size_vec2().y
                                        / texture.size_vec2().x.max(1.0);
                                    let rect = Rect::from_min_size(
                                        stage_rect.left_top() + Vec2::new(14.0, 14.0),
                                        Vec2::new(width, height.min(stage_rect.height() * 0.4)),
                                    );
                                    painter.image(
                                        texture.id(),
                                        rect,
                                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                    painter.text(
                                        rect.left_bottom() + Vec2::new(0.0, 3.0),
                                        egui::Align2::LEFT_TOP,
                                        "Thành viên nhóm",
                                        FontId::proportional(11.0),
                                        Color32::WHITE,
                                    );
                                }
                            }
                            if call.cam_enabled {
                                if let Some(ltex) = self.sticker_textures.get("call_local_video") {
                                    let inner_pip = pip_rect.shrink(2.0);
                                    let lsize = ltex.size_vec2();
                                    let laspect = if lsize.y > 0.0 {
                                        lsize.x / lsize.y
                                    } else {
                                        4.0 / 3.0
                                    };
                                    let paspect = inner_pip.width() / inner_pip.height().max(1.0);
                                    let uv = if laspect > paspect {
                                        let frac = paspect / laspect;
                                        let m = (1.0 - frac) * 0.5;
                                        Rect::from_min_max(
                                            Pos2::new(m, 0.0),
                                            Pos2::new(1.0 - m, 1.0),
                                        )
                                    } else {
                                        let frac = laspect / paspect;
                                        let m = (1.0 - frac) * 0.5;
                                        Rect::from_min_max(
                                            Pos2::new(0.0, m),
                                            Pos2::new(1.0, 1.0 - m),
                                        )
                                    };
                                    painter.image(ltex.id(), inner_pip, uv, Color32::WHITE);
                                } else {
                                    let self_disp = if self.user_name.is_empty() {
                                        "Tôi"
                                    } else {
                                        &self.user_name
                                    };
                                    paint_user_avatar(
                                        painter,
                                        Pos2::new(pip_rect.center().x, pip_rect.center().y - 10.0),
                                        20.0,
                                        self_disp,
                                        false,
                                        self.user_avatar.as_deref(),
                                        &self.sticker_textures,
                                        &mut self.loading_stickers,
                                    );
                                }
                            } else {
                                let self_disp = if self.user_name.is_empty() {
                                    "Tôi"
                                } else {
                                    &self.user_name
                                };
                                paint_user_avatar(
                                    painter,
                                    Pos2::new(pip_rect.center().x, pip_rect.center().y - 10.0),
                                    20.0,
                                    self_disp,
                                    false,
                                    self.user_avatar.as_deref(),
                                    &self.sticker_textures,
                                    &mut self.loading_stickers,
                                );
                            }
                            let pip_badge_rect = Rect::from_center_size(
                                Pos2::new(pip_rect.center().x, pip_rect.bottom() - 14.0),
                                Vec2::new(124.0, 20.0),
                            );
                            painter.rect_filled(
                                pip_badge_rect,
                                Rounding::same(10.0),
                                Color32::from_rgba_unmultiplied(8, 11, 18, 185),
                            );
                            let pip_status = if call.cam_enabled {
                                format!("{} Camera đang bật", lucide::VIDEO)
                            } else {
                                format!("{} Camera đã tắt", lucide::VIDEO_OFF)
                            };
                            painter.text(
                                pip_badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                pip_status,
                                FontId::proportional(10.5),
                                if call.cam_enabled {
                                    pal.accent_online
                                } else {
                                    pal.text_muted
                                },
                            );
                            painter.rect_stroke(
                                pip_rect,
                                Rounding::same(12.0),
                                Stroke::new(
                                    1.4_f32,
                                    if call.cam_enabled {
                                        pal.primary_hover
                                    } else {
                                        pal.border
                                    },
                                ),
                            );
                            ui.add_space(10.0);
                        } else {
                            ui.add_space(18.0);
                            let (av_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(96.0), Sense::hover());
                            let center = av_rect.center();
                            let ring_col = if has_err {
                                pal.danger
                            } else if call.is_incoming_ringing {
                                pal.accent_online
                            } else {
                                pal.primary_hover
                            };
                            let pulse = ((elapsed_f * 2.6).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                            ui.painter().circle_filled(
                                center,
                                44.0 + pulse * 5.0,
                                ring_col.linear_multiply(0.10),
                            );
                            ui.painter().circle_stroke(
                                center,
                                42.0,
                                Stroke::new(1.8_f32, ring_col.linear_multiply(0.75)),
                            );
                            paint_user_avatar(
                                ui.painter(),
                                center,
                                38.0,
                                &call.partner_name,
                                call.is_group,
                                call_ava.as_deref(),
                                &self.sticker_textures,
                                &mut self.loading_stickers,
                            );

                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(&call.partner_name)
                                    .size(18.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                        }

                        // 3. Status + Timer Pill — Perfectly Centered
                        if let Some(err_msg) = &call.error_reason {
                            ui.label(
                                RichText::new(crate::i18n::tr("Cuộc gọi chưa kết nối được"))
                                    .size(12.5)
                                    .color(pal.danger)
                                    .strong(),
                            );
                            ui.add_space(6.0);
                            let clean_err = err_msg
                                .strip_prefix("Lỗi khởi tạo cuộc gọi (401): ")
                                .unwrap_or(err_msg);
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(127, 29, 29, 68))
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    Color32::from_rgba_unmultiplied(239, 68, 68, 110),
                                ))
                                .rounding(Rounding::same(12.0))
                                .inner_margin(Margin::symmetric(12.0, 8.0))
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(clean_err).size(11.8).color(pal.danger),
                                        )
                                        .wrap(),
                                    );
                                });
                        } else {
                            egui::Frame::none()
                                .fill(Color32::from_rgb(30, 41, 59))
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .rounding(Rounding::same(12.0))
                                .inner_margin(Margin::symmetric(14.0, 4.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        ui.label(
                                            RichText::new(&call.state_label)
                                                .size(12.5)
                                                .color(Color32::from_rgb(203, 213, 225)),
                                        );
                                        ui.label(
                                            RichText::new("·").size(13.0).color(pal.text_muted),
                                        );
                                        ui.label(
                                            RichText::new(&timer_str)
                                                .monospace()
                                                .size(13.5)
                                                .color(pal.accent_online)
                                                .strong(),
                                        );
                                    });
                                });
                        }

                        ui.add_space(10.0);

                        // 4. Bottom Action Dock — Exact Mathematical Center (`196.0 px` total width: `48 + 22 + 56 + 22 + 48`)
                        if has_err {
                            let dock_w = 216.0_f32;
                            let (dock_rect, _) =
                                ui.allocate_exact_size(Vec2::new(dock_w, 40.0), Sense::hover());
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(dock_rect), |ui| {
                                ui.horizontal_centered(|ui| {
                                    ui.spacing_mut().item_spacing.x = 12.0;
                                    let retry_btn = egui::Button::new(
                                        RichText::new(format!("{} Gọi lại", lucide::PHONE_CALL))
                                            .size(12.8)
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(pal.primary)
                                    .rounding(Rounding::same(18.0))
                                    .min_size(Vec2::new(104.0, 36.0));
                                    if ui
                                        .add(retry_btn)
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        retry_clicked = true;
                                    }

                                    let close_btn = egui::Button::new(
                                        RichText::new(format!("{} Đóng", lucide::X))
                                            .size(12.8)
                                            .color(Color32::from_rgb(226, 232, 240))
                                            .strong(),
                                    )
                                    .fill(Color32::from_rgb(30, 41, 59))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(51, 65, 85)))
                                    .rounding(Rounding::same(18.0))
                                    .min_size(Vec2::new(100.0, 36.0));
                                    if ui
                                        .add(close_btn)
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        hangup_clicked = true;
                                    }
                                });
                            });
                        } else if call.is_incoming_ringing {
                            let dock_w = 148.0_f32;
                            let (dock_rect, _) =
                                ui.allocate_exact_size(Vec2::new(dock_w, 72.0), Sense::hover());
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(dock_rect), |ui| {
                                ui.horizontal_centered(|ui| {
                                    ui.spacing_mut().item_spacing.x = 44.0;
                                    ui.vertical_centered(|ui| {
                                        let (dec_r, dec_resp) = ui
                                            .allocate_exact_size(Vec2::splat(52.0), Sense::click());
                                        ui.painter().circle_filled(
                                            dec_r.center(),
                                            25.0,
                                            pal.danger,
                                        );
                                        draw_lucide_icon(
                                            ui.painter(),
                                            dec_r.center(),
                                            lucide::PHONE_OFF,
                                            20.0,
                                            Color32::WHITE,
                                        );
                                        if dec_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(crate::i18n::tr("Từ chối cuộc gọi"))
                                            .clicked()
                                        {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::AnswerCall {
                                                    accept: false,
                                                    call_id: call.call_id,
                                                });
                                            }
                                            self.active_call = None;
                                        }
                                        ui.label(
                                            RichText::new(crate::i18n::tr("Từ chối"))
                                                .size(11.0)
                                                .color(pal.text_secondary),
                                        );
                                    });

                                    ui.vertical_centered(|ui| {
                                        let (acc_r, acc_resp) = ui
                                            .allocate_exact_size(Vec2::splat(52.0), Sense::click());
                                        ui.painter().circle_filled(
                                            acc_r.center(),
                                            25.0,
                                            pal.accent_online,
                                        );
                                        draw_lucide_icon(
                                            ui.painter(),
                                            acc_r.center(),
                                            lucide::PHONE_CALL,
                                            20.0,
                                            Color32::WHITE,
                                        );
                                        if acc_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(crate::i18n::tr("Nghe máy"))
                                            .clicked()
                                        {
                                            call.is_incoming_ringing = false;
                                            call.state_label = "Đang kết nối cuộc gọi…".into();
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::AnswerCall {
                                                    accept: true,
                                                    call_id: call.call_id,
                                                });
                                            }
                                        }
                                        ui.label(
                                            RichText::new(crate::i18n::tr("Nghe máy"))
                                                .size(11.0)
                                                .color(pal.accent_online),
                                        );
                                    });
                                });
                            });
                        } else {
                            let dock_w = 196.0_f32;
                            let (dock_rect, _) =
                                ui.allocate_exact_size(Vec2::new(dock_w, 56.0), Sense::hover());
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(dock_rect), |ui| {
                                ui.horizontal_centered(|ui| {
                                    ui.spacing_mut().item_spacing.x = 22.0;

                                    // 1) Circular Mic Button
                                    let (mic_r, mic_resp) =
                                        ui.allocate_exact_size(Vec2::splat(48.0), Sense::click());
                                    let mic_bg = if call.mic_muted {
                                        pal.warning.linear_multiply(0.28)
                                    } else if mic_resp.hovered() {
                                        Color32::from_rgb(51, 65, 85)
                                    } else {
                                        Color32::from_rgb(30, 41, 59)
                                    };
                                    ui.painter().circle_filled(mic_r.center(), 23.0, mic_bg);
                                    ui.painter().circle_stroke(
                                        mic_r.center(),
                                        23.0,
                                        Stroke::new(
                                            1.0_f32,
                                            if call.mic_muted {
                                                pal.warning
                                            } else {
                                                Color32::from_rgb(71, 85, 105)
                                            },
                                        ),
                                    );
                                    draw_lucide_icon(
                                        ui.painter(),
                                        mic_r.center(),
                                        if call.mic_muted {
                                            lucide::MIC_OFF
                                        } else {
                                            lucide::MIC
                                        },
                                        18.5,
                                        if call.mic_muted {
                                            pal.warning
                                        } else {
                                            Color32::WHITE
                                        },
                                    );
                                    if mic_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(if call.mic_muted {
                                            "Bật Mic"
                                        } else {
                                            "Tắt Mic"
                                        })
                                        .clicked()
                                    {
                                        call.mic_muted = !call.mic_muted;
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::SetCallMediaState {
                                                is_on_mic: !call.mic_muted,
                                                is_on_cam: call.cam_enabled,
                                            });
                                        }
                                    }

                                    // 2) Prominent Circular Red Hangup Button (dead-center of window)
                                    let (end_r, end_resp) =
                                        ui.allocate_exact_size(Vec2::splat(56.0), Sense::click());
                                    ui.painter().circle_filled(
                                        end_r.center(),
                                        27.0,
                                        if end_resp.hovered() {
                                            Color32::from_rgb(220, 38, 38)
                                        } else {
                                            pal.danger
                                        },
                                    );
                                    draw_lucide_icon(
                                        ui.painter(),
                                        end_r.center(),
                                        lucide::PHONE_OFF,
                                        21.0,
                                        Color32::WHITE,
                                    );
                                    if end_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(crate::i18n::tr("Kết thúc cuộc gọi"))
                                        .clicked()
                                    {
                                        hangup_clicked = true;
                                    }

                                    // 3) Circular Camera Button
                                    let (cam_r, cam_resp) =
                                        ui.allocate_exact_size(Vec2::splat(48.0), Sense::click());
                                    let cam_bg = if call.cam_enabled {
                                        pal.primary.linear_multiply(0.32)
                                    } else if cam_resp.hovered() {
                                        Color32::from_rgb(51, 65, 85)
                                    } else {
                                        Color32::from_rgb(30, 41, 59)
                                    };
                                    ui.painter().circle_filled(cam_r.center(), 23.0, cam_bg);
                                    ui.painter().circle_stroke(
                                        cam_r.center(),
                                        23.0,
                                        Stroke::new(
                                            1.0_f32,
                                            if call.cam_enabled {
                                                pal.primary_hover
                                            } else {
                                                Color32::from_rgb(71, 85, 105)
                                            },
                                        ),
                                    );
                                    draw_lucide_icon(
                                        ui.painter(),
                                        cam_r.center(),
                                        if call.cam_enabled {
                                            lucide::VIDEO
                                        } else {
                                            lucide::VIDEO_OFF
                                        },
                                        18.5,
                                        Color32::WHITE,
                                    );
                                    if cam_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(if call.cam_enabled {
                                            "Tắt Camera"
                                        } else {
                                            "Bật Camera"
                                        })
                                        .clicked()
                                    {
                                        call.cam_enabled = !call.cam_enabled;
                                        call.is_video |= call.cam_enabled;
                                        if !call.cam_enabled {
                                            self.sticker_textures.remove("call_local_video");
                                        }
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::SetCallMediaState {
                                                is_on_mic: !call.mic_muted,
                                                is_on_cam: call.cam_enabled,
                                            });
                                        }
                                    }
                                });
                            });
                        }
                    });
                });
        });

        if retry_clicked {
            call.error_code = None;
            call.error_reason = None;
            call.started_at = std::time::Instant::now();
            call.state_label = if call.is_video {
                "Đang kết nối cuộc gọi video…".into()
            } else {
                "Đang kết nối cuộc gọi thoại…".into()
            };
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::StartCall {
                    partner_id: call.partner_id.clone(),
                    partner_name: call.partner_name.clone(),
                    partner_avatar: None,
                    is_video: call.is_video,
                    is_group: call.is_group,
                });
            }
            self.active_call = Some(call);
        } else if hangup_clicked {
            self.active_call = None;
            self.sticker_textures.remove("call_remote_video");
            self.sticker_textures.remove("call_remote_video_1");
            self.sticker_textures.remove("call_local_video");
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::EndCall {
                    call_id: call.call_id,
                });
            }
        } else if self.active_call.is_some() {
            self.active_call = Some(call);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ForwardItemStatus {
    Queued,
    Sending,
    Success,
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct ForwardTargetDelivery {
    pub(crate) target_id: String,
    pub(crate) target_name: String,
    pub(crate) is_group: bool,
    #[allow(dead_code)]
    pub(crate) avatar: Option<String>,
    pub(crate) forward_msg: ChatMessage,
    pub(crate) forward_status: ForwardItemStatus,
    pub(crate) note_msg: Option<ChatMessage>,
    pub(crate) note_status: Option<ForwardItemStatus>,
}

#[derive(Clone, Debug)]
pub(crate) struct ForwardSessionState {
    pub(crate) original_msg: ChatMessage,
    pub(crate) note_text: String,
    pub(crate) targets: Vec<ForwardTargetDelivery>,
}

impl ForwardSessionState {
    pub(crate) fn all_finished(&self) -> bool {
        self.targets.iter().all(|t| {
            let fwd_done = matches!(
                t.forward_status,
                ForwardItemStatus::Success | ForwardItemStatus::Failed(_)
            );
            let note_done = t.note_status.as_ref().is_none_or(|s| {
                matches!(s, ForwardItemStatus::Success | ForwardItemStatus::Failed(_))
            });
            fwd_done && note_done
        })
    }

    pub(super) fn counts(&self) -> (usize, usize, usize) {
        let total = self.targets.len();
        let succeeded = self
            .targets
            .iter()
            .filter(|t| {
                t.forward_status == ForwardItemStatus::Success
                    && t.note_status
                        .as_ref()
                        .is_none_or(|s| *s == ForwardItemStatus::Success)
            })
            .count();
        let failed = self
            .targets
            .iter()
            .filter(|t| {
                matches!(t.forward_status, ForwardItemStatus::Failed(_))
                    || t.note_status
                        .as_ref()
                        .is_some_and(|s| matches!(s, ForwardItemStatus::Failed(_)))
            })
            .count();
        (total, succeeded, failed)
    }

    pub(super) fn update_outgoing_state(&mut self, thread_id: &str, message: &ChatMessage) -> bool {
        let mut changed = false;
        for target in &mut self.targets {
            if target.target_id == thread_id {
                if target.forward_msg.client_message_id == message.client_message_id {
                    target.forward_msg = message.clone();
                    let new_status = match message.delivery {
                        crate::message_model::DeliveryState::ServerAccepted
                        | crate::message_model::DeliveryState::Delivered
                        | crate::message_model::DeliveryState::Read => ForwardItemStatus::Success,
                        crate::message_model::DeliveryState::Failed => {
                            let err = message
                                .outgoing_job
                                .as_ref()
                                .and_then(|j| {
                                    if j.error.is_empty() {
                                        None
                                    } else {
                                        Some(j.error.clone())
                                    }
                                })
                                .unwrap_or_else(|| "Máy chủ từ chối tin nhắn".to_string());
                            ForwardItemStatus::Failed(err)
                        }
                        _ => ForwardItemStatus::Sending,
                    };
                    if target.forward_status != new_status {
                        target.forward_status = new_status;
                        changed = true;
                    }
                }
                if let Some(note) = &mut target.note_msg {
                    if note.client_message_id == message.client_message_id {
                        *note = message.clone();
                        let new_status = match message.delivery {
                            crate::message_model::DeliveryState::ServerAccepted
                            | crate::message_model::DeliveryState::Delivered
                            | crate::message_model::DeliveryState::Read => {
                                ForwardItemStatus::Success
                            }
                            crate::message_model::DeliveryState::Failed => {
                                let err = message
                                    .outgoing_job
                                    .as_ref()
                                    .and_then(|j| {
                                        if j.error.is_empty() {
                                            None
                                        } else {
                                            Some(j.error.clone())
                                        }
                                    })
                                    .unwrap_or_else(|| "Máy chủ từ chối lời nhắn".to_string());
                                ForwardItemStatus::Failed(err)
                            }
                            _ => ForwardItemStatus::Sending,
                        };
                        if target.note_status.as_ref() != Some(&new_status) {
                            target.note_status = Some(new_status);
                            changed = true;
                        }
                    }
                }
            }
        }
        changed
    }

    /// Retry failed mutations for a specific target, returning the number of commands sent.
    /// Preserves client identity and never resends mutations that already succeeded.
    pub(super) fn retry_target(&mut self, target_id: &str, tx: Option<&WorkerSender>) -> usize {
        let mut sent_count = 0;
        for target in &mut self.targets {
            if target.target_id == target_id {
                if matches!(target.forward_status, ForwardItemStatus::Failed(_)) {
                    let cmd = WorkerCmd::RetryOutgoing {
                        thread_id: target.target_id.clone(),
                        message: target.forward_msg.clone(),
                    };
                    if let Some(tx) = tx {
                        if tx.send(cmd).is_ok() {
                            target.forward_status = ForwardItemStatus::Sending;
                            sent_count += 1;
                        } else {
                            target.forward_status =
                                ForwardItemStatus::Failed("Không thể gửi lệnh tới worker".into());
                        }
                    } else {
                        target.forward_status =
                            ForwardItemStatus::Failed("Worker chưa sẵn sàng".into());
                    }
                }
                if let Some(note) = &target.note_msg {
                    if matches!(target.note_status, Some(ForwardItemStatus::Failed(_))) {
                        let cmd = WorkerCmd::RetryOutgoing {
                            thread_id: target.target_id.clone(),
                            message: note.clone(),
                        };
                        if let Some(tx) = tx {
                            if tx.send(cmd).is_ok() {
                                target.note_status = Some(ForwardItemStatus::Sending);
                                sent_count += 1;
                            } else {
                                target.note_status = Some(ForwardItemStatus::Failed(
                                    "Không thể gửi lệnh tới worker".into(),
                                ));
                            }
                        } else {
                            target.note_status =
                                Some(ForwardItemStatus::Failed("Worker chưa sẵn sàng".into()));
                        }
                    }
                }
            }
        }
        sent_count
    }
}

impl ZaloGuiApp {
    /// Start an active tracking session for outbound forwards and optional accompanying notes.
    /// Preserves exact client identity, verifies queueing per target, and tracks delivery state.
    pub(super) fn start_forward_session(&mut self) -> Option<usize> {
        let msg = self.forward_msg.as_ref()?.clone();
        if self.forward_targets.is_empty() {
            return None;
        }
        if self.forward_cloud_only {
            if self.own_cloud_id.is_empty() {
                return None;
            }
            self.forward_targets.clear();
            self.forward_targets.insert(self.own_cloud_id.clone());
        }
        let note = self.forward_note.trim().to_string();
        let mut target_deliveries = Vec::new();
        let originals = if self.forward_batch.is_empty() {
            vec![msg.clone()]
        } else {
            self.forward_batch.clone()
        };
        if originals.len() > 100
            || originals
                .iter()
                .any(|message| !crate::chat::can_forward_message(message))
        {
            return None;
        }

        for t in &self.threads {
            if !self.forward_targets.contains(&t.id) || !self.private_access(&t.id, t.is_group) {
                continue;
            }
            for (index, source) in originals.iter().enumerate() {
                let mut original = source.clone();
                original.outgoing_job = None;
                let mut forward_msg = crate::outgoing::pending(
                    source.text.clone(),
                    t.is_group,
                    crate::outgoing::Payload::Forward(Box::new(original)),
                );
                forward_msg.sticker_id = source.sticker_id;
                forward_msg.sticker_cate_id = source.sticker_cate_id;
                forward_msg.sticker_type = source.sticker_type;

                let note_msg = if !note.is_empty() && index == 0 {
                    Some(crate::outgoing::pending(
                        note.clone(),
                        t.is_group,
                        crate::outgoing::Payload::Text,
                    ))
                } else {
                    None
                };

                target_deliveries.push(ForwardTargetDelivery {
                    target_id: t.id.clone(),
                    target_name: if originals.len() > 1 {
                        format!(
                            "{} · {}",
                            self.resolve_thread_name(t),
                            truncate_chars(&crate::message_view::summary(source), 50)
                        )
                    } else {
                        self.resolve_thread_name(t).to_string()
                    },
                    is_group: t.is_group,
                    avatar: t.avatar.clone(),
                    forward_msg,
                    forward_status: ForwardItemStatus::Queued,
                    note_msg,
                    note_status: if !note.is_empty() && index == 0 {
                        Some(ForwardItemStatus::Queued)
                    } else {
                        None
                    },
                });
            }
        }

        if target_deliveries.is_empty() {
            return None;
        }

        let tx = WORKER_TX.get();
        for target in &mut target_deliveries {
            let fwd_cmd = WorkerCmd::RetryOutgoing {
                thread_id: target.target_id.clone(),
                message: target.forward_msg.clone(),
            };
            if let Some(ref tx) = tx {
                if tx.send(fwd_cmd).is_ok() {
                    target.forward_status = ForwardItemStatus::Sending;
                } else {
                    target.forward_status =
                        ForwardItemStatus::Failed("Không thể gửi lệnh tới worker".into());
                }
            } else {
                target.forward_status = ForwardItemStatus::Failed("Worker chưa sẵn sàng".into());
            }

            if let Some(note_msg) = &target.note_msg {
                let note_cmd = WorkerCmd::RetryOutgoing {
                    thread_id: target.target_id.clone(),
                    message: note_msg.clone(),
                };
                if let Some(ref tx) = tx {
                    if tx.send(note_cmd).is_ok() {
                        target.note_status = Some(ForwardItemStatus::Sending);
                    } else {
                        target.note_status = Some(ForwardItemStatus::Failed(
                            "Không thể gửi lệnh tới worker".into(),
                        ));
                    }
                } else {
                    target.note_status =
                        Some(ForwardItemStatus::Failed("Worker chưa sẵn sàng".into()));
                }
            }
        }

        let target_count = target_deliveries.len();
        self.forward_session = Some(ForwardSessionState {
            original_msg: msg,
            note_text: note,
            targets: target_deliveries,
        });
        self.forward_msg = None;
        self.forward_batch.clear();
        self.forward_targets.clear();
        self.forward_note.clear();
        self.forward_filter.clear();
        Some(target_count)
    }

    /// Build outbound forward and optional note commands for all selected targets.
    #[allow(dead_code)]
    pub(super) fn build_forward_commands(&self) -> (usize, Vec<WorkerCmd>) {
        let Some(msg) = self.forward_msg.as_ref() else {
            return (0, Vec::new());
        };
        if self.forward_targets.is_empty() {
            return (0, Vec::new());
        }
        let note = self.forward_note.trim();
        let mut target_count = 0usize;
        let mut cmds = Vec::new();
        for t in &self.threads {
            if !self.forward_targets.contains(&t.id) || !self.private_access(&t.id, t.is_group) {
                continue;
            }
            target_count += 1;
            cmds.push(WorkerCmd::Forward {
                target_id: t.id.clone(),
                target_is_group: t.is_group,
                msg: msg.clone(),
            });
            if !note.is_empty() {
                cmds.push(WorkerCmd::SendMessage {
                    to_id: t.id.clone(),
                    text: note.to_owned(),
                    is_group: t.is_group,
                });
            }
        }
        (target_count, cmds)
    }

    pub(super) fn render_forward_session_modal(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if self.forward_session.is_none() {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.clear_forward_dialog();
            return;
        }

        render_modal_backdrop(ctx, "forward_session_modal_backdrop");

        let mut open = true;
        let mut do_close = false;
        let mut retry_target_id: Option<String> = None;
        let mut retry_all = false;

        let win_w = 460.0_f32.min((ctx.screen_rect().width() - 32.0).max(280.0));
        let win_h = 520.0_f32.min((ctx.screen_rect().height() - 48.0).max(280.0));

        let (total, succeeded, failed, all_done, preview, note_text, targets_summary) = {
            let session = self.forward_session.as_ref().unwrap();
            let (tot, succ, fail) = session.counts();
            let done = session.all_finished();
            let prev = truncate_chars(&crate::message_view::summary(&session.original_msg), 100)
                .into_owned();
            let note = session.note_text.clone();
            let summaries: Vec<(
                String,
                String,
                bool,
                ForwardItemStatus,
                Option<ForwardItemStatus>,
            )> = session
                .targets
                .iter()
                .map(|t| {
                    (
                        t.target_id.clone(),
                        t.target_name.clone(),
                        t.is_group,
                        t.forward_status.clone(),
                        t.note_status.clone(),
                    )
                })
                .collect();
            (tot, succ, fail, done, prev, note, summaries)
        };

        egui::Window::new(crate::i18n::tr("Trạng thái chia sẻ"))
            .id(Id::new("forward_session_status_modal"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(win_w, win_h))
            .show(ctx, |ui| {
                // Header summary banner
                let (status_text, status_color) = if all_done {
                    if failed == 0 {
                        (
                            format!("✓ Đã xác nhận {succeeded}/{total} lượt chia sẻ"),
                            pal.accent_online,
                        )
                    } else if succeeded == 0 {
                        (
                            format!("✗ Thất bại ở tất cả {failed} lượt chia sẻ"),
                            pal.danger,
                        )
                    } else {
                        (
                            format!("! Hoàn tất với {succeeded} thành công, {failed} thất bại"),
                            pal.warning,
                        )
                    }
                } else {
                    let in_progress = total.saturating_sub(succeeded + failed);
                    (
                        format!(
                            "Đang gửi... ({succeeded} hoàn thành, {failed} lỗi, {in_progress} đang chờ)",
                        ),
                        pal.primary,
                    )
                };

                egui::Frame::none()
                    .fill(status_color.linear_multiply(0.12))
                    .stroke(Stroke::new(1.0_f32, status_color))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(status_text)
                                .size(13.0)
                                .color(status_color)
                                .strong(),
                        );
                    });

                ui.add_space(8.0);

                // Content preview
                egui::Frame::none()
                    .fill(pal.bg_input)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(10.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(crate::i18n::tr("Tin nhắn chia sẻ"))
                                .size(11.0)
                                .color(pal.text_muted)
                                .strong(),
                        );
                        ui.label(RichText::new(preview).size(12.0).color(pal.text_primary));
                        if !note_text.is_empty() {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("Lời nhắn kèm: {note_text}"))
                                    .size(11.5)
                                    .color(pal.text_secondary)
                                    .italics(),
                            );
                        }
                    });

                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Chi tiết từng lượt chia sẻ ({total}):"))
                        .size(12.0)
                        .color(pal.text_secondary)
                        .strong(),
                );
                ui.add_space(4.0);

                let list_h = (win_h - 240.0).clamp(100.0, 260.0);
                egui::ScrollArea::vertical()
                    .id_salt("forward_session_targets_scroll")
                    .max_height(list_h)
                    .show(ui, |ui| {
                        for (target_id, target_name, is_group, forward_status, note_status) in
                            &targets_summary
                        {
                            egui::Frame::none()
                                .fill(pal.bg_input)
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(Margin::symmetric(8.0, 6.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let has_failed =
                                            matches!(forward_status, ForwardItemStatus::Failed(_))
                                                || note_status.as_ref().is_some_and(|s| {
                                                    matches!(s, ForwardItemStatus::Failed(_))
                                                });

                                        let display_name = if *is_group {
                                            format!("[Nhóm] {target_name}")
                                        } else {
                                            target_name.clone()
                                        };

                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(truncate_chars(&display_name, 28))
                                                    .size(12.5)
                                                    .color(pal.text_primary)
                                                    .strong(),
                                            );
                                            // Forward status
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new("Tin nhắn: ")
                                                        .size(11.0)
                                                        .color(pal.text_muted),
                                                );
                                                match forward_status {
                                                    ForwardItemStatus::Queued => {
                                                        ui.label(
                                                            RichText::new("Chờ gửi...")
                                                                .size(11.0)
                                                                .color(pal.text_muted),
                                                        );
                                                    }
                                                    ForwardItemStatus::Sending => {
                                                        ui.label(
                                                            RichText::new("Đang gửi...")
                                                                .size(11.0)
                                                                .color(pal.primary),
                                                        );
                                                    }
                                                    ForwardItemStatus::Success => {
                                                        ui.label(
                                                            RichText::new("✓ Đã gửi")
                                                                .size(11.0)
                                                                .color(pal.accent_online),
                                                        );
                                                    }
                                                    ForwardItemStatus::Failed(err) => {
                                                        ui.label(
                                                            RichText::new(format!("✗ {err}"))
                                                                .size(11.0)
                                                                .color(pal.danger),
                                                        );
                                                    }
                                                }
                                            });

                                            // Note status if any
                                            if let Some(note_st) = note_status {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new("Lời nhắn: ")
                                                            .size(11.0)
                                                            .color(pal.text_muted),
                                                    );
                                                    match note_st {
                                                        ForwardItemStatus::Queued => {
                                                            ui.label(
                                                                RichText::new("Chờ gửi...")
                                                                    .size(11.0)
                                                                    .color(pal.text_muted),
                                                            );
                                                        }
                                                        ForwardItemStatus::Sending => {
                                                            ui.label(
                                                                RichText::new("Đang gửi...")
                                                                    .size(11.0)
                                                                    .color(pal.primary),
                                                            );
                                                        }
                                                        ForwardItemStatus::Success => {
                                                            ui.label(
                                                                RichText::new("✓ Đã gửi")
                                                                    .size(11.0)
                                                                    .color(pal.accent_online),
                                                            );
                                                        }
                                                        ForwardItemStatus::Failed(err) => {
                                                            ui.label(
                                                                RichText::new(format!("✗ {err}"))
                                                                    .size(11.0)
                                                                    .color(pal.danger),
                                                            );
                                                        }
                                                    }
                                                });
                                            }
                                        });

                                        if has_failed {
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    let retry_btn = egui::Button::new(
                                                        RichText::new("Thử lại")
                                                            .size(11.5)
                                                            .color(Color32::WHITE),
                                                    )
                                                    .fill(pal.primary)
                                                    .rounding(Rounding::same(4.0))
                                                    .min_size(Vec2::new(60.0, 24.0));
                                                    if ui.add(retry_btn).clicked() {
                                                        retry_target_id = Some(target_id.clone());
                                                    }
                                                },
                                            );
                                        }
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if failed > 0 {
                        let retry_all_btn = egui::Button::new(
                            RichText::new("Thử lại tất cả lỗi")
                                .size(12.0)
                                .color(pal.primary),
                        )
                        .fill(pal.bg_input)
                        .stroke(Stroke::new(1.0_f32, pal.primary))
                        .rounding(Rounding::same(6.0))
                        .min_size(Vec2::new(120.0, 28.0));
                        if ui.add(retry_all_btn).clicked() {
                            retry_all = true;
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let close_label = if all_done { "Hoàn tất" } else { "Đóng" };
                        let close_btn = egui::Button::new(
                            RichText::new(crate::i18n::tr(close_label))
                                .size(13.0)
                                .color(pal.text_primary),
                        )
                        .fill(pal.bg_input)
                        .rounding(Rounding::same(6.0))
                        .min_size(Vec2::new(80.0, 30.0));
                        if ui.add(close_btn).clicked() {
                            do_close = true;
                        }
                    });
                });
            });

        if let Some(target_id) = retry_target_id {
            if let Some(session) = &mut self.forward_session {
                session.retry_target(&target_id, WORKER_TX.get().as_ref());
                ctx.request_repaint();
            }
        }
        if retry_all {
            if let Some(session) = &mut self.forward_session {
                let failed_ids: Vec<String> = session
                    .targets
                    .iter()
                    .filter(|t| {
                        matches!(t.forward_status, ForwardItemStatus::Failed(_))
                            || t.note_status
                                .as_ref()
                                .is_some_and(|s| matches!(s, ForwardItemStatus::Failed(_)))
                    })
                    .map(|t| t.target_id.clone())
                    .collect();
                for id in failed_ids {
                    session.retry_target(&id, WORKER_TX.get().as_ref());
                }
                ctx.request_repaint();
            }
        }

        if do_close || !open {
            self.clear_forward_dialog();
            if total > 0 {
                if failed == 0 {
                    let notice = format!("Đã chia sẻ tin nhắn tới {succeeded} hội thoại");
                    self.status_msg = notice.clone();
                    self.show_toast(notice);
                } else if succeeded > 0 {
                    let notice =
                        format!("Chia sẻ hoàn tất: {succeeded} thành công, {failed} thất bại");
                    self.status_msg = notice.clone();
                    self.show_toast(notice);
                } else {
                    let notice = format!("Chia sẻ thất bại ({failed} lỗi)");
                    self.status_msg = notice.clone();
                    self.show_toast(notice);
                }
            }
        }
    }

    pub(super) fn render_forward_dialog(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if self.forward_session.is_some() {
            self.render_forward_session_modal(ctx, pal);
            return;
        }
        if self.forward_msg.is_none() {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.clear_forward_dialog();
            return;
        }
        let preview = self
            .forward_msg
            .as_ref()
            .map(|m| truncate_chars(&crate::message_view::summary(m), 120).into_owned())
            .unwrap_or_default();
        let preview = if self.forward_batch.len() > 1 {
            format!(
                "{} tin nhắn gốc đã chọn. Mục đầu: {preview}",
                self.forward_batch.len()
            )
        } else {
            preview
        };
        let query = crate::search::fold(self.forward_filter.trim());
        let candidates: Vec<(String, String, bool, Option<String>)> = self
            .threads
            .iter()
            .filter(|t| {
                (!self.forward_cloud_only || (t.id == self.own_cloud_id && !t.is_group))
                    && self.private_access(&t.id, t.is_group)
                    && !self
                        .conversation_preferences
                        .get(&t.id)
                        .is_some_and(|prefs| prefs.hidden)
            })
            .filter(|t| {
                query.is_empty()
                    || crate::search::fold(self.resolve_thread_name(t)).contains(&query)
                    || t.presentation
                        .base_name
                        .as_deref()
                        .is_some_and(|b| crate::search::fold(b).contains(&query))
                    || t.id.contains(&query)
            })
            .take(500)
            .map(|t| {
                (
                    t.id.clone(),
                    self.resolve_thread_name(t).to_owned(),
                    t.is_group,
                    t.avatar.clone(),
                )
            })
            .collect();
        render_modal_backdrop(ctx, "forward_modal_backdrop");

        let mut open = true;
        let mut do_cancel = false;
        let mut do_send = false;
        let mut toggled_id: Option<String> = None;
        let mut clear_selection = false;

        let win_w = 420.0_f32.min((ctx.screen_rect().width() - 32.0).max(260.0));
        let win_h = 540.0_f32.min((ctx.screen_rect().height() - 48.0).max(260.0));
        egui::Window::new(crate::i18n::tr("Chia sẻ"))
            .id(Id::new("forward_message_modal"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(win_w, win_h))
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.forward_filter)
                        .hint_text(crate::i18n::tr("Tìm hội thoại, bạn bè, nhóm…"))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    let count = self.forward_targets.len();
                    ui.label(
                        RichText::new(format!("Đã chọn: {count}"))
                            .size(12.0)
                            .color(if count > 0 {
                                pal.primary
                            } else {
                                pal.text_secondary
                            })
                            .strong(),
                    );
                    if count > 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(crate::i18n::tr("Bỏ chọn tất cả"))
                                            .size(11.5)
                                            .color(pal.text_secondary),
                                    )
                                    .frame(false),
                                )
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked()
                            {
                                clear_selection = true;
                            }
                        });
                    }
                });
                ui.add_space(4.0);

                let list_h = (win_h - 265.0).clamp(110.0, 250.0);
                egui::ScrollArea::vertical()
                    .id_salt("forward_targets_scroll")
                    .max_height(list_h)
                    .show(ui, |ui| {
                        if candidates.is_empty() {
                            ui.add_space(16.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(crate::i18n::tr(
                                        "Không tìm thấy hội thoại phù hợp",
                                    ))
                                    .size(12.5)
                                    .color(pal.text_muted),
                                );
                            });
                            return;
                        }
                        for (id, name, is_group, avatar) in &candidates {
                            let selected = self.forward_targets.contains(id);
                            let row_w = ui.available_width();
                            let (row_rect, row_resp) =
                                ui.allocate_exact_size(Vec2::new(row_w, 38.0), Sense::click());
                            let row_bg = if selected {
                                pal.primary.linear_multiply(0.12)
                            } else if row_resp.hovered() {
                                pal.bg_hover
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter()
                                .rect_filled(row_rect, Rounding::same(6.0), row_bg);

                            let check_center =
                                egui::pos2(row_rect.left() + 14.0, row_rect.center().y);
                            let check_box =
                                egui::Rect::from_center_size(check_center, Vec2::splat(16.0));
                            if selected {
                                ui.painter().rect_filled(
                                    check_box,
                                    Rounding::same(4.0),
                                    pal.primary,
                                );
                                ui.painter().text(
                                    check_center,
                                    egui::Align2::CENTER_CENTER,
                                    "✓",
                                    FontId::proportional(11.5),
                                    Color32::WHITE,
                                );
                            } else {
                                ui.painter().rect_stroke(
                                    check_box,
                                    Rounding::same(4.0),
                                    Stroke::new(1.2_f32, pal.border),
                                );
                            }

                            let ava_center =
                                egui::pos2(row_rect.left() + 42.0, row_rect.center().y);
                            paint_user_avatar(
                                ui.painter(),
                                ava_center,
                                13.0,
                                name,
                                *is_group,
                                avatar.as_deref(),
                                &self.sticker_textures,
                                &mut self.loading_stickers,
                            );

                            let display_name = if *is_group {
                                format!("[Nhóm] {name}")
                            } else {
                                name.clone()
                            };
                            let text_pos = egui::pos2(row_rect.left() + 62.0, row_rect.center().y);
                            ui.painter().text(
                                text_pos,
                                egui::Align2::LEFT_CENTER,
                                truncate_chars(&display_name, 34),
                                FontId::proportional(13.2),
                                pal.text_primary,
                            );

                            if row_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                toggled_id = Some(id.clone());
                            }
                            ui.add_space(2.0);
                        }
                    });

                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(pal.bg_input)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(crate::i18n::tr("Nội dung chia sẻ"))
                                .size(11.0)
                                .color(pal.text_muted)
                                .strong(),
                        );
                        ui.add_space(2.0);
                        ui.label(RichText::new(&preview).size(12.5).color(pal.text_primary));
                        if self.forward_batch.len() > 1 {
                            egui::ScrollArea::vertical()
                                .id_salt("archive_share_preview")
                                .max_height(90.0)
                                .show(ui, |ui| {
                                    for (index, message) in self.forward_batch.iter().enumerate() {
                                        ui.small(format!(
                                            "{}. {}",
                                            index + 1,
                                            truncate_chars(
                                                &crate::message_view::summary(message),
                                                100
                                            )
                                        ));
                                    }
                                });
                        }
                    });

                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.forward_note)
                        .hint_text(crate::i18n::tr("Nhập lời nhắn..."))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_send = !self.forward_targets.is_empty();
                        let share_btn = egui::Button::new(
                            RichText::new(if self.forward_cloud_only {
                                "Lưu vào My Documents"
                            } else {
                                crate::i18n::tr("Chia sẻ")
                            })
                            .size(13.0)
                            .color(if can_send {
                                Color32::WHITE
                            } else {
                                pal.text_muted
                            })
                            .strong(),
                        )
                        .fill(if can_send { pal.primary } else { pal.bg_input })
                        .rounding(Rounding::same(6.0))
                        .min_size(Vec2::new(92.0, 32.0));
                        if ui
                            .add_enabled(can_send, share_btn)
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            do_send = true;
                        }

                        let cancel_btn = egui::Button::new(
                            RichText::new(crate::i18n::tr("Hủy"))
                                .size(13.0)
                                .color(pal.text_primary),
                        )
                        .fill(pal.bg_input)
                        .rounding(Rounding::same(6.0))
                        .min_size(Vec2::new(76.0, 32.0));
                        if ui
                            .add(cancel_btn)
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            do_cancel = true;
                        }
                    });
                });
            });

        if self.forward_cloud_only {
            clear_selection = false;
            toggled_id = None;
        }
        if clear_selection {
            self.forward_targets.clear();
        }
        if let Some(id) = toggled_id {
            if !self.forward_targets.remove(&id) {
                self.forward_targets.insert(id);
            }
        }

        if do_cancel || !open {
            self.clear_forward_dialog();
        } else if do_send {
            if self.start_forward_session().is_some() {
                ctx.request_repaint();
            } else {
                self.show_toast("Vui lòng chọn ít nhất một hội thoại để chia sẻ");
            }
        }
    }

    pub(super) fn open_media_viewer(&mut self, thread_id: &str, message: &ChatMessage) {
        let Some(thread) = self
            .threads
            .iter()
            .find(|t| t.id == thread_id && self.private_access(&t.id, t.is_group))
        else {
            return;
        };
        let media = crate::message_view::media(message);
        let video = media.kind == crate::message_view::MediaKind::Video;
        let url = if video {
            media.thumbnail.clone()
        } else {
            media
                .preview
                .clone()
                .or_else(|| media.original.clone())
                .or_else(|| media.thumbnail.clone())
        }
        .unwrap_or_default();
        let key = crate::media_recovery::image_key(thread_id, &message.id, 0, !video);
        if !url.is_empty()
            && !self.sticker_textures.contains_key(&key)
            && self.loading_stickers.insert(key.clone())
        {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchChatImage {
                    key: key.clone(),
                    url: url.clone(),
                    thread: thread_id.into(),
                    group: thread.is_group,
                    message: Box::new(message.clone()),
                    preview: !video,
                });
            }
        }
        self.image_viewer = Some(ImageViewerModal {
            source: super::worker::ChatImage {
                thread: thread_id.into(),
                group: thread.is_group,
                message: message.clone(),
                preview: !video,
            },
            is_video: video,
            navigation_pending: false,
            thread_id: thread_id.into(),
            message_id: message.id.clone(),
            dimensions: media.width.zip(media.height),
            tex_key: crate::media_recovery::image_key(thread_id, &message.id, 0, false),
            hd_tex_key: (!video).then_some(key),
            original_url: media.original.unwrap_or_default(),
            title: media.name,
            sender: message.sender.clone(),
            time: crate::clock::message_time(message.ts),
            zoom: 1.0,
            pan: Vec2::ZERO,
            video_state: if video {
                Some(super::VideoPlayerState {
                    playing: false,
                    position_secs: 0.0,
                    duration_secs: media.duration.map(|d| d as f32).unwrap_or(0.0),
                    volume: 1.0,
                })
            } else {
                None
            },
        });
    }

    pub(super) fn render_image_viewer_modal(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let Some(mut viewer) = self.image_viewer.clone() else {
            return;
        };

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.image_viewer = None;
            return;
        }

        // WP15-4: Keyboard shortcuts (Task 4b)
        let ctrl_pressed = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if ctrl_pressed && ctx.input(|i| i.key_pressed(Key::S)) {
            // Ctrl+S: Save media
            if let Some(ref viewer) = self.image_viewer {
                if !self.content_protected(&viewer.source.thread, viewer.source.group)
                    && !viewer.original_url.is_empty()
                {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::DownloadMedia {
                            url: viewer.original_url.clone(),
                            name: viewer.title.clone(),
                            play: false,
                            open: false,
                            context: Some(viewer.source.clone()),
                        });
                    }
                }
            }
        }
        if ctrl_pressed && ctx.input(|i| i.key_pressed(Key::C)) && !viewer.is_video {
            // Ctrl+C: Copy image
            if !self.content_protected(&viewer.source.thread, viewer.source.group) {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::CopyArchiveImage {
                        owner: self.user_uin.clone(),
                        media: crate::message_view::Media {
                            original: Some(viewer.original_url.clone()),
                            ..Default::default()
                        },
                        context: viewer.source.clone(),
                    });
                }
            }
        }

        let screen_rect = ctx.screen_rect();
        // Windows owns the native caption outside the client area.
        let viewer_top = screen_rect.top();
        let toolbar_h = if screen_rect.width() < 760.0 {
            116.0_f32
        } else {
            88.0_f32
        };
        let toolbar_rect = Rect::from_min_max(
            Pos2::new(screen_rect.left(), viewer_top),
            Pos2::new(
                screen_rect.right(),
                (viewer_top + toolbar_h).min(screen_rect.bottom()),
            ),
        );
        let stage_area_rect = Rect::from_min_max(
            Pos2::new(screen_rect.left(), toolbar_rect.bottom()),
            screen_rect.max,
        );
        let mut should_close = false;
        let mut navigate = None;
        let mut retry = false;
        if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
            navigate = Some(false);
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
            navigate = Some(true);
        }

        let active_tex = viewer
            .hd_tex_key
            .as_ref()
            .and_then(|hk| self.sticker_textures.get(hk))
            .or_else(|| self.sticker_textures.get(&viewer.tex_key))
            .cloned();

        let max_stage_w = (stage_area_rect.width() - 48.0).max(160.0);
        let max_stage_h = (stage_area_rect.height() - 36.0).max(120.0);

        let (base_w, base_h, natural_dims) = if let Some(ref tex) = active_tex {
            let [tw, th] = tex.size();
            let tw_f = (tw.max(1)) as f32;
            let th_f = (th.max(1)) as f32;
            let fit_scale = (max_stage_w / tw_f).min(max_stage_h / th_f).min(1.0);
            (
                tw_f * fit_scale,
                th_f * fit_scale,
                viewer.dimensions.map(|(w, h)| (w as usize, h as usize)),
            )
        } else {
            (340.0_f32.min(max_stage_w), 220.0_f32.min(max_stage_h), None)
        };

        // Mouse-wheel zoom (zooms smoothly around cursor without clamping disp_w/disp_h to max_stage)
        let scroll_y = ctx.input(|i| {
            if i.raw_scroll_delta.y.abs() > 0.1 {
                i.raw_scroll_delta.y
            } else {
                i.smooth_scroll_delta.y
            }
        });
        if scroll_y.abs() > 0.1 {
            let old_zoom = viewer.zoom;
            let factor = if scroll_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
            let new_zoom = (old_zoom * factor).clamp(0.25, 32.0);
            if let Some(ptr) = ctx.input(|i| i.pointer.hover_pos()) {
                if stage_area_rect.contains(ptr) {
                    let rel = ptr - (stage_area_rect.center() + viewer.pan);
                    viewer.pan -= rel * (new_zoom / old_zoom - 1.0);
                }
            }
            viewer.zoom = new_zoom;
        }

        let disp_w = (base_w * viewer.zoom).max(48.0);
        let disp_h = (base_h * viewer.zoom).max(48.0);

        // Clamp pan so the image stays centered when <= stage size, and pans within bounds when enlarged
        let max_pan_x = ((disp_w - stage_area_rect.width() + 40.0) * 0.5).max(0.0);
        let max_pan_y = ((disp_h - stage_area_rect.height() + 40.0) * 0.5).max(0.0);
        viewer.pan.x = viewer.pan.x.clamp(-max_pan_x, max_pan_x);
        viewer.pan.y = viewer.pan.y.clamp(-max_pan_y, max_pan_y);

        // 1. Unified Clipped Stage Canvas (never uses a floating Area at img_pos so it NEVER shifts left!)
        egui::Area::new(Id::new("image_viewer_backdrop"))
            .order(egui::Order::Foreground)
            .constrain(false)
            .fixed_pos(stage_area_rect.min)
            .show(ctx, |ui| {
                let (bg_rect, bg_resp) =
                    ui.allocate_exact_size(stage_area_rect.size(), Sense::click_and_drag());
                let painter = ui.painter().with_clip_rect(bg_rect);
                let bg_col = if pal.is_dark {
                    Color32::from_rgb(24, 26, 31)
                } else {
                    Color32::from_rgb(24, 28, 36)
                };
                painter.rect_filled(bg_rect, Rounding::ZERO, bg_col);

                let img_center = bg_rect.center() + viewer.pan;
                let img_rect = Rect::from_center_size(img_center, Vec2::new(disp_w, disp_h));

                if let Some(ref tex) = active_tex {
                    painter.image(
                        tex.id(),
                        img_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    painter.rect_stroke(img_rect, Rounding::ZERO, Stroke::new(1.0_f32, pal.border));

                    if bg_resp.dragged_by(egui::PointerButton::Primary) {
                        viewer.pan += bg_resp.drag_delta();
                        viewer.pan.x = viewer.pan.x.clamp(-max_pan_x, max_pan_x);
                        viewer.pan.y = viewer.pan.y.clamp(-max_pan_y, max_pan_y);
                        ctx.set_cursor_icon(CursorIcon::Grabbing);
                    } else if let Some(ptr) = bg_resp.hover_pos() {
                        if img_rect.contains(ptr) && (max_pan_x > 1.0 || max_pan_y > 1.0) {
                            ctx.set_cursor_icon(CursorIcon::Grab);
                        }
                    }

                    if bg_resp.double_clicked() {
                        if (viewer.zoom - 1.0).abs() < 0.05 {
                            viewer.zoom = 2.0;
                        } else {
                            viewer.zoom = 1.0;
                            viewer.pan = Vec2::ZERO;
                        }
                    } else if bg_resp.clicked() {
                        let ptr = bg_resp
                            .interact_pointer_pos()
                            .or_else(|| ctx.input(|i| i.pointer.latest_pos()));
                        if ptr.map(|p| !img_rect.contains(p)).unwrap_or(false) {
                            should_close = true;
                        }
                    }
                } else {
                    painter.rect_filled(img_rect, Rounding::same(8.0), pal.bg_elevated);
                    painter.rect_stroke(
                        img_rect,
                        Rounding::same(8.0),
                        Stroke::new(1.0_f32, pal.border),
                    );
                    painter.text(
                        img_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        viewer
                            .hd_tex_key
                            .as_ref()
                            .and_then(|k| self.image_failures.get(k))
                            .or_else(|| self.image_failures.get(&viewer.tex_key))
                            .map(|e| e.failure.message())
                            .unwrap_or(if viewer.is_video {
                                "Chưa có ảnh xem trước. Bấm Phát video để mở."
                            } else {
                                "Đang tải hình ảnh…"
                            }),
                        FontId::proportional(13.0),
                        pal.text_primary,
                    );
                    if bg_resp.clicked() {
                        should_close = true;
                    }
                }
            });
        egui::Area::new(Id::new("image_viewer_top_bar"))
            .order(egui::Order::Tooltip)
            .fixed_pos(toolbar_rect.min)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(pal.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .inner_margin(Margin::symmetric(12.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_width((toolbar_rect.width() - 24.0).max(240.0));
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(truncate_chars(
                                        &viewer.title,
                                        if screen_rect.width() < 760.0 { 24 } else { 64 },
                                    ))
                                    .strong()
                                    .color(pal.text_primary),
                                );
                                let dims = natural_dims
                                    .map(|(w, h)| format!(" • {w}×{h} px"))
                                    .unwrap_or_default();
                                ui.label(
                                    RichText::new(format!(
                                        "{} • {}{}",
                                        truncate_chars(&viewer.sender, 24),
                                        viewer.time,
                                        dims
                                    ))
                                    .size(11.0)
                                    .color(pal.text_secondary),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .button(crate::i18n::tr("Đóng ×"))
                                    .on_hover_text(crate::i18n::tr("Đóng (Esc)"))
                                    .clicked()
                                {
                                    should_close = true;
                                }
                            });
                        });
                        ui.horizontal_wrapped(|ui| {
                            let failed = self.image_failures.contains_key(&viewer.tex_key)
                                || viewer
                                    .hd_tex_key
                                    .as_ref()
                                    .is_some_and(|k| self.image_failures.contains_key(k));
                            if failed && ui.button(crate::i18n::tr("Thử tải lại")).clicked() {
                                retry = true;
                            }

                            if ui
                                .add_enabled(
                                    !viewer.navigation_pending,
                                    egui::Button::new(crate::i18n::tr("Trước")),
                                )
                                .on_hover_text(crate::i18n::tr("Trước (←)"))
                                .clicked()
                            {
                                navigate = Some(false);
                            }
                            if ui
                                .add_enabled(
                                    !viewer.navigation_pending,
                                    egui::Button::new(crate::i18n::tr("Tiếp")),
                                )
                                .on_hover_text(crate::i18n::tr("Tiếp (→)"))
                                .clicked()
                            {
                                navigate = Some(true);
                            }
                            if viewer.navigation_pending {
                                ui.spinner();
                            }

                            // WP15-4: Album navigation counter (Task 4b) - placeholder for current/total
                            // TODO: Wire MediaNeighbor response to track position in album
                            // ui.label(RichText::new("3 / 12").size(11.0).color(pal.text_secondary));

                            // WP15-4: Video player controls (Task 4a)
                            if let Some(ref mut vstate) = viewer.video_state {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    let is_active = crate::video_player::is_video_window_active();
                                    let is_playing = vstate.playing || is_active;
                                    let play_icon = if is_playing { "⏸" } else { "▶" };
                                    let play_tip = if is_playing {
                                        "Tạm dừng"
                                    } else {
                                        "Phát video"
                                    };

                                    if ui.small_button(play_icon).on_hover_text(play_tip).clicked()
                                    {
                                        if is_active {
                                            crate::video_player::control_play_pause();
                                            vstate.playing = !vstate.playing;
                                        } else {
                                            vstate.playing = true;
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::DownloadMedia {
                                                    url: viewer.original_url.clone(),
                                                    name: viewer.title.clone(),
                                                    play: true,
                                                    open: false,
                                                    context: Some(viewer.source.clone()),
                                                });
                                            }
                                        }
                                    }

                                    // Time display
                                    let pos_min = (vstate.position_secs / 60.0) as u32;
                                    let pos_sec = (vstate.position_secs % 60.0) as u32;
                                    let dur_min = (vstate.duration_secs / 60.0) as u32;
                                    let dur_sec = (vstate.duration_secs % 60.0) as u32;
                                    ui.label(format!(
                                        "{:02}:{:02} / {:02}:{:02}",
                                        pos_min, pos_sec, dur_min, dur_sec
                                    ));

                                    // Seek scrubber
                                    let max_dur = vstate.duration_secs.max(1.0);
                                    let mut pos = vstate.position_secs;
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut pos, 0.0..=max_dur)
                                                .show_value(false),
                                        )
                                        .changed()
                                    {
                                        vstate.position_secs = pos;
                                        crate::video_player::control_seek(pos);
                                    }

                                    // Volume control
                                    ui.label("🔊");
                                    let mut vol = vstate.volume;
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut vol, 0.0..=1.0)
                                                .show_value(false)
                                                .fixed_decimals(2),
                                        )
                                        .changed()
                                    {
                                        vstate.volume = vol;
                                        crate::video_player::control_set_volume(vol);
                                    }
                                });
                            }

                            if viewer.is_video {
                                if ui
                                    .add_enabled(
                                        !self.content_protected(
                                            &viewer.source.thread,
                                            viewer.source.group,
                                        ) && !viewer.original_url.is_empty(),
                                        egui::Button::new(crate::i18n::tr("▶ Cửa sổ video riêng")),
                                    )
                                    .on_hover_text(crate::i18n::tr(
                                        "Mở video trong cửa sổ native riêng biệt",
                                    ))
                                    .clicked()
                                {
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::DownloadMedia {
                                            url: viewer.original_url.clone(),
                                            name: viewer.title.clone(),
                                            play: true,
                                            open: false,
                                            context: Some(viewer.source.clone()),
                                        });
                                    }
                                }
                            } else {
                                if ui.small_button("−").clicked() {
                                    viewer.zoom = (viewer.zoom / 1.25).max(0.25);
                                }
                                ui.label(format!(
                                    "{:.0}%",
                                    base_w * viewer.zoom
                                        / viewer
                                            .dimensions
                                            .map(|(w, _)| w as f32)
                                            .unwrap_or(base_w)
                                            .max(1.0)
                                        * 100.0
                                ));
                                if ui.small_button("+").clicked() {
                                    viewer.zoom = (viewer.zoom * 1.25).min(32.0);
                                }
                                if ui.small_button(crate::i18n::tr("Vừa cửa sổ")).clicked() {
                                    viewer.zoom = 1.0;
                                    viewer.pan = Vec2::ZERO;
                                }
                                if ui
                                    .small_button("1:1")
                                    .on_hover_text(crate::i18n::tr("Kích thước ảnh gốc"))
                                    .clicked()
                                {
                                    viewer.zoom = (viewer
                                        .dimensions
                                        .map(|(w, _)| w as f32)
                                        .unwrap_or(base_w)
                                        / base_w.max(1.0))
                                    .clamp(0.25, 32.0);
                                    viewer.pan = Vec2::ZERO;
                                }
                            }
                            if ui
                                .add_enabled(
                                    !self.content_protected(
                                        &viewer.source.thread,
                                        viewer.source.group,
                                    ) && !viewer.original_url.is_empty(),
                                    egui::Button::new(crate::i18n::tr("Lưu thành…")),
                                )
                                .clicked()
                            {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::DownloadMedia {
                                        url: viewer.original_url.clone(),
                                        name: viewer.title.clone(),
                                        play: false,
                                        open: false,
                                        context: Some(viewer.source.clone()),
                                    });
                                }
                            }
                            if !viewer.is_video
                                && ui
                                    .add_enabled(
                                        !self.content_protected(
                                            &viewer.source.thread,
                                            viewer.source.group,
                                        ),
                                        egui::Button::new("Sao chép ảnh"),
                                    )
                                    .on_hover_text(crate::i18n::tr(
                                        "Sao chép ảnh ở chất lượng đang xem",
                                    ))
                                    .clicked()
                            {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::CopyArchiveImage {
                                        owner: self.user_uin.clone(),
                                        media: crate::message_view::Media {
                                            original: Some(viewer.original_url.clone()),
                                            ..Default::default()
                                        },
                                        context: viewer.source.clone(),
                                    });
                                }
                            }
                            if ui
                                .add_enabled(
                                    !self.content_protected(
                                        &viewer.source.thread,
                                        viewer.source.group,
                                    ) && !viewer.original_url.is_empty(),
                                    egui::Button::new(crate::i18n::tr("Sao chép link")),
                                )
                                .clicked()
                            {
                                ctx.copy_text(viewer.original_url.clone());
                                self.show_toast("Đã sao chép đường dẫn");
                            }
                        });
                    });
            });

        if should_close {
            self.image_viewer = None;
        } else {
            if let Some(next) = navigate.filter(|_| !viewer.navigation_pending) {
                if let Some(tx) = WORKER_TX.get() {
                    viewer.navigation_pending = tx
                        .send(WorkerCmd::MediaNeighbor {
                            owner: self.user_uin.clone(),
                            thread: viewer.thread_id.clone(),
                            anchor: viewer.message_id.clone(),
                            next,
                        })
                        .is_ok();
                }
            }
            if retry {
                for key in [Some(&viewer.tex_key), viewer.hd_tex_key.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    self.image_failures.remove(key);
                    self.loading_stickers.remove(key);
                    self.image_retry_at.remove(key);
                }
                self.open_media_viewer(&viewer.thread_id, &viewer.source.message);
            } else {
                self.image_viewer = Some(viewer);
            }
        }
    }

    #[allow(dead_code)]
    pub(super) fn open_safety_number_modal(&mut self, peer_id: String, peer_name: String) {
        let is_verified = self.trusted_peers.contains(&peer_id);
        self.safety_number_modal = Some(super::SafetyNumberModalState {
            peer_id: peer_id.clone(),
            peer_name: peer_name.clone(),
            safety_number: String::new(),
            is_verified,
            identity_changed: false,
            loading: true,
        });
        if let Some(tx) = WORKER_TX.get() {
            let _ = tx.send(WorkerCmd::LoadSafetyNumber {
                owner: self.user_uin.clone(),
                peer_id,
                peer_name,
            });
        }
    }

    // Retained for compatibility with the dormant E2EE state/engine; MAP-10 no longer
    // exposes this experimental modal in the product UI.
    #[allow(dead_code)]
    pub(super) fn render_safety_number_modal(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let Some(ref mut modal) = self.safety_number_modal else {
            return;
        };

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.safety_number_modal = None;
            return;
        }

        render_modal_backdrop(ctx, "safety_number_modal_backdrop");

        let mut is_open = true;
        let mut close_clicked = false;
        let mut trust_action = false;
        let peer_id = modal.peer_id.clone();
        let peer_name = modal.peer_name.clone();

        egui::Window::new(format!("🔒 Mã an toàn E2EE — {peer_name}"))
            .id(Id::new("safety_number_verification_modal"))
            .order(egui::Order::Foreground)
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .default_width(440.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::window(&ctx.style())
                    .rounding(Rounding::same(12.0))
                    .inner_margin(Margin::symmetric(18.0, 16.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    if modal.identity_changed {
                        ui.colored_label(
                            Color32::from_rgb(239, 68, 68),
                            "⚠️ Thiết bị của đối phương đã thay đổi!",
                        );
                        ui.label(RichText::new("Mã an toàn trước đây không còn khớp. Hãy xác minh lại.").size(12.0).color(pal.warning));
                        ui.add_space(8.0);
                    }

                    if modal.loading {
                        ui.spinner();
                        ui.add_space(6.0);
                        ui.label(RichText::new("Đang tính toán mã an toàn E2EE…").color(pal.text_secondary));
                    } else {
                        ui.label(
                            RichText::new("Mã an toàn (60 chữ số chia thành 12 nhóm, mỗi nhóm 5 chữ số):")
                                .size(13.0)
                                .color(pal.text_primary)
                                .strong(),
                        );
                        ui.add_space(10.0);

                        // Render safety number in 4 rows of 3 groups
                        let groups: Vec<&str> = modal.safety_number.split_whitespace().collect();
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .rounding(Rounding::same(8.0))
                            .inner_margin(Margin::symmetric(16.0, 12.0))
                            .show(ui, |ui| {
                                for row in groups.chunks(3) {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(18.0, 6.0);
                                        for group in row {
                                            ui.monospace(
                                                RichText::new(*group)
                                                    .size(15.5)
                                                    .strong()
                                                    .color(pal.text_primary),
                                            );
                                        }
                                    });
                                    ui.add_space(4.0);
                                }
                            });

                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!(
                                "So sánh mã số này với {peer_name} qua điện thoại hoặc trực tiếp để xác nhận phiên trò chuyện an toàn."
                            ))
                            .size(11.5)
                            .color(pal.text_secondary),
                        );
                        ui.add_space(14.0);

                        if modal.is_verified {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("✓ Đã xác minh an toàn")
                                        .size(13.5)
                                        .color(Color32::from_rgb(34, 197, 94))
                                        .strong(),
                                );
                            });
                        } else {
                            let btn = egui::Button::new(
                                RichText::new("✓ Xác minh danh tính")
                                    .size(13.0)
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(pal.primary)
                            .stroke(Stroke::new(1.0_f32, pal.primary_hover))
                            .rounding(Rounding::same(8.0))
                            .min_size(Vec2::new(190.0, 32.0));

                            if ui.add(btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                trust_action = true;
                            }
                        }
                    }

                    ui.add_space(14.0);
                    if ui.button(crate::i18n::tr("Đóng")).clicked() {
                        close_clicked = true;
                    }
                    ui.add_space(4.0);
                });
            });

        if trust_action {
            if let Some(ref mut m) = self.safety_number_modal {
                m.loading = true;
            }
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::TrustIdentity {
                    owner: self.user_uin.clone(),
                    peer_id,
                    safety_number: self
                        .safety_number_modal
                        .as_ref()
                        .map(|m| m.safety_number.clone())
                        .unwrap_or_default(),
                });
            }
        }

        if !is_open || close_clicked {
            self.safety_number_modal = None;
        }
    }

    pub(super) fn render_auto_delete_modal(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let Some(ref mut modal) = self.auto_delete_modal else {
            return;
        };

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.auto_delete_modal = None;
            return;
        }

        render_modal_backdrop(ctx, "auto_delete_modal_backdrop");

        let mut is_open = true;
        let mut cancel_clicked = false;
        let mut save_ttl: Option<(String, bool, i64)> = None;
        let thread_id = modal.thread_id.clone();
        let is_group = modal.is_group;
        let thread_name = modal.thread_name.clone();
        let pending = self.policy_panel.pending;

        egui::Window::new("Tin nhắn tự xóa")
            .id(Id::new("auto_delete_modal"))
            .order(egui::Order::Foreground)
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .default_width(380.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::window(&ctx.style())
                    .rounding(Rounding::same(12.0))
                    .inner_margin(Margin::symmetric(20.0, 16.0)),
            )
            .show(ctx, |ui| {
                if !thread_name.is_empty() {
                    ui.label(
                        RichText::new(thread_name)
                            .size(12.0)
                            .color(pal.text_muted),
                    );
                    ui.add_space(4.0);
                }
                ui.label(
                    RichText::new(
                        "Tin nhắn sẽ tự động biến mất đối với cả hai bên sau khoảng thời gian đã chọn.",
                    )
                    .size(12.5)
                    .color(pal.text_secondary),
                );
                ui.add_space(12.0);

                egui::Frame::none()
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(14.0, 10.0))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        for (ttl, label) in [
                            (0_i64, "Không bao giờ (Tắt)"),
                            (86_400_000_i64, "1 ngày"),
                            (604_800_000_i64, "7 ngày"),
                            (2_592_000_000_i64, "30 ngày"),
                        ] {
                            ui.radio_value(&mut modal.selected_ttl, ttl, label);
                        }
                    });

                ui.add_space(14.0);
                if pending {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new("Đang đồng bộ thiết lập với Zalo…")
                                .size(11.5)
                                .color(pal.text_secondary),
                        );
                    });
                    ui.add_space(6.0);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let save_btn = egui::Button::new(
                        RichText::new("Lưu")
                            .size(13.0)
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(pal.primary)
                    .rounding(Rounding::same(8.0))
                    .min_size(Vec2::new(88.0, 32.0));
                    if ui
                        .add_enabled(!pending, save_btn)
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        save_ttl = Some((thread_id, is_group, modal.selected_ttl));
                    }

                    ui.add_space(6.0);
                    let cancel_btn = egui::Button::new(
                        RichText::new("Hủy").size(13.0).color(pal.text_primary),
                    )
                    .rounding(Rounding::same(8.0))
                    .min_size(Vec2::new(80.0, 32.0));
                    if ui
                        .add(cancel_btn)
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        cancel_clicked = true;
                    }
                });
            });

        if let Some((thread, group, ttl)) = save_ttl {
            self.policy_command(crate::conversation_policy::Action::SetTtl { thread, group, ttl });
            self.auto_delete_modal = None;
            let toast_label = match ttl {
                0 => "Đã gửi yêu cầu tắt tin nhắn tự xóa".to_string(),
                86_400_000 => "Đã gửi yêu cầu đặt tin nhắn tự xóa: 1 ngày".to_string(),
                604_800_000 => "Đã gửi yêu cầu đặt tin nhắn tự xóa: 7 ngày".to_string(),
                2_592_000_000 => "Đã gửi yêu cầu đặt tin nhắn tự xóa: 30 ngày".to_string(),
                _ => format!(
                    "Đã gửi yêu cầu đặt tin nhắn tự xóa: {} giờ",
                    ttl / 3_600_000
                ),
            };
            self.show_toast(toast_label);
        } else if !is_open || cancel_clicked {
            self.auto_delete_modal = None;
        }
    }

    pub(super) fn render_content_protection_modal(
        &mut self,
        ctx: &egui::Context,
        pal: &ThemePalette,
    ) {
        let Some(ref modal) = self.content_protection_modal else {
            return;
        };

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.content_protection_modal = None;
            return;
        }

        render_modal_backdrop(ctx, "content_protection_modal_backdrop");

        let mut is_open = true;
        let mut cancel_clicked = false;
        let mut confirm_action: Option<(String, bool, bool)> = None;
        let thread_id = modal.thread_id.clone();
        let is_group = modal.is_group;
        let thread_name = modal.thread_name.clone();
        let current_enabled = modal.current_enabled;
        let pending = self.policy_panel.pending;

        egui::Window::new("Chặn sao chép & chia sẻ")
            .id(Id::new("content_protection_modal"))
            .order(egui::Order::Foreground)
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .default_width(400.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::window(&ctx.style())
                    .rounding(Rounding::same(12.0))
                    .inner_margin(Margin::symmetric(20.0, 16.0)),
            )
            .show(ctx, |ui| {
                if !thread_name.is_empty() {
                    ui.label(
                        RichText::new(thread_name)
                            .size(12.0)
                            .color(pal.text_muted),
                    );
                    ui.add_space(4.0);
                }
                let desc = if current_enabled {
                    "Hội thoại này hiện đang bật chế độ chặn sao chép, tải về và chuyển tiếp nội dung. Bạn có muốn tắt bảo vệ nội dung?"
                } else {
                    "Khi bật, các thành viên sẽ bị hạn chế sao chép, tải về và chuyển tiếp tin nhắn, ảnh hoặc tập tin trong hội thoại này."
                };
                ui.label(RichText::new(desc).size(12.5).color(pal.text_secondary));
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Lưu ý: Máy chủ Zalo quyết định quyền thực thi; tính năng này không ngăn được mọi cách chụp màn hình.")
                        .size(11.0)
                        .color(pal.text_muted),
                );
                ui.add_space(14.0);

                if pending {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new("Đang đồng bộ thiết lập với Zalo…")
                                .size(11.5)
                                .color(pal.text_secondary),
                        );
                    });
                    ui.add_space(6.0);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let confirm_text = if current_enabled {
                        "Tắt chặn chia sẻ"
                    } else {
                        "Bật chặn chia sẻ"
                    };
                    let confirm_btn = egui::Button::new(
                        RichText::new(confirm_text)
                            .size(13.0)
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(pal.primary)
                    .rounding(Rounding::same(8.0))
                    .min_size(Vec2::new(124.0, 32.0));
                    if ui
                        .add_enabled(!pending, confirm_btn)
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        confirm_action = Some((thread_id, is_group, !current_enabled));
                    }

                    ui.add_space(6.0);
                    let cancel_btn = egui::Button::new(
                        RichText::new("Hủy").size(13.0).color(pal.text_primary),
                    )
                    .rounding(Rounding::same(8.0))
                    .min_size(Vec2::new(80.0, 32.0));
                    if ui
                        .add(cancel_btn)
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        cancel_clicked = true;
                    }
                });
            });

        if let Some((thread, group, enabled)) = confirm_action {
            self.policy_command(crate::conversation_policy::Action::SetProtection {
                thread,
                group,
                enabled,
            });
            self.content_protection_modal = None;
            self.show_toast(if enabled {
                "Đã gửi yêu cầu bật chặn chia sẻ nội dung"
            } else {
                "Đã gửi yêu cầu tắt chặn chia sẻ nội dung"
            });
        } else if !is_open || cancel_clicked {
            self.content_protection_modal = None;
        }
    }

    /// Detect an `http://` or `https://` URL in `self.input_message` and asynchronously fetch its
    /// OpenGraph / Zalo `parselink` metadata (`title`, `desc`, `thumb`, `src`).
    pub(super) fn resolve_composer_link_preview(
        &mut self,
        ctx: &egui::Context,
        thread_id: &str,
        is_group: bool,
    ) -> Option<crate::chat::LinkMetadata> {
        if !is_group && self.trial_peers.contains(thread_id) {
            return None;
        }
        let debounce_key = Id::new((
            "composer_link_preview_debounce",
            self.user_uin.as_str(),
            thread_id,
        ));
        let attempt_key = Id::new((
            "composer_preview_attempt",
            &self.user_uin,
            self.account_runtime_id,
            thread_id,
            is_group,
        ));
        let Some(url) = crate::chat::extract_first_url(&self.input_message) else {
            self.dismissed_link_preview_for.clear();
            ctx.data_mut(|data| {
                data.remove::<(String, std::time::Instant)>(attempt_key);
                data.remove::<(
                    String,
                    String,
                    String,
                    std::time::Instant,
                    std::time::Instant,
                )>(debounce_key);
            });
            return None;
        };
        if self.dismissed_link_preview_for == url {
            return None;
        }

        let owner = self.user_uin.clone();
        let last_edit = self.last_composer_edit_at;
        let state = ctx.data(|data| {
            data.get_temp::<(
                String,
                String,
                String,
                std::time::Instant,
                std::time::Instant,
            )>(debounce_key)
        });
        let (scheduled_at, same_draft) = match state {
            Some((saved_owner, saved_thread, saved_url, saved_edit, scheduled_at))
                if saved_owner == owner
                    && saved_thread == thread_id
                    && saved_url == url
                    && saved_edit == last_edit =>
            {
                (scheduled_at, true)
            }
            Some((saved_owner, saved_thread, saved_url, _, _))
                if saved_owner == owner && saved_thread == thread_id && saved_url == url =>
            {
                (last_edit, false)
            }
            _ => (std::time::Instant::now(), false),
        };
        if !same_draft {
            ctx.data_mut(|data| {
                data.insert_temp(
                    debounce_key,
                    (
                        owner,
                        thread_id.to_owned(),
                        url.clone(),
                        last_edit,
                        scheduled_at,
                    ),
                );
            });
        }

        let debounce = Duration::from_millis(350);
        let elapsed = scheduled_at.elapsed().min(last_edit.elapsed());
        if elapsed < debounce {
            ctx.request_repaint_after(debounce - elapsed + Duration::from_millis(15));
            return self.link_metadata_cache.get(&url).cloned();
        }
        if let Some(cached) = self.link_metadata_cache.get(&url).cloned() {
            return Some(cached);
        }
        if !self.loading_link_previews.contains(&url) {
            if let Some(tx) = WORKER_TX
                .get()
                .filter(|_| mark_preview_attempt(ctx, attempt_key, &url, last_edit))
            {
                self.loading_link_previews.insert(url.clone());
                if tx
                    .send(WorkerCmd::ParseLinkPreview {
                        url: url.clone(),
                        origin: crate::chat::LinkOrigin::Outgoing,
                        thread: Some((thread_id.to_owned(), is_group)),
                    })
                    .is_err()
                {
                    self.loading_link_previews.remove(&url);
                }
            }
        }
        let domain = url
            .split("://")
            .nth(1)
            .and_then(|r| r.split('/').next())
            .unwrap_or("Liên kết")
            .to_string();
        let title = if self.loading_link_previews.contains(&url) {
            "Đang tải thông tin liên kết…"
        } else {
            "Không tải được liên kết; sửa nội dung để thử lại"
        }
        .to_owned();
        Some(crate::chat::LinkMetadata {
            url,
            title,
            desc: String::new(),
            thumb: String::new(),
            src: domain,
        })
    }

    /// Render a floating Link Preview card (`egui::Area`) anchored directly above `composer_rect`
    /// when the user types or pastes a URL into the composer input.
    pub(super) fn render_composer_link_preview_popover(
        &mut self,
        ctx: &egui::Context,
        composer_rect: Rect,
        meta: &crate::chat::LinkMetadata,
        pal: &ThemePalette,
    ) {
        let strip_h = 72.0_f32;
        let strip_w = composer_rect.width().max(220.0);
        let pos = Pos2::new(
            composer_rect.left(),
            (composer_rect.top() - strip_h).max(48.0),
        );
        let mut dismiss_clicked = false;

        egui::Area::new(Id::new("zalo_composer_link_preview_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(if pal.is_dark {
                        Color32::from_rgba_unmultiplied(33, 37, 43, 248)
                    } else {
                        Color32::from_rgba_unmultiplied(255, 255, 255, 250)
                    })
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding {
                        nw: 12.0,
                        ne: 12.0,
                        sw: 0.0,
                        se: 0.0,
                    })
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, -3.0),
                        blur: 12.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(if pal.is_dark { 85 } else { 24 }),
                    })
                    .inner_margin(Margin::symmetric(14.0, 8.0))
                    .show(ui, |ui| {
                        let inner_w = (strip_w - 28.0).max(160.0);
                        ui.set_min_width(inner_w);
                        ui.set_max_width(inner_w);
                        ui.set_max_height(54.0);
                        ui.horizontal(|ui| {
                            let thumb_key = if !meta.thumb.is_empty() {
                                Some(format!("thumb_{}", crate::crypto::md5_hex(&meta.thumb)))
                            } else {
                                None
                            };
                            if let Some(tkey) = thumb_key.as_deref() {
                                if let Some(tex) = self.sticker_textures.get(tkey) {
                                    ui.add(
                                        egui::Image::new(tex)
                                            .fit_to_exact_size(Vec2::new(48.0, 48.0))
                                            .rounding(Rounding::same(7.0)),
                                    );
                                    ui.add_space(6.0);
                                }
                            }
                            let text_w = (inner_w - 94.0).max(100.0);
                            ui.vertical(|ui| {
                                ui.set_max_width(text_w);
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        lucide::GLOBE,
                                        truncate_chars(&meta.title, 54)
                                    ))
                                    .size(12.5)
                                    .color(pal.primary_hover)
                                    .strong(),
                                );
                                let sub = if !meta.desc.is_empty() {
                                    truncate_chars(&meta.desc, 68).into_owned()
                                } else {
                                    clean_display_url(&meta.url, 48)
                                };
                                ui.label(RichText::new(sub).size(11.3).color(pal.text_secondary));
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .small_button(lucide::X)
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Ẩn xem trước liên kết"))
                                    .clicked()
                                {
                                    dismiss_clicked = true;
                                }
                            });
                        });
                    });
            });

        if dismiss_clicked {
            self.dismissed_link_preview_for = meta.url.clone();
        }
    }

    pub(super) fn render_dev_logs(&mut self, ui: &mut egui::Ui) {
        let entries = logger::get_entries();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(360.0);
                egui::ScrollArea::vertical()
                    .id_salt("dev_logs_list_scroll")
                    .show(ui, |ui| {
                        for e in entries.iter().rev() {
                            let st_str = e
                                .status
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "ERR".into());
                            let row = ui.selectable_label(
                                self.selected_log
                                    .as_ref()
                                    .map(|s| s.id == e.id)
                                    .unwrap_or(false),
                                format!(
                                    "[{}] {} {} ({}ms)",
                                    e.timestamp, e.method, st_str, e.latency_ms
                                ),
                            );
                            if row.clicked() {
                                self.selected_log = Some(e.clone());
                            }
                        }
                    });
            });

            ui.separator();

            ui.vertical(|ui| {
                if let Some(e) = &self.selected_log {
                    ui.strong(format!("{} {}", e.method, e.url));
                    ui.label(format!(
                        "Thời gian: {} · Độ trễ: {}ms · HTTP: {:?}",
                        e.timestamp, e.latency_ms, e.status
                    ));
                    if let Some(err) = &e.error_msg {
                        ui.colored_label(Color32::RED, format!("Lỗi: {err}"));
                    }
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("dev_logs_detail_scroll")
                        .show(ui, |ui| {
                            if let Some(dec) = &e.decrypted_preview {
                                ui.label(crate::i18n::tr("Dữ liệu giải mã (Decrypted Payload):"));
                                ui.code(dec);
                            } else {
                                ui.label("Payload:");
                                ui.code(&e.payload_preview);
                            }
                        });
                } else {
                    ui.label(crate::i18n::tr(
                        "Chọn một request bên trái để kiểm tra chi tiết.",
                    ));
                }
            });
        });
    }

    pub(super) fn render_dev_scanner(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("dev_scanner_scroll")
            .show(ui, |ui| {
                ui.heading(crate::i18n::tr("Báo cáo quét chẩn đoán tự động:"));
                ui.add_space(6.0);

                if self.scanner_findings.is_empty() {
                    ui.label(crate::i18n::tr("Bấm 'Làm mới quét' để chạy kiểm tra."));
                }

                for f in &self.scanner_findings {
                    let (badge_text, color) = match f.level {
                        FindingLevel::Info => ("INFO", Color32::from_rgb(40, 167, 69)),
                        FindingLevel::Warning => ("CẢNH BÁO", self.palette().warning),
                        FindingLevel::Critical => ("NGHIÊM TRỌNG", Color32::RED),
                    };

                    egui::Frame::group(ui.style())
                        .stroke(Stroke::new(1.0_f32, color))
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(color, format!("[{badge_text}]"));
                                ui.strong(&f.title);
                            });
                            ui.label(&f.detail);
                            ui.label(
                                RichText::new(format!("Khắc phục: {}", f.suggestion))
                                    .color(Color32::from_rgb(0, 104, 255)),
                            );
                        });
                    ui.add_space(6.0);
                }
            });
    }

    pub(super) fn render_dev_crypto(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("dev_crypto_scroll")
            .show(ui, |ui| {
                ui.heading(crate::i18n::tr("Thông số mật mã & Khung giao thức:"));
                ui.add_space(8.0);

                egui::Grid::new("crypto_grid").striped(true).show(ui, |ui| {
                    ui.label("API Version (zpw_ver):");
                    ui.code(crate::session::ZPW_VER);
                    ui.end_row();

                    ui.label("API Type (zpw_type):");
                    ui.code(crate::session::ZPW_TYPE);
                    ui.end_row();

                    ui.label("ParamCipher Master Key:");
                    ui.code(format!("{:02X?}", crate::param_cipher::PARAM_CIPHER_KEY));
                    ui.end_row();

                    ui.label("Zero IV:");
                    ui.code("00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 (16 zero bytes)");
                    ui.end_row();

                    ui.label(crate::i18n::tr("Thuật toán mã hoá:"));
                    ui.label("AES-128-CBC / AES-256-CBC with PKCS7 padding");
                    ui.end_row();

                    ui.label(crate::i18n::tr("Chữ ký SignKey:"));
                    ui.code("md5('zsecure' + tag + sorted_values)");
                    ui.end_row();
                });
            });
    }
}

/// Verify the selected directory with an isolated temporary file, outside the UI thread.
fn validate_download_folder(path: &std::path::Path) -> anyhow::Result<()> {
    use std::io::Write;
    anyhow::ensure!(path.is_dir(), "Thư mục không còn tồn tại");
    let mut probe = tempfile::Builder::new()
        .prefix(".ctdozlite-write-")
        .tempfile_in(path)?;
    probe.write_all(b"write-check")?;
    probe.flush()?;
    Ok(())
}

#[cfg(test)]
mod download_folder_tests {
    use super::*;
    #[test]
    fn validation_preserves_directory_and_rejects_files_or_missing_paths() {
        let root = tempfile::tempdir().unwrap();
        validate_download_folder(root.path()).unwrap();
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        let file = root.path().join("a-file");
        std::fs::write(&file, b"keep").unwrap();
        assert!(validate_download_folder(&file).is_err());
        assert!(validate_download_folder(&root.path().join("missing")).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"keep");
    }
}

#[cfg(test)]
mod safety_number_modal_tests {

    #[test]
    fn safety_number_calculation_and_chunking() {
        let our_key = [1u8; 32];
        let peer_key = [2u8; 32];
        let sn = crate::crypto::safety::safety_number(&our_key, &peer_key).unwrap();

        // 60 digits divided into 12 groups of 5 characters, separated by space:
        // Total chars: 60 digits + 11 spaces = 71 chars.
        assert_eq!(sn.len(), 71);
        let groups: Vec<&str> = sn.split_whitespace().collect();
        assert_eq!(groups.len(), 12);
        for g in &groups {
            assert_eq!(g.len(), 5);
            assert!(g.chars().all(|c| c.is_ascii_digit()));
        }

        // Verify Symmetry:
        let sn_reverse = crate::crypto::safety::safety_number(&peer_key, &our_key).unwrap();
        assert_eq!(sn, sn_reverse);
    }

    #[test]
    fn safety_number_modal_state_lifecycle() {
        let mut modal = super::super::SafetyNumberModalState {
            peer_id: "user_bob_123".into(),
            peer_name: "Bob".into(),
            safety_number: String::new(),
            is_verified: false,
            identity_changed: false,
            loading: true,
        };

        assert!(modal.loading);
        assert!(!modal.is_verified);

        // Simulate loaded event
        let sn = crate::crypto::safety::safety_number(&[10u8; 32], &[20u8; 32]).unwrap();
        modal.safety_number = sn;
        modal.loading = false;
        assert!(!modal.loading);
        assert_eq!(modal.safety_number.split_whitespace().count(), 12);

        // Simulate identity trusted
        modal.is_verified = true;
        assert!(modal.is_verified);
    }
}

#[cfg(test)]
mod forward_dialog_tests {
    use super::*;

    fn test_app() -> (ZaloGuiApp, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![
                Thread {
                    id: "u1".into(),
                    name: "Bạn A".into(),
                    is_group: false,
                    ..Default::default()
                },
                Thread {
                    id: "g2".into(),
                    name: "Nhóm B".into(),
                    is_group: true,
                    ..Default::default()
                },
            ],
            false,
        );
        app.is_logged_in = true;
        app.hidden_panel.view.ready = true;
        (app, ctx)
    }

    #[test]
    fn modal_gate_tracks_open_modals_and_dialogs() {
        let (mut app, _) = test_app();
        assert!(!app.is_any_modal_open());

        app.settings_modal_open = true;
        assert!(app.is_any_modal_open());
        app.settings_modal_open = false;

        app.pin_setup_modal_open = true;
        assert!(app.is_any_modal_open());
        app.pin_setup_modal_open = false;

        app.auto_delete_modal = Some(super::super::AutoDeleteModalState {
            thread_id: "u1".into(),
            is_group: false,
            thread_name: "Bạn A".into(),
            selected_ttl: 0,
        });
        assert!(app.is_any_modal_open());
        app.auto_delete_modal = None;

        app.content_protection_modal = Some(super::super::ContentProtectionModalState {
            thread_id: "u1".into(),
            is_group: false,
            thread_name: "Bạn A".into(),
            current_enabled: false,
        });
        assert!(app.is_any_modal_open());
        app.content_protection_modal = None;

        app.profile_panel.open = true;
        assert!(app.is_any_modal_open());
        app.profile_panel.open = false;

        app.forward_msg = Some(ChatMessage {
            id: "m1".into(),
            text: "Xin chào".into(),
            ..Default::default()
        });
        assert!(app.is_any_modal_open());
        app.clear_forward_dialog();
        assert!(!app.is_any_modal_open());
    }

    fn find_text_center(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<Pos2> {
        fn walk(shape: &egui::Shape, needle: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.job.text.contains(needle) => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                egui::Shape::Vec(children) => children.iter().find_map(|s| walk(s, needle)),
                _ => None,
            }
        }
        shapes.iter().find_map(|cs| walk(&cs.shape, needle))
    }

    #[test]
    fn forward_dialog_selects_targets_and_builds_commands_without_instant_send() {
        let (mut app, ctx) = test_app();
        app.forward_msg = Some(ChatMessage {
            id: "m-fwd".into(),
            text: "Tài liệu cuộc họp".into(),
            ..Default::default()
        });
        let pal = ThemePalette::light();

        // Run two warmup frames so egui::Window anchor & ScrollArea lay out the rows.
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(900.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| app.render_forward_dialog(ctx, &pal),
        );
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(900.0, 700.0),
                )),
                time: Some(0.05),
                ..Default::default()
            },
            |ctx| app.render_forward_dialog(ctx, &pal),
        );
        let row_pos =
            find_text_center(&output.shapes, "Bạn A").expect("recipient row should render");

        // Clicking recipient toggles selection only and keeps the dialog open.
        for (pressed, t) in [(true, 0.1), (false, 0.2)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(900.0, 700.0),
                    )),
                    time: Some(t),
                    events: vec![
                        egui::Event::PointerMoved(row_pos),
                        egui::Event::PointerButton {
                            pos: row_pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.render_forward_dialog(ctx, &pal),
            );
        }
        assert!(app.forward_msg.is_some());
        assert!(app.forward_targets.contains("u1"));
        assert_eq!(app.forward_targets.len(), 1);

        // Selecting a second target and adding an accompanying note prepares 4 commands.
        app.forward_targets.insert("g2".into());
        app.forward_note = "Xem giúp mình nhé".into();
        let (target_count, cmds) = app.build_forward_commands();
        assert_eq!(target_count, 2);
        assert_eq!(cmds.len(), 4);
        assert!(matches!(
            &cmds[0],
            WorkerCmd::Forward { target_id, target_is_group: false, .. } if target_id == "u1"
        ));
        assert!(matches!(
            &cmds[1],
            WorkerCmd::SendMessage { to_id, text, is_group: false } if to_id == "u1" && text == "Xem giúp mình nhé"
        ));
        assert!(matches!(
            &cmds[2],
            WorkerCmd::Forward { target_id, target_is_group: true, .. } if target_id == "g2"
        ));
        assert!(matches!(
            &cmds[3],
            WorkerCmd::SendMessage { to_id, text, is_group: true } if to_id == "g2" && text == "Xem giúp mình nhé"
        ));

        // Clicking "Hủy" closes the dialog and wipes all forward state.
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(900.0, 700.0),
                )),
                time: Some(0.3),
                ..Default::default()
            },
            |ctx| app.render_forward_dialog(ctx, &pal),
        );
        let cancel_pos =
            find_text_center(&output.shapes, "Hủy").expect("cancel button should render");
        for (pressed, t) in [(true, 0.4), (false, 0.5)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(900.0, 700.0),
                    )),
                    time: Some(t),
                    events: vec![
                        egui::Event::PointerMoved(cancel_pos),
                        egui::Event::PointerButton {
                            pos: cancel_pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.render_forward_dialog(ctx, &pal),
            );
        }
        assert!(app.forward_msg.is_none());
        assert!(app.forward_targets.is_empty());
        assert!(app.forward_note.is_empty());
        assert!(app.forward_filter.is_empty());
    }

    #[test]
    fn auto_delete_modal_selects_ttl_and_saves_or_cancels() {
        let (mut app, ctx) = test_app();
        app.auto_delete_modal = Some(super::super::AutoDeleteModalState {
            thread_id: "u1".into(),
            is_group: false,
            thread_name: "Bạn A".into(),
            selected_ttl: 0,
        });
        let pal = ThemePalette::light();

        for t in [0.0, 0.05] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(900.0, 700.0),
                    )),
                    time: Some(t),
                    ..Default::default()
                },
                |ctx| app.render_auto_delete_modal(ctx, &pal),
            );
        }

        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(900.0, 700.0),
                )),
                time: Some(0.1),
                ..Default::default()
            },
            |ctx| app.render_auto_delete_modal(ctx, &pal),
        );
        let radio_7d =
            find_text_center(&output.shapes, "7 ngày").expect("7-day radio option should render");
        for (pressed, t) in [(true, 0.15), (false, 0.2)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(900.0, 700.0),
                    )),
                    time: Some(t),
                    events: vec![
                        egui::Event::PointerMoved(radio_7d),
                        egui::Event::PointerButton {
                            pos: radio_7d,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.render_auto_delete_modal(ctx, &pal),
            );
        }
        assert_eq!(
            app.auto_delete_modal.as_ref().map(|m| m.selected_ttl),
            Some(604_800_000)
        );

        let save_pos = find_text_center(&output.shapes, "Lưu").expect("save button should render");
        for (pressed, t) in [(true, 0.25), (false, 0.3)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(900.0, 700.0),
                    )),
                    time: Some(t),
                    events: vec![
                        egui::Event::PointerMoved(save_pos),
                        egui::Event::PointerButton {
                            pos: save_pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.render_auto_delete_modal(ctx, &pal),
            );
        }
        assert!(app.auto_delete_modal.is_none());
        assert!(app
            .toast_notice
            .as_ref()
            .is_some_and(|(msg, _)| msg.contains("7 ngày")));
    }

    #[test]
    fn my_documents_save_binds_exact_server_self_target_and_keeps_source_payload() {
        let (mut app, _) = test_app();
        app.own_cloud_id = "self-documents".into();
        app.threads.push(Thread {
            id: "self-documents".into(),
            name: "My Documents".into(),
            ..Default::default()
        });
        let source = ChatMessage {
            id: "source-one".into(),
            text: "Original saved payload".into(),
            ..Default::default()
        };
        app.forward_msg = Some(source.clone());
        app.forward_batch = vec![source];
        app.forward_cloud_only = true;
        app.forward_targets.insert("u1".into());
        assert_eq!(app.start_forward_session(), Some(1));
        let session = app.forward_session.as_ref().unwrap();
        assert_eq!(session.targets[0].target_id, "self-documents");
        assert!(!session.targets[0].is_group);
        assert!(
            matches!(&session.targets[0].forward_msg.outgoing_job.as_ref().unwrap().payload,
            crate::outgoing::Payload::Forward(original) if original.id=="source-one" && original.text=="Original saved payload")
        );
    }

    #[test]
    fn archive_batch_share_tracks_each_source_and_sends_note_once_per_recipient() {
        let (mut app, _) = test_app();
        let first = ChatMessage {
            id: "source-one".into(),
            text: "First source".into(),
            ..Default::default()
        };
        let second = ChatMessage {
            id: "source-two".into(),
            text: "Second source".into(),
            ..Default::default()
        };
        app.forward_msg = Some(first.clone());
        app.forward_batch = vec![first, second];
        app.forward_note = "One note".into();
        app.forward_targets.insert("u1".into());
        assert_eq!(app.start_forward_session(), Some(2));
        let session = app.forward_session.as_ref().unwrap();
        assert_eq!(session.targets.len(), 2);
        assert_ne!(
            session.targets[0].forward_msg.client_message_id,
            session.targets[1].forward_msg.client_message_id
        );
        assert_eq!(
            session
                .targets
                .iter()
                .filter(|target| target.note_msg.is_some())
                .count(),
            1
        );
        let source_ids: Vec<_> = session
            .targets
            .iter()
            .map(
                |target| match &target.forward_msg.outgoing_job.as_ref().unwrap().payload {
                    crate::outgoing::Payload::Forward(source) => source.id.as_str(),
                    _ => panic!("Selected source payload must be retained"),
                },
            )
            .collect();
        assert_eq!(source_ids, vec!["source-one", "source-two"]);
        assert!(app.forward_batch.is_empty());
    }

    #[test]
    fn forward_server_rejected_shows_failure_and_permits_retry() {
        let (mut app, ctx) = test_app();
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let sender = super::super::worker::WorkerSender::new_for_test(cmd_tx, ctx.clone());

        let fwd_msg = ChatMessage {
            id: "msg_orig".into(),
            client_message_id: "1111".into(),
            text: "Báo cáo tiến độ".into(),
            ..Default::default()
        };
        app.forward_msg = Some(fwd_msg.clone());
        app.forward_targets.insert("u1".into());

        // Start session: creates session state and tracks delivery per recipient
        let target_count = app.start_forward_session();
        assert_eq!(target_count, Some(1));
        assert!(app.forward_session.is_some());
        assert!(app.forward_msg.is_none());

        let session = app.forward_session.as_mut().unwrap();
        let fwd_client_id = session.targets[0].forward_msg.client_message_id.clone();

        // Simulate server rejection
        let mut failed_msg = session.targets[0].forward_msg.clone();
        failed_msg.delivery = crate::message_model::DeliveryState::Failed;
        failed_msg.outgoing_job = Some(Box::new(crate::outgoing::Job {
            payload: crate::outgoing::Payload::Text,
            is_group: false,
            is_e2ee: false,
            attempt: 1,
            error: "Hội thoại bị chặn".into(),
        }));

        let changed = session.update_outgoing_state("u1", &failed_msg);
        assert!(changed);
        assert_eq!(
            session.targets[0].forward_status,
            ForwardItemStatus::Failed("Hội thoại bị chặn".into())
        );
        let (total, succeeded, failed) = session.counts();
        assert_eq!(total, 1);
        assert_eq!(succeeded, 0);
        assert_eq!(failed, 1);
        assert!(session.all_finished());

        // Retry preserves original client identity and transitions to Sending
        let retried = session.retry_target("u1", Some(&sender));
        assert_eq!(retried, 1);
        assert_eq!(
            session.targets[0].forward_status,
            ForwardItemStatus::Sending
        );
        assert_eq!(
            session.targets[0].forward_msg.client_message_id,
            fwd_client_id
        );

        let sent_cmd = cmd_rx.try_recv().expect("must receive retry command");
        match sent_cmd {
            WorkerCmd::RetryOutgoing { thread_id, message } => {
                assert_eq!(thread_id, "u1");
                assert_eq!(message.client_message_id, fwd_client_id);
            }
            _ => panic!("unexpected command sent on retry"),
        }
    }

    #[test]
    fn forward_mixed_batch_and_selective_retry_preserves_succeeded_targets() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let sender =
            super::super::worker::WorkerSender::new_for_test(cmd_tx, egui::Context::default());

        let fwd_msg = ChatMessage {
            id: "msg_batch".into(),
            client_message_id: "2222".into(),
            text: "Kế hoạch tuần tới".into(),
            ..Default::default()
        };

        let t1_msg = crate::outgoing::pending(
            fwd_msg.text.clone(),
            false,
            crate::outgoing::Payload::Forward(Box::new(fwd_msg.clone())),
        );
        let t2_msg = crate::outgoing::pending(
            fwd_msg.text.clone(),
            true,
            crate::outgoing::Payload::Forward(Box::new(fwd_msg.clone())),
        );
        let t1_client_id = t1_msg.client_message_id.clone();
        let t2_client_id = t2_msg.client_message_id.clone();
        assert_ne!(t1_client_id, t2_client_id);

        let mut session = ForwardSessionState {
            original_msg: fwd_msg,
            note_text: String::new(),
            targets: vec![
                ForwardTargetDelivery {
                    target_id: "u1".into(),
                    target_name: "Bạn A".into(),
                    is_group: false,
                    avatar: None,
                    forward_msg: t1_msg,
                    forward_status: ForwardItemStatus::Sending,
                    note_msg: None,
                    note_status: None,
                },
                ForwardTargetDelivery {
                    target_id: "g2".into(),
                    target_name: "Nhóm B".into(),
                    is_group: true,
                    avatar: None,
                    forward_msg: t2_msg,
                    forward_status: ForwardItemStatus::Sending,
                    note_msg: None,
                    note_status: None,
                },
            ],
        };

        // Target 1 succeeds
        let mut ack_t1 = session.targets[0].forward_msg.clone();
        ack_t1.delivery = crate::message_model::DeliveryState::ServerAccepted;
        session.update_outgoing_state("u1", &ack_t1);

        // Target 2 fails
        let mut rej_t2 = session.targets[1].forward_msg.clone();
        rej_t2.delivery = crate::message_model::DeliveryState::Failed;
        rej_t2.outgoing_job = Some(Box::new(crate::outgoing::Job {
            payload: crate::outgoing::Payload::Text,
            is_group: true,
            is_e2ee: false,
            attempt: 1,
            error: "Nhóm đã giải tán".into(),
        }));
        session.update_outgoing_state("g2", &rej_t2);

        let (total, succ, fail) = session.counts();
        assert_eq!(total, 2);
        assert_eq!(succ, 1);
        assert_eq!(fail, 1);
        assert!(session.all_finished());

        // Selective retry on g2 only resends g2; u1 is NOT re-sent
        let retried = session.retry_target("g2", Some(&sender));
        assert_eq!(retried, 1);
        assert_eq!(
            session.targets[0].forward_status,
            ForwardItemStatus::Success
        );
        assert_eq!(
            session.targets[1].forward_status,
            ForwardItemStatus::Sending
        );

        let sent_cmd = cmd_rx.try_recv().expect("must receive retry for g2");
        match sent_cmd {
            WorkerCmd::RetryOutgoing { thread_id, message } => {
                assert_eq!(thread_id, "g2");
                assert_eq!(message.client_message_id, t2_client_id);
            }
            _ => panic!("unexpected command sent"),
        }
        // Verify no other commands sent (u1 was untouched)
        assert!(cmd_rx.try_recv().is_err());
    }

    #[test]
    fn forward_queue_failure_midway_preserves_state() {
        let (mut app, _) = test_app();
        let fwd_msg = ChatMessage {
            id: "msg_nofw".into(),
            text: "Nội dung cần chuyển".into(),
            ..Default::default()
        };
        app.forward_msg = Some(fwd_msg);
        app.forward_targets.insert("u1".into());

        // When worker is not initialized, start_forward_session marks target as Failed but preserves state
        let count = app.start_forward_session();
        assert_eq!(count, Some(1));
        assert!(app.forward_session.is_some());

        let session = app.forward_session.as_ref().unwrap();
        assert_eq!(session.targets.len(), 1);
        assert!(matches!(
            session.targets[0].forward_status,
            ForwardItemStatus::Failed(_)
        ));
        let (total, succ, fail) = session.counts();
        assert_eq!(total, 1);
        assert_eq!(succ, 0);
        assert_eq!(fail, 1);
    }

    #[test]
    fn forward_note_mutation_independent_tracking_and_retry() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let sender =
            super::super::worker::WorkerSender::new_for_test(cmd_tx, egui::Context::default());

        let fwd_msg = ChatMessage {
            id: "fwd_item".into(),
            client_message_id: "3001".into(),
            text: "Ảnh đính kèm".into(),
            ..Default::default()
        };
        let note_msg = ChatMessage {
            id: "note_item".into(),
            client_message_id: "3002".into(),
            text: "Gửi bạn xem nhé".into(),
            ..Default::default()
        };

        let mut session = ForwardSessionState {
            original_msg: fwd_msg.clone(),
            note_text: "Gửi bạn xem nhé".into(),
            targets: vec![ForwardTargetDelivery {
                target_id: "u1".into(),
                target_name: "Bạn A".into(),
                is_group: false,
                avatar: None,
                forward_msg: fwd_msg,
                forward_status: ForwardItemStatus::Sending,
                note_msg: Some(note_msg),
                note_status: Some(ForwardItemStatus::Sending),
            }],
        };

        // Forward message succeeds
        let mut fwd_ack = session.targets[0].forward_msg.clone();
        fwd_ack.delivery = crate::message_model::DeliveryState::ServerAccepted;
        session.update_outgoing_state("u1", &fwd_ack);

        // Note message fails
        let mut note_fail = session.targets[0].note_msg.as_ref().unwrap().clone();
        note_fail.delivery = crate::message_model::DeliveryState::Failed;
        note_fail.outgoing_job = Some(Box::new(crate::outgoing::Job {
            payload: crate::outgoing::Payload::Text,
            is_group: false,
            is_e2ee: false,
            attempt: 1,
            error: "Quá giới hạn tốc độ gửi".into(),
        }));
        session.update_outgoing_state("u1", &note_fail);

        assert_eq!(
            session.targets[0].forward_status,
            ForwardItemStatus::Success
        );
        assert_eq!(
            session.targets[0].note_status,
            Some(ForwardItemStatus::Failed("Quá giới hạn tốc độ gửi".into()))
        );

        // Retry on u1 MUST only retry the failed note mutation, NOT resend forward
        let retried = session.retry_target("u1", Some(&sender));
        assert_eq!(retried, 1);
        assert_eq!(
            session.targets[0].forward_status,
            ForwardItemStatus::Success
        );
        assert_eq!(
            session.targets[0].note_status,
            Some(ForwardItemStatus::Sending)
        );

        let sent_cmd = cmd_rx
            .try_recv()
            .expect("must receive retry command for note");
        match sent_cmd {
            WorkerCmd::RetryOutgoing { thread_id, message } => {
                assert_eq!(thread_id, "u1");
                assert_eq!(message.client_message_id, "3002");
                assert_eq!(message.text, "Gửi bạn xem nhé");
            }
            _ => panic!("unexpected command sent"),
        }
        assert!(cmd_rx.try_recv().is_err());
    }
}
